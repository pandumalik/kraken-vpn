pub mod dashboard;
pub mod diagnostics;
pub mod modals;
pub mod monitoring;
pub mod profiles;
pub mod settings;
pub mod sidebar;
pub mod state;
pub mod theme;

pub use modals::paint_backdrop;
pub use state::{NavTab, ProfileFormState, ToastNotice};

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use eframe::egui::{self, Margin, Rounding, Stroke, TextureHandle, Vec2};
use uuid::Uuid;

use crate::models::{
    AppSettings, ConnectionHistoryEntry, DiagnosticsLine, LogLevel, VpnProfile, VpnProtocol,
};
use crate::ovpn::profile_from_ovpn;
use crate::platform::{check_binary, is_elevated, relaunch_as_admin, BinaryCheckResult};
use crate::store::ProfileStore;
use crate::telemetry::TelemetrySnapshot;
use crate::tray::TrayController;
use crate::ui::theme::{apply_stitch_theme, install_fonts, semibold, Palette};
use crate::updater::{check_for_updates, UpdateCheckResult};
use crate::vpn::manager::VpnManager;
use crate::vpn::openconnect::OpenConnectTestResult;
use crate::vpn::state::TunnelState;

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
    pub cached_logs: Vec<DiagnosticsLine>,
    pub cached_logs_version: u64,
    pub update_check_rx: Option<Receiver<UpdateCheckResult>>,
    pub update_status: Option<UpdateCheckResult>,
    pub update_notify_on_finish: bool,
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

        let mut app = Self {
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
            cached_logs: Vec::new(),
            cached_logs_version: u64::MAX,
            update_check_rx: None,
            update_status: None,
            update_notify_on_finish: false,
        };
        app.start_update_check(false);
        app
    }

    pub fn start_update_check(&mut self, notify_on_finish: bool) {
        if self.update_check_rx.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.update_check_rx = Some(rx);
        self.update_notify_on_finish = notify_on_finish;
        std::thread::spawn(move || {
            let res = check_for_updates();
            let _ = tx.send(res);
        });
    }

    pub fn logs_snapshot(&mut self) -> &[DiagnosticsLine] {
        let ver = self.manager.diagnostics_version();
        if ver != self.cached_logs_version {
            self.cached_logs = self.manager.diagnostics_snapshot();
            self.cached_logs_version = ver;
        }
        &self.cached_logs
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

        if let Some(ref rx) = self.update_check_rx {
            if let Ok(result) = rx.try_recv() {
                let notify = self.update_notify_on_finish;
                self.update_check_rx = None;
                self.update_notify_on_finish = false;
                match &result {
                    UpdateCheckResult::UpdateAvailable { release, .. } => {
                        self.manager.push_system_log(
                            LogLevel::System,
                            format!(
                                "Update available: Kraken VPN v{} ({})",
                                release.version, release.html_url
                            ),
                        );
                        if notify {
                            self.notify(
                                format!("Update available: Kraken VPN v{}.", release.version),
                                false,
                            );
                        }
                    }
                    UpdateCheckResult::UpToDate { latest, .. } if notify => {
                        self.notify(format!("You are up to date (v{latest})."), false);
                    }
                    UpdateCheckResult::NoReleases { .. } if notify => {
                        self.notify("No GitHub releases published yet.", false);
                    }
                    UpdateCheckResult::Error(err) if notify => {
                        self.notify(err.clone(), true);
                    }
                    _ => {}
                }
                self.update_status = Some(result);
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
}

impl eframe::App for KrakenApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let quit_now = self.quit_requested
            || self
                .tray
                .as_ref()
                .is_some_and(TrayController::is_quit_requested);

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
        let state = self.manager.state();

        if let Some(ref tray) = self.tray {
            let tooltip = match &state {
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

        let refresh = if state.is_busy()
            || self.openconnect_test_rx.is_some()
            || self.update_check_rx.is_some()
        {
            Duration::from_millis(250)
        } else {
            Duration::from_secs(1)
        };
        ctx.request_repaint_after(refresh);

        let p = self.palette();

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

        self.render_sidebar(ctx, &state);

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
                            NavTab::Home => dashboard::render(self, ui, &state),
                            NavTab::Monitoring => monitoring::render(self, ui, &state),
                            NavTab::Logs => diagnostics::render(self, ui, &state),
                            NavTab::Settings => settings::render(self, ui, ctx),
                        });
                } else {
                    diagnostics::render(self, ui, &state);
                }
            });

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
