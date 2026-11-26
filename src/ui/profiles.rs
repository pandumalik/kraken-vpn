//! Responsive centered modal dialog for adding or editing a VPN profile.
//! Uses dynamic proportional widths and an inner vertical ScrollArea so the form
//! NEVER clips or splits in half when the application window is not fullscreen.

use eframe::egui::{self, Align2, Color32, Margin, Order, RichText, Rounding, Stroke, Vec2};

use crate::models::{OpenConnectProtocol, VpnProtocol};
use crate::ui::theme::{
    divider, pill_badge, primary_button, responsive_modal_size, row_secondary_button,
    secondary_button, semibold, styled_text_edit, Palette, CONTROL_HEIGHT,
};
use crate::ui::{paint_backdrop, KrakenApp, ProfileFormState};

pub fn render_modal(app: &mut KrakenApp, ctx: &egui::Context) {
    let p = app.palette();
    let screen = ctx.screen_rect();
    let modal_size = responsive_modal_size(screen, 540.0, 620.0);

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.profile_modal_open = false;
        return;
    }

    paint_backdrop(ctx, &p, "profile_modal_backdrop");

    let mut close_requested = false;

    egui::Area::new(egui::Id::new("add_edit_profile_modal"))
        .order(Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(p.surface_lowest)
                .rounding(Rounding::same(12.0))
                .stroke(Stroke::new(1.0_f32, p.outline_variant))
                .shadow(egui::epaint::Shadow {
                    offset: Vec2::new(0.0, 12.0),
                    blur: 32.0,
                    spread: 0.0,
                    color: Color32::from_black_alpha(55),
                })
                .inner_margin(Margin::same(0.0))
                .show(ui, |ui| {
                    ui.set_width(modal_size.x);

                    // Fixed Modal Header
                    egui::Frame::none()
                        .inner_margin(Margin::symmetric(22.0, 16.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    let title = if app.profile_form.editing_id.is_some() {
                                        "Edit VPN profile"
                                    } else {
                                        "Add VPN profile"
                                    };
                                    ui.label(semibold(title, 17.5, p.on_surface));
                                    ui.label(
                                        RichText::new(
                                            "Configure an OpenVPN (.ovpn) or OpenConnect SSL gateway.",
                                        )
                                        .size(12.0)
                                        .color(p.on_surface_variant),
                                    );
                                });
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::TOP),
                                    |ui| {
                                        let close_btn = egui::Button::new(
                                            semibold("✕", 13.0, p.on_surface_variant),
                                        )
                                        .fill(p.surface_low)
                                        .stroke(Stroke::new(1.0_f32, p.outline_variant))
                                        .rounding(Rounding::same(7.0))
                                        .min_size(Vec2::splat(28.0));
                                        if ui.add(close_btn).clicked() {
                                            close_requested = true;
                                        }
                                    },
                                );
                            });
                        });

                    divider(ui, &p);

                    // Scrollable Form Body (adapts cleanly to any non-fullscreen height)
                    let body_max_h = (modal_size.y - 138.0).max(160.0);
                    egui::ScrollArea::vertical()
                        .id_salt("profile_modal_body_scroll")
                        .max_height(body_max_h)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            egui::Frame::none()
                                .inner_margin(Margin::symmetric(22.0, 16.0))
                                .show(ui, |ui| {
                                    let field_w = (modal_size.x - 44.0).max(220.0);
                                    ui.set_width(field_w);
                                    render_form_fields(app, ui, &p, field_w);
                                });
                        });

                    divider(ui, &p);

                    // Fixed Modal Footer
                    egui::Frame::none()
                        .fill(p.surface_low)
                        .rounding(Rounding {
                            nw: 0.0,
                            ne: 0.0,
                            sw: 11.0,
                            se: 11.0,
                        })
                        .inner_margin(Margin::symmetric(22.0, 13.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                if app.profile_form.protocol == VpnProtocol::OpenConnect {
                                    let testing = app.openconnect_test_rx.is_some();
                                    let label = if testing {
                                        "Testing..."
                                    } else {
                                        "Test endpoint"
                                    };
                                    let btn = egui::Button::new(semibold(
                                        label,
                                        13.0,
                                        if testing { p.outline } else { p.on_surface },
                                    ))
                                    .fill(p.surface_lowest)
                                    .stroke(Stroke::new(1.0_f32, p.outline_variant))
                                    .rounding(Rounding::same(8.0))
                                    .min_size(Vec2::new(0.0, 34.0));
                                    if ui.add_enabled(!testing, btn).clicked() {
                                        match app.profile_form.to_profile() {
                                            Ok(prof) => {
                                                app.profile_form.validation_error = None;
                                                app.start_openconnect_test(prof);
                                            }
                                            Err(e) => {
                                                app.profile_form.validation_error = Some(e);
                                            }
                                        }
                                    }
                                }

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if primary_button(ui, &p, "Save profile").clicked() {
                                            match app.profile_form.to_profile() {
                                                Ok(new_prof) => {
                                                    if let Some(pos) = app
                                                        .profiles
                                                        .iter()
                                                        .position(|x| x.id == new_prof.id)
                                                    {
                                                        app.profiles[pos] = new_prof.clone();
                                                        app.notify(
                                                            format!(
                                                                "Updated profile '{}'.",
                                                                new_prof.name
                                                            ),
                                                            false,
                                                        );
                                                    } else {
                                                        app.profiles.push(new_prof.clone());
                                                        app.notify(
                                                            format!(
                                                                "Saved profile '{}'.",
                                                                new_prof.name
                                                            ),
                                                            false,
                                                        );
                                                    }
                                                    app.save_profiles_to_disk();
                                                    app.profile_form = ProfileFormState::default();
                                                    close_requested = true;
                                                }
                                                Err(e) => {
                                                    app.profile_form.validation_error = Some(e);
                                                }
                                            }
                                        }
                                        if secondary_button(ui, &p, "Cancel").clicked() {
                                            close_requested = true;
                                        }
                                    },
                                );
                            });
                        });
                });
        });

    if close_requested {
        app.profile_modal_open = false;
    }
}

