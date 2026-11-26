use chrono::Utc;
use eframe::egui::{self, Margin, RichText, Rounding, Stroke, Vec2};
use uuid::Uuid;

use crate::models::{SwitchPolicy, VpnProfile, VpnProtocol};
use crate::telemetry::{format_bytes, format_duration, format_speed};
use crate::ui::theme::{
    card_frame, destructive_button, divider, page_container, pill_badge, primary_button,
    row_danger_button, row_primary_button, row_secondary_button, secondary_button, semibold,
    styled_text_edit, table_card_frame, tunnel_status_style, Palette,
};
use crate::ui::{KrakenApp, NavTab};
use crate::vpn::state::TunnelState;

pub fn render(app: &mut KrakenApp, ui: &mut egui::Ui) {
    let p = app.palette();
    let state = app.manager.state();

    page_container(ui, 920.0, |ui| {
        // Page Header
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(semibold("Home", 23.0, p.on_surface));
                ui.label(
                    RichText::new("Manage your VPN connections")
                        .size(13.5)
                        .color(p.on_surface_variant),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if primary_button(ui, &p, "+ Add profile").clicked() {
                    app.open_new_profile_modal(VpnProtocol::OpenVpn);
                }
                if secondary_button(ui, &p, "Import .ovpn").clicked() {
                    app.import_ovpn_file_dialog();
                }
            });
        });

        ui.add_space(20.0);

        // Active Tunnel Hero Card
        render_active_tunnel_card(app, ui, &p, &state);

        ui.add_space(24.0);

        // Saved Profiles Section
        render_saved_profiles_table(app, ui, &p, &state);
    });
}

fn render_active_tunnel_card(
    app: &mut KrakenApp,
    ui: &mut egui::Ui,
    p: &Palette,
    state: &TunnelState,
) {
    let mut quick_connect: Option<VpnProfile> = None;
    let mut disconnect_now = false;
    let mut open_logs = false;

    card_frame(p).show(ui, |ui| {
        let inner_w = ui.available_width();
        ui.set_min_width(inner_w);
        ui.set_max_width(inner_w);
        let left_w = (inner_w - 230.0).max(180.0);

        // Top row: badges + title + CTA
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width(left_w);
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.horizontal(|ui| {
                    let (bg, fg, dot, status_label) = tunnel_status_style(p, state);
                    pill_badge(ui, bg, fg, Some(dot), status_label);

                    match state {
                        TunnelState::Connected { protocol, .. }
                        | TunnelState::Connecting { protocol, .. } => {
                            pill_badge(
                                ui,
                                p.surface_container,
                                p.on_surface_variant,
                                None,
                                protocol.label(),
                            );
                        }
                        _ => {
                            pill_badge(
                                ui,
                                p.surface_container,
                                p.on_surface_variant,
                                None,
                                "Single-tunnel guard",
                            );
                        }
                    }
                });

                match state {
                    TunnelState::Connected {
                        profile_name,
                        endpoint,
                        ..
                    } => {
                        ui.label(semibold(profile_name, 19.5, p.on_surface));
                        ui.label(
                            RichText::new(endpoint)
                                .monospace()
                                .size(12.5)
                                .color(p.on_surface_variant),
                        );
                    }
                    TunnelState::Connecting {
                        profile_name,
                        endpoint,
                        ..
                    } => {
                        ui.label(semibold(profile_name, 19.5, p.on_surface));
                        ui.label(
                            RichText::new(format!("Establishing tunnel to {endpoint}..."))
                                .size(12.5)
                                .color(p.on_surface_variant),
                        );
                    }
                    TunnelState::Disconnecting { profile_name, .. } => {
                        ui.label(semibold(profile_name, 19.5, p.on_surface));
                        ui.label(
                            RichText::new("Closing tunnel and cleaning up routes...")
                                .size(12.5)
                                .color(p.on_surface_variant),
                        );
                    }
                    TunnelState::Error {
                        profile_name,
                        message,
                        ..
                    } => {
                        ui.label(semibold(profile_name, 19.5, p.on_surface));
                        ui.add(
                            egui::Label::new(
                                RichText::new(message).size(12.5).color(p.error),
                            )
                            .wrap(),
                        );
                    }
                    TunnelState::Disconnected => {
                        ui.label(semibold("No active tunnel", 19.5, p.on_surface));
                        ui.label(
                            RichText::new(
                                "Select a saved profile below to establish an encrypted VPN tunnel.",
                            )
                            .size(13.0)
                            .color(p.on_surface_variant),
                        );
                    }
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if state.is_busy() {
                    if destructive_button(ui, p, "Disconnect").clicked() {
                        disconnect_now = true;
                    }
                } else if let Some(first) = app.profiles.first() {
                    if primary_button(ui, p, "Quick connect").clicked() {
                        quick_connect = Some(first.clone());
                    }
                }
                if matches!(state, TunnelState::Error { .. })
                    && secondary_button(ui, p, "View logs").clicked()
                {
                    open_logs = true;
                }
            });
        });

        ui.add_space(16.0);
        divider(ui, p);
        ui.add_space(14.0);

        // Bottom 4-column tunnel metrics strip (matching Stitch Home hero footer)
        let is_connected = state.is_connected();
        let iface_text = if is_connected {
            app.telemetry
                .active_interface
                .clone()
                .unwrap_or_else(|| "Tunnel".to_string())
        } else {
            "—".to_string()
        };
        let dur_text = if is_connected {
            format_duration(state.connected_duration_secs(Utc::now()))
        } else {
            "—".to_string()
        };
        let dl_text = if is_connected {
            format_speed(app.telemetry.rx_bytes_per_sec)
        } else {
            "—".to_string()
        };
        let ul_text = if is_connected {
            format_speed(app.telemetry.tx_bytes_per_sec)
        } else {
            "—".to_string()
        };
        let dl_sub = if is_connected {
            Some(format_bytes(app.telemetry.session_rx_bytes))
        } else {
            None
        };
        let ul_sub = if is_connected {
            Some(format_bytes(app.telemetry.session_tx_bytes))
        } else {
            None
        };

        ui.columns(4, |cols| {
            hero_stat(&mut cols[0], p, "Interface", &iface_text, None);
            hero_stat(&mut cols[1], p, "Duration", &dur_text, None);
            hero_stat(&mut cols[2], p, "Download", &dl_text, dl_sub.as_deref());
            hero_stat(&mut cols[3], p, "Upload", &ul_text, ul_sub.as_deref());
        });
    });

    if open_logs {
        app.diagnostics_subtab = crate::ui::diagnostics::DiagnosticsSubtab::Console;
        app.active_tab = NavTab::Logs;
    } else if disconnect_now {
        app.manager.disconnect();
    } else if let Some(target) = quick_connect {
        if let Err(e) = app.manager.connect(target, &app.settings) {
            app.notify(e, true);
        }
    }
}

