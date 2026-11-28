use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::models::VpnProfile;

#[derive(Debug)]
pub struct SecureTempFile {
    path: PathBuf,
    shred: bool,
}

impl SecureTempFile {
    pub fn write(
        dir: &Path,
        prefix: &str,
        ext: &str,
        content: &str,
        shred: bool,
    ) -> Result<Self, String> {
        fs::create_dir_all(dir).map_err(|e| format!("Failed to prepare temp directory: {e}"))?;
        let filename = format!("{prefix}_{}.{ext}", Uuid::new_v4().simple());
        let path = dir.join(filename);
        fs::write(&path, content.as_bytes())
            .map_err(|e| format!("Failed to write temporary OpenVPN file: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o600));
        }

        Ok(Self { path, shred })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for SecureTempFile {
    fn drop(&mut self) {
        if self.shred {
            if let Ok(meta) = fs::metadata(&self.path) {
                let zeros = vec![0u8; meta.len().min(64 * 1024) as usize];
                let _ = fs::write(&self.path, zeros);
            }
        }
        let _ = fs::remove_file(&self.path);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenVpnEvent {
    Connected,
    AuthFailed(String),
    Error(String),
    Warn(String),
    Log,
}

pub fn pick_ephemeral_management_port() -> Option<u16> {
    TcpListener::bind(("127.0.0.1", 0))
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
}

pub struct PreparedOpenVpn {
    pub args: Vec<String>,
    pub management_port: Option<u16>,
    pub working_dir: Option<PathBuf>,
    pub _temp_files: Vec<SecureTempFile>,
}

pub fn config_requires_interactive_auth(config_text: &str) -> bool {
    let mut in_block = false;
    for raw_line in config_text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with("</") && line.ends_with('>') {
            in_block = false;
            continue;
        }
        if line.starts_with('<') && line.ends_with('>') {
            in_block = true;
            continue;
        }
        if in_block {
            continue;
        }
        let mut parts = line.split_whitespace();
        if let Some(directive) = parts.next() {
            if directive.eq_ignore_ascii_case("auth-user-pass") {
                let next_arg = parts.next();
                if next_arg.is_none()
                    || next_arg.is_some_and(|a| a.starts_with('#') || a.starts_with(';'))
                {
                    return true;
                }
            }
        }
    }
    false
}

pub fn config_has_encrypted_key(config_text: &str) -> bool {
    config_text.contains("ENCRYPTED PRIVATE KEY")
        || config_text.contains("Proc-Type: 4,ENCRYPTED")
        || config_text.contains("DEK-Info:")
        || config_text.to_ascii_lowercase().contains("<pkcs12>")
}

pub fn is_generic_fatal_footer(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("exiting due to fatal error")
        || lower.contains("use --help for more information")
}

pub fn strip_openvpn_timestamp(line: &str) -> &str {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    if bytes.len() > 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b' '
    {
        trimmed[20..].trim()
    } else {
        trimmed
    }
}

pub fn prepare_openvpn(profile: &VpnProfile, temp_dir: &Path) -> Result<PreparedOpenVpn, String> {
    let mut temp_files = Vec::new();

    let working_dir = profile
        .ovpn_path
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|p| Path::new(p).parent().map(Path::to_path_buf))
        .filter(|dir| dir.is_dir());

    let (config_path, config_text): (PathBuf, String) = if let Some(inline) = profile
        .ovpn_content
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        let tmp = SecureTempFile::write(temp_dir, "ovpn_cfg", "ovpn", inline, false)?;
        let p = tmp.path().to_path_buf();
        temp_files.push(tmp);
        (p, inline.to_string())
    } else if let Some(disk_path) = profile
        .ovpn_path
        .as_deref()
        .filter(|s| !s.trim().is_empty())
    {
        let p = PathBuf::from(disk_path.trim());
        if !p.is_file() {
            return Err(format!("OpenVPN config file not found: {}", p.display()));
        }
        let txt = fs::read_to_string(&p)
            .map_err(|e| format!("Failed to read .ovpn file {}: {e}", p.display()))?;
        (p, txt)
    } else {
        return Err(
            "OpenVPN profile requires an .ovpn file path or imported config content.".to_string(),
        );
    };

    let requires_auth = config_requires_interactive_auth(&config_text);
    let has_creds = !profile.username.trim().is_empty() || !profile.password.is_empty();
    let has_encrypted_key = config_has_encrypted_key(&config_text);

    let resolved_askpass = if !profile.key_passphrase.is_empty() {
        Some(profile.key_passphrase.as_str())
    } else if has_encrypted_key && !profile.password.is_empty() {
        Some(profile.password.as_str())
    } else {
        None
    };

    if requires_auth && !has_creds {
        return Err(
            "This .ovpn profile requires a username and password (auth-user-pass). Edit the profile and enter your credentials."
                .to_string(),
        );
    }

    if has_encrypted_key && resolved_askpass.is_none() {
        return Err(
            "This .ovpn profile uses an encrypted private key. Edit the profile and enter the Private Key Passphrase."
                .to_string(),
        );
    }

    let wants_user_pass = requires_auth
        || (!has_encrypted_key
            && !profile.username.trim().is_empty()
            && !profile.password.is_empty());

    let mut auth_file_path: Option<PathBuf> = None;
    if wants_user_pass && has_creds {
        let auth_payload = format!("{}\n{}\n", profile.username.trim(), profile.password);
        let auth_tmp = SecureTempFile::write(temp_dir, "ovpn_auth", "txt", &auth_payload, true)?;
        auth_file_path = Some(auth_tmp.path().to_path_buf());
        temp_files.push(auth_tmp);
    }

    let mut askpass_path: Option<PathBuf> = None;
    if let Some(passphrase) = resolved_askpass {
        if has_encrypted_key || !profile.key_passphrase.is_empty() {
            let ask_tmp = SecureTempFile::write(
                temp_dir,
                "ovpn_askpass",
                "txt",
                &format!("{passphrase}\n"),
                true,
            )?;
            askpass_path = Some(ask_tmp.path().to_path_buf());
            temp_files.push(ask_tmp);
        }
    }

    let management_port = pick_ephemeral_management_port();
    let args = build_openvpn_args(
        &config_path,
        auth_file_path.as_deref(),
        askpass_path.as_deref(),
        management_port,
    );

    Ok(PreparedOpenVpn {
        args,
        management_port,
        working_dir,
        _temp_files: temp_files,
    })
}

pub fn build_openvpn_args(
    config_path: &Path,
    auth_file: Option<&Path>,
    askpass_file: Option<&Path>,
    management_port: Option<u16>,
) -> Vec<String> {
    let mut args = vec![
        "--config".to_string(),
        config_path.to_string_lossy().into_owned(),
    ];
    if let Some(port) = management_port {
        args.push("--management".to_string());
        args.push("127.0.0.1".to_string());
        args.push(port.to_string());
    }
    if let Some(auth) = auth_file {
        args.push("--auth-user-pass".to_string());
        args.push(auth.to_string_lossy().into_owned());
    }
    if let Some(askpass) = askpass_file {
        args.push("--askpass".to_string());
        args.push(askpass.to_string_lossy().into_owned());
    }
    args
}

pub fn parse_openvpn_line(line: &str) -> OpenVpnEvent {
    let trimmed = line.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.contains("Initialization Sequence Completed") {
        return OpenVpnEvent::Connected;
    }
    if trimmed.contains("AUTH_FAILED")
        || lower.contains("auth-failure")
        || lower.contains("auth username is empty")
        || lower.contains("could not read auth")
    {
        return OpenVpnEvent::AuthFailed(trimmed.to_string());
    }
    if lower.contains("options error:")
        || lower.contains("error:")
        || lower.contains("openssl: error:")
        || trimmed.contains("FATAL")
        || trimmed.contains("TLS Error:")
        || trimmed.contains("TLS key negotiation failed")
        || trimmed.contains("Exiting due to fatal error")
        || trimmed.contains("Cannot open TUN/TAP dev")
        || trimmed.contains("CreateFile failed on TAP device")
        || trimmed.contains("RESOLVE: Cannot resolve host")
        || lower.contains("cannot load private key")
        || lower.contains("private key password verification failed")
        || lower.contains("unrecognized option")
        || lower.contains("no such file or directory")
        || lower.contains("cannot open")
        || lower.contains("access is denied")
        || lower.contains("administrator privileges")
        || lower.contains("failed to open")
        || lower.contains("all tap-windows adapters")
    {
        return OpenVpnEvent::Error(trimmed.to_string());
    }
    if trimmed.contains("WARNING:") || trimmed.contains("DEPRECATED OPTION:") {
        return OpenVpnEvent::Warn(trimmed.to_string());
    }
    OpenVpnEvent::Log
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_openvpn_args_cleanly() {
        let args = build_openvpn_args(
            Path::new("/tmp/test.ovpn"),
            Some(Path::new("/tmp/auth.txt")),
            None,
            Some(12345),
        );
        assert_eq!(
            args,
            vec![
                "--config",
                "/tmp/test.ovpn",
                "--management",
                "127.0.0.1",
                "12345",
                "--auth-user-pass",
                "/tmp/auth.txt",
            ]
        );
    }

    #[test]
    fn parses_openvpn_output_events() {
        assert_eq!(
            parse_openvpn_line("2026-11-25 14:00:00 Initialization Sequence Completed"),
            OpenVpnEvent::Connected
        );
        assert!(matches!(
            parse_openvpn_line("AUTH: Received control message: AUTH_FAILED"),
            OpenVpnEvent::AuthFailed(_)
        ));
        assert!(matches!(
            parse_openvpn_line("TLS Error: TLS handshake failed"),
            OpenVpnEvent::Error(_)
        ));
        assert_eq!(
            parse_openvpn_line("UDPv4 link remote: [AF_INET]1.2.3.4:1194"),
            OpenVpnEvent::Log
        );
        assert!(matches!(
            parse_openvpn_line("2026-11-25 17:14:18 ERROR: Auth username is empty"),
            OpenVpnEvent::AuthFailed(_)
        ));
        assert!(matches!(
            parse_openvpn_line("Options error: Unrecognized option or missing parameter"),
            OpenVpnEvent::Error(_)
        ));
        assert!(matches!(
            parse_openvpn_line("2026-11-25 17:14:16 WARNING: Using --management without password"),
            OpenVpnEvent::Warn(_)
        ));
    }

    #[test]
    fn detects_interactive_auth_requirement_and_strips_timestamps() {
        assert!(config_requires_interactive_auth("client\nauth-user-pass\n"));
        assert!(!config_requires_interactive_auth(
            "client\n# auth-user-pass\n"
        ));
        assert!(!config_requires_interactive_auth(
            "client\nauth-user-pass creds.txt\n"
        ));
        assert_eq!(
            strip_openvpn_timestamp("2026-11-25 17:14:18 ERROR: Auth username is empty"),
            "ERROR: Auth username is empty"
        );
        assert_eq!(
            strip_openvpn_timestamp("Options error: You must define CA file"),
            "Options error: You must define CA file"
        );
    }

    #[test]
    fn prepare_openvpn_rejects_missing_credentials_clearly() {
        let dir = std::env::temp_dir().join(format!("kraken_auth_req_{}", Uuid::new_v4()));
        let mut prof = VpnProfile::new_openvpn("Needs Auth");
        prof.ovpn_content =
            Some("client\ndev tun\nremote vpn.example 1194\nauth-user-pass\n".into());
        let err = prepare_openvpn(&prof, &dir).err().unwrap();
        assert!(err.contains("username and password"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prepare_openvpn_passes_askpass_for_encrypted_key_even_when_username_is_set() {
        let dir = std::env::temp_dir().join(format!("kraken_askpass_{}", Uuid::new_v4()));
        let mut prof = VpnProfile::new_openvpn("Encrypted Key Profile");
        prof.ovpn_content = Some(
            "client\ndev tun\nremote vpn.example 1194\n<key>\n-----BEGIN ENCRYPTED PRIVATE KEY-----\nABC\n-----END ENCRYPTED PRIVATE KEY-----\n</key>\n"
                .into(),
        );

        let err = prepare_openvpn(&prof, &dir).err().unwrap();
        assert!(err.contains("Private Key Passphrase"));

        prof.username = "pandu".into();
        prof.password = "my-key-pass".into();
        let prepared = prepare_openvpn(&prof, &dir).unwrap();
        let ask_idx = prepared
            .args
            .iter()
            .position(|a| a == "--askpass")
            .expect("expected --askpass flag");
        assert_eq!(
            fs::read_to_string(&prepared.args[ask_idx + 1]).unwrap(),
            "my-key-pass\n"
        );
        assert!(
            !prepared.args.iter().any(|a| a == "--auth-user-pass"),
            "should not pass --auth-user-pass when config has no auth-user-pass"
        );

        prof.ovpn_content = Some(
            "client\ndev tun\nremote vpn.example 1194\nauth-user-pass\n<pkcs12>\nMII\n</pkcs12>\n"
                .into(),
        );
        prof.key_passphrase = "separate-p12-pass".into();
        let both = prepare_openvpn(&prof, &dir).unwrap();
        assert!(both.args.iter().any(|a| a == "--auth-user-pass"));
        let ask_idx2 = both.args.iter().position(|a| a == "--askpass").unwrap();
        assert_eq!(
            fs::read_to_string(&both.args[ask_idx2 + 1]).unwrap(),
            "separate-p12-pass\n"
        );

        assert!(matches!(
            parse_openvpn_line(
                "2026-11-25 17:25:42 Error: private key password verification failed"
            ),
            OpenVpnEvent::Error(_)
        ));
        assert!(matches!(
            parse_openvpn_line("2026-11-25 17:25:42 Cannot load private key file [[INLINE]]"),
            OpenVpnEvent::Error(_)
        ));

        drop(prepared);
        drop(both);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn secure_temp_file_cleans_up_on_drop() {
        let dir = std::env::temp_dir().join(format!("kraken_ovpn_tmp_{}", Uuid::new_v4()));
        let path = {
            let tmp = SecureTempFile::write(&dir, "auth", "txt", "user\nsecret\n", true).unwrap();
            let p = tmp.path().to_path_buf();
            assert!(p.exists());
            p
        };
        assert!(!path.exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
