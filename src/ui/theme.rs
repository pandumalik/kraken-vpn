use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, Margin, Pos2, Rect, Response, RichText,
    Rounding, Sense, Stroke, Ui, Vec2,
};

use crate::models::{AppSettings, ColorPreset, CustomThemeColors, ThemeMode};
use crate::vpn::state::TunnelState;

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub surface: Color32,
    pub surface_lowest: Color32,
    pub surface_low: Color32,
    pub surface_container: Color32,
    pub surface_high: Color32,
    pub surface_highest: Color32,
    pub on_surface: Color32,
    pub on_surface_variant: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
    pub divider: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub on_primary: Color32,
    pub secondary: Color32,
    pub success: Color32,
    pub success_container: Color32,
    pub on_success_container: Color32,
    pub warning: Color32,
    pub warning_container: Color32,
    pub on_warning_container: Color32,
    pub error: Color32,
    pub error_container: Color32,
    pub on_error_container: Color32,
    pub chart_area_fill: Color32,
    pub chart_grid: Color32,
    pub backdrop: Color32,
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn blend_rgb(a: [u8; 3], b: [u8; 3], t: f32) -> Color32 {
    let mix = |x: u8, y: u8| -> u8 {
        ((x as f32) * (1.0 - t) + (y as f32) * t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(a[0], b[0]), mix(a[1], b[1]), mix(a[2], b[2]))
}

fn is_light_color(c: [u8; 3]) -> bool {
    let lum = 0.2126 * (c[0] as f32) + 0.7152 * (c[1] as f32) + 0.0722 * (c[2] as f32);
    lum > 145.0
}

pub fn preset_colors(mode: ThemeMode, preset: ColorPreset) -> CustomThemeColors {
    match (mode, preset) {
        (ThemeMode::Light, ColorPreset::ForestSage) => CustomThemeColors {
            primary: [0, 69, 42],
            secondary: [160, 64, 19],
            surface: [248, 250, 245],
            card: [255, 255, 255],
            border: [191, 201, 192],
            text: [25, 28, 26],
            text_muted: [64, 73, 66],
            success: [21, 110, 66],
            warning: [180, 83, 9],
            error: [186, 26, 26],
        },
        (ThemeMode::Dark, ColorPreset::ForestSage) => CustomThemeColors {
            primary: [147, 213, 174],
            secondary: [255, 181, 153],
            surface: [17, 20, 18],
            card: [22, 26, 23],
            border: [56, 64, 58],
            text: [225, 227, 222],
            text_muted: [191, 201, 192],
            success: [116, 214, 158],
            warning: [251, 191, 36],
            error: [255, 180, 171],
        },
        (ThemeMode::Light, ColorPreset::OceanSlate) => CustomThemeColors {
            primary: [37, 99, 235],
            secondary: [14, 165, 233],
            surface: [248, 250, 252],
            card: [255, 255, 255],
            border: [203, 213, 225],
            text: [15, 23, 42],
            text_muted: [71, 85, 105],
            success: [22, 163, 74],
            warning: [217, 119, 6],
            error: [220, 38, 38],
        },
        (ThemeMode::Dark, ColorPreset::OceanSlate) => CustomThemeColors {
            primary: [96, 165, 250],
            secondary: [56, 189, 248],
            surface: [15, 23, 42],
            card: [22, 31, 52],
            border: [51, 65, 85],
            text: [241, 245, 249],
            text_muted: [148, 163, 184],
            success: [74, 222, 128],
            warning: [251, 191, 36],
            error: [248, 113, 113],
        },
        (ThemeMode::Light, ColorPreset::NordFrost) => CustomThemeColors {
            primary: [94, 129, 172],
            secondary: [136, 192, 208],
            surface: [236, 239, 244],
            card: [248, 249, 251],
            border: [216, 222, 233],
            text: [46, 52, 64],
            text_muted: [76, 86, 106],
            success: [106, 153, 85],
            warning: [208, 135, 112],
            error: [191, 97, 106],
        },
        (ThemeMode::Dark, ColorPreset::NordFrost) => CustomThemeColors {
            primary: [136, 192, 208],
            secondary: [129, 161, 193],
            surface: [46, 52, 64],
            card: [59, 66, 82],
            border: [76, 86, 106],
            text: [236, 239, 244],
            text_muted: [216, 222, 233],
            success: [163, 190, 140],
            warning: [235, 203, 139],
            error: [191, 97, 106],
        },
        (ThemeMode::Light, ColorPreset::MidnightViolet) => CustomThemeColors {
            primary: [99, 102, 241],
            secondary: [168, 85, 247],
            surface: [249, 248, 253],
            card: [255, 255, 255],
            border: [216, 212, 234],
            text: [24, 22, 38],
            text_muted: [88, 84, 112],
            success: [16, 156, 104],
            warning: [217, 119, 6],
            error: [225, 29, 72],
        },
        (ThemeMode::Dark, ColorPreset::MidnightViolet) => CustomThemeColors {
            primary: [165, 180, 252],
            secondary: [192, 132, 252],
            surface: [17, 16, 25],
            card: [26, 24, 38],
            border: [58, 54, 82],
            text: [238, 237, 247],
            text_muted: [168, 164, 194],
            success: [52, 211, 153],
            warning: [251, 191, 36],
            error: [251, 113, 133],
        },
        (ThemeMode::Light, ColorPreset::AmberCarbon) => CustomThemeColors {
            primary: [180, 83, 9],
            secondary: [234, 88, 12],
            surface: [250, 249, 246],
            card: [255, 255, 255],
            border: [218, 212, 202],
            text: [28, 25, 23],
            text_muted: [87, 83, 78],
            success: [21, 128, 61],
            warning: [202, 138, 4],
            error: [185, 28, 28],
        },
        (ThemeMode::Dark, ColorPreset::AmberCarbon) => CustomThemeColors {
            primary: [251, 191, 36],
            secondary: [251, 146, 60],
            surface: [20, 19, 18],
            card: [28, 26, 24],
            border: [68, 64, 60],
            text: [245, 245, 244],
            text_muted: [168, 162, 158],
            success: [74, 222, 128],
            warning: [250, 204, 21],
            error: [248, 113, 113],
        },
        (ThemeMode::Light, ColorPreset::Monochrome) => CustomThemeColors {
            primary: [24, 24, 27],
            secondary: [82, 82, 91],
            surface: [250, 250, 250],
            card: [255, 255, 255],
            border: [212, 212, 216],
            text: [24, 24, 27],
            text_muted: [82, 82, 91],
            success: [22, 163, 74],
            warning: [202, 138, 4],
            error: [220, 38, 38],
        },
        (ThemeMode::Dark, ColorPreset::Monochrome) => CustomThemeColors {
            primary: [244, 244, 245],
            secondary: [161, 161, 170],
            surface: [16, 16, 18],
            card: [24, 24, 27],
            border: [56, 56, 62],
            text: [244, 244, 245],
            text_muted: [161, 161, 170],
            success: [74, 222, 128],
            warning: [250, 204, 21],
            error: [248, 113, 113],
        },
    }
}

pub fn active_theme_colors(settings: &AppSettings) -> CustomThemeColors {
    let custom = match settings.theme {
        ThemeMode::Light => settings.custom_light_colors,
        ThemeMode::Dark => settings.custom_dark_colors,
    };
    custom.unwrap_or_else(|| preset_colors(settings.theme, settings.color_preset))
}

impl Palette {
    pub fn from_settings(settings: &AppSettings) -> Self {
        let custom = match settings.theme {
            ThemeMode::Light => settings.custom_light_colors,
            ThemeMode::Dark => settings.custom_dark_colors,
        };
        match custom {
            Some(c) => Self::from_custom(settings.theme, &c),
            None => Self::from_preset(settings.theme, settings.color_preset),
        }
    }

    pub fn from_preset(mode: ThemeMode, preset: ColorPreset) -> Self {
        Self::from_custom(mode, &preset_colors(mode, preset))
    }

    pub fn from_custom(mode: ThemeMode, c: &CustomThemeColors) -> Self {
        let is_dark = matches!(mode, ThemeMode::Dark);
        let dark_anchor: [u8; 3] = [0, 0, 0];
        let light_anchor: [u8; 3] = [255, 255, 255];

        let surface_low = if is_dark {
            blend_rgb(c.card, light_anchor, 0.03)
        } else {
            blend_rgb(c.surface, dark_anchor, 0.03)
        };
        let surface_container = if is_dark {
            blend_rgb(c.card, light_anchor, 0.06)
        } else {
            blend_rgb(c.surface, dark_anchor, 0.055)
        };
        let surface_high = if is_dark {
            blend_rgb(c.card, light_anchor, 0.11)
        } else {
            blend_rgb(c.surface, dark_anchor, 0.085)
        };
        let surface_highest = if is_dark {
            blend_rgb(c.card, light_anchor, 0.16)
        } else {
            blend_rgb(c.surface, dark_anchor, 0.115)
        };

        let outline = blend_rgb(c.text_muted, c.border, 0.45);
        let divider = blend_rgb(c.border, c.card, 0.35);

        let primary_hover = if is_light_color(c.primary) {
            blend_rgb(c.primary, dark_anchor, 0.10)
        } else {
            blend_rgb(c.primary, light_anchor, 0.12)
        };
        let on_primary = if is_light_color(c.primary) {
            Color32::from_rgb(16, 20, 18)
        } else {
            Color32::WHITE
        };

        let make_container = |accent: [u8; 3]| -> (Color32, Color32) {
            if is_dark {
                let bg = blend_rgb(c.card, accent, 0.22);
                let fg = blend_rgb(accent, light_anchor, 0.38);
                (bg, fg)
            } else {
                let bg = blend_rgb(c.card, accent, 0.14);
                let fg = blend_rgb(accent, dark_anchor, 0.28);
                (bg, fg)
            }
        };

        let (success_container, on_success_container) = make_container(c.success);
        let (warning_container, on_warning_container) = make_container(c.warning);
        let (error_container, on_error_container) = make_container(c.error);

        let chart_area_fill = Color32::from_rgba_unmultiplied(
            c.primary[0],
            c.primary[1],
            c.primary[2],
            if is_dark { 36 } else { 28 },
        );
        let chart_grid = Color32::from_rgba_unmultiplied(
            c.border[0],
            c.border[1],
            c.border[2],
            if is_dark { 120 } else { 115 },
        );
        let backdrop = if is_dark {
            Color32::from_rgba_unmultiplied(5, 8, 6, 165)
        } else {
            Color32::from_rgba_unmultiplied(15, 23, 18, 110)
        };

        Self {
            surface: rgb(c.surface),
            surface_lowest: rgb(c.card),
            surface_low,
            surface_container,
            surface_high,
            surface_highest,
            on_surface: rgb(c.text),
            on_surface_variant: rgb(c.text_muted),
            outline,
            outline_variant: rgb(c.border),
            divider,
            primary: rgb(c.primary),
            primary_hover,
            on_primary,
            secondary: rgb(c.secondary),
            success: rgb(c.success),
            success_container,
            on_success_container,
            warning: rgb(c.warning),
            warning_container,
            on_warning_container,
            error: rgb(c.error),
            error_container,
            on_error_container,
            chart_area_fill,
            chart_grid,
            backdrop,
        }
    }

    pub fn light() -> Self {
        Self::from_preset(ThemeMode::Light, ColorPreset::ForestSage)
    }

    pub fn dark() -> Self {
        Self::from_preset(ThemeMode::Dark, ColorPreset::ForestSage)
    }
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    #[cfg(windows)]
    {
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\segoeui.ttf") {
            fonts
                .font_data
                .insert("segoe_ui".to_owned(), FontData::from_owned(bytes));
            fonts
                .families
                .entry(FontFamily::Proportional)
                .or_default()
                .insert(0, "segoe_ui".to_owned());
        }
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\seguisb.ttf") {
            fonts
                .font_data
                .insert("segoe_ui_semibold".to_owned(), FontData::from_owned(bytes));
            fonts.families.insert(
                FontFamily::Name("semibold".into()),
                vec!["segoe_ui_semibold".to_owned(), "segoe_ui".to_owned()],
            );
        }
        if let Ok(bytes) = std::fs::read(r"C:\Windows\Fonts\consola.ttf") {
            fonts
                .font_data
                .insert("consolas".to_owned(), FontData::from_owned(bytes));
            fonts
                .families
                .entry(FontFamily::Monospace)
                .or_default()
                .insert(0, "consolas".to_owned());
        }
    }

    let fallback_prop = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    fonts
        .families
        .entry(FontFamily::Name("semibold".into()))
        .or_insert(fallback_prop);

    ctx.set_fonts(fonts);
}

