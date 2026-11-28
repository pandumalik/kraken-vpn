use eframe::egui::{
    self, Color32, FontFamily, Margin, Pos2, Response, RichText, Rounding, Sense, Stroke, Ui, Vec2,
};

use super::{semibold, Palette};

pub const CONTROL_HEIGHT: f32 = 30.0;

pub fn card_frame(p: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(p.surface_lowest)
        .rounding(Rounding::same(12.0))
        .stroke(Stroke::new(1.0_f32, p.outline_variant))
        .inner_margin(Margin::same(20.0))
}

pub fn table_card_frame(p: &Palette) -> egui::Frame {
    egui::Frame::none()
        .fill(p.surface_lowest)
        .rounding(Rounding::same(12.0))
        .stroke(Stroke::new(1.0_f32, p.outline_variant))
        .inner_margin(Margin::same(0.0))
}

pub fn card_header_frame(p: &Palette, margin: Margin) -> egui::Frame {
    egui::Frame::none()
        .fill(p.surface_low)
        .rounding(Rounding {
            nw: 11.0,
            ne: 11.0,
            sw: 0.0,
            se: 0.0,
        })
        .inner_margin(margin)
}

pub fn modal_card_frame(p: &Palette, margin: Margin) -> egui::Frame {
    egui::Frame::none()
        .fill(p.surface_lowest)
        .rounding(Rounding::same(12.0))
        .stroke(Stroke::new(1.0_f32, p.outline_variant))
        .shadow(egui::epaint::Shadow {
            offset: Vec2::new(0.0, 10.0),
            blur: 28.0,
            spread: 0.0,
            color: Color32::from_black_alpha(52),
        })
        .inner_margin(margin)
}

pub fn page_header(ui: &mut Ui, p: &Palette, title: &str, subtitle: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.label(semibold(title, 23.0, p.on_surface));
        ui.label(
            RichText::new(subtitle)
                .size(13.5)
                .color(p.on_surface_variant),
        );
    });
}

pub fn table_header_cell(ui: &mut Ui, p: &Palette, text: &str, width: f32) {
    ui.allocate_ui_with_layout(
        Vec2::new(width, 18.0),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(semibold(text, 11.5, p.on_surface_variant));
        },
    );
}

pub fn divider(ui: &mut Ui, p: &Palette) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, p.divider),
    );
}

pub fn primary_button(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    let btn = egui::Button::new(semibold(text, 13.5, p.on_primary))
        .fill(p.primary)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(8.0))
        .min_size(Vec2::new(0.0, 34.0));
    ui.add(btn)
}

pub fn secondary_button(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    let btn = egui::Button::new(semibold(text, 13.5, p.on_surface))
        .fill(p.surface_lowest)
        .stroke(Stroke::new(1.0_f32, p.outline_variant))
        .rounding(Rounding::same(8.0))
        .min_size(Vec2::new(0.0, 34.0));
    ui.add(btn)
}

pub fn destructive_button(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    let btn = egui::Button::new(semibold(text, 13.5, p.on_error_container))
        .fill(p.error_container)
        .stroke(Stroke::new(1.0_f32, p.error.linear_multiply(0.35)))
        .rounding(Rounding::same(8.0))
        .min_size(Vec2::new(0.0, 34.0));
    ui.add(btn)
}

pub fn styled_text_edit(text: &mut String, width: f32) -> egui::TextEdit<'_> {
    egui::TextEdit::singleline(text)
        .desired_width(width)
        .min_size(Vec2::new(width, CONTROL_HEIGHT))
        .margin(Margin::symmetric(10.0, 5.0))
        .vertical_align(egui::Align::Center)
}

pub fn row_primary_button(ui: &mut Ui, p: &Palette, text: &str, enabled: bool) -> Response {
    let fill = if enabled {
        p.primary
    } else {
        p.surface_highest
    };
    let fg = if enabled { p.on_primary } else { p.outline };
    let btn = egui::Button::new(semibold(text, 12.5, fg))
        .fill(fill)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(78.0, CONTROL_HEIGHT));
    ui.add_enabled(enabled, btn)
}

pub fn row_secondary_button(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    let btn = egui::Button::new(semibold(text, 12.5, p.on_surface_variant))
        .fill(p.surface_lowest)
        .stroke(Stroke::new(1.0_f32, p.outline_variant))
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(0.0, CONTROL_HEIGHT));
    ui.add(btn)
}

pub fn row_danger_button(ui: &mut Ui, p: &Palette, text: &str) -> Response {
    let btn = egui::Button::new(semibold(text, 12.5, p.on_error_container))
        .fill(p.error_container)
        .stroke(Stroke::NONE)
        .rounding(Rounding::same(7.0))
        .min_size(Vec2::new(0.0, CONTROL_HEIGHT));
    ui.add(btn)
}

pub fn pill_badge(ui: &mut Ui, bg: Color32, fg: Color32, dot: Option<Color32>, text: &str) {
    let font = egui::FontId::new(11.0, FontFamily::Name("semibold".into()));
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let text_size = galley.size();

    let pad_x = 8.0_f32;
    let pad_y = 3.0_f32;
    let dot_diameter = 6.0_f32;
    let dot_gap = 5.0_f32;
    let dot_w = if dot.is_some() {
        dot_diameter + dot_gap
    } else {
        0.0
    };

    let size = Vec2::new(
        (pad_x * 2.0 + dot_w + text_size.x).ceil(),
        (text_size.y + pad_y * 2.0).max(19.0).ceil(),
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::same(5.0), bg);

        let mut x = rect.left() + pad_x;
        let cy = rect.center().y;
        if let Some(dot_color) = dot {
            painter.circle_filled(Pos2::new(x + dot_diameter * 0.5, cy), 2.8, dot_color);
            x += dot_w;
        }
        let text_pos = Pos2::new(x, cy - text_size.y * 0.5);
        painter.galley(text_pos, galley, fg);
    }
}

pub fn toggle_switch(ui: &mut Ui, p: &Palette, on: &mut bool) -> Response {
    let desired_size = Vec2::new(44.0, 24.0);
    let (rect, mut response) = ui.allocate_exact_size(desired_size, Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(response.id, *on);
        let radius = 0.5 * rect.height();
        let track_color = if *on { p.primary } else { p.surface_highest };
        let border_color = if *on { p.primary } else { p.outline_variant };
        ui.painter().rect(
            rect,
            Rounding::same(radius),
            track_color,
            Stroke::new(1.0_f32, border_color),
        );

        let knob_radius = radius - 3.0;
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = Pos2::new(circle_x, rect.center().y);
        let knob_color = if *on { p.on_primary } else { Color32::WHITE };
        ui.painter().circle_filled(center, knob_radius, knob_color);
    }
    response
}

pub fn page_container(ui: &mut Ui, max_width: f32, add_contents: impl FnOnce(&mut Ui)) {
    let avail = (ui.available_width() - 6.0).max(260.0);
    let target_w = avail.min(max_width).max(260.0);
    let side_pad = ((avail - target_w) * 0.5).max(0.0);
    ui.horizontal(|ui| {
        if side_pad > 0.5 {
            ui.add_space(side_pad);
        }
        ui.vertical(|ui| {
            ui.set_min_width(target_w);
            ui.set_max_width(target_w);
            add_contents(ui);
        });
    });
}
