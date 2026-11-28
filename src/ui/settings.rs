use eframe::egui::{self, Color32, Margin, Pos2, RichText, Rounding, Sense, Stroke, Vec2};

use crate::models::{AppSettings, ColorPreset, SwitchPolicy, ThemeMode};
use crate::platform::{check_binary, BinaryCheckResult};
use crate::ui::theme::{
    active_theme_colors, destructive_button, divider, page_container, pill_badge, preset_colors,
    row_secondary_button, semibold, short_binary_version, styled_text_edit, table_card_frame,
    toggle_switch, Palette, CONTROL_HEIGHT,
};
use crate::ui::KrakenApp;

pub fn render(app: &mut KrakenApp, ui: &mut egui::Ui, ctx: &egui::Context) {
    let p = app.palette();
    let mut changed = false;

    page_container(ui, 820.0, |ui| {
        // Page Header
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(semibold("Settings", 23.0, p.on_surface));
            ui.label(
                RichText::new(
                    "Connection policy, VPN client executables, and application preferences",
                )
                .size(13.5)
                .color(p.on_surface_variant),
            );
        });

        ui.add_space(20.0);

        // Group 1: Connection & Switch Policy
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

        // Group 2: VPN CLI Executables
        section_card(&p, ui, "VPN CLI executables", |ui| {
            binary_path_row(
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
            binary_path_row(
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

        // Group 3: Appearance & Local Storage
        section_card(&p, ui, "Appearance & storage", |ui| {
            // Theme switcher row
            egui::Frame::none()
                .inner_margin(Margin::symmetric(20.0, 14.0))
                .show(ui, |ui| {
                    let row_w = ui.available_width();
                    ui.set_min_width(row_w);
                    ui.set_max_width(row_w);
                    let text_w = (row_w - 156.0).max(160.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(text_w);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(semibold("Theme mode", 13.5, p.on_surface));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(
                                        "Switch between Light and Dark modes (each mode keeps its own custom color overrides).",
                                    )
                                    .size(12.0)
                                    .color(p.on_surface_variant),
                                )
                                .wrap(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            for (mode, label) in
                                [(ThemeMode::Dark, "Dark"), (ThemeMode::Light, "Light")]
                            {
                                let active = app.settings.theme == mode;
                                let fill = if active { p.primary } else { p.surface_lowest };
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
                                    .rounding(Rounding::same(7.0))
                                    .min_size(Vec2::new(66.0, 30.0));
                                if ui.add(btn).clicked() && !active {
                                    app.settings.theme = mode;
                                    changed = true;
                                }
                            }
                        });
                    });
                });

            divider(ui, &p);

            // Color presets + collapsible compact custom color editor
            egui::Frame::none()
                .inner_margin(Margin::symmetric(20.0, 14.0))
                .show(ui, |ui| {
                    let row_w = ui.available_width();
                    ui.set_min_width(row_w);
                    ui.set_max_width(row_w);
                    let mode_label = match app.settings.theme {
                        ThemeMode::Light => "Light",
                        ThemeMode::Dark => "Dark",
                    };
                    let has_custom = match app.settings.theme {
                        ThemeMode::Light => app.settings.custom_light_colors.is_some(),
                        ThemeMode::Dark => app.settings.custom_dark_colors.is_some(),
                    };
                    let is_modified = app.settings.color_preset != ColorPreset::ForestSage
                        || app.settings.custom_light_colors.is_some()
                        || app.settings.custom_dark_colors.is_some();

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        ui.horizontal(|ui| {
                            ui.label(semibold("Color palette", 13.5, p.on_surface));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 6.0;
                                    if is_modified
                                        && row_secondary_button(ui, &p, "Reset to original")
                                            .clicked()
                                    {
                                        app.settings.color_preset = ColorPreset::ForestSage;
                                        app.settings.custom_light_colors = None;
                                        app.settings.custom_dark_colors = None;
                                        changed = true;
                                        app.notify(
                                            "Colors restored to original Forest Sage palette.",
                                            false,
                                        );
                                    }
                                    let toggle_lbl = if app.custom_colors_expanded {
                                        format!("Customize ({mode_label}) ▾")
                                    } else {
                                        format!("Customize ({mode_label}) ▸")
                                    };
                                    if row_secondary_button(ui, &p, &toggle_lbl).clicked() {
                                        app.custom_colors_expanded = !app.custom_colors_expanded;
                                    }
                                },
                            );
                        });

                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                            for preset in ColorPreset::ALL {
                                let selected = !has_custom && app.settings.color_preset == *preset;
                                let preview = preset_colors(app.settings.theme, *preset);
                                if preset_chip(
                                    ui,
                                    &p,
                                    preset.label(),
                                    Color32::from_rgb(
                                        preview.surface[0],
                                        preview.surface[1],
                                        preview.surface[2],
                                    ),
                                    Color32::from_rgb(
                                        preview.primary[0],
                                        preview.primary[1],
                                        preview.primary[2],
                                    ),
                                    selected,
                                ) {
                                    app.settings.color_preset = *preset;
                                    match app.settings.theme {
                                        ThemeMode::Light => app.settings.custom_light_colors = None,
                                        ThemeMode::Dark => app.settings.custom_dark_colors = None,
                                    }
                                    changed = true;
                                }
                            }
                        });

                        if app.custom_colors_expanded {
                            ui.add_space(4.0);
                            let base = preset_colors(app.settings.theme, app.settings.color_preset);
                            let mut colors = active_theme_colors(&app.settings);
                            let mut color_edited = false;

                            egui::Frame::none()
                                .fill(p.surface_low)
                                .rounding(Rounding::same(8.0))
                                .stroke(Stroke::new(1.0_f32, p.outline_variant))
                                .inner_margin(Margin::symmetric(12.0, 10.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal_wrapped(|ui| {
                                        ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Primary",
                                            &mut colors.primary,
                                            base.primary,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Secondary",
                                            &mut colors.secondary,
                                            base.secondary,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Background",
                                            &mut colors.surface,
                                            base.surface,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Card",
                                            &mut colors.card,
                                            base.card,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Border",
                                            &mut colors.border,
                                            base.border,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Text",
                                            &mut colors.text,
                                            base.text,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Muted",
                                            &mut colors.text_muted,
                                            base.text_muted,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Success",
                                            &mut colors.success,
                                            base.success,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Warning",
                                            &mut colors.warning,
                                            base.warning,
                                        );
                                        color_edited |= compact_color_chip(
                                            ui,
                                            &p,
                                            "Error",
                                            &mut colors.error,
                                            base.error,
                                        );
                                    });
                                });

                            if color_edited {
                                let matches_preset = colors == base;
                                let next_val = if matches_preset { None } else { Some(colors) };
                                match app.settings.theme {
                                    ThemeMode::Light => {
                                        app.settings.custom_light_colors = next_val;
                                    }
                                    ThemeMode::Dark => {
                                        app.settings.custom_dark_colors = next_val;
                                    }
                                }
                                changed = true;
                            }
                        }
                    });
                });

            divider(ui, &p);

            // Local data & DPAPI info row
            egui::Frame::none()
                .inner_margin(Margin::symmetric(20.0, 14.0))
                .show(ui, |ui| {
                    let row_w = ui.available_width();
                    ui.set_min_width(row_w);
                    ui.set_max_width(row_w);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 5.0;
                        ui.horizontal_wrapped(|ui| {
                            ui.label(semibold(
                                "Local configuration & credentials",
                                13.5,
                                p.on_surface,
                            ));
                            #[cfg(windows)]
                            pill_badge(
                                ui,
                                p.success_container,
                                p.on_success_container,
                                Some(p.success),
                                "Windows DPAPI encrypted",
                            );
                        });
                        ui.add(
                            egui::Label::new(
                                RichText::new(app.store.root_dir().display().to_string())
                                    .monospace()
                                    .size(12.0)
                                    .color(p.outline),
                            )
                            .wrap(),
                        );
                    });
                });

            divider(ui, &p);

            // Reset preferences row
            egui::Frame::none()
                .inner_margin(Margin::symmetric(20.0, 14.0))
                .show(ui, |ui| {
                    let row_w = ui.available_width();
                    ui.set_min_width(row_w);
                    ui.set_max_width(row_w);
                    let text_w = (row_w - 148.0).max(160.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(text_w);
                            ui.spacing_mut().item_spacing.y = 2.0;
                            ui.label(semibold("Reset preferences", 13.5, p.on_surface));
                            ui.add(
                                egui::Label::new(
                                    RichText::new(
                                        "Restore default executable paths, switch policy, and theme. Saved profiles are preserved.",
                                    )
                                    .size(12.0)
                                    .color(p.on_surface_variant),
                                )
                                .wrap(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if destructive_button(ui, &p, "Reset settings").clicked() {
                                app.settings = AppSettings::default();
                                app.openvpn_check = Some(check_binary(&app.settings.openvpn_path));
                                app.openconnect_check =
                                    Some(check_binary(&app.settings.openconnect_path));
                                changed = true;
                                app.notify("Settings restored to defaults.", false);
                            }
                        });
                    });
                });
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
        egui::Frame::none()
            .fill(p.surface_low)
            .rounding(Rounding {
                nw: 11.0,
                ne: 11.0,
                sw: 0.0,
                se: 0.0,
            })
            .inner_margin(Margin::symmetric(20.0, 11.0))
            .show(ui, |ui| {
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

#[allow(clippy::too_many_arguments)]
fn binary_path_row(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    dialog_title: &str,
    download_url: &str,
    path_value: &mut String,
    check_result: &mut Option<BinaryCheckResult>,
    changed: &mut bool,
) {
    egui::Frame::none()
        .inner_margin(Margin::symmetric(20.0, 14.0))
        .show(ui, |ui| {
            let row_w = ui.available_width();
            ui.set_min_width(row_w);
            ui.set_max_width(row_w);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;

                // Title + compact status badge (wraps cleanly if window is narrow)
                ui.horizontal_wrapped(|ui| {
                    ui.label(semibold(label, 13.5, p.on_surface));
                    if let Some(ref res) = check_result {
                        if res.found {
                            let short = res
                                .version_line
                                .as_deref()
                                .map(short_binary_version)
                                .unwrap_or_else(|| "Found".to_string());
                            pill_badge(
                                ui,
                                p.success_container,
                                p.on_success_container,
                                Some(p.success),
                                &short,
                            );
                        } else {
                            pill_badge(
                                ui,
                                p.error_container,
                                p.on_error_container,
                                Some(p.error),
                                "Not found",
                            );
                        }
                    }
                });

                // Right-to-left layout allocates Download (if missing), Test & Browse first so TextEdit fills exact remaining width
                let is_missing = check_result.as_ref().is_some_and(|r| !r.found);
                ui.allocate_ui_with_layout(
                    Vec2::new(row_w, CONTROL_HEIGHT),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if is_missing && row_secondary_button(ui, p, "Download").clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(download_url));
                        }
                        if row_secondary_button(ui, p, "Test").clicked() {
                            *check_result = Some(check_binary(path_value));
                        }
                        if row_secondary_button(ui, p, "Browse").clicked() {
                            if let Some(picked) = rfd::FileDialog::new()
                                .set_title(dialog_title)
                                .add_filter("Executable", &["exe"])
                                .pick_file()
                            {
                                *path_value = picked.to_string_lossy().into_owned();
                                *check_result = Some(check_binary(path_value));
                                *changed = true;
                            }
                        }
                        let input_w = ui.available_width().max(80.0);
                        if ui
                            .add(
                                styled_text_edit(path_value, input_w)
                                    .font(egui::TextStyle::Monospace),
                            )
                            .changed()
                        {
                            *changed = true;
                        }
                    },
                );

                // Full wrapped version details or error message below input
                if let Some(ref res) = check_result {
                    if res.found {
                        if let Some(ref full_ver) = res.version_line {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(full_ver)
                                        .monospace()
                                        .size(11.0)
                                        .color(p.outline),
                                )
                                .wrap(),
                            );
                        }
                    } else if let Some(ref err) = res.error {
                        ui.add(
                            egui::Label::new(RichText::new(err).size(11.5).color(p.error)).wrap(),
                        );
                    }
                }
            });
        });
}

