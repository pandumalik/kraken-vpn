use eframe::egui::{self, Color32, FontId, Margin, RichText, Rounding, Stroke, TextFormat, Vec2};

use crate::models::{DiagnosticsLine, HistoryStatus, LogLevel};
use crate::telemetry::format_duration;
use crate::ui::theme::{
    destructive_button, divider, page_container, pill_badge, secondary_button, semibold,
    styled_text_edit, table_card_frame, tunnel_status_style, Palette,
};
use crate::ui::KrakenApp;
use crate::vpn::state::TunnelState;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsSubtab {
    Console,
    History,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevelFilter {
    All,
    Info,
    Warn,
    Error,
}

impl LogLevelFilter {
    pub fn matches(self, level: LogLevel) -> bool {
        match self {
            LogLevelFilter::All => true,
            LogLevelFilter::Info => matches!(level, LogLevel::Info | LogLevel::System),
            LogLevelFilter::Warn => matches!(level, LogLevel::Warn),
            LogLevelFilter::Error => matches!(level, LogLevel::Error),
        }
    }
}

pub fn render(app: &mut KrakenApp, ui: &mut egui::Ui) {
    let p = app.palette();

    page_container(ui, 920.0, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(semibold("Logs", 23.0, p.on_surface));
                ui.label(
                    RichText::new("Connection events, client output, and session history")
                        .size(13.5)
                        .color(p.on_surface_variant),
                );
            });
            ui.with_layout(
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| match app.diagnostics_subtab {
                    DiagnosticsSubtab::Console => {
                        if destructive_button(ui, &p, "Clear").clicked() {
                            app.manager.clear_diagnostics();
                        }
                        if secondary_button(ui, &p, "Copy logs").clicked() {
                            let all_text = app
                                .manager
                                .diagnostics_snapshot()
                                .iter()
                                .map(|l| l.format_line())
                                .collect::<Vec<_>>()
                                .join("\n");
                            ui.ctx().copy_text(all_text);
                            app.notify("Copied logs to clipboard.", false);
                        }
                    }
                    DiagnosticsSubtab::History => {
                        if !app.history.is_empty()
                            && destructive_button(ui, &p, "Clear history").clicked()
                        {
                            app.history.clear();
                            let _ = app.store.save_history(&app.history);
                            app.notify("Connection history cleared.", false);
                        }
                    }
                },
            );
        });

        ui.add_space(18.0);

        let lines = app.manager.diagnostics_snapshot();
        let info_count = lines
            .iter()
            .filter(|e| matches!(e.level, LogLevel::Info | LogLevel::System))
            .count();
        let warn_count = lines
            .iter()
            .filter(|e| matches!(e.level, LogLevel::Warn))
            .count();
        let err_count = lines
            .iter()
            .filter(|e| matches!(e.level, LogLevel::Error))
            .count();

        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);

            segment_pill(
                ui,
                &p,
                "Live terminal",
                app.diagnostics_subtab == DiagnosticsSubtab::Console,
                || app.diagnostics_subtab = DiagnosticsSubtab::Console,
            );
            segment_pill(
                ui,
                &p,
                &format!("History ({})", app.history.len()),
                app.diagnostics_subtab == DiagnosticsSubtab::History,
                || app.diagnostics_subtab = DiagnosticsSubtab::History,
            );

            if app.diagnostics_subtab == DiagnosticsSubtab::Console {
                ui.add_space(8.0);
                for (lvl, label) in [
                    (LogLevelFilter::All, format!("All ({})", lines.len())),
                    (LogLevelFilter::Info, format!("Info ({info_count})")),
                    (LogLevelFilter::Warn, format!("Warn ({warn_count})")),
                    (LogLevelFilter::Error, format!("Error ({err_count})")),
                ] {
                    filter_pill(ui, &p, &label, app.log_level_filter == lvl, || {
                        app.log_level_filter = lvl
                    });
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let search_w = (ui.available_width() * 0.65).clamp(140.0, 220.0);
                    ui.add(
                        styled_text_edit(&mut app.console_filter, search_w)
                            .hint_text("Filter logs..."),
                    );
                    ui.checkbox(&mut app.console_auto_scroll, "Auto-scroll");
                });
            }
        });

        ui.add_space(12.0);

        match app.diagnostics_subtab {
            DiagnosticsSubtab::Console => render_console_terminal(app, ui, &p, &lines),
            DiagnosticsSubtab::History => render_history_table(app, ui, &p),
        }
    });
}

