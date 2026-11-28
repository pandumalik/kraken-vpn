use eframe::egui::{self, Color32, Margin, Pos2, RichText, Rounding, Sense, Stroke, Vec2};

use crate::models::{AppSettings, ColorPreset, ThemeMode};
use crate::platform::check_binary;
use crate::ui::theme::{
    active_theme_colors, destructive_button, divider, pill_badge, preset_colors,
    row_secondary_button, semibold, Palette,
};
use crate::ui::KrakenApp;

pub(super) fn render_appearance_and_storage(
    app: &mut KrakenApp,
    ui: &mut egui::Ui,
    p: &Palette,
    changed: &mut bool,
) {
    render_theme_mode_row(app, ui, p, changed);
    divider(ui, p);
    render_color_palette_row(app, ui, p, changed);
    divider(ui, p);
    render_storage_row(app, ui, p);
    divider(ui, p);
    render_reset_row(app, ui, p, changed);
}

fn render_theme_mode_row(app: &mut KrakenApp, ui: &mut egui::Ui, p: &Palette, changed: &mut bool) {
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
                    for (mode, label) in [(ThemeMode::Dark, "Dark"), (ThemeMode::Light, "Light")] {
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
                            *changed = true;
                        }
                    }
                });
            });
        });
}

fn render_color_palette_row(
    app: &mut KrakenApp,
    ui: &mut egui::Ui,
    p: &Palette,
    changed: &mut bool,
) {
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
            let has_custom = app.settings.active_custom_colors().is_some();
            let is_modified = app.settings.color_preset != ColorPreset::ForestSage
                || app.settings.custom_light_colors.is_some()
                || app.settings.custom_dark_colors.is_some();

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                ui.horizontal(|ui| {
                    ui.label(semibold("Color palette", 13.5, p.on_surface));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if is_modified && row_secondary_button(ui, p, "Reset to original").clicked()
                        {
                            app.settings.color_preset = ColorPreset::ForestSage;
                            app.settings.custom_light_colors = None;
                            app.settings.custom_dark_colors = None;
                            *changed = true;
                            app.notify("Colors restored to original Forest Sage palette.", false);
                        }
                        let toggle_lbl = if app.custom_colors_expanded {
                            format!("Customize ({mode_label}) ▾")
                        } else {
                            format!("Customize ({mode_label}) ▸")
                        };
                        if row_secondary_button(ui, p, &toggle_lbl).clicked() {
                            app.custom_colors_expanded = !app.custom_colors_expanded;
                        }
                    });
                });

                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                    for preset in ColorPreset::ALL {
                        let selected = !has_custom && app.settings.color_preset == *preset;
                        let preview = preset_colors(app.settings.theme, *preset);
                        if preset_chip(
                            ui,
                            p,
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
                            *app.settings.active_custom_colors_mut() = None;
                            *changed = true;
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
                                let fields: [(&str, &mut [u8; 3], [u8; 3]); 10] = [
                                    ("Primary", &mut colors.primary, base.primary),
                                    ("Secondary", &mut colors.secondary, base.secondary),
                                    ("Background", &mut colors.surface, base.surface),
                                    ("Card", &mut colors.card, base.card),
                                    ("Border", &mut colors.border, base.border),
                                    ("Text", &mut colors.text, base.text),
                                    ("Muted", &mut colors.text_muted, base.text_muted),
                                    ("Success", &mut colors.success, base.success),
                                    ("Warning", &mut colors.warning, base.warning),
                                    ("Error", &mut colors.error, base.error),
                                ];
                                for (label, value, orig) in fields {
                                    color_edited |= compact_color_chip(ui, p, label, value, orig);
                                }
                            });
                        });

                    if color_edited {
                        *app.settings.active_custom_colors_mut() =
                            (colors != base).then_some(colors);
                        *changed = true;
                    }
                }
            });
        });
}

fn render_storage_row(app: &KrakenApp, ui: &mut egui::Ui, p: &Palette) {
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
}

fn render_reset_row(app: &mut KrakenApp, ui: &mut egui::Ui, p: &Palette, changed: &mut bool) {
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
                    if destructive_button(ui, p, "Reset settings").clicked() {
                        app.settings = AppSettings::default();
                        app.openvpn_check = Some(check_binary(&app.settings.openvpn_path));
                        app.openconnect_check = Some(check_binary(&app.settings.openconnect_path));
                        *changed = true;
                        app.notify("Settings restored to defaults.", false);
                    }
                });
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
