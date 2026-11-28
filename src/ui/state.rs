use std::time::Instant;

use chrono::Utc;
use uuid::Uuid;

use crate::models::{OpenConnectProtocol, VpnProfile, VpnProtocol};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavTab {
    Home,
    Monitoring,
    Logs,
    Settings,
}

#[derive(Debug, Clone)]
pub struct ToastNotice {
    pub message: String,
    pub is_error: bool,
    pub expires_at: Instant,
}

pub struct ProfileFormState {
    pub editing_id: Option<Uuid>,
    pub name: String,
    pub protocol: VpnProtocol,
    pub ovpn_path: String,
    pub ovpn_content: Option<String>,
    pub openconnect_protocol: OpenConnectProtocol,
    pub server: String,
    pub port_text: String,
    pub authgroup: String,
    pub servercert: String,
    pub interface_name: String,
    pub username: String,
    pub password: String,
    pub show_password: bool,
    pub key_passphrase: String,
    pub show_key_passphrase: bool,
    pub notes: String,
    pub validation_error: Option<String>,
}

impl Default for ProfileFormState {
    fn default() -> Self {
        Self::new_for_protocol(VpnProtocol::OpenVpn)
    }
}

impl ProfileFormState {
    pub fn new_for_protocol(protocol: VpnProtocol) -> Self {
        let default_port = match protocol {
            VpnProtocol::OpenVpn => "1194",
            VpnProtocol::OpenConnect => "443",
        };
        Self {
            editing_id: None,
            name: String::new(),
            protocol,
            ovpn_path: String::new(),
            ovpn_content: None,
            openconnect_protocol: OpenConnectProtocol::AnyConnect,
            server: String::new(),
            port_text: default_port.to_string(),
            authgroup: String::new(),
            servercert: String::new(),
            interface_name: String::new(),
            username: String::new(),
            password: String::new(),
            show_password: false,
            key_passphrase: String::new(),
            show_key_passphrase: false,
            notes: String::new(),
            validation_error: None,
        }
    }

    pub fn from_profile(p: &VpnProfile) -> Self {
        Self {
            editing_id: Some(p.id),
            name: p.name.clone(),
            protocol: p.protocol,
            ovpn_path: p.ovpn_path.clone().unwrap_or_default(),
            ovpn_content: p.ovpn_content.clone(),
            openconnect_protocol: p.openconnect_protocol,
            server: p.server.clone(),
            port_text: p.port.map(|v| v.to_string()).unwrap_or_default(),
            authgroup: p.authgroup.clone().unwrap_or_default(),
            servercert: p.servercert.clone().unwrap_or_default(),
            interface_name: p.interface_name.clone().unwrap_or_default(),
            username: p.username.clone(),
            password: p.password.clone(),
            show_password: false,
            key_passphrase: p.key_passphrase.clone(),
            show_key_passphrase: false,
            notes: p.notes.clone(),
            validation_error: None,
        }
    }

    pub fn to_profile(&self) -> Result<VpnProfile, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("Profile name is required.".to_string());
        }
        let port = if self.port_text.trim().is_empty() {
            None
        } else {
            Some(
                self.port_text
                    .trim()
                    .parse::<u16>()
                    .map_err(|_| "Port must be a number between 1 and 65535.".to_string())?,
            )
        };

        match self.protocol {
            VpnProtocol::OpenVpn => {
                let has_path = !self.ovpn_path.trim().is_empty();
                let has_content = self
                    .ovpn_content
                    .as_deref()
                    .is_some_and(|c| !c.trim().is_empty());
                if !has_path && !has_content {
                    return Err(
                        "OpenVPN profile requires an .ovpn file path or imported config."
                            .to_string(),
                    );
                }
            }
            VpnProtocol::OpenConnect => {
                if self.server.trim().is_empty() {
                    return Err(
                        "OpenConnect profile requires a server URL or hostname.".to_string()
                    );
                }
            }
        }

        let opt_trim = |s: &str| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        };

        Ok(VpnProfile {
            id: self.editing_id.unwrap_or_else(Uuid::new_v4),
            name: name.to_string(),
            protocol: self.protocol,
            ovpn_path: opt_trim(&self.ovpn_path),
            ovpn_content: self.ovpn_content.clone(),
            openconnect_protocol: self.openconnect_protocol,
            server: self.server.trim().to_string(),
            port,
            authgroup: opt_trim(&self.authgroup),
            servercert: opt_trim(&self.servercert),
            interface_name: opt_trim(&self.interface_name),
            username: self.username.trim().to_string(),
            password: self.password.clone(),
            encrypted_password: None,
            key_passphrase: self.key_passphrase.clone(),
            encrypted_key_passphrase: None,
            notes: self.notes.trim().to_string(),
            created_at: Utc::now(),
        })
    }
}
