mod appearance;
mod clients;
mod updates;

use eframe::egui::{self, Margin, RichText};

use crate::models::SwitchPolicy;
use crate::ui::theme::{
    card_header_frame, divider, page_container, page_header, semibold, table_card_frame,
    toggle_switch, Palette,
};
use crate::ui::KrakenApp;

pub fn render(app: &mut KrakenApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let p = app.palette();
    let mut changed = false;

    page_container(ui, 820.0, |ui| {
        page_header(
            ui,
            &p,
            "Settings",
            "Connection policy, VPN client executables, and application preferences",
        );

        ui.add_space(20.0);

        section_card(&p, ui, "Connection", |ui| {
            let mut auto_switch = app.settings.switch_policy == SwitchPolicy::AutoSwitch;
            setting_toggle_row(
                ui,
                &p,
                "Auto-switch active tunnel",
                "Cleanly disconnect the current tunnel before connecting to another profile (strictly one active VPN at a time).",
                &mut auto_switch,
                &mut changed,
            );
            app.settings.switch_policy = if auto_switch {
                SwitchPolicy::AutoSwitch
            } else {
                SwitchPolicy::RefuseWhileActive
            };

            divider(ui, &p);

            setting_toggle_row(
                ui,
                &p,
                "Auto-pin unknown OpenConnect certificates",
                "Automatically trust and pin a gateway's SHA-256 certificate on first connect instead of prompting for manual approval.",
                &mut app.settings.auto_trust_server_cert,
                &mut changed,
            );

            divider(ui, &p);

            setting_toggle_row(
                ui,
                &p,
                "Keep running in system tray on close",
                "Closing the window hides Kraken VPN to the system tray so active VPN tunnels stay connected. Use Quit in the tray menu to exit completely.",
                &mut app.settings.minimize_to_tray_on_close,
                &mut changed,
            );
        });

        ui.add_space(16.0);

        section_card(&p, ui, "VPN CLI executables", |ui| {
            clients::binary_path_row(
                ui,
                &p,
                "OpenVPN executable",
                "Select openvpn.exe",
                "https://openvpn.net/community-downloads/",
                &mut app.settings.openvpn_path,
                &mut app.openvpn_check,
                &mut changed,
            );
            divider(ui, &p);
            clients::binary_path_row(
                ui,
                &p,
                "OpenConnect executable",
                "Select openconnect.exe",
                "https://gui.openconnect-vpn.net/",
                &mut app.settings.openconnect_path,
                &mut app.openconnect_check,
                &mut changed,
            );
        });

        ui.add_space(16.0);

        section_card(&p, ui, "Appearance & storage", |ui| {
            appearance::render_appearance_and_storage(app, ui, &p, &mut changed);
        });

        ui.add_space(16.0);

        section_card(&p, ui, "Software updates", |ui| {
            updates::render_updates_section(app, ui, &p);
        });
    });

    if changed {
        app.save_settings_to_disk(ctx);
    }
}

fn section_card(p: &Palette, ui: &mut egui::Ui, title: &str, body: impl FnOnce(&mut egui::Ui)) {
    let card_w = ui.available_width();
    table_card_frame(p).show(ui, |ui| {
        ui.set_min_width(card_w);
        ui.set_max_width(card_w);
        card_header_frame(p, Margin::symmetric(20.0, 11.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(semibold(title, 13.5, p.on_surface));
        });
        divider(ui, p);
        body(ui);
    });
}

fn setting_toggle_row(
    ui: &mut egui::Ui,
    p: &Palette,
    title: &str,
    description: &str,
    value: &mut bool,
    changed: &mut bool,
) {
    egui::Frame::none()
        .inner_margin(Margin::symmetric(20.0, 14.0))
        .show(ui, |ui| {
            let row_w = ui.available_width();
            ui.set_min_width(row_w);
            ui.set_max_width(row_w);
            let text_w = (row_w - 68.0).max(160.0);
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(text_w);
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.label(semibold(title, 13.5, p.on_surface));
                    ui.add(
                        egui::Label::new(
                            RichText::new(description)
                                .size(12.0)
                                .color(p.on_surface_variant),
                        )
                        .wrap(),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if toggle_switch(ui, p, value).changed() {
                        *changed = true;
                    }
                });
            });
        });
}
