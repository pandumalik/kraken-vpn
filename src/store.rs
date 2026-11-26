//! Persistent JSON storage for VPN profiles, settings, and connection history
//! inside the OS application data directory (`%APPDATA%\KrakenVPN\` on Windows).

use std::fs;
use std::path::{Path, PathBuf};

use directories::BaseDirs;

use crate::models::{AppSettings, ConnectionHistoryEntry, VpnProfile};
use crate::secrets::{protect_secret, unprotect_secret};

const MAX_HISTORY_ENTRIES: usize = 200;

#[derive(Debug, Clone)]
pub struct ProfileStore {
    root_dir: PathBuf,
}

impl ProfileStore {
    /// Opens the standard OS app-data store (`%APPDATA%\KrakenVPN` on Windows).
    pub fn default_os_store() -> Result<Self, String> {
        let base = BaseDirs::new()
            .ok_or_else(|| "Could not determine OS application data directory.".to_string())?;
        let dir = base.config_dir().join("KrakenVPN");
        Self::in_dir(dir)
    }

    /// Opens a store rooted at an explicit directory (also used by unit tests).
    pub fn in_dir(root_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let root_dir = root_dir.into();
        fs::create_dir_all(&root_dir)
            .map_err(|e| format!("Failed to create KrakenVPN data directory: {e}"))?;
        Ok(Self { root_dir })
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn runtime_temp_dir(&self) -> Result<PathBuf, String> {
        let dir = self.root_dir.join("runtime");
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create runtime temp directory: {e}"))?;
        Ok(dir)
    }

    fn profiles_path(&self) -> PathBuf {
        self.root_dir.join("profiles.json")
    }

    fn settings_path(&self) -> PathBuf {
        self.root_dir.join("settings.json")
    }

    fn history_path(&self) -> PathBuf {
        self.root_dir.join("history.json")
    }

    pub fn load_profiles(&self) -> Result<Vec<VpnProfile>, String> {
        let path = self.profiles_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw =
            fs::read_to_string(&path).map_err(|e| format!("Failed to read profiles store: {e}"))?;
        let mut profiles: Vec<VpnProfile> = serde_json::from_str(&raw)
            .map_err(|e| format!("Failed to parse profiles store: {e}"))?;
        for p in &mut profiles {
            p.password = unprotect_secret(p.encrypted_password.as_deref()).unwrap_or_default();
            p.key_passphrase =
                unprotect_secret(p.encrypted_key_passphrase.as_deref()).unwrap_or_default();
        }
        Ok(profiles)
    }

    pub fn save_profiles(&self, profiles: &[VpnProfile]) -> Result<(), String> {
        let mut to_store = Vec::with_capacity(profiles.len());
        for p in profiles {
            let mut copy = p.clone();
            copy.encrypted_password = protect_secret(&p.password)?;
            copy.password.clear();
            copy.encrypted_key_passphrase = protect_secret(&p.key_passphrase)?;
            copy.key_passphrase.clear();
            to_store.push(copy);
        }
        let json = serde_json::to_string_pretty(&to_store)
            .map_err(|e| format!("Failed to serialize profiles: {e}"))?;
        atomic_write(&self.profiles_path(), json.as_bytes())
    }

    pub fn load_settings(&self) -> Result<AppSettings, String> {
        let path = self.settings_path();
        if !path.exists() {
            return Ok(AppSettings::default());
        }
        let raw = fs::read_to_string(&path).map_err(|e| format!("Failed to read settings: {e}"))?;
        serde_json::from_str(&raw).map_err(|e| format!("Failed to parse settings: {e}"))
    }

    pub fn save_settings(&self, settings: &AppSettings) -> Result<(), String> {
        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        atomic_write(&self.settings_path(), json.as_bytes())
    }

    pub fn load_history(&self) -> Result<Vec<ConnectionHistoryEntry>, String> {
        let path = self.history_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read connection history: {e}"))?;
        serde_json::from_str(&raw).map_err(|e| format!("Failed to parse connection history: {e}"))
    }

    pub fn save_history(&self, history: &[ConnectionHistoryEntry]) -> Result<(), String> {
        let capped = &history[..history.len().min(MAX_HISTORY_ENTRIES)];
        let json = serde_json::to_string_pretty(capped)
            .map_err(|e| format!("Failed to serialize history: {e}"))?;
        atomic_write(&self.history_path(), json.as_bytes())
    }

    pub fn append_history(
        &self,
        entry: ConnectionHistoryEntry,
    ) -> Result<Vec<ConnectionHistoryEntry>, String> {
        let mut list = self.load_history().unwrap_or_default();
        list.insert(0, entry);
        list.truncate(MAX_HISTORY_ENTRIES);
        self.save_history(&list)?;
        Ok(list)
    }
}

fn atomic_write(dest: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = dest.with_extension("json.tmp");
    fs::write(&tmp, bytes).map_err(|e| format!("Failed to write {}: {e}", tmp.display()))?;
    // On Windows, fs::rename fails if dest exists in older runtimes unless removed or replaced;
    // try rename first, fall back to remove + rename.
    if fs::rename(&tmp, dest).is_err() {
        let _ = fs::remove_file(dest);
        fs::rename(&tmp, dest).map_err(|e| format!("Failed to commit {}: {e}", dest.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{HistoryStatus, OpenConnectProtocol, SwitchPolicy, ThemeMode};
    use chrono::Utc;
    use uuid::Uuid;

    fn temp_store() -> (ProfileStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!("kraken_vpn_test_{}", Uuid::new_v4()));
        let store = ProfileStore::in_dir(&dir).unwrap();
        (store, dir)
    }

    #[test]
    fn profile_round_trip_protects_password_on_disk() {
        let (store, dir) = temp_store();
        let mut p = VpnProfile::new_openconnect(
            "Corp VPN",
            "vpn.corp.example",
            OpenConnectProtocol::Fortinet,
        );
        p.username = "alice".into();
        p.password = "super-secret-vpn-pw".into();
        p.key_passphrase = "super-secret-key-pass".into();

        store.save_profiles(&[p.clone()]).unwrap();

        // Plaintext password & key passphrase must NEVER appear on disk
        let raw = fs::read_to_string(dir.join("profiles.json")).unwrap();
        assert!(!raw.contains("super-secret-vpn-pw"));
        assert!(!raw.contains("super-secret-key-pass"));

        // Reloading must restore both secrets cleanly
        let loaded = store.load_profiles().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "Corp VPN");
        assert_eq!(loaded[0].password, "super-secret-vpn-pw");
        assert_eq!(loaded[0].key_passphrase, "super-secret-key-pass");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_and_history_round_trip() {
        let (store, dir) = temp_store();
        let settings = AppSettings {
            openvpn_path: r"C:\Custom\openvpn.exe".into(),
            openconnect_path: r"C:\Custom\openconnect.exe".into(),
            theme: ThemeMode::Light,
            color_preset: crate::models::ColorPreset::OceanSlate,
            custom_light_colors: Some(crate::models::CustomThemeColors {
                primary: [37, 99, 235],
                secondary: [14, 165, 233],
                surface: [248, 250, 252],
                card: [255, 255, 255],
                border: [203, 213, 225],
                text: [15, 23, 42],
                text_muted: [71, 85, 105],
                success: [22, 163, 74],
                warning: [217, 119, 6],
                error: [220, 38, 38],
            }),
            custom_dark_colors: None,
            switch_policy: SwitchPolicy::RefuseWhileActive,
            auto_trust_server_cert: true,
        };
        store.save_settings(&settings).unwrap();
        assert_eq!(store.load_settings().unwrap(), settings);

        let entry = ConnectionHistoryEntry {
            id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
            profile_name: "Prod EU".into(),
            protocol: "OpenVPN".into(),
            endpoint: "eu.example:1194".into(),
            status: HistoryStatus::Disconnected,
            started_at: Utc::now(),
            ended_at: Some(Utc::now()),
            duration_secs: 120,
            message: "Disconnected cleanly".into(),
        };
        let list = store.append_history(entry.clone()).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(store.load_history().unwrap()[0].profile_name, "Prod EU");

        let _ = fs::remove_dir_all(&dir);
    }
}
