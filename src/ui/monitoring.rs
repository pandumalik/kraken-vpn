use chrono::Utc;
use eframe::egui::{
    self, Align2, Color32, FontId, Margin, Mesh, Pos2, Rect, RichText, Sense, Shape, Stroke, Vec2,
};

use crate::telemetry::{format_bytes, format_duration, format_speed};
use crate::ui::theme::{
    card_frame, card_header_frame, divider, page_container, page_header, secondary_button,
    semibold, table_card_frame, Palette,
};
use crate::ui::KrakenApp;
use crate::vpn::state::TunnelState;

pub fn render(app: &mut KrakenApp, ui: &mut egui::Ui, state: &TunnelState) {
    let p = app.palette();

    page_container(ui, 920.0, |ui| {
        ui.horizontal(|ui| {
            page_header(
                ui,
                &p,
                "Monitoring",
                "Live tunnel bandwidth and session telemetry",
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if secondary_button(ui, &p, "Reset counters").clicked() {
                    app.manager.reset_telemetry_counters();
                    app.telemetry = app.manager.sample_telemetry();
                    app.notify("Session traffic counters reset.", false);
                }
            });
        });

        ui.add_space(20.0);

        let total_session = app
            .telemetry
            .session_rx_bytes
            .saturating_add(app.telemetry.session_tx_bytes);
        let dl_peak = format!("Peak {}", format_speed(app.telemetry.peak_rx_bytes_per_sec));
        let ul_peak = format!("Peak {}", format_speed(app.telemetry.peak_tx_bytes_per_sec));
        let total_breakdown = format!(
            "Down {}  ·  Up {}",
            format_bytes(app.telemetry.session_rx_bytes),
            format_bytes(app.telemetry.session_tx_bytes),
        );

        ui.columns(3, |cols| {
            summary_metric_card(
                &mut cols[0],
                &p,
                "Download speed",
                &format_speed(app.telemetry.rx_bytes_per_sec),
                &dl_peak,
                p.primary,
            );
            summary_metric_card(
                &mut cols[1],
                &p,
                "Upload speed",
                &format_speed(app.telemetry.tx_bytes_per_sec),
                &ul_peak,
                p.secondary,
            );
            summary_metric_card(
                &mut cols[2],
                &p,
                "Session total",
                &format_bytes(total_session),
                &total_breakdown,
                p.on_surface,
            );
        });

        ui.add_space(18.0);

        card_frame(&p).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(semibold("Live traffic", 15.0, p.on_surface));
                ui.label(
                    RichText::new("(Last 60 seconds)")
                        .size(12.5)
                        .color(p.outline),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 14.0;
                    legend_item(ui, &p, p.secondary, "Upload");
                    legend_item(ui, &p, p.primary, "Download");
                });
            });

            ui.add_space(14.0);
            paint_live_traffic_chart(
                ui,
                &p,
                &app.telemetry.rx_history,
                &app.telemetry.tx_history,
                state.is_connected(),
            );
        });

        ui.add_space(18.0);

        render_tunnel_details_card(app, ui, &p, state);
    });
}

fn summary_metric_card(
    ui: &mut egui::Ui,
    p: &Palette,
    title: &str,
    value: &str,
    subtitle: &str,
    accent: Color32,
) {
    card_frame(p).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(12.5).color(p.on_surface_variant));
        ui.add_space(6.0);
        ui.label(
            RichText::new(value)
                .monospace()
                .size(21.0)
                .strong()
                .color(accent),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(subtitle)
                .monospace()
                .size(11.5)
                .color(p.outline),
        );
    });
}

fn legend_item(ui: &mut egui::Ui, p: &Palette, dot_color: Color32, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(r.center(), 3.6, dot_color);
        ui.label(RichText::new(text).size(12.0).color(p.on_surface_variant));
    });
}

