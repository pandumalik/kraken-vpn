use chrono::Utc;
use eframe::egui::{self, Align2, Color32, Margin, Pos2, RichText, Rounding, Sense, Stroke, Vec2};

use crate::models::ThemeMode;
use crate::telemetry::format_duration;
use crate::ui::state::NavTab;
use crate::ui::theme::{divider, semibold, tunnel_status_style, Palette};
use crate::ui::KrakenApp;
use crate::vpn::state::TunnelState;

impl KrakenApp {
    pub(super) fn render_sidebar(&mut self, ctx: &egui::Context, state: &TunnelState) {
        let p = self.palette();

        egui::SidePanel::left("kraken_sidebar")
            .resizable(false)
            .exact_width(224.0)
            .frame(
                egui::Frame::none()
                    .fill(p.surface_low)
                    .stroke(Stroke::new(1.0_f32, p.outline_variant))
                    .inner_margin(Margin::symmetric(14.0, 18.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if let Some(ref tex) = self.icon_texture {
                        ui.image((tex.id(), Vec2::splat(28.0)));
                    } else {
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
                        ui.painter().rect_filled(r, Rounding::same(7.0), p.primary);
                        ui.painter().text(
                            r.center(),
                            Align2::CENTER_CENTER,
                            "K",
                            egui::FontId::proportional(15.0),
                            p.on_primary,
                        );
                    }
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(semibold("Kraken VPN", 16.5, p.on_surface));
                        ui.label(
                            RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                                .size(11.0)
                                .color(p.outline),
                        );
                    });
                });

                ui.add_space(22.0);

                for (tab, label) in [
                    (NavTab::Home, "Home"),
                    (NavTab::Monitoring, "Monitoring"),
                    (NavTab::Logs, "Logs"),
                    (NavTab::Settings, "Settings"),
                ] {
                    let selected = self.active_tab == tab;
                    if nav_rail_item(ui, &p, tab, label, selected).clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(4.0);
                }

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(2.0);

                    ui.horizontal(|ui| {
                        let (elev_col, elev_label) = if self.is_admin {
                            (p.success, "Administrator")
                        } else {
                            (p.warning, "Standard User")
                        };
                        let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 3.2, elev_col);
                        ui.label(
                            RichText::new(elev_label)
                                .size(11.5)
                                .color(p.on_surface_variant),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let mode_label = match self.settings.theme {
                                ThemeMode::Light => "Dark",
                                ThemeMode::Dark => "Light",
                            };
                            let btn = egui::Button::new(
                                RichText::new(mode_label)
                                    .size(11.0)
                                    .color(p.on_surface_variant),
                            )
                            .fill(p.surface_container)
                            .stroke(Stroke::new(1.0_f32, p.outline_variant))
                            .rounding(Rounding::same(6.0))
                            .min_size(Vec2::new(42.0, 22.0));
                            if ui
                                .add(btn)
                                .on_hover_text("Switch Light / Dark theme")
                                .clicked()
                            {
                                self.settings.theme = match self.settings.theme {
                                    ThemeMode::Light => ThemeMode::Dark,
                                    ThemeMode::Dark => ThemeMode::Light,
                                };
                                self.save_settings_to_disk(ctx);
                            }
                        });
                    });

                    ui.add_space(8.0);

                    let (_, _, dot_col, status_txt) = tunnel_status_style(&p, state);
                    let detail_line = match state {
                        TunnelState::Connected { endpoint, .. } => {
                            let dur = format_duration(state.connected_duration_secs(Utc::now()));
                            let short_ep = truncate_middle(endpoint, 16);
                            format!("{dur} • {short_ep}")
                        }
                        TunnelState::Connecting { profile_name, .. } => {
                            format!("Dialing {}", truncate_middle(profile_name, 18))
                        }
                        TunnelState::Disconnecting { profile_name, .. } => {
                            format!("Closing {}", truncate_middle(profile_name, 18))
                        }
                        TunnelState::Error { profile_name, .. } => {
                            format!("Error: {}", truncate_middle(profile_name, 16))
                        }
                        TunnelState::Disconnected => "Single-tunnel guard on".to_string(),
                    };

                    ui.label(
                        RichText::new(detail_line)
                            .monospace()
                            .size(11.0)
                            .color(p.outline),
                    );
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 7.0;
                        let (dot_r, _) = ui.allocate_exact_size(Vec2::splat(9.0), Sense::hover());
                        ui.painter().circle_filled(dot_r.center(), 4.0, dot_col);
                        ui.label(semibold(status_txt, 13.0, p.on_surface));
                    });

                    ui.add_space(10.0);
                    divider(ui, &p);
                });
            });
    }
}

