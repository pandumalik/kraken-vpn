use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::models::{
    AppSettings, CertPinPrompt, ConnectionHistoryEntry, DiagnosticsLine, HistoryStatus, LogLevel,
    VpnProfile, VpnProtocol,
};
use crate::platform::{
    configure_background_command, is_elevated, resolve_binary, terminate_child_tree, ChildJobGuard,
};
use crate::telemetry::{parse_openvpn_bytecount, TelemetrySnapshot, TelemetryTracker};
use crate::vpn::openconnect::{
    build_openconnect_args, extract_servercert_pin, find_windows_vpnc_script,
    parse_openconnect_output, OpenConnectEvent,
};
use crate::vpn::openvpn::{
    is_generic_fatal_footer, parse_openvpn_line, prepare_openvpn, strip_openvpn_timestamp,
    OpenVpnEvent, SecureTempFile,
};
use crate::vpn::state::{decide_connect, ConnectDecision, TunnelState};

const MAX_DIAGNOSTICS_LINES: usize = 1000;

#[derive(Default)]
struct SessionErrorCapture {
    root_error: Option<String>,
    last_line: Option<String>,
}

impl SessionErrorCapture {
    fn record_error(&mut self, msg: &str) {
        let clean = strip_openvpn_timestamp(msg);
        if clean.is_empty() {
            return;
        }
        if is_generic_fatal_footer(clean) && self.root_error.is_some() {
            return;
        }
        self.root_error = Some(clean.to_string());
    }

    fn record_line(&mut self, line: &str) {
        let clean = strip_openvpn_timestamp(line);
        if clean.is_empty() || is_generic_fatal_footer(clean) {
            return;
        }
        self.last_line = Some(clean.to_string());
    }

    fn summary(&self) -> Option<String> {
        self.root_error.clone().or_else(|| self.last_line.clone())
    }
}

struct ActiveTunnel {
    session_id: Uuid,
    profile: VpnProfile,
    started_at: DateTime<Utc>,
    connected_at: Option<DateTime<Utc>>,
    child: Arc<Mutex<Child>>,
    stop_flag: Arc<AtomicBool>,
    _temp_files: Vec<SecureTempFile>,
}

struct ManagerInner {
    state: TunnelState,
    active: Option<ActiveTunnel>,
    diagnostics: VecDeque<DiagnosticsLine>,
    diagnostics_version: u64,
    pending_cert_prompt: Option<CertPinPrompt>,
    pending_history: Vec<ConnectionHistoryEntry>,
    auto_pinned_certs: Vec<(Uuid, String)>,
    telemetry: TelemetryTracker,
}

impl ManagerInner {
    fn push_log(&mut self, level: LogLevel, source: impl Into<String>, message: impl Into<String>) {
        if self.diagnostics.len() >= MAX_DIAGNOSTICS_LINES {
            self.diagnostics.pop_front();
        }
        self.diagnostics
            .push_back(DiagnosticsLine::new(level, source, message));
        self.diagnostics_version = self.diagnostics_version.wrapping_add(1);
    }

    fn record_history(&mut self, entry: ConnectionHistoryEntry) {
        self.pending_history.push(entry);
    }
}

#[derive(Clone)]
pub struct VpnManager {
    inner: Arc<Mutex<ManagerInner>>,
    job_guard: Arc<Option<ChildJobGuard>>,
    runtime_temp_dir: PathBuf,
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