fn hero_stat(ui: &mut egui::Ui, p: &Palette, label: &str, value: &str, sub: Option<&str>) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        ui.label(RichText::new(label).size(11.5).color(p.on_surface_variant));
        ui.label(
            RichText::new(value)
                .monospace()
                .size(13.5)
                .color(p.on_surface),
        );
        if let Some(sub_val) = sub {
            ui.label(
                RichText::new(sub_val)
                    .monospace()
                    .size(11.0)
                    .color(p.outline),
            );
        }
    });
}

fn render_saved_profiles_table(
    app: &mut KrakenApp,
    ui: &mut egui::Ui,
    p: &Palette,
    state: &TunnelState,
) {
    // Section header + search box
    ui.horizontal(|ui| {
        ui.label(semibold("Saved profiles", 15.5, p.on_surface));
        pill_badge(
            ui,
            p.surface_container,
            p.on_surface_variant,
            None,
            &app.profiles.len().to_string(),
        );

        if !app.profiles.is_empty() {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let search_w = (ui.available_width() * 0.45).clamp(140.0, 220.0);
                ui.add(
                    styled_text_edit(&mut app.profile_search, search_w)
                        .hint_text("Filter profiles..."),
                );
            });
        }
    });

    ui.add_space(10.0);

    let mut connect_target: Option<VpnProfile> = None;
    let mut disconnect_clicked = false;
    let mut edit_target: Option<VpnProfile> = None;
    let mut delete_target: Option<Uuid> = None;
    let active_id = state.active_profile_id();
    let search = app.profile_search.trim().to_ascii_lowercase();

    table_card_frame(p).show(ui, |ui| {
        ui.set_width(ui.available_width());

        if app.profiles.is_empty() {
            egui::Frame::none()
                .inner_margin(Margin::symmetric(24.0, 34.0))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(semibold("No VPN profiles yet", 15.0, p.on_surface));
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(
                                "Import an OpenVPN (.ovpn) file or add an OpenConnect endpoint to get started.",
                            )
                            .size(13.0)
                            .color(p.on_surface_variant),
                        );
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            let group_w = 248.0_f32;
                            let pad = ((ui.available_width() - group_w) * 0.5).max(0.0);
                            ui.add_space(pad);
                            if primary_button(ui, p, "+ Add profile").clicked() {
                                app.open_new_profile_modal(VpnProtocol::OpenVpn);
                            }
                            if secondary_button(ui, p, "Import .ovpn").clicked() {
                                app.import_ovpn_file_dialog();
                            }
                        });
                    });
                });
            return;
        }

        let total_w = ui.available_width();
        let col_profile = (total_w * 0.25).max(110.0);
        let col_proto = (total_w * 0.19).max(96.0);
        let col_action = 205.0_f32.min(total_w * 0.34);
        let col_gateway = (total_w - col_profile - col_proto - col_action - 48.0).max(80.0);

        // Table Header Bar
        egui::Frame::none()
            .fill(p.surface_low)
            .rounding(Rounding {
                nw: 11.0,
                ne: 11.0,
                sw: 0.0,
                se: 0.0,
            })
            .inner_margin(Margin::symmetric(18.0, 10.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    header_cell(ui, p, "Profile", col_profile);
                    header_cell(ui, p, "Protocol", col_proto);
                    header_cell(ui, p, "Gateway", col_gateway);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(semibold("Action", 11.5, p.on_surface_variant));
                    });
                });
            });
        divider(ui, p);

        let filtered: Vec<&VpnProfile> = app
            .profiles
            .iter()
            .filter(|prof| {
                search.is_empty()
                    || prof.name.to_ascii_lowercase().contains(&search)
                    || prof.endpoint_display().to_ascii_lowercase().contains(&search)
                    || prof.protocol_badge().to_ascii_lowercase().contains(&search)
            })
            .collect();

        if filtered.is_empty() {
            egui::Frame::none()
                .inner_margin(Margin::symmetric(20.0, 22.0))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("No profiles match your search.")
                            .size(13.0)
                            .color(p.outline),
                    );
                });
            return;
        }

        for (i, profile) in filtered.iter().enumerate() {
            if i > 0 {
                divider(ui, p);
            }
            let is_this_active = active_id == Some(profile.id);
            let other_active = active_id.is_some() && !is_this_active;
            let row_bg = if is_this_active {
                p.surface_low
            } else {
                p.surface_lowest
            };

            egui::Frame::none()
                .fill(row_bg)
                .rounding(if i + 1 == filtered.len() {
                    Rounding {
                        nw: 0.0,
                        ne: 0.0,
                        sw: 11.0,
                        se: 11.0,
                    }
                } else {
                    Rounding::ZERO
                })
                .inner_margin(Margin::symmetric(18.0, 12.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        // Profile Name cell
                        ui.allocate_ui_with_layout(
                            Vec2::new(col_profile, 28.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(semibold(&profile.name, 13.5, p.on_surface))
                                        .truncate(),
                                );
                            },
                        );

                        // Protocol pill cell
                        ui.allocate_ui_with_layout(
                            Vec2::new(col_proto, 28.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                pill_badge(
                                    ui,
                                    p.surface_container,
                                    p.on_surface_variant,
                                    None,
                                    &profile.protocol_badge(),
                                );
                            },
                        );

                        // Gateway monospace cell
                        ui.allocate_ui_with_layout(
                            Vec2::new(col_gateway, 28.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(profile.endpoint_display())
                                            .monospace()
                                            .size(12.0)
                                            .color(p.on_surface_variant),
                                    )
                                    .truncate(),
                                );
                            },
                        );

                        // Right-aligned row actions
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if row_danger_button(ui, p, "Delete").clicked() {
                                delete_target = Some(profile.id);
                            }
                            if row_secondary_button(ui, p, "Edit").clicked() {
                                edit_target = Some((*profile).clone());
                            }
                            if is_this_active {
                                let disc = egui::Button::new(semibold("Disconnect", 12.5, p.error))
                                    .fill(p.surface_lowest)
                                    .stroke(Stroke::new(1.0_f32, p.error.linear_multiply(0.5)))
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(84.0, 28.0));
                                if ui.add(disc).clicked() {
                                    disconnect_clicked = true;
                                }
                            } else {
                                let (btn_label, enabled) = if other_active {
                                    match app.settings.switch_policy {
                                        SwitchPolicy::AutoSwitch => ("Switch", true),
                                        SwitchPolicy::RefuseWhileActive => ("Busy", false),
                                    }
                                } else {
                                    ("Connect", true)
                                };
                                if row_primary_button(ui, p, btn_label, enabled).clicked() {
                                    connect_target = Some((*profile).clone());
                                }
                            }
                        });
                    });
                });
        }
    });

    if disconnect_clicked {
        app.manager.disconnect();
    } else if let Some(target) = connect_target {
        if let Err(e) = app.manager.connect(target, &app.settings) {
            app.notify(e, true);
        }
    }
    if let Some(prof) = edit_target {
        app.open_edit_profile_modal(&prof);
    }
    if let Some(id) = delete_target {
        app.delete_confirm_id = Some(id);
    }
}

fn header_cell(ui: &mut egui::Ui, p: &Palette, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 18.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(semibold(text, 11.5, p.on_surface_variant));
        },
    );
}
