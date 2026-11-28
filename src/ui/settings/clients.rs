use eframe::egui::{self, Margin, RichText, Vec2};

use crate::platform::{check_binary, BinaryCheckResult};
use crate::ui::theme::{
    pill_badge, row_secondary_button, semibold, short_binary_version, styled_text_edit, Palette,
    CONTROL_HEIGHT,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn binary_path_row(
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
