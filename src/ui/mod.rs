pub mod dashboard;
pub mod diagnostics;
pub mod monitoring;
pub mod profiles;
pub mod settings;
pub mod theme;

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use chrono::Utc;
use eframe::egui::{
    self, Align2, Color32, Margin, Order, Pos2, Rect, RichText, Rounding, Sense, Stroke,
    TextureHandle, Vec2,
};
use uuid::Uuid;

use crate::models::{
    AppSettings, ConnectionHistoryEntry, LogLevel, OpenConnectProtocol, ThemeMode, VpnProfile,
    VpnProtocol,
};
use crate::ovpn::profile_from_ovpn;
use crate::platform::{check_binary, is_elevated, relaunch_as_admin, BinaryCheckResult};
use crate::store::ProfileStore;
use crate::telemetry::{format_duration, TelemetrySnapshot};
use crate::tray::TrayController;
use crate::ui::theme::{
    apply_stitch_theme, destructive_button, divider, install_fonts, primary_button,
    responsive_modal_size, secondary_button, semibold, tunnel_status_style, Palette,
};
use crate::vpn::manager::VpnManager;
use crate::vpn::openconnect::OpenConnectTestResult;
use crate::vpn::state::TunnelState;

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

pub struct KrakenApp {
    pub store: ProfileStore,
    pub manager: VpnManager,
    pub profiles: Vec<VpnProfile>,
    pub settings: AppSettings,
    pub history: Vec<ConnectionHistoryEntry>,
    pub active_tab: NavTab,
    pub is_admin: bool,
    pub icon_texture: Option<TextureHandle>,
    pub telemetry: TelemetrySnapshot,
    pub last_telemetry_poll: Instant,
    pub toast: Option<ToastNotice>,
    pub profile_form: ProfileFormState,
    pub profile_modal_open: bool,
    pub delete_confirm_id: Option<Uuid>,
    pub profile_search: String,
    pub console_filter: String,
    pub console_auto_scroll: bool,
    pub log_level_filter: diagnostics::LogLevelFilter,
    pub diagnostics_subtab: diagnostics::DiagnosticsSubtab,
    pub openvpn_check: Option<BinaryCheckResult>,
    pub openconnect_check: Option<BinaryCheckResult>,
    pub openconnect_test_rx: Option<Receiver<OpenConnectTestResult>>,
    pub openconnect_test_result: Option<OpenConnectTestResult>,
    pub custom_colors_expanded: bool,
    pub tray: Option<TrayController>,
    pub last_tray_tooltip: String,
    pub quit_requested: bool,
}

impl KrakenApp {
    pub fn new(cc: &eframe::CreationContext<'_>, icon_png: &[u8]) -> Self {
        let store = ProfileStore::default_os_store().unwrap_or_else(|_| {
            ProfileStore::in_dir(std::env::temp_dir().join("KrakenVPN"))
                .expect("fallback temp data directory")
        });
        let settings = store.load_settings().unwrap_or_default();
        let profiles = store.load_profiles().unwrap_or_default();
        let history = store.load_history().unwrap_or_default();
        let temp_dir = store
            .runtime_temp_dir()
            .unwrap_or_else(|_| std::env::temp_dir().join("KrakenVPN_runtime"));

        install_fonts(&cc.egui_ctx);
        apply_stitch_theme(&cc.egui_ctx, &settings);

        let icon_texture = image::load_from_memory(icon_png).ok().map(|img| {
            let rgba = img
                .resize(64, 64, image::imageops::FilterType::Lanczos3)
                .into_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            cc.egui_ctx.load_texture(
                "kraken_brand_icon",
                color_image,
                egui::TextureOptions::LINEAR,
            )
        });

        let openvpn_check = Some(check_binary(&settings.openvpn_path));
        let openconnect_check = Some(check_binary(&settings.openconnect_path));

        let manager = VpnManager::new(temp_dir);
        let tray = TrayController::new(cc.egui_ctx.clone(), icon_png, manager.clone());
        manager.push_system_log(
            LogLevel::System,
            format!(
                "Kraken VPN v{} ready ({} profile(s) loaded).",
                env!("CARGO_PKG_VERSION"),
                profiles.len()
            ),
        );

        Self {
            store,
            manager,
            profiles,
            settings,
            history,
            active_tab: NavTab::Home,
            is_admin: is_elevated(),
            icon_texture,
            telemetry: TelemetrySnapshot::default(),
            last_telemetry_poll: Instant::now() - Duration::from_secs(2),
            toast: None,
            profile_form: ProfileFormState::default(),
            profile_modal_open: false,
            delete_confirm_id: None,
            profile_search: String::new(),
            console_filter: String::new(),
            console_auto_scroll: true,
            log_level_filter: diagnostics::LogLevelFilter::All,
            diagnostics_subtab: diagnostics::DiagnosticsSubtab::Console,
            openvpn_check,
            openconnect_check,
            openconnect_test_rx: None,
            openconnect_test_result: None,
            custom_colors_expanded: false,
            tray,
            last_tray_tooltip: "Kraken VPN".to_string(),
            quit_requested: false,
        }
    }