fn preset_chip(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    bg_preview: Color32,
    accent_preview: Color32,
    selected: bool,
) -> bool {
    let font = egui::FontId::new(12.0, egui::FontFamily::Name("semibold".into()));
    let fg = if selected {
        p.primary
    } else {
        p.on_surface_variant
    };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, fg);
    let pad_x = 10.0_f32;
    let dots_w = 24.0_f32;
    let size = Vec2::new((pad_x * 2.0 + dots_w + galley.size().x).ceil(), 30.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());

    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.surface_container
        } else if response.hovered() {
            p.surface_low
        } else {
            p.surface_lowest
        };
        let stroke = if selected {
            Stroke::new(1.5_f32, p.primary)
        } else {
            Stroke::new(1.0_f32, p.outline_variant)
        };
        let painter = ui.painter();
        painter.rect(rect, Rounding::same(7.0), fill, stroke);

        let cy = rect.center().y;
        let x0 = rect.left() + pad_x + 5.0;
        painter.circle(
            Pos2::new(x0, cy),
            5.0,
            bg_preview,
            Stroke::new(1.0_f32, p.outline_variant),
        );
        painter.circle(
            Pos2::new(x0 + 8.0, cy),
            5.0,
            accent_preview,
            Stroke::new(1.0_f32, p.outline_variant),
        );

        let text_pos = Pos2::new(rect.left() + pad_x + dots_w, cy - galley.size().y * 0.5);
        painter.galley(text_pos, galley, fg);
    }

    response.clicked()
}

