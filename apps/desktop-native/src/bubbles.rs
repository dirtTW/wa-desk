//! Bubble chrome shared by every message kind: the message menu, the reaction
//! chips that hang off a bubble, and the day chip. They only see a `Ui`, the
//! palette, the model and the message, so they live outside `app.rs`.

use crate::app::{Icon, menu_row};
use crate::model::BubbleAction;
use crate::model::Model;
use wa_types::MessageView;

/// WhatsApp's message menu: the quick reaction bar, then the rows `wa-ui`
/// allows for this message.
pub(crate) fn context_menu(
    response: &egui::Response,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    action: &mut Option<BubbleAction>,
) {
    response.context_menu(|ui| {
        ui.set_min_width(212.0);
        ui.add_space(2.0);
        // The quick reaction bar.
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            for emoji in ["👍", "❤️", "😂", "😮", "😢", "🙏"] {
                let reaction = ui.add(
                    egui::Button::new(egui::RichText::new(emoji).size(19.0))
                        .frame(false)
                        .min_size(egui::vec2(29.0, 29.0)),
                );
                if reaction
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    *action = Some(BubbleAction::React(message.id.clone(), emoji.to_string()));
                    ui.close();
                }
            }
        });
        ui.add_space(2.0);
        let menu = model.message_menu(message);
        if menu.reply && menu_row(ui, palette, Icon::Reply, "Reply") {
            *action = Some(BubbleAction::Reply(message.id.clone()));
            ui.close();
        }
        if menu.copy && menu_row(ui, palette, Icon::Copy, "Copy") {
            ui.ctx().copy_text(message.text.clone());
            ui.close();
        }
        if menu.forward && menu_row(ui, palette, Icon::Forward, "Forward") {
            *action = Some(BubbleAction::Forward(message.id.clone()));
            ui.close();
        }
        if menu.edit && menu_row(ui, palette, Icon::Edit, "Edit") {
            *action = Some(BubbleAction::Edit(message.id.clone()));
            ui.close();
        }
        if menu.star && menu_row(ui, palette, Icon::Star, &menu.star_label) {
            *action = Some(BubbleAction::Star(message.id.clone()));
            ui.close();
        }
        ui.add_space(2.0);
        if menu_row(ui, palette, Icon::Delete, "Delete") {
            *action = Some(BubbleAction::Delete(message.id.clone()));
            ui.close();
        }
        ui.add_space(2.0);
    });
}

/// The reaction pills under a message, ours in blue like WhatsApp.
pub(crate) fn pills_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    model: &Model,
    message: &MessageView,
    bubble: egui::Rect,
) {
    let pills = model.reactions_for(&message.id);
    if pills.is_empty() {
        return;
    }
    // Left-to-right, so our own reactions under a right-aligned bubble still
    // start at the bubble's left edge like the official client.
    // Left-to-right, so our own reactions under a right-aligned bubble still
    // start at the bubble's edge, not at the message column's.
    let shift = (bubble.left() + 8.0 - ui.cursor().left()).max(0.0);
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
        ui.add_space(shift);
        ui.spacing_mut().item_spacing.x = 4.0;
        for (emoji, count, mine) in pills {
            let label = (count > 1).then(|| count.to_string());
            let font = egui::FontId::proportional(11.5);
            let label_width = label
                .as_ref()
                .map(|text| {
                    ui.painter()
                        .layout_no_wrap(text.clone(), font.clone(), palette.secondary)
                        .size()
                        .x
                })
                .unwrap_or(0.0);
            let width = 8.0
                + 14.0
                + if label.is_some() {
                    3.0 + label_width
                } else {
                    0.0
                }
                + 8.0;
            let (slot, _) = ui.allocate_exact_size(egui::vec2(width, 22.0), egui::Sense::hover());
            // Straddle the bubble's bottom edge, like the official chip.
            let rect = slot.translate(egui::vec2(0.0, -12.0));
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(11), palette.elevated);
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(11),
                egui::Stroke::new(1.0_f32, palette.border),
                egui::StrokeKind::Inside,
            );
            let glyph = emoji.chars().next().unwrap_or('?');
            if let Some(texture) = crate::emoji_glyph::texture_shared(ui.ctx(), glyph, 14) {
                let size = texture.size_vec2();
                let scale = (14.0 / size.x.max(1.0)).min(14.0 / size.y.max(1.0));
                ui.painter().image(
                    texture.id(),
                    egui::Rect::from_center_size(
                        egui::pos2(rect.left() + 15.0, rect.center().y),
                        size * scale,
                    ),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            if let Some(label) = label {
                ui.painter().text(
                    egui::pos2(rect.left() + 24.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    label,
                    font,
                    if mine {
                        palette.read
                    } else {
                        palette.secondary
                    },
                );
            }
        }
    });
}

/// A centered date chip ("Today", "12/10/2026") between two days of a chat.
pub(crate) fn day_chip(ui: &mut egui::Ui, palette: &crate::theme::Palette, label: &str) {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(11.5),
        palette.secondary,
    );
    let width = galley.size().x + 22.0;
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 20.0), egui::Sense::hover());
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(10), palette.elevated);
        ui.painter().galley(
            egui::pos2(
                rect.center().x - galley.size().x / 2.0,
                rect.center().y - galley.size().y / 2.0,
            ),
            galley.clone(),
            palette.secondary,
        );
    });
    ui.add_space(4.0);
}
