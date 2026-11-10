//! WhatsApp's design tokens, and the egui style built from them.
//!
//! The values are the official client's `--WDS-*` tokens (see AGENTS.md): the
//! dark set is the desktop app's surface/elevated/rail palette, the light set is
//! WhatsApp Web's paper-and-green palette.

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, TextStyle, Visuals};

/// One resolved theme.
#[derive(Debug, Clone)]
pub struct Palette {
    pub dark: bool,
    /// Window and panel fill.
    pub panel: Color32,
    /// The rail's fill (slightly off the panel).
    pub rail: Color32,
    /// The conversation wallpaper.
    pub wallpaper: Color32,
    /// Menus, popovers and inputs.
    pub elevated: Color32,
    /// The message field: `#2A3942` dark, white on the light bar.
    pub raised: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub accent: Color32,
    pub bubble_in: Color32,
    pub bubble_out: Color32,
    pub link: Color32,
    pub read: Color32,
    pub danger: Color32,
    pub border: Color32,
    pub unread: Color32,
}

fn hex(value: u32) -> Color32 {
    Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

/// `theme` is `light`, `dark` or `system`; `system_dark` is the OS preference.
pub fn palette(theme: &str, system_dark: bool) -> Palette {
    let dark = match theme {
        "dark" => true,
        "light" => false,
        _ => system_dark,
    };
    if dark {
        // The official dark theme (WhatsApp Web / Desktop): a #0B141A
        // wallpaper, #111B21 panels, #202C33 headers, inputs and incoming
        // bubbles, and #005C4B for our own bubbles.
        Palette {
            dark,
            panel: hex(0x111B21),
            rail: hex(0x111B21),
            wallpaper: hex(0x0B141A),
            elevated: hex(0x202C33),
            raised: hex(0x2A3942),
            hover: Color32::from_rgba_unmultiplied(255, 255, 255, 16),
            selected: hex(0x2A3942),
            text: hex(0xE9EDEF),
            secondary: hex(0x8696A0),
            accent: hex(0x00A884),
            bubble_in: hex(0x202C33),
            bubble_out: hex(0x005C4B),
            link: hex(0x53BDEB),
            read: hex(0x53BDEB),
            danger: hex(0xF15C6D),
            border: hex(0x2A3942),
            unread: hex(0x00A884),
        }
    } else {
        Palette {
            dark,
            panel: hex(0xFFFFFF),
            rail: hex(0xF7F5F3),
            wallpaper: hex(0xF5F1EB),
            elevated: hex(0xFFFFFF),
            raised: hex(0xFFFFFF),
            hover: hex(0xF5F6F6),
            selected: hex(0xF0F2F5),
            text: hex(0x111B21),
            secondary: hex(0x667781),
            accent: hex(0x1DAA61),
            bubble_in: hex(0xFFFFFF),
            bubble_out: hex(0xD9FDD3),
            link: hex(0x1B8755),
            read: hex(0x007BFC),
            danger: hex(0xFB5061),
            border: hex(0xE9EDEF),
            unread: hex(0x25D366),
        }
    }
}

/// Install the palette and the typography scale on the context.
pub fn apply(ctx: &egui::Context, p: &Palette) {
    let mut visuals = if p.dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    visuals.panel_fill = p.panel;
    visuals.window_fill = p.elevated;
    visuals.extreme_bg_color = p.panel;
    visuals.faint_bg_color = p.hover;
    visuals.override_text_color = Some(p.text);
    visuals.hyperlink_color = p.link;
    visuals.selection.bg_fill = p.selected;
    visuals.selection.stroke = Stroke::new(1.0_f32, p.text);
    visuals.window_corner_radius = CornerRadius::same(12);
    visuals.menu_corner_radius = CornerRadius::same(10);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.secondary);
    visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    visuals.widgets.hovered.weak_bg_fill = p.hover;
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, p.text);
    visuals.widgets.active.weak_bg_fill = p.selected;
    visuals.widgets.active.fg_stroke = Stroke::new(1.0_f32, p.text);
    visuals.widgets.open.weak_bg_fill = p.hover;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.global_style()).clone();
    style.visuals.window_shadow = egui::epaint::Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(120),
    };
    style.visuals.popup_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(110),
    };
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 6.0);
    style.spacing.menu_margin = egui::Margin::same(4);
    style.text_styles = [
        (
            TextStyle::Heading,
            FontId::new(19.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(14.5, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(14.5, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
    ]
    .into();
    ctx.set_global_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_theme_choices_resolve() {
        assert!(palette("dark", false).dark);
        assert!(!palette("light", true).dark);
        assert!(palette("system", true).dark);
        assert!(!palette("system", false).dark);
    }

    #[test]
    fn the_official_tokens_are_used() {
        // The dark set is the official desktop theme the reference screenshot
        // shows: #0B141A wallpaper, #111B21 panels, #202C33 bubbles,
        // #005C4B outgoing.
        let dark = palette("dark", true);
        assert_eq!(dark.wallpaper, Color32::from_rgb(0x0B, 0x14, 0x1A));
        assert_eq!(dark.panel, Color32::from_rgb(0x11, 0x1B, 0x21));
        assert_eq!(dark.elevated, Color32::from_rgb(0x20, 0x2C, 0x33));
        assert_eq!(dark.bubble_in, Color32::from_rgb(0x20, 0x2C, 0x33));
        assert_eq!(dark.bubble_out, Color32::from_rgb(0x00, 0x5C, 0x4B));
        assert_eq!(dark.read, Color32::from_rgb(0x53, 0xBD, 0xEB));
        let light = palette("light", false);
        assert_eq!(light.bubble_out, Color32::from_rgb(0xD9, 0xFD, 0xD3));
        assert_eq!(light.wallpaper, Color32::from_rgb(0xF5, 0xF1, 0xEB));
        assert_eq!(light.accent, Color32::from_rgb(0x1D, 0xAA, 0x61));
    }
}
