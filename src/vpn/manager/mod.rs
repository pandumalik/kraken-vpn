mod lifecycle;
mod openconnect;
mod openvpn;

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{
    AppSettings, CertPinPrompt, ConnectionHistoryEntry, DiagnosticsLine, HistoryStatus, LogLevel,
    VpnProfile, VpnProtocol,
};
use crate::platform::{is_elevated, terminate_child_tree, ChildJobGuard};
use crate::telemetry::{TelemetrySnapshot, TelemetryTracker};
use crate::vpn::openvpn::{is_generic_fatal_footer, strip_openvpn_timestamp, SecureTempFile};
use crate::vpn::state::{decide_connect, ConnectDecision, TunnelState};

const MAX_DIAGNOSTICS_LINES: usize = 1000;

#[derive(Default)]
pub(super) struct SessionErrorCapture {
    root_error: Option<String>,
    last_line: Option<String>,
}

impl SessionErrorCapture {
    pub(super) fn record_error(&mut self, msg: &str) {
        let clean = strip_openvpn_timestamp(msg);
        if clean.is_empty() {
            return;
        }
        if is_generic_fatal_footer(clean) && self.root_error.is_some() {
            return;
        }
        self.root_error = Some(clean.to_string());
    }

    pub(super) fn record_line(&mut self, line: &str) {
        let clean = strip_openvpn_timestamp(line);
        if clean.is_empty() || is_generic_fatal_footer(clean) {
            return;
        }
        self.last_line = Some(clean.to_string());
    }

    pub(super) fn summary(&self) -> Option<String> {
        self.root_error.clone().or_else(|| self.last_line.clone())
    }
}

pub(super) struct ActiveTunnel {
    pub(super) session_id: Uuid,
    pub(super) profile: VpnProfile,
    pub(super) started_at: DateTime<Utc>,
    pub(super) connected_at: Option<DateTime<Utc>>,
    pub(super) child: Arc<Mutex<Child>>,
    pub(super) stop_flag: Arc<AtomicBool>,
    pub(super) _temp_files: Vec<SecureTempFile>,
}

pub(super) struct ManagerInner {
    pub(super) state: TunnelState,
    pub(super) active: Option<ActiveTunnel>,
    pub(super) diagnostics: VecDeque<DiagnosticsLine>,
    pub(super) diagnostics_version: u64,
    pub(super) pending_cert_prompt: Option<CertPinPrompt>,
    pub(super) pending_history: Vec<ConnectionHistoryEntry>,
    pub(super) auto_pinned_certs: Vec<(Uuid, String)>,
    pub(super) telemetry: TelemetryTracker,
}

impl ManagerInner {
    pub(super) fn push_log(
        &mut self,
        level: LogLevel,
        source: impl Into<String>,
        message: impl Into<String>,
    ) {
        if self.diagnostics.len() >= MAX_DIAGNOSTICS_LINES {
            self.diagnostics.pop_front();
        }
        self.diagnostics
            .push_back(DiagnosticsLine::new(level, source, message));
        self.diagnostics_version = self.diagnostics_version.wrapping_add(1);
    }

    pub(super) fn record_history(&mut self, entry: ConnectionHistoryEntry) {
        self.pending_history.push(entry);
    }
}

#[derive(Clone)]
pub struct VpnManager {
    pub(super) inner: Arc<Mutex<ManagerInner>>,
    pub(super) job_guard: Arc<Option<ChildJobGuard>>,
    pub(super) runtime_temp_dir: PathBuf,
}

