use std::process::Child;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::models::{ConnectionHistoryEntry, HistoryStatus, LogLevel};
use crate::vpn::manager::{SessionErrorCapture, VpnManager};
use crate::vpn::state::TunnelState;

impl VpnManager {
    pub(super) fn spawn_exit_watcher(
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
