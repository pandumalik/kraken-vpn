use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use regex::Regex;

use crate::models::{AppSettings, VpnProfile};
use crate::platform::{configure_background_command, resolve_binary, terminate_child_tree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenConnectEvent {
    Connected,
    CertPin(String),
    CertPrompt,
    PasswordPrompt,
    AuthFailed(String),
    Error(String),
    Log,
}

#[derive(Debug, Clone)]
pub struct OpenConnectTestResult {
    pub success: bool,
    pub message: String,
    pub detected_pin: Option<String>,
}

pub fn format_openconnect_target(server: &str, port: Option<u16>) -> Result<String, String> {
    let trimmed = server.trim();
    if trimmed.is_empty() {
        return Err("OpenConnect server URL or hostname is required.".to_string());
    }
    if trimmed.contains("://") {
        if let Some(p) = port {
            let after_scheme = trimmed
                .split_once("://")
                .map(|(_, rest)| rest)
                .unwrap_or("");
            let host_part = after_scheme.split('/').next().unwrap_or("");
            if !host_part.contains(':') && p != 443 {
                let (scheme, rest) = trimmed.split_once("://").unwrap();
                if let Some((host, path)) = rest.split_once('/') {
                    return Ok(format!("{scheme}://{host}:{p}/{path}"));
                }
                return Ok(format!("{scheme}://{rest}:{p}"));
            }
        }
        return Ok(trimmed.to_string());
    }
    if trimmed.contains(':') {
        Ok(trimmed.to_string())
    } else if let Some(p) = port {
        Ok(format!("{trimmed}:{p}"))
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn find_windows_vpnc_script(openconnect_binary: Option<&Path>) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Some(bin) = openconnect_binary {
            if let Some(parent) = bin.parent() {
                candidates.push(parent.join("vpnc-script-win.js"));
                candidates.push(parent.join("vpnc-script.js"));
            }
        }
        candidates.push(PathBuf::from(
            r"C:\Program Files\OpenConnect-GUI\vpnc-script-win.js",
        ));
        candidates.push(PathBuf::from(
            r"C:\Program Files\OpenConnect-GUI\vpnc-script.js",
        ));
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join("vpnc-script-win.js"));
            candidates.push(cwd.join("vpnc-script.js"));
        }
        candidates.into_iter().find(|p| p.is_file())
    }
    #[cfg(not(windows))]
    {
        let _ = openconnect_binary;
        None
    }
}

pub fn build_openconnect_args(
    profile: &VpnProfile,
    vpnc_script: Option<&Path>,
    servercert_override: Option<&str>,
) -> Result<Vec<String>, String> {
    let target = format_openconnect_target(&profile.server, profile.port)?;
    let mut args = vec![
        format!("--protocol={}", profile.openconnect_protocol.cli_flag()),
        target,
        "--no-dtls".to_string(),
        "--non-inter".to_string(),
    ];

    if matches!(
        profile.openconnect_protocol,
        crate::models::OpenConnectProtocol::Fortinet
    ) {
        args.push("--force-dpd=60".to_string());
    }

    if !profile.password.is_empty() {
        args.push("--passwd-on-stdin".to_string());
    }

    if let Some(script) = vpnc_script {
        args.push("--script".to_string());
        args.push(script.to_string_lossy().into_owned());
    }

    if let Some(group) = profile
        .authgroup
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        args.push(format!("--authgroup={group}"));
    }

    let cert_pin = servercert_override
        .or(profile.servercert.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(pin) = cert_pin {
        args.push("--servercert".to_string());
        args.push(pin.to_string());
    }

    if !profile.username.trim().is_empty() {
        args.push("--user".to_string());
        args.push(profile.username.trim().to_string());
    }

    if let Some(iface) = profile
        .interface_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        args.push(format!("--interface={iface}"));
    }

    Ok(args)
}

pub fn extract_servercert_pin(text: &str) -> Option<String> {
    static RE_PIN: OnceLock<Regex> = OnceLock::new();
    let re = RE_PIN.get_or_init(|| Regex::new(r"(?i)pin-sha256:([a-zA-Z0-9+/=]+)").unwrap());
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| format!("pin-sha256:{}", m.as_str()))
}