    pub fn palette(&self) -> Palette {
        Palette::from_settings(&self.settings)
    }

    pub fn notify(&mut self, msg: impl Into<String>, is_error: bool) {
        self.toast = Some(ToastNotice {
            message: msg.into(),
            is_error,
            expires_at: Instant::now() + Duration::from_secs(5),
        });
    }

    pub fn open_new_profile_modal(&mut self, protocol: VpnProtocol) {
        self.profile_form = ProfileFormState::new_for_protocol(protocol);
        self.openconnect_test_result = None;
        self.profile_modal_open = true;
    }

    pub fn open_edit_profile_modal(&mut self, profile: &VpnProfile) {
        self.profile_form = ProfileFormState::from_profile(profile);
        self.openconnect_test_result = None;
        self.profile_modal_open = true;
    }

    pub fn import_ovpn_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title("Import OpenVPN Configuration (.ovpn)")
            .add_filter("OpenVPN Config", &["ovpn"])
            .pick_file()
        {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let fname = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("imported.ovpn");
                    match profile_from_ovpn(Some(&path), fname, &content) {
                        Ok(imported) => {
                            self.profile_form = ProfileFormState::from_profile(&imported);
                            self.profile_form.editing_id = None;
                            self.openconnect_test_result = None;
                            self.profile_modal_open = true;
                            self.notify(
                                format!("Parsed '{fname}'. Review details and click Save profile."),
                                false,
                            );
                        }
                        Err(e) => self.notify(e, true),
                    }
                }
                Err(e) => self.notify(format!("Failed to read file: {e}"), true),
            }
        }
    }

    pub fn save_profiles_to_disk(&mut self) {
        if let Err(e) = self.store.save_profiles(&self.profiles) {
            self.notify(format!("Failed to save profiles: {e}"), true);
        }
    }

    pub fn save_settings_to_disk(&mut self, ctx: &egui::Context) {
        apply_stitch_theme(ctx, &self.settings);
        if let Err(e) = self.store.save_settings(&self.settings) {
            self.notify(format!("Failed to save settings: {e}"), true);
        }
    }

    pub fn start_openconnect_test(&mut self, profile: VpnProfile) {
        let settings = self.settings.clone();
        let (tx, rx) = mpsc::channel();
        self.openconnect_test_rx = Some(rx);
        self.openconnect_test_result = None;
        std::thread::spawn(move || {
            let res = crate::vpn::openconnect::test_openconnect_endpoint(&profile, &settings);
            let _ = tx.send(res);
        });
    }

    fn sync_background_events(&mut self) {
        let new_history = self.manager.take_pending_history();
        for entry in new_history {
            match self.store.append_history(entry) {
                Ok(updated) => self.history = updated,
                Err(e) => self
                    .manager
                    .push_system_log(LogLevel::Warn, format!("History save error: {e}")),
            }
        }

        let pinned = self.manager.take_auto_pinned_certs();
        if !pinned.is_empty() {
            for (pid, pin) in pinned {
                if let Some(p) = self.profiles.iter_mut().find(|x| x.id == pid) {
                    p.servercert = Some(pin);
                }
            }
            self.save_profiles_to_disk();
        }

        if let Some(ref rx) = self.openconnect_test_rx {
            if let Ok(result) = rx.try_recv() {
                if let Some(ref pin) = result.detected_pin {
                    if self.profile_form.servercert.trim().is_empty() {
                        self.profile_form.servercert = pin.clone();
                    }
                }
                self.openconnect_test_result = Some(result);
                self.openconnect_test_rx = None;
            }
        }

        if self.last_telemetry_poll.elapsed() >= Duration::from_secs(1) {
            self.telemetry = self.manager.sample_telemetry();
            self.last_telemetry_poll = Instant::now();
        }

        if self
            .toast
            .as_ref()
            .is_some_and(|t| Instant::now() >= t.expires_at)
        {
            self.toast = None;
        }
    }

    fn render_sidebar(&mut self, ctx: &egui::Context) {
        let p = self.palette();
        let state = self.manager.state();

        egui::SidePanel::left("kraken_sidebar")
            .resizable(false)
            .exact_width(224.0)
            .frame(
                egui::Frame::none()
                    .fill(p.surface_low)
                    .stroke(Stroke::new(1.0_f32, p.outline_variant))
                    .inner_margin(Margin::symmetric(14.0, 18.0)),
            )
            .show(ctx, |ui| {
                // Brand header
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if let Some(ref tex) = self.icon_texture {
                        ui.image((tex.id(), Vec2::splat(28.0)));
                    } else {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        ui.painter().rect_filled(r, Rounding::same(7.0), p.primary);
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            "K",
                            egui::FontId::proportional(15.0),
                            p.on_primary,
                        );
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(semibold("Kraken VPN", 16.5, p.on_surface));
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(11.0)
                                .color(p.outline),
                        );
                    });
                });

                ui.add_space(22.0);

                // Navigation items
                for (tab, label) in [
                    (NavTab::Home, "Home"),
                    (NavTab::Monitoring, "Monitoring"),
                    (NavTab::Logs, "Logs"),
                    (NavTab::Settings, "Settings"),
                ] {
                    let selected = self.active_tab == tab;
                    if nav_rail_item(ui, &p, tab, label, selected).clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(4.0);
                }

                // Bottom rail status footer
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(2.0);

                    // Elevation & quick theme toggle row
                    ui.horizontal(|ui| {
                        let (elev_col, elev_label) = if self.is_admin {
                            (p.success, "Administrator")
                        } else {
                            (p.warning, "Standard User")
                        };
                        let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 3.2, elev_col);
                        ui.label(
                            RichText::new(elev_label)
                                .size(11.5)
                                .color(p.on_surface_variant),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mode_label = match self.settings.theme {
                                ThemeMode::Light => "Dark",
                                ThemeMode::Dark => "Light",
                            };
                            let btn = egui::Button::new(
                                RichText::new(mode_label)
                                    .size(11.0)
                                    .color(p.on_surface_variant),
                            )
                            .fill(p.surface_container)
                            .stroke(Stroke::new(1.0_f32, p.outline_variant))
                            .rounding(Rounding::same(6.0))
                            .min_size(Vec2::new(42.0, 22.0));
                            if ui
                                .add(btn)
                                .on_hover_text("Switch Light / Dark theme")
                                .clicked()
                            {
                                self.settings.theme = match self.settings.theme {
                                    ThemeMode::Light => ThemeMode::Dark,
                                    ThemeMode::Dark => ThemeMode::Light,
                                };
                                self.save_settings_to_disk(ctx);
                            }
                        });
                    });

                    ui.add_space(8.0);

                    // Connection status summary block (Stitch rail footer)
                    let (_, _, dot_col, status_txt) = tunnel_status_style(&p, &state);
                    let detail_line = match &state {
                        TunnelState::Connected { endpoint, .. } => {
                            let dur = format_duration(state.connected_duration_secs(Utc::now()));
                            let short_ep = truncate_middle(endpoint, 16);
                            format!("{dur} • {short_ep}")
                        }
                        TunnelState::Connecting { profile_name, .. } => {
                            format!("Dialing {}", truncate_middle(profile_name, 18))
                        }
                        TunnelState::Disconnecting { profile_name, .. } => {
                            format!("Closing {}", truncate_middle(profile_name, 18))
                        }
                        TunnelState::Error { profile_name, .. } => {
                            format!("Error: {}", truncate_middle(profile_name, 16))
                        }
                        TunnelState::Disconnected => "Single-tunnel guard on".to_string(),
                    };

                    ui.label(
                        RichText::new(detail_line)
                            .monospace()
                            .size(11.0)
                            .color(p.outline),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 7.0;
                        let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 4.0, dot_col);
                        ui.label(semibold(status_txt, 13.0, p.on_surface));
                    });

                    ui.add_space(10.0);
                    divider(ui, &p);
                });
            });
    }

    fn render_cert_pin_modal(&mut self, ctx: &egui::Context) {
        let Some(prompt) = self.manager.pending_cert_prompt() else {
            return;
        };
        let p = self.palette();
        let screen = ctx.screen_rect();
        let size = responsive_modal_size(screen, 480.0, 340.0);

        paint_backdrop(ctx, &p, "cert_pin_backdrop");

        egui::Area::new(egui::Id::new("cert_pin_modal"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(p.surface_lowest)
                    .rounding(Rounding::same(12.0))
                    .stroke(Stroke::new(1.0_f32, p.outline_variant))
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 10.0),
                        blur: 28.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(50),
                    })
                    .inner_margin(Margin::same(22.0))
                    .show(ui, |ui| {
                        ui.set_width(size.x - 44.0);
                        ui.label(semibold("Verify server certificate", 17.0, p.on_surface));
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "OpenConnect could not verify the TLS certificate for '{}' ({}).",
                                    prompt.profile_name, prompt.server
                                ))
                                .size(13.0)
                                .color(p.on_surface_variant),
                            )
                            .wrap(),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("SHA-256 certificate pin")
                                .size(12.0)
                                .color(p.on_surface_variant),
                        );
                        ui.add_space(4.0);
                        egui::Frame::none()
                            .fill(p.surface_low)
                            .rounding(Rounding::same(8.0))
                            .stroke(Stroke::new(1.0_f32, p.outline_variant))
                            .inner_margin(Margin::same(10.0))
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&prompt.fingerprint)
                                            .monospace()
                                            .size(12.0)
                                            .color(p.on_surface),
                                    )
                                    .wrap(),
                                );
                            });
                        ui.add_space(10.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(
                                    "Only trust this fingerprint if it matches your organization's VPN gateway.",
                                )
                                .size(12.0)
                                .color(p.outline),
                            )
                            .wrap(),
                        );
                        ui.add_space(16.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if primary_button(ui, &p, "Trust, pin & reconnect").clicked() {
                                if let Some(prof) =
                                    self.profiles.iter_mut().find(|x| x.id == prompt.profile_id)
                                {
                                    prof.servercert = Some(prompt.fingerprint.clone());
                                    let cloned = prof.clone();
                                    self.save_profiles_to_disk();
                                    if let Err(e) = self.manager.trust_cert_and_connect(
                                        cloned,
                                        &self.settings,
                                        prompt.fingerprint,
                                    ) {
                                        self.notify(e, true);
                                    }
                                } else {
                                    self.manager.dismiss_cert_prompt();
                                }
                            }
                            if secondary_button(ui, &p, "Cancel").clicked() {
                                self.manager.dismiss_cert_prompt();
                            }
                        });
                    });
            });
    }

    fn render_delete_confirm_modal(&mut self, ctx: &egui::Context) {
        let Some(target_id) = self.delete_confirm_id else {
            return;
        };
        let Some(target_name) = self
            .profiles
            .iter()
            .find(|x| x.id == target_id)
            .map(|x| x.name.clone())
        else {
            self.delete_confirm_id = None;
            return;
        };

        let p = self.palette();
        let size = responsive_modal_size(ctx.screen_rect(), 420.0, 220.0);
        paint_backdrop(ctx, &p, "delete_confirm_backdrop");

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.delete_confirm_id = None;
            return;
        }

        egui::Area::new(egui::Id::new("delete_confirm_modal"))
            .order(Order::Foreground)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(p.surface_lowest)
                    .rounding(Rounding::same(12.0))
                    .stroke(Stroke::new(1.0_f32, p.outline_variant))
                    .shadow(egui::epaint::Shadow {
                        offset: Vec2::new(0.0, 10.0),
                        blur: 28.0,
                        spread: 0.0,
                        color: Color32::from_black_alpha(50),
                    })
                    .inner_margin(Margin::same(22.0))
                    .show(ui, |ui| {
                        ui.set_width(size.x - 44.0);
                        ui.label(semibold("Delete VPN profile?", 17.0, p.on_surface));
                        ui.add_space(6.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(format!(
                                    "Remove '{}' and its stored credentials from this computer? If it is currently connected, the tunnel will be closed first.",
                                    target_name
                                ))
                                .size(13.0)
                                .color(p.on_surface_variant),
                            )
                            .wrap(),
                        );
                        ui.add_space(18.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if destructive_button(ui, &p, "Delete profile").clicked() {
                                if self.manager.state().active_profile_id() == Some(target_id) {
                                    self.manager.disconnect_sync("Deleted active profile");
                                }
                                self.profiles.retain(|x| x.id != target_id);
                                if self.profile_form.editing_id == Some(target_id) {
                                    self.profile_form = ProfileFormState::default();
                                    self.profile_modal_open = false;
                                }
                                self.save_profiles_to_disk();
                                self.delete_confirm_id = None;
                                self.notify(format!("Deleted profile '{target_name}'."), false);
                            }
                            if secondary_button(ui, &p, "Cancel").clicked() {
                                self.delete_confirm_id = None;
                            }
                        });
                    });
            });
    }
}

