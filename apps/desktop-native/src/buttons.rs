//! The window's own buttons: the filter pills, the round `+`, the rail entries,
//! the green compose FAB and the empty state's round actions.
//!
//! Split out of `app.rs`; `app.rs` re-exports these names so the call sites keep
//! importing them from there.

use crate::app::{Icon, draw_icon};

/// An outlined filter pill (`All`, `Groups 3`, …), filled when selected.
/// The round `+` that ends the filter row (the live "New list" control).
pub(crate) fn pill_plus_button(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
    let fill = if response.hovered() {
        palette.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(16),
        fill,
        egui::Stroke::new(1.0_f32, palette.border),
        egui::StrokeKind::Inside,
    );
    draw_icon(ui, rect.shrink(9.0), Icon::Add, palette.secondary);
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

pub(crate) fn pill_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    selected: bool,
) -> bool {
    let font = egui::FontId::proportional(12.5);
    let text = ui
        .painter()
        .layout_no_wrap(label.to_string(), font.clone(), palette.text);
    let width = text.size().x + 26.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 32.0), egui::Sense::click());
    // The live selected pill is a dark-green fill with light-green text; the
    // others are transparent with a hairline border.
    let fill = if selected {
        palette.accent.gamma_multiply(0.22)
    } else if response.hovered() {
        palette.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect(
        rect,
        egui::CornerRadius::same(16),
        fill,
        egui::Stroke::new(1.0_f32, palette.border),
        egui::StrokeKind::Inside,
    );
    let color = if selected {
        palette.accent
    } else {
        palette.secondary
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        color,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// One rail button, with an optional badge for unread counts.
pub(crate) fn rail_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    selected: bool,
    badge: i64,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), egui::Sense::click());
    if selected || response.hovered() {
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(12),
            if selected {
                palette.selected
            } else {
                palette.hover
            },
        );
    }
    let color = if selected {
        palette.accent
    } else {
        palette.secondary
    };
    draw_icon(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(24.0, 24.0)),
        icon,
        color,
    );
    if badge > 0 {
        let label = if badge > 99 {
            "99+".to_string()
        } else {
            badge.to_string()
        };
        let width = 14.0 + label.len() as f32 * 3.5;
        let pill = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 4.0, rect.top() + 10.0),
            egui::vec2(width, 16.0),
        );
        ui.painter().rect_filled(pill, 8.0, palette.unread);
        ui.painter().text(
            pill.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(10.0),
            egui::Color32::WHITE,
        );
    }
    response
}

/// The green "new chat" floating button in the Chats header.
pub(crate) fn fab_button(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
    let fill = if response.hovered() {
        palette.accent.linear_multiply(1.15)
    } else {
        palette.accent
    };
    ui.painter().circle_filled(rect.center(), 20.0, fill);
    let ink = if palette.dark {
        egui::Color32::from_rgb(11, 20, 26)
    } else {
        egui::Color32::WHITE
    };
    draw_icon(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(20.0, 20.0)),
        Icon::Add,
        ink,
    );
    response
}

/// One of the round empty-state actions (circle + caption).
pub(crate) fn action_button(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    label: &str,
) -> bool {
    ui.vertical(|ui| {
        let (rect, response) = ui.allocate_exact_size(egui::vec2(72.0, 64.0), egui::Sense::click());
        let fill = if response.hovered() {
            palette.hover
        } else {
            palette.elevated
        };
        ui.painter().circle_filled(rect.center(), 28.0, fill);
        draw_icon(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(rect.center().x, rect.center().y - 2.0),
                egui::vec2(22.0, 22.0),
            ),
            icon,
            palette.secondary,
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(label)
                .size(11.5)
                .color(palette.secondary),
        );
        response.clicked()
    })
    .inner
}
