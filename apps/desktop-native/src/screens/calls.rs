//! The full-window screens that replace the chat list: Calls so far. Each one
//! is an `impl WaDesk` block plus the small painters it needs, so `app.rs` keeps
//! the window and the conversation and the screens live here.

use crate::app::{Icon, WaDesk, draw_icon, icon_button, now_ms};
use crate::model::Command;
use crate::widgets::{avatar_color, fit_text, initials};

/// The Calls screen: the 64 px header, then the call log as 72 px rows with
/// the direction glyph, the date and a call button - the official layout.
impl WaDesk {
    /// The Calls screen: the official header, then the call log with an
    /// avatar, the direction glyph, the time and a call button per row.
    pub(crate) fn calls_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Calls").size(20.0).color(palette.text));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Back to chats")
                                        .clicked()
                                    {
                                        self.show_calls = false;
                                    }
                                },
                            );
                        });
                    });

                if self.model.call_log.is_empty() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No calls yet")
                                .size(14.0)
                                .color(palette.secondary),
                        );
                    });
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let entries = self.model.call_log.clone();
                        for entry in entries {
                            self.ensure_avatar(ui.ctx(), &entry.jid);
                            let avatar = self.avatar_for(&entry.jid).cloned();
                            if let Some(call) = call_row(ui, &palette, &entry, avatar.as_ref()) {
                                // Start a call with that person from the list.
                                self.model.open_chat(&entry.jid);
                                self.model.calls_outgoing = true;
                                self.send(Command::CallStart {
                                    chat: entry.jid.clone(),
                                    video: call,
                                });
                            }
                        }
                    });
            });
    }
}

/// One 72 px call row: avatar, name, direction + time, and a call button.
/// Returns `Some(video)` when the call button was pressed (false = voice).
pub(crate) fn call_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    entry: &crate::model::CallLogEntry,
    avatar: Option<&egui::TextureHandle>,
) -> Option<bool> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let card = rect.shrink2(egui::vec2(8.0, 0.0));
    if response.hovered() {
        ui.painter()
            .rect_filled(card, egui::CornerRadius::same(12), palette.hover);
    }
    let center = egui::pos2(rect.left() + 44.0, rect.center().y);
    if let Some(texture) = avatar {
        ui.painter().image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(48.0, 48.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter()
            .circle_filled(center, 24.0, avatar_color(&entry.name));
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(&entry.name),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        );
    }
    // Missed calls are red in the official list; the rest are secondary.
    let name_color = if entry.missed {
        palette.danger
    } else {
        palette.text
    };
    ui.painter().text(
        egui::pos2(rect.left() + 82.0, rect.top() + 26.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            ui.painter(),
            &entry.name,
            egui::FontId::proportional(15.0),
            name_color,
            rect.width() - 200.0,
        ),
        egui::FontId::proportional(15.0),
        name_color,
    );
    // Direction glyph + time on the second line.
    let glyph = if entry.missed {
        Icon::CallEnd
    } else if entry.outgoing {
        Icon::Forward
    } else {
        Icon::Reply
    };
    let glyph_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 82.0, rect.top() + 44.0),
        egui::vec2(14.0, 14.0),
    );
    draw_icon(ui, glyph_rect, glyph, name_color);
    let stamp = if entry.at > 0 {
        wa_ui::labels::stamp_of(entry.at, now_ms())
    } else {
        String::new()
    };
    let kind = if entry.video {
        "Video call"
    } else {
        "Voice call"
    };
    let detail = if entry.missed {
        stamp
    } else if entry.seconds > 0 {
        let minutes = entry.seconds / 60;
        let seconds = entry.seconds % 60;
        format!("{stamp}  ·  {kind}  ·  {minutes}:{seconds:02}")
    } else {
        format!("{stamp}  ·  {kind}")
    };
    ui.painter().text(
        egui::pos2(rect.left() + 101.0, rect.top() + 51.0),
        egui::Align2::LEFT_CENTER,
        detail,
        egui::FontId::proportional(12.5),
        palette.secondary,
    );

    // The call button on the right.
    let button = egui::Rect::from_center_size(
        egui::pos2(rect.right() - 34.0, rect.center().y),
        egui::vec2(34.0, 34.0),
    );
    let hit = ui.interact(
        button,
        ui.id().with(("call", &entry.jid)),
        egui::Sense::click(),
    );
    if hit.hovered() {
        ui.painter()
            .rect_filled(button, egui::CornerRadius::same(17), palette.hover);
    }
    draw_icon(
        ui,
        button.shrink(8.0),
        if entry.video {
            Icon::VideoCall
        } else {
            Icon::Phone
        },
        palette.accent,
    );
    if hit.clicked() {
        return Some(entry.video);
    }
    None
}
