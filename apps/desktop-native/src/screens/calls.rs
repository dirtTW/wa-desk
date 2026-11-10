//! The Calls drawer: the official header (title, kebab, green phone FAB), the
//! "Search name, number or @username" field, the Favorites section and the
//! Recent list. Sampled from web.whatsapp.com on 2026-11-05
//! (`target/live_calls.jpg`, `target/live_calls_list.jpg`).

use crate::app::{Icon, WaDesk, draw_icon, icon_button, now_ms};
use crate::model::Command;
use crate::widgets::{avatar_color, fit_text, initials};

impl WaDesk {
    /// The Calls drawer, in the same column as the chat list, so the messages
    /// pane stays beside it like the official client.
    pub(crate) fn calls_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette.clone();
        egui::Panel::left("calls")
            .resizable(true)
            .default_size(368.0)
            .size_range(300.0..=460.0)
            .show(ui, |ui| {
                // Header: "Calls", the kebab and the green new-call button.
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_height(44.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Calls").size(26.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // The live FAB is a phone with a plus badge,
                                    // not the plain compose FAB.
                                    if call_fab(ui, &palette).clicked() {
                                        self.show_new_chat = true;
                                        self.send(Command::Contacts);
                                    }
                                    if icon_button(ui, Icon::Menu, &palette)
                                        .on_hover_text("Menu")
                                        .clicked()
                                    {
                                        self.notice = Some((
                                            "Call settings live on the phone".to_string(),
                                            std::time::Instant::now(),
                                        ));
                                    }
                                },
                            );
                        });
                    });

                // The live search field: rounded, inset, with the placeholder
                // "Search name, number or @username".
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    egui::Frame::new()
                        .fill(palette.elevated)
                        .corner_radius(egui::CornerRadius::same(9))
                        .inner_margin(egui::Margin::symmetric(10, 5))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(15.0, 15.0),
                                    egui::Sense::hover(),
                                );
                                draw_icon(ui, rect, Icon::Search, palette.secondary);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.calls_query)
                                        .frame(egui::Frame::NONE)
                                        .hint_text("Search name, number or @username")
                                        .desired_width(ui.available_width() - 8.0),
                                );
                            });
                        });
                    ui.add_space(10.0);
                });
                ui.add_space(6.0);

                let query = self.calls_query.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Favorites: the live drawer opens with the heading and
                        // an "Add favorite" row above Recent.
                        section_heading(ui, &palette, "Favorites");
                        if let Some(notice) = add_favorite_row(ui, &palette) {
                            self.notice = Some((notice, std::time::Instant::now()));
                        }
                        section_heading(ui, &palette, "Recent");

                        let entries: Vec<crate::model::CallLogEntry> = self
                            .model
                            .call_log
                            .iter()
                            .filter(|entry| {
                                query.is_empty() || entry.name.to_lowercase().contains(&query)
                            })
                            .cloned()
                            .collect();
                        if entries.is_empty() {
                            ui.add_space(24.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(if query.is_empty() {
                                        "No calls yet"
                                    } else {
                                        "No calls match that search"
                                    })
                                    .size(13.5)
                                    .color(palette.secondary),
                                );
                            });
                            return;
                        }
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

