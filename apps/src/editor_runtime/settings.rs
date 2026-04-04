use slint::{Color, SharedString};

use crate::{config, AppWindow};

pub fn configure_app_settings(ui: &AppWindow) {
    let settings = config::AppConfig::load_or_initialize();

    ui.set_app_language(SharedString::from(settings.language.current));
    ui.set_app_theme_mode(SharedString::from(settings.theme.current));
    ui.set_pressed_keys_enabled(settings.pressed_keys.enabled);
    ui.set_pressed_keys_show_mouse_buttons(settings.pressed_keys.show_mouse_buttons);

    let custom = settings.theme.custom;
    ui.set_theme_custom_primary_bg(parse_hex_color(&custom.primary_bg, "#20242B"));
    ui.set_theme_custom_secondary_bg(parse_hex_color(&custom.secondary_bg, "#2B313A"));
    ui.set_theme_custom_primary_text(parse_hex_color(&custom.primary_text, "#F5F7FA"));
    ui.set_theme_custom_secondary_text(parse_hex_color(&custom.secondary_text, "#98A4B3"));
    ui.set_theme_custom_disabled_text(parse_hex_color(&custom.disabled_text, "#677382"));
    ui.set_theme_custom_border_color(parse_hex_color(&custom.border_color, "#3C4552"));
    ui.set_theme_custom_divider_color(parse_hex_color(&custom.divider_color, "#444D5A"));
    ui.set_theme_custom_hover_color(parse_hex_color(&custom.hover_color, "#323B47"));
    ui.set_theme_custom_active_color(parse_hex_color(&custom.active_color, "#FFD24D"));
    ui.set_theme_custom_button_color(parse_hex_color(&custom.button_color, "#445066"));
    ui.set_theme_custom_tooltip_bg(parse_hex_color(&custom.tooltip_bg, "#1F2630"));
    ui.set_theme_custom_overlay_bg(parse_hex_color(&custom.overlay_bg, "#00000066"));
    ui.set_theme_custom_overlay_strong_bg(parse_hex_color(&custom.overlay_strong_bg, "#00000090"));
    ui.set_theme_custom_shadow_color(parse_hex_color(&custom.shadow_color, "#00000040"));
    ui.set_theme_custom_shadow_size(clamp_metric(custom.shadow_size, 10.0, 0.0, 80.0));
    ui.set_theme_custom_border_radius(clamp_metric(custom.border_radius, 6.0, 0.0, 48.0));
    ui.set_theme_custom_error_bg(parse_hex_color(&custom.error_bg, "#FF444420"));
    ui.set_theme_custom_error_text(parse_hex_color(&custom.error_text, "#FF6666"));
    ui.set_theme_custom_success_color(parse_hex_color(&custom.success_color, "#7DD97D"));
    ui.set_theme_custom_warning_color(parse_hex_color(&custom.warning_color, "#F5DC78"));
    ui.set_theme_custom_selection_border(parse_hex_color(&custom.selection_border, "#4EA1FF"));
    ui.set_theme_custom_selection_fill(parse_hex_color(&custom.selection_fill, "#4EA1FF33"));
    ui.set_theme_custom_selection_panel_bg(parse_hex_color(
        &custom.selection_panel_bg,
        "#10203ACC",
    ));
    ui.set_theme_custom_selection_text(parse_hex_color(&custom.selection_text, "#D9E7FF"));
    ui.set_theme_custom_on_accent_text(parse_hex_color(&custom.on_accent_text, "#000000"));
    ui.set_theme_custom_canvas_bg(parse_hex_color(&custom.canvas_bg, "#0F0F0F"));
    ui.set_theme_custom_canvas_grid_color(parse_hex_color(&custom.canvas_grid_color, "#2B2B2B"));
}

fn parse_hex_color(value: &str, fallback: &str) -> Color {
    parse_hex_color_inner(value).unwrap_or_else(|| {
        parse_hex_color_inner(fallback).unwrap_or_else(|| Color::from_argb_u8(255, 0, 0, 0))
    })
}

fn parse_hex_color_inner(value: &str) -> Option<Color> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    if !hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return None;
    }

    if hex.len() == 6 {
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        return Some(Color::from_rgb_u8(r, g, b));
    }

    let a = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let r = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let g = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let b = u8::from_str_radix(&hex[6..8], 16).ok()?;
    Some(Color::from_argb_u8(a, r, g, b))
}

fn clamp_metric(value: f32, fallback: f32, min: f32, max: f32) -> f32 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
