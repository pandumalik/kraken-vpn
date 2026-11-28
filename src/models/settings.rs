use serde::{Deserialize, Serialize};

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

impl AppSettings {
    pub fn active_custom_colors(&self) -> Option<CustomThemeColors> {
        match self.theme {
            ThemeMode::Light => self.custom_light_colors,
            ThemeMode::Dark => self.custom_dark_colors,
        }
    }

    pub fn active_custom_colors_mut(&mut self) -> &mut Option<CustomThemeColors> {
        match self.theme {
            ThemeMode::Light => &mut self.custom_light_colors,
            ThemeMode::Dark => &mut self.custom_dark_colors,
        }
    }
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