fn render_console_terminal(
    app: &KrakenApp,
    ui: &mut egui::Ui,
    p: &Palette,
    lines: &[DiagnosticsLine],
) {
    let filter_lower = app.console_filter.trim().to_ascii_lowercase();
    let filtered: Vec<(usize, &DiagnosticsLine)> = lines
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            app.log_level_filter.matches(entry.level)
                && (filter_lower.is_empty()
                    || entry.message.to_ascii_lowercase().contains(&filter_lower)
                    || entry.source.to_ascii_lowercase().contains(&filter_lower))
        })
        .collect();

    let state = app.manager.state();
    let (status_bg, status_fg, status_dot, status_label) = tunnel_status_style(p, &state);
    let session_meta = match &state {
        TunnelState::Connected {
            profile_name,
            endpoint,
            ..
        } => {
            let iface = app
                .telemetry
                .active_interface
                .as_deref()
                .unwrap_or("tunnel");
            format!("{profile_name} · {endpoint} · {iface}")
        }
        TunnelState::Connecting {
            profile_name,
            endpoint,
            ..
        } => format!("{profile_name} · {endpoint}"),
        TunnelState::Disconnecting { profile_name, .. } => format!("{profile_name} · closing"),
        TunnelState::Error { profile_name, .. } => format!("{profile_name} · fault"),
        TunnelState::Disconnected => "idle · ready to monitor".to_string(),
    };

    let term_bg = Color32::from_rgb(13, 17, 15);
    let term_header_bg = Color32::from_rgb(20, 25, 22);
    let term_border = p.outline_variant;
    let term_divider = Color32::from_rgb(36, 44, 39);
    let term_muted = Color32::from_rgb(112, 128, 118);
    let term_text = Color32::from_rgb(226, 232, 228);
    let term_sys = Color32::from_rgb(125, 211, 252);
    let term_info = Color32::from_rgb(116, 214, 158);
    let term_warn = Color32::from_rgb(251, 191, 36);
    let term_err = Color32::from_rgb(252, 129, 129);
    let term_source = Color32::from_rgb(167, 196, 178);
    let warn_row_bg = Color32::from_rgba_unmultiplied(251, 191, 36, 16);
    let err_row_bg = Color32::from_rgba_unmultiplied(248, 113, 113, 22);

    egui::Frame::none()
        .fill(term_bg)
        .rounding(Rounding::same(12.0))
        .stroke(Stroke::new(1.0_f32, term_border))
        .inner_margin(Margin::same(0.0))
        .show(ui, |ui| {
            let card_w = ui.available_width();
            ui.set_min_width(card_w);
            ui.set_max_width(card_w);

            egui::Frame::none()
                .fill(term_header_bg)
                .rounding(Rounding {
                    nw: 11.0,
                    ne: 11.0,
                    sw: 0.0,
                    se: 0.0,
                })
                .inner_margin(Margin::symmetric(14.0, 9.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        pill_badge(ui, status_bg, status_fg, Some(status_dot), status_label);
                        ui.add(
                            egui::Label::new(
                                RichText::new(&session_meta)
                                    .monospace()
                                    .size(11.5)
                                    .color(term_text),
                            )
                            .truncate(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let count_text = if filtered.len() == lines.len() {
                                format!("{} lines", lines.len())
                            } else {
                                format!("{} / {} lines", filtered.len(), lines.len())
                            };
                            ui.label(
                                RichText::new(count_text)
                                    .monospace()
                                    .size(11.0)
                                    .color(term_muted),
                            );
                        });
                    });
                });

            let (div_rect, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
            ui.painter().line_segment(
                [div_rect.left_center(), div_rect.right_center()],
                Stroke::new(1.0_f32, term_divider),
            );

            let scroll_h = (ui.available_height() - 2.0).max(200.0);

            if filtered.is_empty() {
                egui::Frame::none()
                    .inner_margin(Margin::symmetric(20.0, 34.0))
                    .show(ui, |ui| {
                        ui.set_min_height((scroll_h - 68.0).max(120.0));
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new(if lines.is_empty() {
                                    "$ waiting for vpn stream... connect a profile to monitor live output"
                                } else {
                                    "$ no log entries match the active filter"
                                })
                                .monospace()
                                .size(12.5)
                                .color(term_muted),
                            );
                        });
                    });
                return;
            }

            egui::ScrollArea::vertical()
                .id_salt("stitch_logs_terminal_scroll")
                .min_scrolled_height(scroll_h)
                .max_height(scroll_h)
                .auto_shrink([false, false])
                .stick_to_bottom(app.console_auto_scroll)
                .show(ui, |ui| {
                    egui::Frame::none()
                        .inner_margin(Margin::symmetric(12.0, 8.0))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.spacing_mut().item_spacing.y = 2.0;

                            let mono = FontId::monospace(12.0);
                            for (idx, entry) in filtered {
                                let (lvl_tag, lvl_color, msg_color, row_bg, accent) =
                                    match entry.level {
                                        LogLevel::System => {
                                            ("SYS ", term_sys, term_text, Color32::TRANSPARENT, None)
                                        }
                                        LogLevel::Info => {
                                            ("INFO", term_info, term_text, Color32::TRANSPARENT, None)
                                        }
                                        LogLevel::Warn => (
                                            "WARN",
                                            term_warn,
                                            term_warn,
                                            warn_row_bg,
                                            Some(term_warn),
                                        ),
                                        LogLevel::Error => (
                                            "ERR ",
                                            term_err,
                                            term_err,
                                            err_row_bg,
                                            Some(term_err),
                                        ),
                                    };

                                let ts = entry
                                    .timestamp
                                    .with_timezone(&chrono::Local)
                                    .format("%H:%M:%S%.3f")
                                    .to_string();

                                let mut job = egui::text::LayoutJob::default();
                                job.append(
                                    &format!("{:>4} ", idx + 1),
                                    0.0,
                                    TextFormat::simple(mono.clone(), term_muted),
                                );
                                job.append(
                                    &format!("{ts} "),
                                    0.0,
                                    TextFormat::simple(mono.clone(), term_muted),
                                );
                                job.append(
                                    &format!("[{lvl_tag}] "),
                                    0.0,
                                    TextFormat::simple(mono.clone(), lvl_color),
                                );
                                job.append(
                                    &format!("{:<11} ", entry.source),
                                    0.0,
                                    TextFormat::simple(mono.clone(), term_source),
                                );
                                job.append(
                                    &entry.message,
                                    0.0,
                                    TextFormat::simple(mono.clone(), msg_color),
                                );

                                let resp = egui::Frame::none()
                                    .fill(row_bg)
                                    .rounding(Rounding::same(4.0))
                                    .inner_margin(Margin::symmetric(8.0, 2.5))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.add(egui::Label::new(job).wrap());
                                    });

                                if let Some(bar_color) = accent {
                                    let r = resp.response.rect;
                                    ui.painter().rect_filled(
                                        egui::Rect::from_min_max(
                                            r.min,
                                            egui::Pos2::new(r.left() + 2.5, r.bottom()),
                                        ),
                                        Rounding::same(1.5),
                                        bar_color,
                                    );
                                }
                            }
                        });
                });
        });
}

