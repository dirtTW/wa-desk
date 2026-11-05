//! Group info: the official drawer head (photo with the camera badge, subject
//! and edit pencil, the four labelled action discs), the description, the flat
//! icon rows with the media strip and the capped member list.
//!
//! Split out of `app.rs` so the window file keeps only state and layout.

use crate::app::{Icon, WaDesk, draw_icon, icon_button};
use crate::model::Command;
use crate::settings_ui::{settings_card, settings_link, settings_radio_row, settings_row};
use crate::widgets::paint_avatar;

/// A flat Group info row: leading glyph, title, optional subtitle and trailing
/// text, with the hover highlight the live drawer uses. Returns `true` on click.
fn group_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    title: &str,
    subtitle: Option<&str>,
    trailing: Option<String>,
) -> bool {
    let height = if subtitle.is_some() { 56.0 } else { 46.0 };
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), palette.hover);
    }
    draw_icon(
        ui,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 24.0, rect.center().y),
            egui::vec2(22.0, 22.0),
        ),
        icon,
        palette.secondary,
    );
    let text_left = rect.left() + 52.0;
    let title_y = if subtitle.is_some() {
        rect.center().y - 10.0
    } else {
        rect.center().y
    };
    ui.painter().text(
        egui::pos2(text_left, title_y),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(15.0),
        palette.text,
    );
    if let Some(subtitle) = subtitle {
        ui.painter().text(
            egui::pos2(text_left, rect.center().y + 11.0),
            egui::Align2::LEFT_CENTER,
            subtitle,
            egui::FontId::proportional(12.5),
            palette.secondary,
        );
    }
    if let Some(trailing) = trailing {
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, title_y),
            egui::Align2::RIGHT_CENTER,
            trailing,
            egui::FontId::proportional(12.5),
            palette.secondary,
        );
    }
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

impl WaDesk {
    /// for the Media strip the live drawer shows.
    pub(crate) fn group_media_thumbnails(&self, jid: &str) -> Vec<(String, String)> {
        let Some(row) = self.model.chats.get(jid) else {
            return Vec::new();
        };
        row.messages
            .iter()
            .rev()
            .filter(|message| matches!(message.kind.as_str(), "image" | "video"))
            .filter_map(|message| {
                self.model
                    .media_path(&message.id)
                    .map(|path| (message.id.clone(), path))
            })
            .take(4)
            .collect()
    }