pub fn semibold(text: impl Into<String>, size: f32, color: Color32) -> RichText {
    RichText::new(text)
        .family(FontFamily::Name("semibold".into()))
        .size(size)
        .color(color)
}

pub fn apply_stitch_theme(ctx: &egui::Context, settings: &AppSettings) {
    let mode = settings.theme;
    let p = Palette::from_settings(settings);
    let mut style = (*ctx.style()).clone();

    let mut visuals = match mode {
        ThemeMode::Light => egui::Visuals::light(),
        ThemeMode::Dark => egui::Visuals::dark(),
    };

    visuals.override_text_color = Some(p.on_surface);
    visuals.panel_fill = p.surface;
    visuals.window_fill = p.surface_lowest;
    visuals.extreme_bg_color = match mode {
        ThemeMode::Light => p.surface_lowest,
        ThemeMode::Dark => {
            let s = p.surface;
            Color32::from_rgb(
                s.r().saturating_sub(4),
                s.g().saturating_sub(4),
                s.b().saturating_sub(4),
            )
        }
    };
    visuals.faint_bg_color = p.surface_low;
    visuals.window_rounding = Rounding::same(12.0);
    visuals.window_stroke = Stroke::new(1.0_f32, p.outline_variant);

    visuals.widgets.noninteractive.bg_fill = p.surface_low;
    visuals.widgets.noninteractive.weak_bg_fill = p.surface_low;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.outline_variant);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    visuals.widgets.noninteractive.rounding = Rounding::same(7.0);

    visuals.widgets.inactive.bg_fill = p.surface_lowest;
    visuals.widgets.inactive.weak_bg_fill = p.surface_lowest;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.outline_variant);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    visuals.widgets.inactive.rounding = Rounding::same(7.0);

    visuals.widgets.hovered.bg_fill = p.surface_container;
    visuals.widgets.hovered.weak_bg_fill = p.surface_container;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.outline);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    visuals.widgets.hovered.rounding = Rounding::same(7.0);

    visuals.widgets.active.bg_fill = p.surface_high;
    visuals.widgets.active.weak_bg_fill = p.surface_high;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.primary);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    visuals.widgets.active.rounding = Rounding::same(7.0);

    visuals.widgets.open.bg_fill = p.surface_lowest;
    visuals.widgets.open.weak_bg_fill = p.surface_lowest;
    visuals.widgets.open.bg_stroke = Stroke::new(1.5_f32, p.primary);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, p.on_surface);
    visuals.widgets.open.rounding = Rounding::same(7.0);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(
        p.primary.r(),
        p.primary.g(),
        p.primary.b(),
        match mode {
            ThemeMode::Light => 42,
            ThemeMode::Dark => 56,
        },
    );
    visuals.selection.stroke = Stroke::new(1.5_f32, p.primary);

    style.visuals = visuals;
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size = Vec2::new(40.0, 28.0);
    style.spacing.text_edit_width = 240.0;
    style.interaction.selectable_labels = true;

    ctx.set_style(style);
}

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