fn is_cert_prompt_lower(lower: &str) -> bool {
    [
        "enter 'yes' to accept",
        "confirm certificate",
        "accept certificate",
        "accept this certificate",
        "do you want to accept",
        "trust this certificate",
        "server certificate verify failed",
        "signer not found",
        "certificate verify failed",
        "(yes/no)",
        "(y/n)",
        "[y/n]",
        "accept always",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_password_prompt_lower(lower: &str) -> bool {
    lower.contains("password:") || lower.contains("enter password")
}

fn is_connected_indicator_lower(lower: &str) -> bool {
    [
        "connected as",
        "configured as",
        "with ssl connected",
        "using tap-windows device",
        "using wintun",
        "tunnel successfully opened",
        "established dtls connection",
        "session tunneling through https",
        "ssl tunnel connected",
        "dtls tunnel connected",
        "esp tunnel connected",
        "tunnel connected",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

pub fn is_cert_prompt(text: &str) -> bool {
    is_cert_prompt_lower(&text.to_ascii_lowercase())
}

pub fn is_password_prompt(text: &str) -> bool {
    is_password_prompt_lower(&text.to_ascii_lowercase())
}

pub fn is_connected_indicator(text: &str) -> bool {
    is_connected_indicator_lower(&text.to_ascii_lowercase())
}

pub fn parse_openconnect_output(text: &str) -> Vec<OpenConnectEvent> {
    let mut events = Vec::new();
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return events;
    }

    if let Some(pin) = extract_servercert_pin(trimmed) {
        events.push(OpenConnectEvent::CertPin(pin));
    }

    let lower = trimmed.to_ascii_lowercase();
    if is_connected_indicator_lower(&lower) {
        events.push(OpenConnectEvent::Connected);
    }
    if is_cert_prompt_lower(&lower) {
        events.push(OpenConnectEvent::CertPrompt);
    }
    if is_password_prompt_lower(&lower) {
        events.push(OpenConnectEvent::PasswordPrompt);
    }

    if lower.contains("authentication failed")
        || lower.contains("login failed")
        || lower.contains("failed to complete authentication")
    {
        events.push(OpenConnectEvent::AuthFailed(trimmed.to_string()));
    } else if lower.contains("failed to connect")
        || lower.contains("unknown host")
        || lower.contains("tls handshake failed")
        || lower.contains("failed to open")
        || lower.contains("getaddrinfo failed")
    {
        events.push(OpenConnectEvent::Error(trimmed.to_string()));
    }

    if events.is_empty() {
        events.push(OpenConnectEvent::Log);
    }
    events
}

pub fn test_openconnect_endpoint(
    profile: &VpnProfile,
    settings: &AppSettings,
) -> OpenConnectTestResult {
    let Some(resolved) = resolve_binary(&settings.openconnect_path) else {
        return OpenConnectTestResult {
            success: false,
            message: format!(
                "OpenConnect binary not found: '{}'",
                settings.openconnect_path
            ),
            detected_pin: None,
        };
    };

    let args = match build_openconnect_args(
        profile,
        find_windows_vpnc_script(Some(&resolved)).as_deref(),
        None,
    ) {
        Ok(a) => a,
        Err(e) => {
            return OpenConnectTestResult {
                success: false,
                message: e,
                detected_pin: None,
            }
        }
    };

    let mut cmd = Command::new(&resolved);
    cmd.args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(parent) = resolved.parent() {
        cmd.current_dir(parent);
    }
    configure_background_command(&mut cmd);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return OpenConnectTestResult {
                success: false,
                message: format!("Failed to spawn openconnect: {e}"),
                detected_pin: None,
            }
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        if !profile.password.is_empty() {
            let _ = writeln!(stdin, "{}", profile.password);
            let _ = stdin.flush();
        }
    }

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    if let Some(mut stdout) = child.stdout.take() {
        let tx_out = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(n) = stdout.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = tx_out.send(String::from_utf8_lossy(&buf[..n]).into_owned());
            }
        });
    }
    if let Some(mut stderr) = child.stderr.take() {
        let tx_err = tx.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok(n) = stderr.read(&mut buf) {
                if n == 0 {
                    break;
                }
                let _ = tx_err.send(String::from_utf8_lossy(&buf[..n]).into_owned());
            }
        });
    }
    drop(tx);

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut detected_pin: Option<String> = None;
    let mut combined = String::new();

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(200))) {
            Ok(chunk) => {
                combined.push_str(&chunk);
                for ev in parse_openconnect_output(&chunk) {
                    match ev {
                        OpenConnectEvent::CertPin(pin) => detected_pin = Some(pin),
                        OpenConnectEvent::Connected => {
                            terminate_child_tree(&mut child);
                            return OpenConnectTestResult {
                                success: true,
                                message: "OpenConnect tunnel handshake succeeded!".to_string(),
                                detected_pin,
                            };
                        }
                        OpenConnectEvent::PasswordPrompt => {
                            if profile.password.is_empty() {
                                terminate_child_tree(&mut child);
                                return OpenConnectTestResult {
                                    success: true,
                                    message: "Server reachable (password prompt received)."
                                        .to_string(),
                                    detected_pin,
                                };
                            }
                        }
                        OpenConnectEvent::AuthFailed(msg) => {
                            let pin = detected_pin
                                .clone()
                                .or_else(|| extract_servercert_pin(&combined));
                            terminate_child_tree(&mut child);
                            if let Some(pin) = pin {
                                return OpenConnectTestResult {
                                    success: false,
                                    message: format!(
                                        "Server certificate not trusted yet. Detected pin: {pin}"
                                    ),
                                    detected_pin: Some(pin),
                                };
                            }
                            return OpenConnectTestResult {
                                success: false,
                                message: format!("Authentication failed: {msg}"),
                                detected_pin: None,
                            };
                        }
                        OpenConnectEvent::Error(msg) => {
                            let pin = detected_pin
                                .clone()
                                .or_else(|| extract_servercert_pin(&combined));
                            if let Some(pin) = pin {
                                terminate_child_tree(&mut child);
                                return OpenConnectTestResult {
                                    success: false,
                                    message: format!(
                                        "Server certificate not trusted yet. Detected pin: {pin}"
                                    ),
                                    detected_pin: Some(pin),
                                };
                            }
                            terminate_child_tree(&mut child);
                            return OpenConnectTestResult {
                                success: false,
                                message: msg,
                                detected_pin: None,
                            };
                        }
                        OpenConnectEvent::CertPrompt => {
                            if let Some(pin) = detected_pin
                                .clone()
                                .or_else(|| extract_servercert_pin(&combined))
                            {
                                terminate_child_tree(&mut child);
                                return OpenConnectTestResult {
                                    success: false,
                                    message: format!(
                                        "Server certificate not trusted yet. Detected pin: {pin}"
                                    ),
                                    detected_pin: Some(pin),
                                };
                            }
                        }
                        OpenConnectEvent::Log => {}
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(Some(status)) = child.try_wait() {
                    let pin = detected_pin.or_else(|| extract_servercert_pin(&combined));
                    let msg = if let Some(ref p) = pin {
                        format!("Server certificate not trusted yet. Detected pin: {p}")
                    } else {
                        format!(
                            "OpenConnect exited with code {}.",
                            status.code().unwrap_or(-1)
                        )
                    };
                    return OpenConnectTestResult {
                        success: false,
                        message: msg,
                        detected_pin: pin,
                    };
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    terminate_child_tree(&mut child);
    let pin = detected_pin.or_else(|| extract_servercert_pin(&combined));
    if let Some(pin) = pin {
        return OpenConnectTestResult {
            success: false,
            message: format!("Server certificate not trusted yet. Detected pin: {pin}"),
            detected_pin: Some(pin),
        };
    }
    OpenConnectTestResult {
        success: false,
        message: "Connection test timed out after 8 seconds.".to_string(),
        detected_pin: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OpenConnectProtocol;

    #[test]
    fn builds_openconnect_args_without_leaking_password() {
        let mut p =
            VpnProfile::new_openconnect("Corp", "vpn.corp.example", OpenConnectProtocol::Fortinet);
        p.port = Some(8443);
        p.username = "jdoe".into();
        p.password = "TOP_SECRET_PASSWORD".into();
        p.authgroup = Some("engineering".into());
        p.servercert = Some("pin-sha256:AbCdEf123456=".into());
        p.interface_name = Some("wintun".into());

        let args = build_openconnect_args(&p, None, None).unwrap();
        assert_eq!(args[0], "--protocol=fortinet");
        assert_eq!(args[1], "vpn.corp.example:8443");
        assert!(args.contains(&"--non-inter".to_string()));
        assert!(args.contains(&"--force-dpd=60".to_string()));
        assert!(args.contains(&"--passwd-on-stdin".to_string()));
        assert!(args.contains(&"--authgroup=engineering".to_string()));
        assert!(args.contains(&"--servercert".to_string()));
        assert!(args.contains(&"pin-sha256:AbCdEf123456=".to_string()));
        assert!(args.contains(&"--user".to_string()));
        assert!(args.contains(&"jdoe".to_string()));
        assert!(args.contains(&"--interface=wintun".to_string()));
        assert!(!args.iter().any(|a| a.contains("TOP_SECRET_PASSWORD")));

        p.password.clear();
        let no_pw_args = build_openconnect_args(&p, None, None).unwrap();
        assert!(no_pw_args.contains(&"--non-inter".to_string()));
        assert!(!no_pw_args.contains(&"--passwd-on-stdin".to_string()));
    }

    #[test]
    fn formats_urls_and_host_port_targets() {
        assert_eq!(
            format_openconnect_target("vpn.example.com", Some(443)).unwrap(),
            "vpn.example.com:443"
        );
        assert_eq!(
            format_openconnect_target("vpn.example.com:8888", Some(443)).unwrap(),
            "vpn.example.com:8888"
        );
        assert_eq!(
            format_openconnect_target("https://vpn.example.com/ssl", Some(443)).unwrap(),
            "https://vpn.example.com/ssl"
        );
        assert_eq!(
            format_openconnect_target("https://vpn.example.com/ssl", Some(8443)).unwrap(),
            "https://vpn.example.com:8443/ssl"
        );
        assert!(format_openconnect_target("   ", Some(443)).is_err());
    }

    #[test]
    fn extracts_cert_pin_and_detects_prompts() {
        let sample = "Server SSL certificate didn't match: pin-sha256:abc123XYZ+/=\nEnter 'yes' to accept, 'no' to abort";
        assert_eq!(
            extract_servercert_pin(sample).as_deref(),
            Some("pin-sha256:abc123XYZ+/=")
        );
        assert!(is_cert_prompt(sample));

        let events = parse_openconnect_output(sample);
        assert!(events.contains(&OpenConnectEvent::CertPin("pin-sha256:abc123XYZ+/=".into())));
        assert!(events.contains(&OpenConnectEvent::CertPrompt));

        assert!(parse_openconnect_output("Password:").contains(&OpenConnectEvent::PasswordPrompt));
        assert!(
            parse_openconnect_output("Connected as 10.200.0.5, using SSL")
                .contains(&OpenConnectEvent::Connected)
        );
    }

    #[test]
    fn maps_all_openconnect_protocols() {
        assert_eq!(OpenConnectProtocol::AnyConnect.cli_flag(), "anyconnect");
        assert_eq!(OpenConnectProtocol::Fortinet.cli_flag(), "fortinet");
        assert_eq!(OpenConnectProtocol::GlobalProtect.cli_flag(), "gp");
        assert_eq!(OpenConnectProtocol::Pulse.cli_flag(), "pulse");
        assert_eq!(OpenConnectProtocol::JuniperNc.cli_flag(), "nc");
        assert_eq!(
            OpenConnectProtocol::parse_flexible("Fortinet SSL"),
            OpenConnectProtocol::Fortinet
        );
        assert_eq!(
            OpenConnectProtocol::parse_flexible("GlobalProtect"),
            OpenConnectProtocol::GlobalProtect
        );
        assert_eq!(
            OpenConnectProtocol::parse_flexible("Juniper"),
            OpenConnectProtocol::JuniperNc
        );
    }
}