fn nav_rail_item(
    ui: &mut egui::Ui,
    p: &Palette,
    tab: NavTab,
    label: &str,
    selected: bool,
) -> egui::Response {
    let desired = Vec2::new(ui.available_width(), 38.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    if ui.is_rect_visible(rect) {
        let fill = if selected {
            p.surface_lowest
        } else if response.hovered() {
            p.surface_container
        } else {
            Color32::TRANSPARENT
        };
        let stroke = if selected {
            Stroke::new(1.0_f32, p.outline_variant)
        } else {
            Stroke::NONE
        };
        ui.painter().rect(rect, Rounding::same(8.0), fill, stroke);

        let fg = if selected {
            p.primary
        } else if response.hovered() {
            p.on_surface
        } else {
            p.on_surface_variant
        };

        let icon_center = Pos2::new(rect.left() + 20.0, rect.center().y);
        paint_nav_icon(ui.painter(), tab, icon_center, fg);

        let text_pos = Pos2::new(rect.left() + 38.0, rect.center().y);
        let font = egui::FontId::new(
            13.5,
            if selected {
                egui::FontFamily::Name("semibold".into())
            } else {
                egui::FontFamily::Proportional
            },
        );
        ui.painter()
            .text(text_pos, Align2::LEFT_CENTER, label, font, fg);
    }

    response
}

fn paint_nav_icon(painter: &egui::Painter, tab: NavTab, c: Pos2, color: Color32) {
    let stroke = Stroke::new(1.6_f32, color);
    match tab {
        NavTab::Home => {
            let roof = vec![
                Pos2::new(c.x - 6.5, c.y - 0.5),
                Pos2::new(c.x, c.y - 6.5),
                Pos2::new(c.x + 6.5, c.y - 0.5),
            ];
            painter.add(egui::Shape::line(roof, stroke));
            let body = vec![
                Pos2::new(c.x - 4.8, c.y - 1.0),
                Pos2::new(c.x - 4.8, c.y + 5.8),
                Pos2::new(c.x + 4.8, c.y + 5.8),
                Pos2::new(c.x + 4.8, c.y - 1.0),
            ];
            painter.add(egui::Shape::line(body, stroke));
        }
        NavTab::Monitoring => {
            let pts = vec![
                Pos2::new(c.x - 7.0, c.y + 1.0),
                Pos2::new(c.x - 3.5, c.y + 1.0),
                Pos2::new(c.x - 1.5, c.y - 5.0),
                Pos2::new(c.x + 1.5, c.y + 5.0),
                Pos2::new(c.x + 3.5, c.y + 1.0),
                Pos2::new(c.x + 7.0, c.y + 1.0),
            ];
            painter.add(egui::Shape::line(pts, stroke));
        }
        NavTab::Logs => {
            for dy in [-4.5_f32, 0.0_f32, 4.5_f32] {
                let end_x = if dy > 2.0 { c.x + 3.5 } else { c.x + 6.5 };
                painter.line_segment(
                    [Pos2::new(c.x - 6.5, c.y + dy), Pos2::new(end_x, c.y + dy)],
                    stroke,
                );
            }
        }
        NavTab::Settings => {
            painter.line_segment(
                [
                    Pos2::new(c.x - 6.5, c.y - 3.2),
                    Pos2::new(c.x + 6.5, c.y - 3.2),
                ],
                stroke,
            );
            painter.circle_filled(Pos2::new(c.x - 2.0, c.y - 3.2), 2.3, color);
            painter.line_segment(
                [
                    Pos2::new(c.x - 6.5, c.y + 3.2),
                    Pos2::new(c.x + 6.5, c.y + 3.2),
                ],
                stroke,
            );
            painter.circle_filled(Pos2::new(c.x + 2.5, c.y + 3.2), 2.3, color);
        }
    }
}

fn truncate_middle(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    let prefix: String = s.chars().take(keep).collect();
    format!("{prefix}…")
}