pub const CONTROL_HEIGHT: f32 = 30.0;

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

pub fn short_binary_version(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "Found".to_string();
    }
    let before_bracket = trimmed.split(['[', '(']).next().unwrap_or(trimmed).trim();
    let base = if before_bracket.is_empty() {
        trimmed
    } else {
        before_bracket
    };
    let max_chars = 28;
    if base.chars().count() <= max_chars {
        base.to_string()
    } else {
        let prefix: String = base.chars().take(max_chars - 1).collect();
        format!("{prefix}…")
    }
}

pub fn responsive_modal_size(screen_rect: Rect, preferred_width: f32, max_height_cap: f32) -> Vec2 {
    let margin = 48.0_f32;
    let avail_w = (screen_rect.width() - margin).max(280.0);
    let avail_h = (screen_rect.height() - margin).max(240.0);

    let width = avail_w
        .min(preferred_width)
        .min((screen_rect.width() - 16.0).max(240.0));
    let height = avail_h
        .min(max_height_cap)
        .min((screen_rect.height() - 16.0).max(220.0));
    Vec2::new(width, height)
}

pub fn tunnel_status_style(
    p: &Palette,
    state: &TunnelState,
) -> (Color32, Color32, Color32, &'static str) {
    match state {
        TunnelState::Connected { .. } => (
            p.success_container,
            p.on_success_container,
            p.success,
            "Connected",
        ),
        TunnelState::Connecting { .. } => (
            p.warning_container,
            p.on_warning_container,
            p.warning,
            "Connecting...",
        ),
        TunnelState::Disconnecting { .. } => (
            p.warning_container,
            p.on_warning_container,
            p.warning,
            "Disconnecting...",
        ),
        TunnelState::Error { .. } => (p.error_container, p.on_error_container, p.error, "Error"),
        TunnelState::Disconnected => (
            p.surface_container,
            p.on_surface_variant,
            p.outline,
            "Disconnected",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responsive_modal_fits_inside_all_window_sizes() {
        for (w, h) in [
            (1920.0, 1080.0),
            (1100.0, 740.0),
            (900.0, 600.0),
            (780.0, 540.0),
            (640.0, 480.0),
        ] {
            let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h));
            let size = responsive_modal_size(rect, 540.0, 620.0);
            assert!(
                size.x <= w - 16.0 && size.x >= 280.0,
                "width {} out of bounds for viewport {}x{}",
                size.x,
                w,
                h
            );
            assert!(
                size.y <= h - 16.0 && size.y >= 240.0,
                "height {} out of bounds for viewport {}x{}",
                size.y,
                w,
                h
            );
        }
    }

    #[test]
    fn light_and_dark_palettes_have_distinct_surfaces() {
        let light = Palette::light();
        let dark = Palette::dark();
        assert_ne!(light.surface, dark.surface);
        assert_ne!(light.primary, dark.primary);
        assert_ne!(light.on_surface, dark.on_surface);
    }

    #[test]
    fn condenses_long_cli_version_banners_for_status_badges() {
        assert_eq!(
            short_binary_version(
                "OpenVPN 2.7.6 [git:v2.7.6/327a33de2aa21883] Windows [SSL (OpenSSL)] [LZO] built on Aug  5 2026"
            ),
            "OpenVPN 2.7.6"
        );
        assert_eq!(
            short_binary_version("OpenConnect version v9.12"),
            "OpenConnect version v9.12"
        );
        assert_eq!(short_binary_version("   "), "Found");
    }
}
