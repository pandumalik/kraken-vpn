//! System tray icon and menu integration so Kraken VPN can keep active tunnels
//! running in the background when the main window is closed.

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Arc;

use eframe::egui;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::platform::{find_own_main_window, post_close_window, restore_window};
use crate::vpn::manager::VpnManager;

pub struct TrayController {
    _tray: TrayIcon,
    hwnd: Arc<AtomicIsize>,
    quit_requested: Arc<AtomicBool>,
}

impl TrayController {
    /// Creates the system tray icon + context menu.
    /// Actions are executed directly in the Win32/tray event handlers so Show, Disconnect,
    /// and Quit work immediately even while eframe's window is hidden (`Visible(false)`).
    pub fn new(ctx: egui::Context, icon_png: &[u8], manager: VpnManager) -> Option<Self> {
        let icon = load_tray_icon(icon_png)?;

        let show_item = MenuItem::new("Show Kraken VPN", true, None);
        let disconnect_item = MenuItem::new("Disconnect active VPN", true, None);
        let quit_item = MenuItem::new("Quit Kraken VPN", true, None);

        let show_id: MenuId = show_item.id().clone();
        let disconnect_id: MenuId = disconnect_item.id().clone();
        let quit_id: MenuId = quit_item.id().clone();

        let menu = Menu::new();
        let _ = menu.append(&show_item);
        let _ = menu.append(&disconnect_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit_item);

        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Kraken VPN")
            .with_icon(icon)
            .build()
            .ok()?;

        let hwnd = Arc::new(AtomicIsize::new(find_own_main_window()));
        let quit_requested = Arc::new(AtomicBool::new(false));

        // Left-click or double-click on the tray icon restores the window immediately.
        let hwnd_click = hwnd.clone();
        let ctx_click = ctx.clone();
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            let should_show = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } | TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                }
            );
            if should_show {
                show_main_window(&hwnd_click, &ctx_click);
            }
        }));

        // Context menu item clicks execute immediately without waiting for hidden eframe::update().
        let hwnd_menu = hwnd.clone();
        let quit_menu = quit_requested.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                show_main_window(&hwnd_menu, &ctx);
            } else if event.id == disconnect_id {
                let mgr = manager.clone();
                let ctx_c = ctx.clone();
                std::thread::spawn(move || {
                    mgr.disconnect();
                    ctx_c.request_repaint();
                });
            } else if event.id == quit_id {
                quit_menu.store(true, Ordering::SeqCst);
                let raw = hwnd_menu.load(Ordering::SeqCst);
                if raw != 0 {
                    // Unhide + post WM_CLOSE so eframe wakes up, runs on_exit() (cleaning up
                    // VPN tunnel + history + tray icon), and exits cleanly.
                    restore_window(raw);
                    post_close_window(raw);
                }
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                ctx.request_repaint();
            }
        }));

        Some(Self {
            _tray: tray,
            hwnd,
            quit_requested,
        })
    }

    /// Captures the main window handle while the window is still visible.
    pub fn capture_window_handle(&self) {
        let found = find_own_main_window();
        if found != 0 {
            self.hwnd.store(found, Ordering::SeqCst);
        }
    }

    pub fn is_quit_requested(&self) -> bool {
        self.quit_requested.load(Ordering::SeqCst)
    }

    pub fn request_quit(&self) {
        self.quit_requested.store(true, Ordering::SeqCst);
    }

    /// Updates the tray icon hover tooltip to reflect the current tunnel status.
    pub fn set_tooltip(&self, tooltip: &str) {
        let _ = self._tray.set_tooltip(Some(tooltip));
    }
}

fn show_main_window(hwnd: &Arc<AtomicIsize>, ctx: &egui::Context) {
    let raw = hwnd.load(Ordering::SeqCst);
    if raw != 0 {
        restore_window(raw);
    }
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}

fn load_tray_icon(icon_png: &[u8]) -> Option<Icon> {
    let img = image::load_from_memory(icon_png).ok()?;
    let rgba = img
        .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
        .into_rgba8();
    let (w, h) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), w, h).ok()
}