fn render_form_fields(app: &mut KrakenApp, ui: &mut egui::Ui, p: &Palette, field_w: f32) {
    // Validation error banner inside modal
    if let Some(ref err) = app.profile_form.validation_error {
        egui::Frame::none()
            .fill(p.error_container)
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::symmetric(12.0, 9.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(semibold(err, 12.5, p.on_error_container)).wrap());
            });
        ui.add_space(10.0);
    }

    // Profile name
    field_label(ui, p, "Profile name");
    ui.add(
        styled_text_edit(&mut app.profile_form.name, field_w).hint_text("e.g. Corporate Frankfurt"),
    );
    ui.add_space(10.0);

    // Protocol segmented switcher
    field_label(ui, p, "Protocol");
    let seg_w = ((field_w - 8.0) * 0.5).max(100.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (proto, label) in [
            (VpnProtocol::OpenVpn, "OpenVPN (.ovpn)"),
            (VpnProtocol::OpenConnect, "OpenConnect SSL"),
        ] {
            let active = app.profile_form.protocol == proto;
            let fill = if active { p.primary } else { p.surface_low };
            let fg = if active {
                p.on_primary
            } else {
                p.on_surface_variant
            };
            let stroke = if active {
                Stroke::NONE
            } else {
                Stroke::new(1.0_f32, p.outline_variant)
            };
            let btn = egui::Button::new(semibold(label, 12.5, fg))
                .fill(fill)
                .stroke(stroke)
                .rounding(Rounding::same(8.0))
                .min_size(Vec2::new(seg_w, 32.0));
            if ui.add(btn).clicked() && !active {
                app.profile_form.protocol = proto;
                app.profile_form.validation_error = None;
                app.openconnect_test_result = None;
                if app.profile_form.port_text.trim().is_empty()
                    || app.profile_form.port_text == "1194"
                    || app.profile_form.port_text == "443"
                {
                    app.profile_form.port_text = match proto {
                        VpnProtocol::OpenVpn => "1194".to_string(),
                        VpnProtocol::OpenConnect => "443".to_string(),
                    };
                }
            }
        }
    });
    ui.add_space(12.0);

    // Protocol-specific fields
    match app.profile_form.protocol {
        VpnProtocol::OpenVpn => {
            field_label(ui, p, "OpenVPN configuration file (.ovpn)");
            ui.allocate_ui_with_layout(
                Vec2::new(field_w, 30.0),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if row_secondary_button(ui, p, "Browse").clicked() {
                        if let Some(picked) = rfd::FileDialog::new()
                            .set_title("Select OpenVPN Config (.ovpn)")
                            .add_filter("OpenVPN Config", &["ovpn"])
                            .pick_file()
                        {
                            app.profile_form.ovpn_path = picked.to_string_lossy().into_owned();
                            if let Ok(content) = std::fs::read_to_string(&picked) {
                                let fname = picked
                                    .file_name()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("config.ovpn");
                                let parsed = crate::ovpn::parse_ovpn(&content, fname);
                                if app.profile_form.name.trim().is_empty() {
                                    app.profile_form.name = parsed.suggested_name;
                                }
                                if let Some(host) = parsed.remote_host {
                                    app.profile_form.server = host;
                                    app.profile_form.port_text = parsed.remote_port.to_string();
                                }
                                app.profile_form.ovpn_content = Some(content);
                                app.profile_form.validation_error = None;
                            }
                        }
                    }
                    let path_w = ui.available_width().max(80.0);
                    ui.add(
                        styled_text_edit(&mut app.profile_form.ovpn_path, path_w)
                            .hint_text(r"C:\path\to\profile.ovpn")
                            .font(egui::TextStyle::Monospace),
                    );
                },
            );
            ui.add_space(6.0);
            if app.profile_form.ovpn_content.is_some() {
                ui.horizontal(|ui| {
                    pill_badge(
                        ui,
                        p.success_container,
                        p.on_success_container,
                        Some(p.success),
                        "Inline .ovpn copy stored",
                    );
                    if row_secondary_button(ui, p, "Use file path only").clicked() {
                        app.profile_form.ovpn_content = None;
                    }
                });
            } else {
                ui.label(
                    RichText::new("Reads configuration directly from the .ovpn path on disk.")
                        .size(11.5)
                        .color(p.outline),
                );
            }
        }
        VpnProtocol::OpenConnect => {
            field_label(ui, p, "Gateway protocol");
            egui::ComboBox::from_id_salt("modal_oc_proto_combo")
                .width(field_w)
                .selected_text(app.profile_form.openconnect_protocol.label())
                .show_ui(ui, |ui| {
                    for proto in OpenConnectProtocol::ALL {
                        ui.selectable_value(
                            &mut app.profile_form.openconnect_protocol,
                            *proto,
                            proto.label(),
                        );
                    }
                });
            ui.add_space(10.0);

            // Server URL + Port side-by-side proportionally
            let port_w = 88.0_f32;
            let server_w = (field_w - port_w - 10.0).max(120.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.vertical(|ui| {
                    ui.set_width(server_w);
                    field_label(ui, p, "Server URL or host");
                    ui.add(
                        styled_text_edit(&mut app.profile_form.server, server_w)
                            .hint_text("vpn.company.com")
                            .font(egui::TextStyle::Monospace),
                    );
                });
                ui.vertical(|ui| {
                    ui.set_width(port_w);
                    field_label(ui, p, "Port");
                    ui.add(
                        styled_text_edit(&mut app.profile_form.port_text, port_w)
                            .hint_text("443")
                            .font(egui::TextStyle::Monospace),
                    );
                });
            });
            ui.add_space(10.0);

            // Authgroup + Interface side-by-side
            let half_w = ((field_w - 10.0) * 0.5).max(100.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                ui.vertical(|ui| {
                    ui.set_width(half_w);
                    field_label(ui, p, "Authgroup (optional)");
                    ui.add(
                        styled_text_edit(&mut app.profile_form.authgroup, half_w)
                            .hint_text("e.g. Employees"),
                    );
                });
                ui.vertical(|ui| {
                    ui.set_width(half_w);
                    field_label(ui, p, "Interface (optional)");
                    ui.add(
                        styled_text_edit(&mut app.profile_form.interface_name, half_w)
                            .hint_text("e.g. wintun"),
                    );
                });
            });
            ui.add_space(10.0);

            field_label(ui, p, "Server certificate pin (optional)");
            ui.add(
                styled_text_edit(&mut app.profile_form.servercert, field_w)
                    .hint_text("pin-sha256:...")
                    .font(egui::TextStyle::Monospace),
            );
        }
    }

    ui.add_space(12.0);
    divider(ui, p);
    ui.add_space(10.0);

    // Credentials row (Username + Password with Show/Hide)
    let half_w = ((field_w - 10.0) * 0.5).max(100.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        ui.vertical(|ui| {
            ui.set_width(half_w);
            field_label(ui, p, "Username (optional)");
            ui.add(styled_text_edit(&mut app.profile_form.username, half_w).hint_text("username"));
        });
        ui.vertical(|ui| {
            ui.set_width(half_w);
            field_label(ui, p, "Password (optional)");
            ui.allocate_ui_with_layout(
                Vec2::new(half_w, CONTROL_HEIGHT),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let lbl = if app.profile_form.show_password {
                        "Hide"
                    } else {
                        "Show"
                    };
                    if row_secondary_button(ui, p, lbl).clicked() {
                        app.profile_form.show_password = !app.profile_form.show_password;
                    }
                    let pw_w = ui.available_width().max(60.0);
                    ui.add(
                        styled_text_edit(&mut app.profile_form.password, pw_w)
                            .password(!app.profile_form.show_password)
                            .hint_text("••••••••"),
                    );
                },
            );
        });
    });

    ui.add_space(10.0);

    // OpenVPN encrypted private key / PKCS#12 passphrase field
    if app.profile_form.protocol == VpnProtocol::OpenVpn {
        let encrypted_detected = app
            .profile_form
            .ovpn_content
            .as_deref()
            .is_some_and(crate::vpn::openvpn::config_has_encrypted_key);
        ui.horizontal_wrapped(|ui| {
            ui.label(semibold(
                "Private key passphrase (optional)",
                12.0,
                p.on_surface_variant,
            ));
            if encrypted_detected {
                pill_badge(
                    ui,
                    p.warning_container,
                    p.on_warning_container,
                    Some(p.warning),
                    "Encrypted key detected",
                );
            }
        });
        ui.add_space(3.0);
        ui.allocate_ui_with_layout(
            Vec2::new(field_w, 30.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let lbl = if app.profile_form.show_key_passphrase {
                    "Hide"
                } else {
                    "Show"
                };
                if row_secondary_button(ui, p, lbl).clicked() {
                    app.profile_form.show_key_passphrase = !app.profile_form.show_key_passphrase;
                }
                let kp_w = ui.available_width().max(80.0);
                ui.add(
                    styled_text_edit(&mut app.profile_form.key_passphrase, kp_w)
                        .password(!app.profile_form.show_key_passphrase)
                        .hint_text("Passphrase for encrypted inline private key / PKCS#12"),
                );
            },
        );
        ui.add_space(10.0);
    }

    field_label(ui, p, "Notes (optional)");
    ui.add(
        styled_text_edit(&mut app.profile_form.notes, field_w)
            .hint_text("Internal notes or gateway description"),
    );

    // Async OpenConnect endpoint test feedback
    if let Some(ref res) = app.openconnect_test_result {
        ui.add_space(10.0);
        let (bg, fg) = if res.success {
            (p.success_container, p.on_success_container)
        } else {
            (p.error_container, p.on_error_container)
        };
        egui::Frame::none()
            .fill(bg)
            .rounding(Rounding::same(8.0))
            .inner_margin(Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(&res.message).size(12.0).color(fg)).wrap());
            });
    }
}

fn field_label(ui: &mut egui::Ui, p: &Palette, text: &str) {
    ui.label(semibold(text, 12.0, p.on_surface_variant));
    ui.add_space(3.0);
}