fn paint_live_traffic_chart(
    ui: &mut egui::Ui,
    p: &Palette,
    rx: &[f64],
    tx: &[f64],
    connected: bool,
) {
    let desired = Vec2::new(ui.available_width(), 220.0);
    let (outer_rect, _) = ui.allocate_exact_size(desired, Sense::hover());
    let painter = ui.painter_at(outer_rect);

    let y_axis_w = 72.0_f32;
    let x_axis_h = 22.0_f32;
    let top_pad = 10.0_f32;
    let right_pad = 8.0_f32;

    let plot = Rect::from_min_max(
        Pos2::new(outer_rect.left() + y_axis_w, outer_rect.top() + top_pad),
        Pos2::new(
            (outer_rect.right() - right_pad).max(outer_rect.left() + y_axis_w + 40.0),
            outer_rect.bottom() - x_axis_h,
        ),
    );

    let raw_max = rx.iter().chain(tx.iter()).copied().fold(0.0_f64, f64::max);
    let ceil_val = nice_speed_ceiling(raw_max);

    for i in 0..=3 {
        let frac = i as f32 / 3.0;
        let y = plot.bottom() - frac * plot.height();
        let grid_stroke = Stroke::new(1.0_f32, p.chart_grid);
        painter.line_segment(
            [Pos2::new(plot.left(), y), Pos2::new(plot.right(), y)],
            grid_stroke,
        );

        let label = format_speed(ceil_val * frac as f64);
        painter.text(
            Pos2::new(plot.left() - 8.0, y),
            Align2::RIGHT_CENTER,
            label,
            FontId::monospace(10.5),
            p.outline,
        );
    }

    let x_y = plot.bottom() + 12.0;
    for (x, label, align) in [
        (plot.left(), "-60s", Align2::LEFT_CENTER),
        (plot.center().x, "-30s", Align2::CENTER_CENTER),
        (plot.right(), "Now", Align2::RIGHT_CENTER),
    ] {
        painter.text(
            Pos2::new(x, x_y),
            align,
            label,
            FontId::monospace(10.5),
            p.outline,
        );
    }

    if !connected && rx.is_empty() && tx.is_empty() {
        painter.text(
            plot.center(),
            Align2::CENTER_CENTER,
            "Connect a VPN profile to stream live throughput",
            FontId::proportional(13.0),
            p.outline,
        );
        return;
    }

    let to_points = |series: &[f64]| -> Vec<Pos2> {
        if series.len() < 2 {
            return Vec::new();
        }
        let step = plot.width() / (series.len() - 1) as f32;
        series
            .iter()
            .enumerate()
            .map(|(i, &val)| {
                let x = plot.left() + i as f32 * step;
                let frac = (val / ceil_val).clamp(0.0, 1.0) as f32;
                let y = plot.bottom() - frac * plot.height();
                Pos2::new(x, y)
            })
            .collect()
    };

    let rx_pts = to_points(rx);
    let tx_pts = to_points(tx);

    if rx_pts.len() >= 2 {
        let mut mesh = Mesh::default();
        for pair in rx_pts.windows(2) {
            let p0 = pair[0];
            let p1 = pair[1];
            let b0 = Pos2::new(p0.x, plot.bottom());
            let b1 = Pos2::new(p1.x, plot.bottom());
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(p0, p.chart_area_fill);
            mesh.colored_vertex(p1, p.chart_area_fill);
            mesh.colored_vertex(b1, Color32::TRANSPARENT);
            mesh.colored_vertex(b0, Color32::TRANSPARENT);
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
        }
        painter.add(Shape::mesh(mesh));
    }

    if rx_pts.len() >= 2 {
        if let Some(&last) = rx_pts.last() {
            painter.add(Shape::line(rx_pts, Stroke::new(2.0_f32, p.primary)));
            painter.circle_filled(last, 3.5, p.primary);
        }
    }
    if tx_pts.len() >= 2 {
        if let Some(&last) = tx_pts.last() {
            painter.add(Shape::line(tx_pts, Stroke::new(2.0_f32, p.secondary)));
            painter.circle_filled(last, 3.5, p.secondary);
        }
    }
}

fn nice_speed_ceiling(raw_max: f64) -> f64 {
    let min_floor = 16.0 * 1024.0;
    let v = raw_max.max(min_floor) * 1.2;
    let mag = 10_f64.powf(v.log10().floor());
    let norm = v / mag;
    let step = if norm <= 1.5 {
        1.5
    } else if norm <= 3.0 {
        3.0
    } else if norm <= 6.0 {
        6.0
    } else {
        10.0
    };
    step * mag
}

fn render_tunnel_details_card(
    app: &KrakenApp,
    ui: &mut egui::Ui,
    p: &Palette,
    state: &TunnelState,
) {
    let (profile_name, protocol, gateway, uptime) = match state {
        TunnelState::Connected {
            profile_name,
            protocol,
            endpoint,
            ..
        } => (
            profile_name.clone(),
            protocol.label().to_string(),
            endpoint.clone(),
            format_duration(state.connected_duration_secs(Utc::now())),
        ),
        TunnelState::Connecting {
            profile_name,
            protocol,
            endpoint,
            ..
        } => (
            profile_name.clone(),
            protocol.label().to_string(),
            endpoint.clone(),
            "Connecting...".to_string(),
        ),
        TunnelState::Disconnecting { profile_name, .. } => (
            profile_name.clone(),
            "—".to_string(),
            "—".to_string(),
            "Disconnecting...".to_string(),
        ),
        TunnelState::Error { profile_name, .. } => (
            profile_name.clone(),
            "—".to_string(),
            "—".to_string(),
            "Error".to_string(),
        ),
        TunnelState::Disconnected => (
            "None".to_string(),
            "—".to_string(),
            "—".to_string(),
            "—".to_string(),
        ),
    };

    let iface = app
        .telemetry
        .active_interface
        .clone()
        .unwrap_or_else(|| "—".to_string());

    table_card_frame(p).show(ui, |ui| {
        ui.set_width(ui.available_width());

        card_header_frame(p, Margin::symmetric(20.0, 11.0)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(semibold("Tunnel details", 14.0, p.on_surface));
        });
        divider(ui, p);

        detail_pair_row(
            ui,
            p,
            ("Active profile", &profile_name, false),
            ("Protocol", &protocol, false),
        );
        divider(ui, p);
        detail_pair_row(
            ui,
            p,
            ("Gateway", &gateway, true),
            ("Network interface", &iface, true),
        );
        divider(ui, p);
        detail_pair_row(
            ui,
            p,
            ("Session duration", &uptime, true),
            ("Switch policy", app.settings.switch_policy.label(), false),
        );
    });
}

fn detail_pair_row(
    ui: &mut egui::Ui,
    p: &Palette,
    left: (&str, &str, bool),
    right: (&str, &str, bool),
) {
    egui::Frame::none()
        .inner_margin(Margin::symmetric(20.0, 12.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.columns(2, |cols| {
                for (idx, (k, v, mono)) in [left, right].into_iter().enumerate() {
                    cols[idx].vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        ui.label(RichText::new(k).size(11.5).color(p.outline));
                        let rt = if mono {
                            RichText::new(v).monospace().size(13.0).color(p.on_surface)
                        } else {
                            RichText::new(v).size(13.0).color(p.on_surface)
                        };
                        ui.add(egui::Label::new(rt).wrap());
                    });
                }
            });
        });
}
