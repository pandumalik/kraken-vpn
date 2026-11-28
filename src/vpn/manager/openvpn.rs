use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Utc;
use uuid::Uuid;

use crate::models::{AppSettings, LogLevel, VpnProfile, VpnProtocol};
use crate::platform::{configure_background_command, resolve_binary};
use crate::telemetry::parse_openvpn_bytecount;
use crate::vpn::manager::{ActiveTunnel, SessionErrorCapture, VpnManager};
use crate::vpn::openvpn::{parse_openvpn_line, prepare_openvpn, OpenVpnEvent};
use crate::vpn::state::TunnelState;

impl VpnManager {
    pub(super) fn spawn_openvpn(
        &self,
        profile: VpnProfile,
        settings: &AppSettings,
    ) -> Result<(), String> {
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
}
