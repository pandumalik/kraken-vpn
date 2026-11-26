//! OpenVPN `.ovpn` configuration file parser and profile importer.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::models::{VpnProfile, VpnProtocol};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedOvpn {
    pub suggested_name: String,
    pub remote_host: Option<String>,
    pub remote_port: u16,
    pub transport: String,
    pub cipher: Option<String>,
    pub requires_user_pass: bool,
    pub has_inline_cert: bool,
    pub has_encrypted_key: bool,
}

/// Parses an `.ovpn` file's text and filename into structured profile metadata.
pub fn parse_ovpn(content: &str, filename: &str) -> ParsedOvpn {
    static RE_REMOTE: OnceLock<Regex> = OnceLock::new();
    static RE_PORT: OnceLock<Regex> = OnceLock::new();
    static RE_PROTO: OnceLock<Regex> = OnceLock::new();
    static RE_CIPHER: OnceLock<Regex> = OnceLock::new();

    let re_remote = RE_REMOTE
        .get_or_init(|| Regex::new(r"(?i)^remote\s+([^\s]+)(?:\s+(\d+))?(?:\s+(\S+))?").unwrap());
    let re_port = RE_PORT.get_or_init(|| Regex::new(r"(?i)^port\s+(\d+)").unwrap());
    let re_proto = RE_PROTO.get_or_init(|| Regex::new(r"(?i)^proto\s+(\S+)").unwrap());
    let re_cipher =
        RE_CIPHER.get_or_init(|| Regex::new(r"(?i)^(?:cipher|data-ciphers)\s+(.+)").unwrap());

    let mut remote_host: Option<String> = None;
    let mut remote_port: u16 = 1194;
    let mut transport = "UDP".to_string();
    let mut cipher: Option<String> = None;
    let mut requires_user_pass = false;
    let mut in_block = false;

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        // Skip inline PEM blocks (<ca>...</ca>, <cert>...</cert>, <key>...</key>, etc.)
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

        if remote_host.is_none() {
            if let Some(caps) = re_remote.captures(line) {
                remote_host = caps.get(1).map(|m| m.as_str().to_string());
                if let Some(p) = caps.get(2).and_then(|m| m.as_str().parse::<u16>().ok()) {
                    remote_port = p;
                }
                if let Some(proto_hint) = caps.get(3).map(|m| m.as_str().to_ascii_lowercase()) {
                    if proto_hint.contains("tcp") {
                        transport = "TCP".to_string();
                    }
                }
            }
        }
        if let Some(caps) = re_port.captures(line) {
            if let Some(p) = caps.get(1).and_then(|m| m.as_str().parse::<u16>().ok()) {
                remote_port = p;
            }
        }
        if let Some(caps) = re_proto.captures(line) {
            let p = caps
                .get(1)
                .map(|m| m.as_str().to_ascii_lowercase())
                .unwrap_or_default();
            transport = if p.contains("tcp") {
                "TCP".to_string()
            } else {
                "UDP".to_string()
            };
        }
        if cipher.is_none() {
            if let Some(caps) = re_cipher.captures(line) {
                cipher = caps.get(1).map(|m| m.as_str().trim().to_string());
            }
        }
        if line.to_ascii_lowercase().starts_with("auth-user-pass") {
            requires_user_pass = true;
        }
    }

    let lower_all = content.to_ascii_lowercase();
    let has_inline_cert = lower_all.contains("<ca>")
        || lower_all.contains("<cert>")
        || lower_all.contains("<key>")
        || lower_all.contains("<pkcs12>");
    let has_encrypted_key = content.contains("ENCRYPTED PRIVATE KEY")
        || content.contains("Proc-Type: 4,ENCRYPTED")
        || content.contains("DEK-Info:")
        || lower_all.contains("<pkcs12>");

    ParsedOvpn {
        suggested_name: humanize_filename(filename),
        remote_host,
        remote_port,
        transport,
        cipher,
        requires_user_pass,
        has_inline_cert,
        has_encrypted_key,
    }
}

