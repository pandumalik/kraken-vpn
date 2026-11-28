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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SwitchPolicy {
    #[default]
    AutoSwitch,

    RefuseWhileActive,
}

impl SwitchPolicy {
    pub fn label(self) -> &'static str {
        match self {
            SwitchPolicy::AutoSwitch => "Auto-switch (disconnect active VPN first)",
            SwitchPolicy::RefuseWhileActive => "Strict (require manual disconnect first)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ThemeMode {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ColorPreset {
    #[default]
    ForestSage,
    OceanSlate,
    NordFrost,
    MidnightViolet,
    AmberCarbon,
    Monochrome,
}

impl ColorPreset {
    pub const ALL: &'static [ColorPreset] = &[
        ColorPreset::ForestSage,
        ColorPreset::OceanSlate,
        ColorPreset::NordFrost,
        ColorPreset::MidnightViolet,
        ColorPreset::AmberCarbon,
        ColorPreset::Monochrome,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ColorPreset::ForestSage => "Forest Sage",
            ColorPreset::OceanSlate => "Ocean Slate",
            ColorPreset::NordFrost => "Nord Frost",
            ColorPreset::MidnightViolet => "Midnight Violet",
            ColorPreset::AmberCarbon => "Amber Carbon",
            ColorPreset::Monochrome => "Monochrome",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CustomThemeColors {
    pub primary: [u8; 3],
    pub secondary: [u8; 3],
    pub surface: [u8; 3],
    pub card: [u8; 3],
    pub border: [u8; 3],
    pub text: [u8; 3],
    pub text_muted: [u8; 3],
    pub success: [u8; 3],
    pub warning: [u8; 3],
    pub error: [u8; 3],
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub openvpn_path: String,
    pub openconnect_path: String,
    pub theme: ThemeMode,
    #[serde(default)]
    pub color_preset: ColorPreset,
    #[serde(default)]
    pub custom_light_colors: Option<CustomThemeColors>,
    #[serde(default)]
    pub custom_dark_colors: Option<CustomThemeColors>,
    pub switch_policy: SwitchPolicy,

    #[serde(default)]
    pub auto_trust_server_cert: bool,

    #[serde(default = "default_true")]
    pub minimize_to_tray_on_close: bool,
}

fn default_true() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        #[cfg(windows)]
        let openvpn_path = r"C:\Program Files\OpenVPN\bin\openvpn.exe".to_string();
        #[cfg(not(windows))]
        let openvpn_path = "openvpn".to_string();

        #[cfg(windows)]
        let openconnect_path = "openconnect.exe".to_string();
        #[cfg(not(windows))]
        let openconnect_path = "openconnect".to_string();

        Self {
            openvpn_path,
            openconnect_path,
            theme: ThemeMode::Light,
            color_preset: ColorPreset::ForestSage,
            custom_light_colors: None,
            custom_dark_colors: None,
            switch_policy: SwitchPolicy::AutoSwitch,
            auto_trust_server_cert: false,
            minimize_to_tray_on_close: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum HistoryStatus {
    Connected,
    Disconnected,
    Failed,
}

impl HistoryStatus {
    pub fn label(&self) -> &'static str {
        match self {
            HistoryStatus::Connected => "Connected",
            HistoryStatus::Disconnected => "Disconnected",
            HistoryStatus::Failed => "Failed",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionHistoryEntry {
    pub id: Uuid,
    pub profile_id: Uuid,
    pub profile_name: String,
    pub protocol: String,
    pub endpoint: String,
    pub status: HistoryStatus,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: u64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    System,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsLine {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

impl DiagnosticsLine {
    pub fn new(level: LogLevel, source: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            level,
            source: source.into(),
            message: message.into(),
        }
    }

    pub fn format_line(&self) -> String {
        let lvl = match self.level {
            LogLevel::Info => "INFO",
            LogLevel::Warn => "WARN",
            LogLevel::Error => "ERR ",
            LogLevel::System => "SYS ",
        };
        format!(
            "[{}] [{}] [{}] {}",
            self.timestamp.format("%H:%M:%S"),
            lvl,
            self.source,
            self.message
        )
    }
}

#[derive(Debug, Clone)]
pub struct CertPinPrompt {
    pub profile_id: Uuid,
    pub profile_name: String,
    pub server: String,
    pub fingerprint: String,
}