    pub(crate) fn group_panel(&mut self, ctx: &egui::Context) {
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
                            ui.label(
                                egui::RichText::new("Group info")
                                    .size(20.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Back").clicked() {
                                        self.show_group = false;
                                    }
                                },
                            );
                        });
                    });

                if let Some(err) = self.model.group_error.clone() {
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new(err).color(palette.danger));
                    });
                    return;
                }
                let Some(info) = self.model.group.clone() else {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Loading group info…")
                                .size(13.5)
                                .color(palette.secondary),
                        );
                    });
                    return;
                };

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Avatar, name, kind.
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            let avatar = self.avatar_for(&info.jid).cloned();
                            // The live pane paints a green camera badge on the
                            // group photo so it can be changed from here.
                            // Paint the photo into a rect we own, so the camera
                            // badge can sit on its corner regardless of layout.
                            let (photo, _) = ui
                                .allocate_exact_size(egui::vec2(96.0, 96.0), egui::Sense::hover());
                            if let Some(texture) = avatar.as_ref() {
                                ui.painter().image(
                                    texture.id(),
                                    photo,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().circle_filled(
                                    photo.center(),
                                    48.0,
                                    crate::widgets::avatar_color(&info.subject),
                                );
                                ui.painter().text(
                                    photo.center(),
                                    egui::Align2::CENTER_CENTER,
                                    crate::widgets::initials(&info.subject),
                                    egui::FontId::proportional(34.0),
                                    egui::Color32::WHITE,
                                );
                            }
                            let badge = egui::pos2(photo.right() - 14.0, photo.bottom() - 14.0);
                            ui.painter().circle_filled(badge, 14.0, palette.accent);
                            draw_icon(
                                ui,
                                egui::Rect::from_center_size(badge, egui::vec2(16.0, 16.0)),
                                Icon::PersonAdd,
                                egui::Color32::from_rgb(11, 20, 26),
                            );
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                // The name sits beside its edit pencil, as live.
                                ui.add_space((ui.available_width() - 260.0).max(0.0) / 2.0);
                                ui.label(
                                    egui::RichText::new(&info.subject)
                                        .size(21.0)
                                        .color(palette.text),
                                );
                                if icon_button(ui, Icon::Edit, &palette)
                                    .on_hover_text("Edit group name")
                                    .clicked()
                                {
                                    self.notice = Some((
                                        "Renaming a group happens on the phone".to_string(),
                                        std::time::Instant::now(),
                                    ));
                                }
                            });
                            // `group_member_count` already reads
                            // "Group · N members"; a community only needs its own
                            // name in front.
                            let count = wa_ui::labels::group_member_count(info.members.len());
                            let label = if info.is_community {
                                format!("Community · {count}")
                            } else {
                                count
                            };
                            ui.label(
                                egui::RichText::new(label)
                                    .size(13.0)
                                    .color(palette.secondary),
                            );
                            ui.add_space(12.0);
                            ui.horizontal(|ui| {
                                // Voice / Video / Add / Search discs with labels.
                                ui.spacing_mut().item_spacing.x = 26.0;
                                ui.add_space(
                                    ((ui.available_width() - 4.0 * 46.0 - 3.0 * 26.0) / 2.0)
                                        .max(0.0),
                                );
                                let actions = [
                                    (Icon::Phone, "Voice"),
                                    (Icon::VideoCall, "Video"),
                                    (Icon::PersonAdd, "Add"),
                                    (Icon::Search, "Search"),
                                ];
                                for (icon, label) in actions {
                                    ui.vertical(|ui| {
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(46.0, 46.0),
                                            egui::Sense::click(),
                                        );
                                        ui.painter().circle_filled(
                                            rect.center(),
                                            23.0,
                                            palette.hover,
                                        );
                                        if response.hovered() {
                                            ui.painter().circle_filled(
                                                rect.center(),
                                                23.0,
                                                palette.selected,
                                            );
                                        }
                                        draw_icon(ui, rect.shrink(12.0), icon, palette.accent);
                                        if response.clicked() {
                                            self.notice = Some((
                                                format!("{label} opens from the group screen"),
                                                std::time::Instant::now(),
                                            ));
                                        }
                                        ui.label(
                                            egui::RichText::new(label)
                                                .size(11.5)
                                                .color(palette.secondary),
                                        );
                                    });
                                }
                            });
                            ui.add_space(14.0);
                        });

                        let width = (ui.available_width() - 40.0).clamp(280.0, 640.0);
                        let side = ((ui.available_width() - width) / 2.0).max(0.0);
                        ui.horizontal(|ui| {
                            ui.add_space(side);
                            ui.vertical(|ui| {
                                ui.set_width(width);
                                // The live pane opens with the creator's card, above
                                // the description.
                                if let Some(creator) = info.creator.clone() {
                                    settings_card(ui, &palette, "GROUP CREATOR", |ui| {
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.add_space(6.0);
                                            paint_avatar(ui, &creator, 36.0, None);
                                            ui.add_space(8.0);
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    egui::RichText::new(&creator)
                                                        .size(14.0)
                                                        .color(palette.text),
                                                );
                                                ui.label(
                                                    egui::RichText::new("Created this group")
                                                        .size(12.0)
                                                        .color(palette.secondary),
                                                );
                                            });
                                        });
                                        ui.add_space(4.0);
                                    });
                                    ui.add_space(10.0);
                                }
                                if let Some(description) =
                                    info.description.clone().filter(|d| !d.trim().is_empty())
                                {
                                    let long = description.chars().count() > 160;
                                    let shown: String = if long && !self.group_desc_open {
                                        let mut cut: String =
                                            description.chars().take(160).collect();
                                        cut.push('\u{2026}');
                                        cut
                                    } else {
                                        description.clone()
                                    };
                                    let mut expand = false;
                                    settings_card(ui, &palette, "DESCRIPTION", |ui| {
                                        ui.add_space(6.0);
                                        ui.label(
                                            egui::RichText::new(shown)
                                                .size(13.5)
                                                .color(palette.text),
                                        );
                                        if long {
                                            let label = if self.group_desc_open {
                                                "Read less"
                                            } else {
                                                "Read more"
                                            };
                                            if settings_link(ui, &palette, label) {
                                                expand = true;
                                            }
                                        }
                                        ui.add_space(6.0);
                                    });
                                    if expand {
                                        self.group_desc_open = !self.group_desc_open;
                                    }
                                    ui.add_space(10.0);
                                } else {
                                    // The live pane offers the link instead of an
                                    // empty card when the group has no description.
                                    ui.horizontal(|ui| {
                                        ui.add_space(4.0);
                                        if settings_link(ui, &palette, "Add group description") {
                                            self.notice = Some((
                                                "Descriptions are set on the phone".to_string(),
                                                std::time::Instant::now(),
                                            ));
                                        }
                                    });
                                    ui.add_space(6.0);
                                }
                                // The live row list (sampled 2026-11-05): flat
                                // rows with a leading glyph - no card - and the
                                // Media row leads into a strip of four thumbnails.
                                let mut media = false;
                                let mut starred = false;
                                let photos = self.group_media_thumbnails(&info.jid);
                                ui.add_space(6.0);
                                if group_row(
                                    ui,
                                    &palette,
                                    Icon::Folder,
                                    "Media, links and docs",
                                    None,
                                    (!photos.is_empty()).then(|| photos.len().to_string()),
                                ) {
                                    media = true;
                                }
                                if !photos.is_empty() {
                                    ui.horizontal(|ui| {
                                        ui.add_space(52.0);
                                        ui.spacing_mut().item_spacing.x = 8.0;
                                        for (id, path) in photos.iter().take(4) {
                                            let texture = crate::bubbles::texture_at(
                                                ui.ctx(),
                                                &mut self.textures,
                                                id,
                                                Some(path.clone()),
                                                160,
                                            );
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(56.0, 56.0),
                                                egui::Sense::click(),
                                            );
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(8),
                                                palette.hover,
                                            );
                                            if let Some(texture) = texture {
                                                ui.painter().image(
                                                    texture.id(),
                                                    rect,
                                                    egui::Rect::from_min_max(
                                                        egui::pos2(0.0, 0.0),
                                                        egui::pos2(1.0, 1.0),
                                                    ),
                                                    egui::Color32::WHITE,
                                                );
                                            }
                                            if response.clicked() {
                                                media = true;
                                            }
                                        }
                                    });
                                    ui.add_space(4.0);
                                }
                                if group_row(
                                    ui,
                                    &palette,
                                    Icon::Star,
                                    "Starred messages",
                                    None,
                                    None,
                                ) {
                                    starred = true;
                                }
                                if group_row(
                                    ui,
                                    &palette,
                                    Icon::Bell,
                                    "Notification settings",
                                    None,
                                    None,
                                ) {
                                    self.notice = Some((
                                        "Muting is in the chat menu".to_string(),
                                        std::time::Instant::now(),
                                    ));
                                }
                                if group_row(
                                    ui,
                                    &palette,
                                    Icon::Palette,
                                    "Chat theme",
                                    None,
                                    Some("Default".to_string()),
                                ) {
                                    self.notice = Some((
                                        "Per-chat wallpaper lives in Settings -> Chats".to_string(),
                                        std::time::Instant::now(),
                                    ));
                                }
                                group_row(
                                    ui,
                                    &palette,
                                    Icon::Lock,
                                    "Encryption",
                                    Some("Messages are end-to-end encrypted. Click to learn more."),
                                    None,
                                );
                                if group_row(
                                    ui,
                                    &palette,
                                    Icon::Lock,
                                    "Advanced chat privacy",
                                    Some("Owned by the phone."),
                                    Some("Off".to_string()),
                                ) {
                                    self.notice = Some((
                                        "Advanced chat privacy is set on the phone".to_string(),
                                        std::time::Instant::now(),
                                    ));
                                }
                                if media {
                                    self.show_media = true;
                                    self.show_group = false;
                                }
                                if starred {
                                    self.show_starred = true;
                                    self.show_group = false;
                                }
                                ui.add_space(10.0);
                                // Disappearing messages: a linked device can set
                                // this for a group (the account-wide default is
                                // the phone's, and stays read-only in Privacy).
                                let current = info.ephemeral;
                                settings_card(ui, &palette, "DISAPPEARING MESSAGES", |ui| {
                                    for (label, seconds, note) in [
                                        ("Off", 0u32, "New messages stay in the chat."),
                                        ("24 hours", 86_400, "New messages vanish after a day."),
                                        ("7 days", 604_800, "New messages vanish after a week."),
                                        (
                                            "90 days",
                                            7_776_000,
                                            "New messages vanish after 90 days.",
                                        ),
                                    ] {
                                        let selected = current.unwrap_or(0) == seconds;
                                        settings_row(ui, &palette, label, note, |ui| {
                                            if settings_radio_row(ui, &palette, "", selected)
                                                .clicked()
                                                && !selected
                                            {
                                                self.send(Command::SetDisappearing {
                                                    chat: info.jid.clone(),
                                                    seconds,
                                                });
                                            }
                                        });
                                    }
                                });
                                ui.add_space(10.0);
                                settings_card(
                                    ui,
                                    &palette,
                                    &format!("{} MEMBERS", info.members.len()),
                                    |ui| {
                                        let show_all = self.group_members_all;
                                        let limit = if show_all { info.members.len() } else { 10 };
                                        for member in info.members.iter().take(limit) {
                                            // The live list prints the participant name,
                                            // not their number or JID.
                                            let name = self
                                                .model
                                                .member_name(&member.jid, member.phone.as_deref());
                                            self.ensure_avatar(ui.ctx(), &member.jid);
                                            let avatar = self.avatar_for(&member.jid).cloned();
                                            ui.horizontal(|ui| {
                                                ui.add_space(10.0);
                                                paint_avatar(ui, &name, 40.0, avatar.as_ref());
                                                ui.add_space(6.0);
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(&name)
                                                            .size(14.5)
                                                            .color(palette.text),
                                                    );
                                                    if member.admin {
                                                        ui.label(
                                                            egui::RichText::new("Group admin")
                                                                .size(12.0)
                                                                .color(palette.secondary),
                                                        );
                                                    }
                                                });
                                            });
                                            ui.add_space(8.0);
                                        }
                                        if info.members.len() > 10 {
                                            let label = if show_all {
                                                "Show fewer".to_string()
                                            } else {
                                                format!(
                                                    "View all ({} more)",
                                                    info.members.len() - 10
                                                )
                                            };
                                            if settings_link(ui, &palette, &label) {
                                                self.group_members_all = !self.group_members_all;
                                            }
                                        }
                                        if settings_link(ui, &palette, "See member changes") {
                                            self.notice = Some((
                                                "Member changes are recorded on the phone"
                                                    .to_string(),
                                                std::time::Instant::now(),
                                            ));
                                        }
                                    },
                                );
                                ui.add_space(10.0);
                                settings_card(ui, &palette, "MORE", |ui| {
                                    let mut add_to_list = false;
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Add to list",
                                        "Put this group in one of your lists.",
                                        |ui| {
                                            if settings_link(ui, &palette, "Add") {
                                                add_to_list = true;
                                            }
                                        },
                                    );
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Export chat",
                                        "WhatsApp exports chats from the phone.",
                                        |_ui| {},
                                    );
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Clear chat",
                                        "Clearing removes the messages on the phone too.",
                                        |_ui| {},
                                    );
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Report group",
                                        "Reports are filed from the phone.",
                                        |_ui| {},
                                    );
                                    if add_to_list {
                                        let name = info.subject.clone();
                                        if let Some(id) =
                                            self.model.create_list(&name, Some(&info.jid))
                                        {
                                            self.model.chat_filter =
                                                crate::model::ChatFilter::List(id);
                                            self.show_group = false;
                                        }
                                    }
                                });
                                ui.add_space(10.0);
                                settings_card(ui, &palette, "LEAVE", |ui| {
                                    settings_row(
                                        ui,
                                        &palette,
                                        "Exit group",
                                        "You will stop receiving messages from this group.",
                                        |ui| {
                                            if ui
                                                .add(
                                                    egui::Button::new(
                                                        egui::RichText::new("Exit")
                                                            .color(palette.danger),
                                                    )
                                                    .frame(false),
                                                )
                                                .clicked()
                                            {
                                                self.send(Command::LeaveGroup {
                                                    chat: info.jid.clone(),
                                                });
                                                self.show_group = false;
                                            }
                                        },
                                    );
                                });
                                ui.add_space(24.0);
                            });
                        });
                    });
            });
    }
}