/// Converts a filename like `uk-lon_prod.ovpn` into `Uk Lon Prod`.
pub fn humanize_filename(filename: &str) -> String {
    let stem = Path::new(filename)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(filename);
    let cleaned = stem.replace(['-', '_'], " ");
    let titled: Vec<String> = cleaned
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect();
    if titled.is_empty() {
        "Imported OpenVPN".to_string()
    } else {
        titled.join(" ")
    }
}

/// Builds a `VpnProfile` ready to save from an `.ovpn` file path and contents.
pub fn profile_from_ovpn(
    path: Option<&Path>,
    filename: &str,
    content: &str,
) -> Result<VpnProfile, String> {
    if content.trim().is_empty() {
        return Err("The selected .ovpn file is empty.".to_string());
    }
    let parsed = parse_ovpn(content, filename);
    if parsed.remote_host.is_none() {
        return Err("No 'remote <host>' directive found in the .ovpn file.".to_string());
    }

    let mut profile = VpnProfile::new_openvpn(parsed.suggested_name);
    profile.protocol = VpnProtocol::OpenVpn;
    profile.ovpn_path = path.map(|p| p.to_string_lossy().into_owned());
    profile.ovpn_content = Some(content.to_string());
    profile.server = parsed.remote_host.unwrap_or_default();
    profile.port = Some(parsed.remote_port);
    let mut notes = vec![format!("Transport: {}", parsed.transport)];
    if let Some(c) = parsed.cipher {
        notes.push(format!("Cipher: {c}"));
    }
    if parsed.requires_user_pass {
        notes.push("Auth: username/password".to_string());
    } else if parsed.has_inline_cert {
        notes.push("Auth: embedded certificate".to_string());
    }
    profile.notes = notes.join(" · ");
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_OVPN: &str = r#"
# Sample OpenVPN Configuration for Kraken VPN
client
dev tun
proto udp
remote uk-lon.krakenvpn.net 1194
resolv-retry infinite
nobind
persist-key
persist-tun
auth-user-pass
cipher AES-256-GCM
verb 3
"#;

    #[test]
    fn parses_sample_ovpn_directives() {
        let parsed = parse_ovpn(SAMPLE_OVPN, "uk-lon_gateway.ovpn");
        assert_eq!(parsed.suggested_name, "Uk Lon Gateway");
        assert_eq!(parsed.remote_host.as_deref(), Some("uk-lon.krakenvpn.net"));
        assert_eq!(parsed.remote_port, 1194);
        assert_eq!(parsed.transport, "UDP");
        assert_eq!(parsed.cipher.as_deref(), Some("AES-256-GCM"));
        assert!(parsed.requires_user_pass);
        assert!(!parsed.has_inline_cert);
        assert!(!parsed.has_encrypted_key);
    }

    #[test]
    fn skips_inline_pem_blocks_and_detects_tcp_and_encrypted_key() {
        let cfg = r#"
client
proto tcp-client
remote vpn.corp.internal 443
<key>
-----BEGIN ENCRYPTED PRIVATE KEY-----
remote fake.ignored.host 9999
-----END ENCRYPTED PRIVATE KEY-----
</key>
"#;
        let parsed = parse_ovpn(cfg, "corp-vpn.ovpn");
        assert_eq!(parsed.remote_host.as_deref(), Some("vpn.corp.internal"));
        assert_eq!(parsed.remote_port, 443);
        assert_eq!(parsed.transport, "TCP");
        assert!(parsed.has_inline_cert);
        assert!(parsed.has_encrypted_key);
        assert!(!parsed.requires_user_pass);
    }

    #[test]
    fn profile_importer_rejects_missing_remote() {
        assert!(profile_from_ovpn(None, "broken.ovpn", "client\ndev tun\n").is_err());
        let ok =
            profile_from_ovpn(Some(Path::new("sample.ovpn")), "sample.ovpn", SAMPLE_OVPN).unwrap();
        assert_eq!(ok.name, "Sample");
        assert_eq!(ok.server, "uk-lon.krakenvpn.net");
        assert_eq!(ok.port, Some(1194));
    }
}