impl eframe::App for KrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let quit_now = self.quit_requested
            || self
                .tray
                .as_ref()
                .is_some_and(TrayController::is_quit_requested);

        // Intercept window close (X) and hide to system tray instead of killing the tunnel
        if ctx.input(|i| i.viewport().close_requested())
            && !quit_now
            && self.settings.minimize_to_tray_on_close
        {
            if let Some(ref tray) = self.tray {
                tray.capture_window_handle();
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                return;
            }
        }

        if let Some(ref tray) = self.tray {
            tray.capture_window_handle();
        }

        self.sync_background_events();

        // Keep tray hover tooltip in sync with live tunnel state
        if let Some(ref tray) = self.tray {
            let tooltip = match self.manager.state() {
                TunnelState::Connected { profile_name, .. } => {
                    format!("Kraken VPN — Connected ({profile_name})")
                }
                TunnelState::Connecting { profile_name, .. } => {
                    format!("Kraken VPN — Connecting ({profile_name})...")
                }
                TunnelState::Disconnecting { profile_name, .. } => {
                    format!("Kraken VPN — Disconnecting ({profile_name})...")
                }
                TunnelState::Error { profile_name, .. } => {
                    format!("Kraken VPN — Error ({profile_name})")
                }
                TunnelState::Disconnected => "Kraken VPN — Disconnected".to_string(),
            };
            if tooltip != self.last_tray_tooltip {
                tray.set_tooltip(&tooltip);
                self.last_tray_tooltip = tooltip;
            }
        }

        let refresh = if self.manager.state().is_busy() || self.openconnect_test_rx.is_some() {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        };
        ctx.request_repaint_after(refresh);

        let p = self.palette();

        // Windows Administrator elevation banner (subtle Stitch amber bar)
        #[cfg(windows)]
        if !self.is_admin {
            egui::TopBottomPanel::top("admin_warning_banner")
                .frame(
                    egui::Frame::none()
                        .fill(p.warning_container)
                        .stroke(Stroke::new(1.0_f32, p.outline_variant))
                        .inner_margin(Margin::symmetric(18.0, 8.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::Label::new(semibold(
                                "Standard user mode — VPN virtual adapters & routing require Administrator elevation.",
                                12.5,
                                p.on_warning_container,
                            ))
                            .wrap(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let btn = egui::Button::new(semibold(
                                "Relaunch as Admin",
                                12.0,
                                p.on_warning_container,
                            ))
                            .fill(p.surface_lowest)
                            .stroke(Stroke::new(1.0_f32, p.warning.linear_multiply(0.5)))
                            .rounding(Rounding::same(7.0))
                            .min_size(Vec2::new(0.0, 26.0));
                            if ui.add(btn).clicked() {
                                match relaunch_as_admin() {
                                    Ok(()) => {
                                        self.quit_requested = true;
                                        if let Some(ref tray) = self.tray {
                                            tray.request_quit();
                                        }
                                        self.manager
                                            .disconnect_sync("Relaunching as Administrator");
                                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                                    }
                                    Err(e) => self.notify(e, true),
                                }
                            }
                        });
                    });
                });
        }

        // Left navigation rail
        self.render_sidebar(ctx);

        // Toast bar at bottom if present
        if let Some(ref toast) = self.toast {
            let (fill, fg) = if toast.is_error {
                (p.error_container, p.on_error_container)
            } else {
                (p.success_container, p.on_success_container)
            };
            egui::TopBottomPanel::bottom("toast_bar")
                .frame(
                    egui::Frame::none()
                        .fill(fill)
                        .stroke(Stroke::new(1.0_f32, p.outline_variant))
                        .inner_margin(Margin::symmetric(20.0, 9.0)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(semibold(&toast.message, 13.0, fg));
                    });
                });
        }

        // Main workspace panel (adaptive padding for compact non-fullscreen windows)
        let screen_w = ctx.screen_rect().width();
        let pad_x = if screen_w < 860.0 { 18.0 } else { 30.0 };
        let pad_y = if screen_w < 860.0 { 18.0 } else { 26.0 };

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(p.surface)
                    .inner_margin(Margin::symmetric(pad_x, pad_y)),
            )
            .show(ctx, |ui| {
                let use_outer_scroll = !(self.active_tab == NavTab::Logs
                    && self.diagnostics_subtab == diagnostics::DiagnosticsSubtab::Console);
                if use_outer_scroll {
                    egui::ScrollArea::vertical()
                        .id_salt(("main_content_scroll", self.active_tab as u8))
                        .auto_shrink([false, false])
                        .show(ui, |ui| match self.active_tab {
                            NavTab::Home => dashboard::render(self, ui),
                            NavTab::Monitoring => monitoring::render(self, ui),
                            NavTab::Logs => diagnostics::render(self, ui),
                            NavTab::Settings => settings::render(self, ui, ctx),
                        });
                } else {
                    diagnostics::render(self, ui);
                }
            });

        // Responsive modals layered cleanly above workspace
        if self.profile_modal_open {
            profiles::render_modal(self, ctx);
        }
        self.render_delete_confirm_modal(ctx);
        self.render_cert_pin_modal(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.tray = None;
        self.manager.disconnect_sync("Application closed");
        for entry in self.manager.take_pending_history() {
            let _ = self.store.append_history(entry);
        }
    }
}