    fn connect_internal(
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

    fn spawn_openvpn(&self, profile: VpnProfile, settings: &AppSettings) -> Result<(), String> {
        let bin = resolve_binary(&settings.openvpn_path).ok_or_else(|| {
            let msg = format!(
                "OpenVPN executable not found at '{}'. Configure it under Settings.",
                settings.openvpn_path
            );
            self.push_system_log(LogLevel::Error, msg.clone());
            msg
        })?;

        let prepared = prepare_openvpn(&profile, &self.runtime_temp_dir).inspect_err(|e| {
            self.push_system_log(LogLevel::Error, format!("OpenVPN pre-flight: {e}"));
        })?;
        let mut cmd = Command::new(&bin);
        cmd.args(&prepared.args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(ref wd) = prepared.working_dir {
            cmd.current_dir(wd);
        }
        configure_background_command(&mut cmd);

        let mut child = cmd.spawn().map_err(|e| {
            let msg = format!("Failed to start OpenVPN ({}): {e}", bin.display());
            self.push_system_log(LogLevel::Error, msg.clone());
            msg
        })?;
        if let Some(ref guard) = *self.job_guard {
            guard.assign(&child);
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let session_id = Uuid::new_v4();
        let started_at = Utc::now();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let child_arc = Arc::new(Mutex::new(child));
        let error_capture = Arc::new(Mutex::new(SessionErrorCapture::default()));
        let endpoint = profile.endpoint_display();
        let iface_pref = profile.interface_name.clone();

        {
            let mut guard = self.inner.lock().unwrap();
            guard.pending_cert_prompt = None;
            guard.telemetry.reset_session(iface_pref);
            guard.state = TunnelState::Connecting {
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                protocol: VpnProtocol::OpenVpn,
                endpoint: endpoint.clone(),
                started_at,
            };
            guard.push_log(
                LogLevel::System,
                "OpenVPN",
                format!(
                    "Starting OpenVPN tunnel for '{}' ({endpoint})...",
                    profile.name
                ),
            );
            if let Some(ref wd) = prepared.working_dir {
                guard.push_log(
                    LogLevel::System,
                    "OpenVPN",
                    format!("Working directory: {}", wd.display()),
                );
            }
            guard.active = Some(ActiveTunnel {
                session_id,
                profile: profile.clone(),
                started_at,
                connected_at: None,
                child: child_arc.clone(),
                stop_flag: stop_flag.clone(),
                _temp_files: prepared._temp_files,
            });
        }

        if let Some(mgmt_port) = prepared.management_port {
            let inner = self.inner.clone();
            let stop = stop_flag.clone();
            std::thread::spawn(move || {
                for _ in 0..10 {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(350));
                    if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", mgmt_port)) {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let _ = stream.write_all(b"bytecount 1\r\n");
                        let reader = BufReader::new(stream);
                        for line in reader.lines() {
                            if stop.load(Ordering::SeqCst) {
                                break;
                            }
                            if let Ok(text) = line {
                                if let Some((b_in, b_out)) = parse_openvpn_bytecount(&text) {
                                    let mut g = inner.lock().unwrap();
                                    if g.active
                                        .as_ref()
                                        .is_some_and(|a| a.session_id == session_id)
                                    {
                                        g.telemetry.update_tunnel_bytecount(b_in, b_out);
                                    } else {
                                        break;
                                    }
                                }
                            }
                        }
                        return;
                    }
                }
            });
        }

        let mut reader_handles = Vec::new();

        if let Some(out) = stdout {
            let inner = self.inner.clone();
            let capture = error_capture.clone();
            let prof_name = profile.name.clone();
            let prof_id = profile.id;
            let ep = endpoint.clone();
            reader_handles.push(std::thread::spawn(move || {
                let reader = BufReader::new(out);
                for line in reader.lines().map_while(Result::ok) {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let event = parse_openvpn_line(trimmed);
                    if let Ok(mut cap) = capture.lock() {
                        cap.record_line(trimmed);
                        match &event {
                            OpenVpnEvent::AuthFailed(m) | OpenVpnEvent::Error(m) => {
                                cap.record_error(m);
                            }
                            _ => {}
                        }
                    }
                    let mut g = inner.lock().unwrap();
                    let still_active = g
                        .active
                        .as_ref()
                        .is_some_and(|a| a.session_id == session_id);
                    match event {
                        OpenVpnEvent::Connected => {
                            if still_active {
                                let now = Utc::now();
                                if let Some(ref mut a) = g.active {
                                    a.connected_at = Some(now);
                                }
                                g.state = TunnelState::Connected {
                                    profile_id: prof_id,
                                    profile_name: prof_name.clone(),
                                    protocol: VpnProtocol::OpenVpn,
                                    endpoint: ep.clone(),
                                    connected_at: now,
                                };
                            }
                            g.push_log(LogLevel::Info, "OpenVPN", trimmed);
                            if still_active {
                                g.push_log(
                                    LogLevel::System,
                                    "OpenVPN",
                                    format!("Connected to '{prof_name}'."),
                                );
                            }
                        }
                        OpenVpnEvent::AuthFailed(msg) => {
                            g.push_log(LogLevel::Error, "OpenVPN", msg);
                            if still_active {
                                g.state = TunnelState::Error {
                                    profile_id: prof_id,
                                    profile_name: prof_name.clone(),
                                    message: "Authentication failed — check username and password."
                                        .to_string(),
                                };
                            }
                        }
                        OpenVpnEvent::Error(msg) => {
                            g.push_log(LogLevel::Error, "OpenVPN", msg);
                        }
                        OpenVpnEvent::Warn(msg) => {
                            g.push_log(LogLevel::Warn, "OpenVPN", msg);
                        }
                        OpenVpnEvent::Log => {
                            g.push_log(LogLevel::Info, "OpenVPN", trimmed);
                        }
                    }
                }
            }));
        }

        if let Some(err_stream) = stderr {
            let inner = self.inner.clone();
            let capture = error_capture.clone();
            reader_handles.push(std::thread::spawn(move || {
                let reader = BufReader::new(err_stream);
                for line in reader.lines().map_while(Result::ok) {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let event = parse_openvpn_line(trimmed);
                    if let Ok(mut cap) = capture.lock() {
                        cap.record_line(trimmed);
                        match &event {
                            OpenVpnEvent::AuthFailed(m) | OpenVpnEvent::Error(m) => {
                                cap.record_error(m);
                            }
                            _ => {}
                        }
                    }
                    let level = match event {
                        OpenVpnEvent::AuthFailed(_) | OpenVpnEvent::Error(_) => LogLevel::Error,
                        _ => LogLevel::Warn,
                    };
                    let mut g = inner.lock().unwrap();
                    g.push_log(level, "OpenVPN", trimmed);
                }
            }));
        }

        self.spawn_exit_watcher(
            session_id,
            child_arc,
            stop_flag,
            reader_handles,
            error_capture,
        );
        Ok(())
    }

    fn spawn_openconnect(
        &self,
        profile: VpnProfile,
        settings: &AppSettings,
        servercert_override: Option<String>,
        already_retried_cert: bool,
    ) -> Result<(), String> {
        let bin = resolve_binary(&settings.openconnect_path).ok_or_else(|| {
            format!(
                "OpenConnect executable not found at '{}'. Configure it under Settings.",
                settings.openconnect_path
            )
        })?;

        let vpnc_script = find_windows_vpnc_script(Some(&bin));
        let args = build_openconnect_args(
            &profile,
            vpnc_script.as_deref(),
            servercert_override.as_deref(),
        )?;

        let mut cmd = Command::new(&bin);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(parent) = bin.parent() {
            cmd.current_dir(parent);
        }
        configure_background_command(&mut cmd);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start OpenConnect ({}): {e}", bin.display()))?;
        if let Some(ref guard) = *self.job_guard {
            guard.assign(&child);
        }

        let mut password_sent_upfront = false;
        if let Some(mut stdin_writer) = child.stdin.take() {
            if !profile.password.is_empty() {
                let _ = writeln!(stdin_writer, "{}", profile.password);
                let _ = stdin_writer.flush();
                password_sent_upfront = true;
            }
        }
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let session_id = Uuid::new_v4();
        let started_at = Utc::now();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let child_arc = Arc::new(Mutex::new(child));
        let endpoint = profile.endpoint_display();
        let iface_pref = profile
            .interface_name
            .clone()
            .or_else(|| Some("wintun".to_string()));

        {
            let mut guard = self.inner.lock().unwrap();
            guard.pending_cert_prompt = None;
            guard.telemetry.reset_session(iface_pref);
            guard.state = TunnelState::Connecting {
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                protocol: VpnProtocol::OpenConnect,
                endpoint: endpoint.clone(),
                started_at,
            };
            guard.push_log(
                LogLevel::System,
                "OpenConnect",
                format!(
                    "Starting OpenConnect ({}) tunnel for '{}' ({endpoint})...",
                    profile.openconnect_protocol.cli_flag(),
                    profile.name
                ),
            );
            if password_sent_upfront {
                guard.push_log(
                    LogLevel::System,
                    "OpenConnect",
                    "Submitted password via stdin (--passwd-on-stdin).",
                );
            }
            guard.active = Some(ActiveTunnel {
                session_id,
                profile: profile.clone(),
                started_at,
                connected_at: None,
                child: child_arc.clone(),
                stop_flag: stop_flag.clone(),
                _temp_files: Vec::new(),
            });
        }

        let (tx, rx) = std::sync::mpsc::channel::<String>();
        for stream_opt in [
            stdout.map(BoxedReader::Stdout),
            stderr.map(BoxedReader::Stderr),
        ]
        .into_iter()
        .flatten()
        {
            let tx_c = tx.clone();
            let stop = stop_flag.clone();
            std::thread::spawn(move || {
                let mut reader = stream_opt;
                let mut buf = [0u8; 1024];
                while !stop.load(Ordering::SeqCst) {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx_c
                                .send(String::from_utf8_lossy(&buf[..n]).into_owned())
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                }
            });
        }
        drop(tx);

        let inner = self.inner.clone();
        let job_guard = self.job_guard.clone();
        let temp_dir = self.runtime_temp_dir.clone();
        let stop_reader = stop_flag.clone();
        let child_for_cert = child_arc.clone();
        let settings_clone = settings.clone();
        let profile_clone = profile.clone();
        let active_pin = servercert_override.or_else(|| profile.servercert.clone());

        let error_capture = Arc::new(Mutex::new(SessionErrorCapture::default()));
        let mut reader_handles = Vec::new();
        let capture_for_oc = error_capture.clone();

        reader_handles.push(std::thread::spawn(move || {
            let mut detected_pin: Option<String> = None;
            let mut line_buf = String::new();

            while let Ok(chunk) = rx.recv() {
                if stop_reader.load(Ordering::SeqCst) {
                    break;
                }
                line_buf.push_str(&chunk);

                while let Some(pos) = line_buf.find('\n') {
                    let line = line_buf[..pos].trim().to_string();
                    line_buf.drain(..=pos);
                    if line.is_empty() {
                        continue;
                    }
                    handle_openconnect_chunk(
                        &inner,
                        session_id,
                        &profile_clone,
                        &endpoint,
                        &line,
                        &mut detected_pin,
                        &capture_for_oc,
                    );
                }

                let rem = line_buf.trim().to_string();
                if !rem.is_empty() {
                    if let Some(pin) = extract_servercert_pin(&rem) {
                        detected_pin = Some(pin);
                    }
                    let events = parse_openconnect_output(&rem);
                    if events.contains(&OpenConnectEvent::PasswordPrompt)
                        || events.contains(&OpenConnectEvent::CertPrompt)
                    {
                        handle_openconnect_chunk(
                            &inner,
                            session_id,
                            &profile_clone,
                            &endpoint,
                            &rem,
                            &mut detected_pin,
                            &capture_for_oc,
                        );
                        line_buf.clear();
                    }
                }

                if let Some(ref pin) = detected_pin {
                    let already_pinned = active_pin.as_deref() == Some(pin.as_str());
                    if !already_pinned && !settings_clone.auto_trust_server_cert {
                        let mut g = inner.lock().unwrap();
                        if g.active
                            .as_ref()
                            .is_some_and(|a| a.session_id == session_id)
                            && g.pending_cert_prompt.is_none()
                        {
                            g.pending_cert_prompt = Some(CertPinPrompt {
                                profile_id: profile_clone.id,
                                profile_name: profile_clone.name.clone(),
                                server: endpoint.clone(),
                                fingerprint: pin.clone(),
                            });
                            g.push_log(
                                LogLevel::Warn,
                                "OpenConnect",
                                format!(
                                    "Unverified server certificate detected ({pin}). Approve in dialog to pin and connect."
                                ),
                            );
                            g.state = TunnelState::Error {
                                profile_id: profile_clone.id,
                                profile_name: profile_clone.name.clone(),
                                message: format!("Server certificate requires approval ({pin})."),
                            };
                            stop_reader.store(true, Ordering::SeqCst);
                            g.active = None;
                        }
                        drop(g);
                        if let Ok(mut ch) = child_for_cert.lock() {
                            terminate_child_tree(&mut ch);
                        }
                        return;
                    }
                }
            }

            let trailing = line_buf.trim().to_string();
            if !trailing.is_empty() && !stop_reader.load(Ordering::SeqCst) {
                handle_openconnect_chunk(
                    &inner,
                    session_id,
                    &profile_clone,
                    &endpoint,
                    &trailing,
                    &mut detected_pin,
                    &capture_for_oc,
                );
            }

            if let Some(pin) = detected_pin {
                let already_pinned = active_pin.as_deref() == Some(pin.as_str());
                let connected = inner.lock().unwrap().state.is_connected();
                if !connected
                    && !already_pinned
                    && !already_retried_cert
                    && settings_clone.auto_trust_server_cert
                    && !stop_reader.load(Ordering::SeqCst)
                {
                    {
                        let mut g = inner.lock().unwrap();
                        if g.active
                            .as_ref()
                            .is_some_and(|a| a.session_id == session_id)
                        {
                            g.active = None;
                            g.state = TunnelState::Disconnected;
                            g.auto_pinned_certs.push((profile_clone.id, pin.clone()));
                            g.push_log(
                                LogLevel::System,
                                "OpenConnect",
                                format!(
                                    "Auto-pinning certificate {pin} and retrying connection..."
                                ),
                            );
                        } else {
                            return;
                        }
                    }
                    let temp_mgr = VpnManager {
                        inner: inner.clone(),
                        job_guard: job_guard.clone(),
                        runtime_temp_dir: temp_dir.clone(),
                    };
                    let mut updated = profile_clone.clone();
                    updated.servercert = Some(pin.clone());
                    let _ = temp_mgr.connect_internal(updated, &settings_clone, Some(pin), true);
                }
            }
        }));

        self.spawn_exit_watcher(
            session_id,
            child_arc,
            stop_flag,
            reader_handles,
            error_capture,
        );
        Ok(())
    }

    fn spawn_exit_watcher(
        &self,
        session_id: Uuid,
        child_arc: Arc<Mutex<Child>>,
        stop_flag: Arc<AtomicBool>,
        reader_handles: Vec<std::thread::JoinHandle<()>>,
        error_capture: Arc<Mutex<SessionErrorCapture>>,
    ) {
        let inner = self.inner.clone();
        std::thread::spawn(move || loop {
            if stop_flag.load(Ordering::SeqCst) {
                return;
            }
            let exit_status = {
                match child_arc.lock() {
                    Ok(mut ch) => ch.try_wait().ok().flatten(),
                    Err(_) => return,
                }
            };
            if let Some(status) = exit_status {
                for handle in reader_handles {
                    let _ = handle.join();
                }

                let error_detail = error_capture.lock().ok().and_then(|cap| cap.summary());

                let mut g = inner.lock().unwrap();
                if !g
                    .active
                    .as_ref()
                    .is_some_and(|a| a.session_id == session_id)
                {
                    return;
                }
                let active = g.active.take().unwrap();
                let ended_at = Utc::now();
                let was_connected = active.connected_at.is_some();
                let duration_secs = active
                    .connected_at
                    .map(|c| ended_at.signed_duration_since(c).num_seconds().max(0) as u64)
                    .unwrap_or(0);
                let code = status.code().unwrap_or(-1);
                let msg = if was_connected {
                    match error_detail {
                        Some(ref detail) if code != 0 => {
                            format!("Tunnel exited (code {code}): {detail}")
                        }
                        _ => format!("Tunnel exited (code {code})."),
                    }
                } else {
                    match error_detail {
                        Some(ref detail) => {
                            format!("Startup failed (exit code {code}): {detail}")
                        }
                        None => format!(
                            "Connection failed on startup (exit code {code}). Check Logs for details."
                        ),
                    }
                };

                let hist_status = if was_connected {
                    HistoryStatus::Disconnected
                } else {
                    HistoryStatus::Failed
                };
                g.record_history(ConnectionHistoryEntry {
                    id: Uuid::new_v4(),
                    profile_id: active.profile.id,
                    profile_name: active.profile.name.clone(),
                    protocol: active.profile.protocol_badge(),
                    endpoint: active.profile.endpoint_display(),
                    status: hist_status,
                    started_at: active.started_at,
                    ended_at: Some(ended_at),
                    duration_secs,
                    message: msg.clone(),
                });

                if !matches!(g.state, TunnelState::Error { .. }) {
                    g.state = if was_connected {
                        TunnelState::Disconnected
                    } else {
                        TunnelState::Error {
                            profile_id: active.profile.id,
                            profile_name: active.profile.name.clone(),
                            message: msg.clone(),
                        }
                    };
                }
                g.telemetry.reset_session(None);
                g.push_log(
                    if was_connected {
                        LogLevel::System
                    } else {
                        LogLevel::Error
                    },
                    "Kraken",
                    msg,
                );
                return;
            }
            std::thread::sleep(Duration::from_millis(120));
        });
    }
}

impl Drop for VpnManager {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.disconnect_sync("Application closed");
        }
    }
}