fn render_history_table(app: &KrakenApp, ui: &mut egui::Ui, p: &Palette) {
    table_card_frame(p).show(ui, |ui| {
        ui.set_width(ui.available_width());

        if app.history.is_empty() {
            egui::Frame::none()
                .inner_margin(Margin::symmetric(20.0, 28.0))
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("No past VPN sessions recorded yet.")
                                .size(13.0)
                                .color(p.outline),
                        );
                    });
                });
            return;
        }

        let total_w = ui.available_width();
        let col_profile = (total_w * 0.20).max(90.0);
        let col_proto = (total_w * 0.16).max(80.0);
        let col_status = 104.0_f32;
        let col_started = 132.0_f32;
        let col_dur = 72.0_f32;
        let col_details =
            (total_w - col_profile - col_proto - col_status - col_started - col_dur - 72.0)
                .max(80.0);

        egui::Frame::none()
            .fill(p.surface_low)
            .rounding(Rounding {
                nw: 11.0,
                ne: 11.0,
                sw: 0.0,
                se: 0.0,
            })
            .inner_margin(Margin::symmetric(16.0, 10.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    header_col(ui, p, "Profile", col_profile);
                    header_col(ui, p, "Protocol", col_proto);
                    header_col(ui, p, "Status", col_status);
                    header_col(ui, p, "Started", col_started);
                    header_col(ui, p, "Duration", col_dur);
                    ui.label(semibold("Details", 11.5, p.on_surface_variant));
                });
            });
        divider(ui, p);

        for (i, h) in app.history.iter().enumerate() {
            if i > 0 {
                divider(ui, p);
            }
            let (bg, fg) = match h.status {
                HistoryStatus::Connected => (p.success_container, p.on_success_container),
                HistoryStatus::Disconnected => (p.surface_container, p.on_surface_variant),
                HistoryStatus::Failed => (p.error_container, p.on_error_container),
            };
            egui::Frame::none()
                .inner_margin(Margin::symmetric(16.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        ui.allocate_ui_with_layout(
                            Vec2::new(col_profile, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(semibold(&h.profile_name, 13.0, p.on_surface))
                                        .truncate(),
                                );
                            },
                        );

                        ui.allocate_ui_with_layout(
                            Vec2::new(col_proto, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&h.protocol)
                                            .size(12.0)
                                            .color(p.on_surface_variant),
                                    )
                                    .truncate(),
                                );
                            },
                        );

                        ui.allocate_ui_with_layout(
                            Vec2::new(col_status, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                pill_badge(ui, bg, fg, None, h.status.label());
                            },
                        );

                        ui.allocate_ui_with_layout(
                            Vec2::new(col_started, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                let started = h
                                    .started_at
                                    .with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d %H:%M")
                                    .to_string();
                                ui.label(
                                    RichText::new(started)
                                        .monospace()
                                        .size(11.5)
                                        .color(p.outline),
                                );
                            },
                        );

                        ui.allocate_ui_with_layout(
                            Vec2::new(col_dur, 22.0),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    RichText::new(format_duration(h.duration_secs))
                                        .monospace()
                                        .size(11.5)
                                        .color(p.on_surface_variant),
                                );
                            },
                        );

                        ui.vertical(|ui| {
                            ui.set_width(col_details);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&h.message)
                                        .size(12.0)
                                        .color(p.on_surface_variant),
                                )
                                .wrap(),
                            );
                        });
                    });
                });
        }
    });
}

fn segment_pill(
    ui: &mut egui::Ui,
    p: &Palette,
    label: &str,
    active: bool,
    on_click: impl FnOnce(),
) {
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
        .rounding(Rounding::same(8.0))
        .min_size(Vec2::new(0.0, 30.0));
    if ui.add(btn).clicked() {
        on_click();
    }
}

fn filter_pill(ui: &mut egui::Ui, p: &Palette, label: &str, active: bool, on_click: impl FnOnce()) {
    let fill = if active {
        p.surface_lowest
    } else {
        p.surface_container
    };
    let fg = if active {
        p.primary
    } else {
        p.on_surface_variant
    };
    let stroke = if active {
        Stroke::new(1.0_f32, p.primary)
    } else {
        Stroke::new(1.0_f32, p.outline_variant)
    };
    let btn = egui::Button::new(semibold(label, 12.0, fg))
        .fill(fill)
        .stroke(stroke)
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(0.0, 30.0));
    if ui.add(btn).clicked() {
        on_click();
    }
}

fn header_col(ui: &mut egui::Ui, p: &Palette, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 18.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(semibold(text, 11.5, p.on_surface_variant));
        },
    );
}