/// Paints a translucent full-window backdrop behind an active modal dialog.
pub fn paint_backdrop(ctx: &egui::Context, p: &Palette, id: &str) {
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new(id))
        .order(Order::Middle)
        .fixed_pos(screen.min)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(screen.size(), Sense::click());
            ui.painter().rect_filled(rect, Rounding::ZERO, p.backdrop);
        });
}

fn nav_rail_item(
    ui: &mut egui::Ui,
    p: &Palette,
    tab: NavTab,
    label: &str,
    selected: bool,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.surface_lowest
        } else if response.hovered() {
            p.surface_container
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if selected {
            Stroke::new(1.0_f32, p.outline_variant)
        } else {
            Stroke::NONE
        };
        ui.painter().rect(rect, Rounding::same(8.0), fill, stroke);

        let fg = if selected {
            p.primary
        } else if response.hovered() {
            p.on_surface
        } else {
            p.on_surface_variant
        };

        let icon_center = Pos2::new(rect.left() + 20.0, rect.center().y);
        paint_nav_icon(ui.painter(), tab, icon_center, fg);

        let text_pos = Pos2::new(rect.left() + 38.0, rect.center().y);
        let font = egui::FontId::new(
            13.5,
            if selected {
                egui::FontFamily::Name("semibold".into())
            } else {
                egui::FontFamily::Proportional
            },
        );
        ui.painter()
            .text(text_pos, Align2::LEFT_CENTER, label, font, fg);
    }

    response
}