/// The Calls FAB: the accent disc with a phone glyph and a small plus at the
/// lower right, as the live header shows.
fn call_fab(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .circle_filled(rect.center(), 21.0, palette.accent.gamma_multiply(0.9));
    }
    ui.painter()
        .circle_filled(rect.center(), 20.0, palette.accent);
    draw_icon(
        ui,
        egui::Rect::from_center_size(rect.center(), egui::vec2(21.0, 21.0)),
        Icon::Phone,
        egui::Color32::from_rgb(11, 20, 26),
    );
    let badge = egui::pos2(rect.center().x + 7.0, rect.center().y + 7.0);
    ui.painter().circle_filled(badge, 6.5, palette.accent);
    ui.painter().text(
        badge,
        egui::Align2::CENTER_CENTER,
        "+",
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(11, 20, 26),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A green 13 px section heading, as the live calls drawer uses.
fn section_heading(ui: &mut egui::Ui, palette: &crate::theme::Palette, text: &str) {
    ui.add_space(10.0);
    ui.label(
        egui::RichText::new(text)
            .size(13.0)
            .strong()
            .color(palette.accent),
    );
    ui.add_space(2.0);
}

/// The live "Add favorite" row: a green disc with a person-plus glyph.
fn add_favorite_row(ui: &mut egui::Ui, palette: &crate::theme::Palette) -> Option<String> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 56.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(10), palette.hover);
    }
    let disc = egui::pos2(rect.left() + 30.0, rect.center().y);
    ui.painter().circle_filled(disc, 22.0, palette.accent);
    draw_icon(
        ui,
        egui::Rect::from_center_size(disc, egui::vec2(22.0, 22.0)),
        Icon::PersonAdd,
        egui::Color32::from_rgb(11, 20, 26),
    );
    ui.painter().text(
        egui::pos2(rect.left() + 60.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Add favorite",
        egui::FontId::proportional(14.5),
        palette.text,
    );
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
        .then(|| "Favourites are set from the chat menu".to_string())
}

/// One 72 px call row: avatar, name, the direction + kind line, and the time at
/// the right. The call buttons appear on hover, like the live list.
pub(crate) fn call_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    entry: &crate::model::CallLogEntry,
    avatar: Option<&egui::TextureHandle>,
) -> Option<bool> {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(10), palette.hover);
    }
    let center = egui::pos2(rect.left() + 40.0, rect.center().y);
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
    // The name is white even for a missed call; only the kind line turns red.
    ui.painter().text(
        egui::pos2(rect.left() + 76.0, rect.top() + 27.0),
        egui::Align2::LEFT_CENTER,
        fit_text(
            ui.painter(),
            &entry.name,
            egui::FontId::proportional(15.5),
            palette.text,
            rect.width() - 190.0,
        ),
        egui::FontId::proportional(15.5),
        palette.text,
    );
    // Second line: the direction glyph then "Missed" / "Outgoing" / "Incoming".
    let glyph = if entry.missed {
        Icon::CallEnd
    } else if entry.outgoing {
        Icon::Forward
    } else {
        Icon::Reply
    };
    let line_color = if entry.missed {
        palette.danger
    } else {
        palette.secondary
    };
    let glyph_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 76.0, rect.top() + 43.0),
        egui::vec2(13.0, 13.0),
    );
    draw_icon(ui, glyph_rect, glyph, line_color);
    let mut kind = if entry.missed {
        "Missed".to_string()
    } else if entry.outgoing {
        "Outgoing".to_string()
    } else {
        "Incoming".to_string()
    };
    if entry.seconds > 0 {
        kind.push_str(&format!(
            " - {}:{:02}",
            entry.seconds / 60,
            entry.seconds % 60
        ));
    }
    ui.painter().text(
        egui::pos2(rect.left() + 94.0, rect.top() + 49.5),
        egui::Align2::LEFT_CENTER,
        kind,
        egui::FontId::proportional(12.5),
        line_color,
    );
    if entry.at > 0 {
        let stamp = wa_ui::labels::stamp_of(entry.at, now_ms());
        ui.painter().text(
            egui::pos2(rect.right() - 14.0, rect.top() + 27.0),
            egui::Align2::RIGHT_CENTER,
            stamp,
            egui::FontId::proportional(12.5),
            palette.secondary,
        );
    }

    // The call buttons appear while the row is hovered, as on the desktop.
    if response.hovered() {
        let button = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 26.0, rect.center().y),
            egui::vec2(32.0, 32.0),
        );
        let hit = ui.interact(
            button,
            ui.scope_id().with(("call", &entry.jid)),
            egui::Sense::click(),
        );
        if hit.hovered() {
            ui.painter()
                .rect_filled(button, egui::CornerRadius::same(16), palette.selected);
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
    }
    None
}