impl VpnManager {
    pub fn new(runtime_temp_dir: PathBuf) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ManagerInner {
                state: TunnelState::Disconnected,
                active: None,
                diagnostics: VecDeque::with_capacity(MAX_DIAGNOSTICS_LINES),
                diagnostics_version: 0,
                pending_cert_prompt: None,
                pending_history: Vec::new(),
                auto_pinned_certs: Vec::new(),
                telemetry: TelemetryTracker::new(),
            })),
            job_guard: Arc::new(ChildJobGuard::new()),
            runtime_temp_dir,
        }
    }

    pub fn state(&self) -> TunnelState {
        self.inner.lock().unwrap().state.clone()
    }

    pub fn diagnostics_version(&self) -> u64 {
        self.inner.lock().unwrap().diagnostics_version
    }

    pub fn diagnostics_snapshot(&self) -> Vec<DiagnosticsLine> {
        self.inner
            .lock()
            .unwrap()
            .diagnostics
            .iter()
            .cloned()
            .collect()
    }

    pub fn clear_diagnostics(&self) {
        let mut g = self.inner.lock().unwrap();
        g.diagnostics.clear();
        g.diagnostics_version = g.diagnostics_version.wrapping_add(1);
    }

    pub fn push_system_log(&self, level: LogLevel, message: impl Into<String>) {
        self.inner
            .lock()
            .unwrap()
            .push_log(level, "Kraken", message);
    }

    pub fn take_pending_history(&self) -> Vec<ConnectionHistoryEntry> {
        std::mem::take(&mut self.inner.lock().unwrap().pending_history)
    }

    pub fn take_auto_pinned_certs(&self) -> Vec<(Uuid, String)> {
        std::mem::take(&mut self.inner.lock().unwrap().auto_pinned_certs)
    }

    pub fn pending_cert_prompt(&self) -> Option<CertPinPrompt> {
        self.inner.lock().unwrap().pending_cert_prompt.clone()
    }

    pub fn dismiss_cert_prompt(&self) {
        self.inner.lock().unwrap().pending_cert_prompt = None;
    }

    pub fn sample_telemetry(&self) -> TelemetrySnapshot {
        let mut guard = self.inner.lock().unwrap();
        let connected = guard.state.is_connected();
        guard.telemetry.sample(connected)
    }

    pub fn reset_telemetry_counters(&self) {
        self.inner.lock().unwrap().telemetry.reset_counters();
    }

    pub fn connect(&self, profile: VpnProfile, settings: &AppSettings) -> Result<(), String> {
        self.connect_internal(profile, settings, None, false)
    }

    pub fn trust_cert_and_connect(
        &self,
        mut profile: VpnProfile,
        settings: &AppSettings,
        fingerprint: String,
    ) -> Result<(), String> {
        {
            let mut guard = self.inner.lock().unwrap();
            guard.pending_cert_prompt = None;
            guard.push_log(
                LogLevel::System,
                "OpenConnect",
                format!("User approved server certificate pin {fingerprint}; reconnecting..."),
            );
        }
        profile.servercert = Some(fingerprint.clone());
        self.connect_internal(profile, settings, Some(fingerprint), true)
    }

    pub(super) fn connect_internal(
        &self,
        profile: VpnProfile,
        settings: &AppSettings,
        servercert_override: Option<String>,
        already_retried_cert: bool,
    ) -> Result<(), String> {
        let decision = {
            let guard = self.inner.lock().unwrap();
            decide_connect(&guard.state, profile.id, settings.switch_policy)
        };

        match decision {
            ConnectDecision::AlreadyActive => return Ok(()),
            ConnectDecision::Refused {
                active_profile_name,
            } => {
                let msg = format!(
                    "Cannot connect '{}': '{}' is currently active. Disconnect it first or enable Auto-switch in Settings.",
                    profile.name, active_profile_name
                );
                self.push_system_log(LogLevel::Warn, msg.clone());
                return Err(msg);
            }
            ConnectDecision::DisconnectThenSwitch {
                active_profile_name,
                ..
            } => {
                self.push_system_log(
                    LogLevel::System,
                    format!(
                        "Auto-switching: disconnecting '{active_profile_name}' before connecting '{}'...",
                        profile.name
                    ),
                );
                self.disconnect_sync("Switched to another VPN profile");
            }
            ConnectDecision::StartNow => {}
        }

        if !is_elevated() {
            self.push_system_log(
                LogLevel::Warn,
                "Not running as Administrator: TUN/Wintun route setup may fail until elevated.",
            );
        }

        match profile.protocol {
            VpnProtocol::OpenVpn => self.spawn_openvpn(profile, settings),
            VpnProtocol::OpenConnect => {
                self.spawn_openconnect(profile, settings, servercert_override, already_retried_cert)
            }
        }
    }

    pub fn disconnect(&self) {
        self.disconnect_sync("Disconnected by user");
    }

    pub fn disconnect_sync(&self, reason: &str) {
        let active_opt = {
            let mut guard = self.inner.lock().unwrap();
            let taken = guard.active.take();
            if let Some(ref a) = taken {
                guard.state = TunnelState::Disconnecting {
                    profile_id: a.profile.id,
                    profile_name: a.profile.name.clone(),
                };
            }
            taken
        };

        if let Some(active) = active_opt {
            active.stop_flag.store(true, Ordering::SeqCst);
            if let Ok(mut child) = active.child.lock() {
                terminate_child_tree(&mut child);
            }
            let ended_at = Utc::now();
            let duration_secs = active
                .connected_at
                .map(|c| ended_at.signed_duration_since(c).num_seconds().max(0) as u64)
                .unwrap_or(0);
            let history = ConnectionHistoryEntry {
                id: Uuid::new_v4(),
                profile_id: active.profile.id,
                profile_name: active.profile.name.clone(),
                protocol: active.profile.protocol_badge(),
                endpoint: active.profile.endpoint_display(),
                status: HistoryStatus::Disconnected,
                started_at: active.started_at,
                ended_at: Some(ended_at),
                duration_secs,
                message: reason.to_string(),
            };
            drop(active);

            let mut guard = self.inner.lock().unwrap();
            guard.state = TunnelState::Disconnected;
            guard.telemetry.reset_session(None);
            guard.record_history(history);
            guard.push_log(
                LogLevel::System,
                "Kraken",
                format!("Tunnel closed: {reason}"),
            );
        } else {
            let mut guard = self.inner.lock().unwrap();
            if !matches!(guard.state, TunnelState::Disconnected) {
                guard.state = TunnelState::Disconnected;
            }
        }
    }
}

impl Drop for VpnManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.disconnect_sync("Application closed");
        }
    }
}