fn compact_color_chip(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    rgb_val: &mut [u8; 3],
    original: [u8; 3],
) -> bool {
    let mut edited = false;
    let hex = format!("#{:02X}{:02X}{:02X}", rgb_val[0], rgb_val[1], rgb_val[2]);
    let modified = *rgb_val != original;

    egui::Frame::none()
        .fill(p.surface_lowest)
        .rounding(Rounding::same(6.0))
        .stroke(Stroke::new(
            1.0_f32,
            if modified {
                p.primary
            } else {
                p.outline_variant
            },
        ))
        .inner_margin(Margin::symmetric(7.0, 4.0))
        .show(ui, |ui| {
            ui.spacing_mut().interact_size = Vec2::new(22.0, 18.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                if ui
                    .color_edit_button_srgb(rgb_val)
                    .on_hover_text(format!("{label} ({hex})"))
                    .changed()
                {
                    edited = true;
                }
                ui.label(semibold(label, 11.5, p.on_surface))
                    .on_hover_text(&hex);
                if modified {
                    let reset_btn = egui::Button::new(
                        RichText::new("↺").size(11.5).color(p.on_surface_variant),
                    )
                    .frame(false);
                    if ui
                        .add(reset_btn)
                        .on_hover_text("Reset this color to preset")
                        .clicked()
                    {
                        *rgb_val = original;
                        edited = true;
                    }
                }
            });
        });
    edited
}
