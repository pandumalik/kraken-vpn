use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VpnProtocol {
    #[default]
    OpenVpn,
    OpenConnect,
}

impl VpnProtocol {
    pub fn label(self) -> &'static str {
        match self {
            VpnProtocol::OpenVpn => "OpenVPN",
            VpnProtocol::OpenConnect => "OpenConnect",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OpenConnectProtocol {
    #[default]
    AnyConnect,
    Fortinet,
    GlobalProtect,
    Pulse,
    JuniperNc,
}

impl OpenConnectProtocol {
    pub const ALL: &'static [OpenConnectProtocol] = &[
        OpenConnectProtocol::AnyConnect,
        OpenConnectProtocol::Fortinet,
        OpenConnectProtocol::GlobalProtect,
        OpenConnectProtocol::Pulse,
        OpenConnectProtocol::JuniperNc,
    ];

    pub fn cli_flag(self) -> &'static str {
        match self {
            OpenConnectProtocol::AnyConnect => "anyconnect",
            OpenConnectProtocol::Fortinet => "fortinet",
            OpenConnectProtocol::GlobalProtect => "gp",
            OpenConnectProtocol::Pulse => "pulse",
            OpenConnectProtocol::JuniperNc => "nc",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            OpenConnectProtocol::AnyConnect => "Cisco AnyConnect (anyconnect)",
            OpenConnectProtocol::Fortinet => "Fortinet SSL (fortinet)",
            OpenConnectProtocol::GlobalProtect => "Palo Alto GlobalProtect (gp)",
            OpenConnectProtocol::Pulse => "Pulse Connect Secure (pulse)",
            OpenConnectProtocol::JuniperNc => "Juniper Network Connect (nc)",
        }
    }

    pub fn parse_flexible(raw: &str) -> Self {
        let lower = raw.trim().to_ascii_lowercase();
        if lower.contains("fortinet") || lower.contains("forti") {
            OpenConnectProtocol::Fortinet
        } else if lower.contains("globalprotect") || lower == "gp" || lower.contains("palo alto") {
            OpenConnectProtocol::GlobalProtect
        } else if lower.contains("pulse") {
            OpenConnectProtocol::Pulse
        } else if lower == "nc" || lower.contains("juniper") {
            OpenConnectProtocol::JuniperNc
        } else {
            OpenConnectProtocol::AnyConnect
        }
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VpnProfile {
    pub id: Uuid,
    pub name: String,
    pub protocol: VpnProtocol,

    #[serde(default)]
    pub ovpn_path: Option<String>,

    #[serde(default)]
    pub ovpn_content: Option<String>,

    #[serde(default)]
    pub openconnect_protocol: OpenConnectProtocol,

    #[serde(default)]
    pub server: String,

    #[serde(default)]
    pub port: Option<u16>,

    #[serde(default)]
    pub authgroup: Option<String>,

    #[serde(default)]
    pub servercert: Option<String>,

    #[serde(default)]
    pub interface_name: Option<String>,

    #[serde(default)]
    pub username: String,

    #[serde(skip)]
    pub password: String,

    #[serde(default)]
    pub encrypted_password: Option<String>,

    #[serde(skip)]
    pub key_passphrase: String,

    #[serde(default)]
    pub encrypted_key_passphrase: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

impl fmt::Debug for VpnProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VpnProfile")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("protocol", &self.protocol)
            .field("ovpn_path", &self.ovpn_path)
            .field(
                "has_ovpn_content",
                &self.ovpn_content.as_ref().is_some_and(|s| !s.is_empty()),
            )
            .field("openconnect_protocol", &self.openconnect_protocol)
            .field("server", &self.server)
            .field("port", &self.port)
            .field("authgroup", &self.authgroup)
            .field("servercert", &self.servercert)
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .field("key_passphrase", &"[REDACTED]")
            .finish()
    }
}

impl VpnProfile {
    pub fn new_openvpn(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            protocol: VpnProtocol::OpenVpn,
            ovpn_path: None,
            ovpn_content: None,
            openconnect_protocol: OpenConnectProtocol::AnyConnect,
            server: String::new(),
            port: Some(1194),
            authgroup: None,
            servercert: None,
            interface_name: None,
            username: String::new(),
            password: String::new(),
            encrypted_password: None,
            key_passphrase: String::new(),
            encrypted_key_passphrase: None,
            notes: String::new(),
            created_at: Utc::now(),
        }
    }

    pub fn new_openconnect(
        name: impl Into<String>,
        server: impl Into<String>,
        proto: OpenConnectProtocol,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            protocol: VpnProtocol::OpenConnect,
            ovpn_path: None,
            ovpn_content: None,
            openconnect_protocol: proto,
            server: server.into(),
            port: Some(443),
            authgroup: None,
            servercert: None,
            interface_name: None,
            username: String::new(),
            password: String::new(),
            encrypted_password: None,
            key_passphrase: String::new(),
            encrypted_key_passphrase: None,
            notes: String::new(),
            created_at: Utc::now(),
        }
    }

    pub fn endpoint_display(&self) -> String {
        match self.protocol {
            VpnProtocol::OpenVpn => {
                if !self.server.trim().is_empty() {
                    match self.port {
                        Some(p) => format!("{}:{}", self.server.trim(), p),
                        None => self.server.trim().to_string(),
                    }
                } else if let Some(ref path) = self.ovpn_path {
                    path.clone()
                } else {
                    "Inline .ovpn config".to_string()
                }
            }
            VpnProtocol::OpenConnect => {
                let s = self.server.trim();
                if s.is_empty() {
                    "(no server set)".to_string()
                } else if s.contains("://") {
                    s.to_string()
                } else if let Some(p) = self.port {
                    if s.contains(':') {
                        s.to_string()
                    } else {
                        format!("{s}:{p}")
                    }
                } else {
                    s.to_string()
                }
            }
        }
    }

    pub fn protocol_badge(&self) -> String {
        match self.protocol {
            VpnProtocol::OpenVpn => "OpenVPN".to_string(),
            VpnProtocol::OpenConnect => {
                format!("OpenConnect · {}", self.openconnect_protocol.cli_flag())
            }
        }
    }
}
