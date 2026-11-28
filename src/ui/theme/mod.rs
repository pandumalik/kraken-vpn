pub mod presets;
pub mod widgets;

pub use presets::{active_theme_colors, preset_colors};
pub use widgets::{
    card_frame, card_header_frame, destructive_button, divider, modal_card_frame, page_container,
    page_header, pill_badge, primary_button, row_danger_button, row_primary_button,
    row_secondary_button, secondary_button, styled_text_edit, table_card_frame, table_header_cell,
    toggle_switch, CONTROL_HEIGHT,
};

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, Rect, RichText, Rounding, Stroke, Vec2,
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

impl Palette {
    pub fn from_settings(settings: &AppSettings) -> Self {
        match settings.active_custom_colors() {
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
    use eframe::egui::Pos2;

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
