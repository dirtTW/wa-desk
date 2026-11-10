//! Small painters shared by the screens: the dialog card and its buttons, the
//! status tiles and the colour helper. They are free functions on purpose -
//! they only see a `Ui` and a `Palette`, so they can move out of `app.rs`
//! without dragging window state along.

use crate::app::{Icon, draw_icon};

/// `#rrggbb` (or `#aarrggbb`) as an egui colour.
pub(crate) fn hex_colour(value: &str) -> egui::Color32 {
    let clean = value.trim_start_matches('#');
    let parsed = u32::from_str_radix(clean, 16).unwrap_or(0xA884);
    if clean.len() == 8 {
        egui::Color32::from_rgba_unmultiplied(
            (parsed >> 16) as u8,
            (parsed >> 8) as u8,
            parsed as u8,
            (parsed >> 24) as u8,
        )
    } else {
        egui::Color32::from_rgb((parsed >> 16) as u8, (parsed >> 8) as u8, parsed as u8)
    }
}

/// A dialog button: the green filled pill for the primary action, a quiet text
/// button for cancel.
pub(crate) fn dialog_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    primary: bool,
) -> egui::Response {
    let width = 92.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 34.0), egui::Sense::click());
    if primary {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(17),
            if response.hovered() {
                palette.accent.gamma_multiply(0.9)
            } else {
                palette.accent
            },
        );
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(17), palette.hover);
    }
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        if primary {
            egui::Color32::from_rgb(11, 20, 26)
        } else {
            palette.text
        },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A modal-ish card in the official style: rounded 12, elevated fill, hairline
/// border, drop shadow, no title bar (egui's default window chrome is the one
/// thing that never matches WhatsApp).
pub(crate) fn dialog_card(
    ctx: &egui::Context,
    id: &str,
    width: f32,
    title: &str,
    palette: &crate::theme::Palette,
    viewport: egui::Rect,
    contents: impl FnOnce(&mut egui::Ui),
) {
    let size = egui::vec2(width, viewport.height() * 0.6);
    egui::Area::new(egui::Id::unique(id))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(
            viewport.center().x - width / 2.0,
            viewport.center().y - size.y / 2.0,
        ))
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(palette.elevated)
                .stroke(egui::Stroke::new(1.0_f32, palette.border))
                .corner_radius(egui::CornerRadius::same(14))
                .shadow(egui::epaint::Shadow {
                    offset: [0, 10],
                    blur: 28,
                    spread: 0,
                    color: egui::Color32::from_black_alpha(140),
                })
                .inner_margin(egui::Margin::symmetric(18, 16))
                .show(ui, |ui| {
                    ui.set_width(width);
                    ui.label(egui::RichText::new(title).size(17.0).color(palette.text));
                    ui.add_space(10.0);
                    contents(ui);
                });
        });
}

/// One "Share statuses" tile: a rounded card with an icon and a label.
pub(crate) fn status_share_tile(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    label: &str,
) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(260.0, 46.0), egui::Sense::click());
    let fill = if response.hovered() {
        palette.hover
    } else {
        palette.raised
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(10), fill);
    draw_icon(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 24.0, rect.center().y),
            egui::vec2(20.0, 20.0),
        ),
        icon,
        palette.text,
    );
    ui.painter().text(
        egui::pos2(rect.left() + 46.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.text,
    );
    response.clicked()
}

/// The green "Send" pill under the text-status box.
pub(crate) fn status_send_button(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(96.0, 34.0), egui::Sense::click());
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(17),
        if response.hovered() {
            palette.accent.gamma_multiply(0.9)
        } else {
            palette.accent
        },
    );
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "Send",
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(11, 20, 26),
    );
    response.clicked()
}

/// Collapse a body onto one line: chat previews and titles are painted with
/// `layout_no_wrap`, which still honours explicit newlines - a multi-line
/// preview (a "Breafing QC" caption, for example) drew over the row below.
pub(crate) fn single_line(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            space = true;
            continue;
        }
        if space && !out.is_empty() {
            out.push(' ');
        }
        space = false;
        out.push(ch);
    }
    out
}

/// Trim `text` until it measures inside `max_width`, adding an ellipsis.
pub(crate) fn fit_text(
    painter: &egui::Painter,
    text: &str,
    font: egui::FontId,
    color: egui::Color32,
    max_width: f32,
) -> String {
    if max_width <= 0.0 {
        return String::new();
    }
    if painter
        .layout_no_wrap(text.to_string(), font.clone(), color)
        .size()
        .x
        <= max_width
    {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut len = chars.len();
    while len > 1 {
        len -= 1;
        let mut candidate: String = chars[..len].iter().collect();
        candidate.push('…');
        if painter
            .layout_no_wrap(candidate.clone(), font.clone(), color)
            .size()
            .x
            <= max_width
        {
            return candidate;
        }
    }
    "…".to_string()
}

pub(crate) fn initials(name: &str) -> String {
    let mut letters = name
        .split_whitespace()
        .filter_map(|word| word.chars().next());
    match (letters.next(), letters.next()) {
        (Some(first), Some(second)) => format!("{first}{second}").to_uppercase(),
        (Some(first), None) => first.to_uppercase().to_string(),
        _ => "?".to_string(),
    }
}

/// The default-avatar colour WhatsApp uses for people without a photo.
///
/// The official client ships two palettes: a muted grey/earth set for the
/// avatar discs and the brighter profile-photo set for sender names
/// (`crate::app::COLORS`).
pub(crate) fn avatar_color(name: &str) -> egui::Color32 {
    const DISC: [u32; 8] = [
        0x8E9BA6, 0xB5766B, 0x7C8FA8, 0x9A7EA8, 0xA48A5B, 0x5F8A76, 0xA8708A, 0x5C7E8A,
    ];
    let mut sum = 0usize;
    for byte in name.bytes() {
        sum = sum.wrapping_mul(31).wrapping_add(byte as usize);
    }
    hex_colour(&format!("#{:06X}", DISC[sum % DISC.len()]))
}

/// A round initials avatar, like WhatsApp's default profile photo.
pub(crate) fn paint_avatar(
    ui: &mut egui::Ui,
    name: &str,
    size: f32,
    texture: Option<&egui::TextureHandle>,
) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
    let painter = ui.painter();
    if let Some(texture) = texture {
        painter.image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
        return;
    }
    painter.circle_filled(rect.center(), size * 0.5, avatar_color(name));
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        initials(name),
        egui::FontId::proportional(size * 0.38),
        egui::Color32::WHITE,
    );
}