fn paint_nav_icon(painter: &egui::Painter, tab: NavTab, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6_f32, color);
    match tab {
        NavTab::Home => {
            // Minimal geometric shield / house outline
            let roof = vec![
                Pos2::new(c.x - 6.5, c.y - 0.5),
                Pos2::new(c.x, c.y - 6.5),
                Pos2::new(c.x + 6.5, c.y - 0.5),
            ];
            painter.add(egui::Shape::line(roof, stroke));
            let body = vec![
                Pos2::new(c.x - 4.8, c.y - 1.0),
                Pos2::new(c.x - 4.8, c.y + 5.8),
                Pos2::new(c.x + 4.8, c.y + 5.8),
                Pos2::new(c.x + 4.8, c.y - 1.0),
            ];
            painter.add(egui::Shape::line(body, stroke));
        }
        NavTab::Monitoring => {
            // Activity pulse waveform
            let pts = vec![
                Pos2::new(c.x - 7.0, c.y + 1.0),
                Pos2::new(c.x - 3.5, c.y + 1.0),
                Pos2::new(c.x - 1.5, c.y - 5.0),
                Pos2::new(c.x + 1.5, c.y + 5.0),
                Pos2::new(c.x + 3.5, c.y + 1.0),
                Pos2::new(c.x + 7.0, c.y + 1.0),
            ];
            painter.add(egui::Shape::line(pts, stroke));
        }
        NavTab::Logs => {
            // Structured log lines
            for dy in [-4.5_f32, 0.0_f32, 4.5_f32] {
                let end_x = if dy > 2.0 { c.x + 3.5 } else { c.x + 6.5 };
                painter.line_segment(
                    [Pos2::new(c.x - 6.5, c.y + dy), Pos2::new(end_x, c.y + dy)],
                    stroke,
                );
            }
        }
        NavTab::Settings => {
            // Sliders icon (two crisp horizontal tracks with knobs)
            painter.line_segment(
                [
                    Pos2::new(c.x - 6.5, c.y - 3.2),
                    Pos2::new(c.x + 6.5, c.y - 3.2),
                ],
                stroke,
            );
            painter.circle_filled(Pos2::new(c.x - 2.0, c.y - 3.2), 2.3, color);
            painter.line_segment(
                [
                    Pos2::new(c.x - 6.5, c.y + 3.2),
                    Pos2::new(c.x + 6.5, c.y + 3.2),
                ],
                stroke,
            );
            painter.circle_filled(Pos2::new(c.x + 2.5, c.y + 3.2), 2.3, color);
        }
    }
}

fn truncate_middle(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let prefix: String = s.chars().take(keep).collect();
    format!("{prefix}…")
}

/// Backward-compatible alias for applying theme visuals.
pub fn apply_theme(ctx: &egui::Context, mode: ThemeMode) {
    let settings = AppSettings {
        theme: mode,
        ..AppSettings::default()
    };
    apply_stitch_theme(ctx, &settings);
}

/// Helper kept for consistent rounded rect calculation if needed externally.
pub fn badge_rect(min: Pos2, size: Vec2) -> Rect {
    Rect::from_min_size(min, size)
}
