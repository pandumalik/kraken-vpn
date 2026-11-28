use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use chrono::Utc;
use uuid::Uuid;

use crate::models::{AppSettings, CertPinPrompt, LogLevel, VpnProfile, VpnProtocol};
use crate::platform::{configure_background_command, resolve_binary, terminate_child_tree};
use crate::vpn::manager::{ActiveTunnel, ManagerInner, SessionErrorCapture, VpnManager};
use crate::vpn::openconnect::{
    build_openconnect_args, extract_servercert_pin, find_windows_vpnc_script,
    parse_openconnect_output, OpenConnectEvent,
};
use crate::vpn::state::TunnelState;

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

impl VpnManager {
    pub(super) fn spawn_openconnect(
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
