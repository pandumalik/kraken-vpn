use eframe::egui::{self, Margin, RichText, Rounding, Stroke};

use crate::ui::theme::{pill_badge, primary_button, row_secondary_button, semibold, Palette};
use crate::ui::KrakenApp;
use crate::updater::{UpdateCheckResult, CURRENT_VERSION, RELEASES_PAGE_URL};

pub(super) fn render_updates_section(app: &mut KrakenApp, ui: &mut egui::Ui, p: &Palette) {
    let checking = app.update_check_rx.is_some();

    egui::Frame::none()
        .inner_margin(Margin::symmetric(20.0, 14.0))
        .show(ui, |ui| {
            let row_w = ui.available_width();
            ui.set_min_width(row_w);
            ui.set_max_width(row_w);

            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 10.0;

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        ui.horizontal_wrapped(|ui| {
                            ui.label(semibold(
                                format!("Kraken VPN v{CURRENT_VERSION}"),
                                13.5,
                                p.on_surface,
                            ));
                            if checking {
                                pill_badge(
                                    ui,
                                    p.warning_container,
                                    p.on_warning_container,
                                    Some(p.warning),
                                    "Checking...",
                                );
                            } else if let Some(ref status) = app.update_status {
                                match status {
                                    UpdateCheckResult::UpToDate { .. } => {
                                        pill_badge(
                                            ui,
                                            p.success_container,
                                            p.on_success_container,
                                            Some(p.success),
                                            "Up to date",
                                        );
                                    }
                                    UpdateCheckResult::UpdateAvailable { release, .. } => {
                                        pill_badge(
                                            ui,
                                            p.warning_container,
                                            p.on_warning_container,
                                            Some(p.warning),
                                            &format!("v{} available", release.version),
                                        );
                                    }
                                    UpdateCheckResult::NoReleases { .. } => {
                                        pill_badge(
                                            ui,
                                            p.surface_container,
                                            p.on_surface_variant,
                                            Some(p.outline),
                                            "No releases published",
                                        );
                                    }
                                    UpdateCheckResult::Error(_) => {
                                        pill_badge(
                                            ui,
                                            p.error_container,
                                            p.on_error_container,
                                            Some(p.error),
                                            "Check failed",
                                        );
                                    }
                                }
                            }
                        });
                        ui.label(
                            RichText::new("Checks the latest release tag on GitHub (pandumalik/kraken-vpn).")
                                .size(12.0)
                                .color(p.on_surface_variant),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        if ui
                            .add_enabled(
                                !checking,
                                egui::Button::new(semibold(
                                    if checking {
                                        "Checking..."
                                    } else {
                                        "Check for updates"
                                    },
                                    12.5,
                                    if checking {
                                        p.outline
                                    } else {
                                        p.on_surface_variant
                                    },
                                ))
                                .fill(p.surface_lowest)
                                .stroke(Stroke::new(1.0_f32, p.outline_variant))
                                .rounding(Rounding::same(7.0))
                                .min_size(egui::Vec2::new(0.0, 30.0)),
                            )
                            .clicked()
                        {
                            app.start_update_check(true);
                        }
                        if row_secondary_button(ui, p, "Releases").clicked() {
                            ui.ctx().open_url(egui::OpenUrl::new_tab(RELEASES_PAGE_URL));
                        }
                    });
                });

                if let Some(ref status) = app.update_status {
                    match status {
                        UpdateCheckResult::UpdateAvailable { release, .. } => {
                            let release = release.clone();
                            egui::Frame::none()
                                .fill(p.surface_low)
                                .rounding(Rounding::same(8.0))
                                .stroke(Stroke::new(1.0_f32, p.outline_variant))
                                .inner_margin(Margin::symmetric(14.0, 12.0))
                                .show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.vertical(|ui| {
                                        ui.spacing_mut().item_spacing.y = 6.0;
                                        ui.label(semibold(&release.name, 13.0, p.on_surface));
                                        if !release.notes_summary.is_empty() {
                                            ui.add(
                                                egui::Label::new(
                                                    RichText::new(&release.notes_summary)
                                                        .size(12.0)
                                                        .color(p.on_surface_variant),
                                                )
                                                .wrap(),
                                            );
                                        }
                                        ui.add_space(2.0);
                                        ui.horizontal(|ui| {
                                            ui.spacing_mut().item_spacing.x = 8.0;
                                            if let Some(ref url) = release.installer_url {
                                                if primary_button(ui, p, "Download installer")
                                                    .clicked()
                                                {
                                                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                                                }
                                            }
                                            if row_secondary_button(ui, p, "View release notes")
                                                .clicked()
                                            {
                                                ui.ctx().open_url(egui::OpenUrl::new_tab(
                                                    &release.html_url,
                                                ));
                                            }
                                        });
                                    });
                                });
                        }
                        UpdateCheckResult::Error(err) => {
                            ui.add(
                                egui::Label::new(RichText::new(err).size(11.5).color(p.error))
                                    .wrap(),
                            );
                        }
                        UpdateCheckResult::UpToDate { latest, .. } => {
                            ui.label(
                                RichText::new(format!(
                                    "You are running the latest version (v{latest})."
                                ))
                                .size(12.0)
                                .color(p.outline),
                            );
                        }
                        UpdateCheckResult::NoReleases { .. } => {
                            ui.label(
                                RichText::new(
                                    "No GitHub releases have been published for this repository yet.",
                                )
                                .size(12.0)
                                .color(p.outline),
                            );
                        }
                    }
                }
            });
        });
}