enum BoxedReader {
    Stdout(std::process::ChildStdout),
    Stderr(std::process::ChildStderr),
}

impl Read for BoxedReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            BoxedReader::Stdout(s) => s.read(buf),
            BoxedReader::Stderr(s) => s.read(buf),
        }
    }
}

fn handle_openconnect_chunk(
    inner: &Arc<Mutex<ManagerInner>>,
    session_id: Uuid,
    profile: &VpnProfile,
    endpoint: &str,
    text: &str,
    detected_pin: &mut Option<String>,
    error_capture: &Arc<Mutex<SessionErrorCapture>>,
) {
    let events = parse_openconnect_output(text);
    if let Ok(mut cap) = error_capture.lock() {
        cap.record_line(text);
        for ev in &events {
            if let OpenConnectEvent::AuthFailed(m) | OpenConnectEvent::Error(m) = ev {
                cap.record_error(m);
            }
        }
    }
    let mut g = inner.lock().unwrap();
    let still_active = g
        .active
        .as_ref()
        .is_some_and(|a| a.session_id == session_id);

    let mut logged = false;
    for ev in events {
        match ev {
            OpenConnectEvent::CertPin(pin) => {
                *detected_pin = Some(pin.clone());
                g.push_log(
                    LogLevel::Warn,
                    "OpenConnect",
                    format!("Detected server cert pin: {pin}"),
                );
                logged = true;
            }
            OpenConnectEvent::Connected => {
                if still_active && !g.state.is_connected() {
                    let now = Utc::now();
                    if let Some(ref mut a) = g.active {
                        a.connected_at = Some(now);
                    }
                    g.state = TunnelState::Connected {
                        profile_id: profile.id,
                        profile_name: profile.name.clone(),
                        protocol: VpnProtocol::OpenConnect,
                        endpoint: endpoint.to_string(),
                        connected_at: now,
                    };
                    g.push_log(LogLevel::Info, "OpenConnect", text);
                    g.push_log(
                        LogLevel::System,
                        "OpenConnect",
                        format!("Connected to '{}'.", profile.name),
                    );
                    logged = true;
                }
            }
            OpenConnectEvent::AuthFailed(msg) => {
                g.push_log(LogLevel::Error, "OpenConnect", msg);
                if still_active && detected_pin.is_none() {
                    g.state = TunnelState::Error {
                        profile_id: profile.id,
                        profile_name: profile.name.clone(),
                        message: "Authentication failed — check username, password, or authgroup."
                            .to_string(),
                    };
                }
                logged = true;
            }
            OpenConnectEvent::Error(msg) => {
                g.push_log(LogLevel::Error, "OpenConnect", msg);
                logged = true;
            }
            OpenConnectEvent::CertPrompt
            | OpenConnectEvent::PasswordPrompt
            | OpenConnectEvent::Log => {}
        }
    }

    if !logged {
        g.push_log(LogLevel::Info, "OpenConnect", text);
    }
}
