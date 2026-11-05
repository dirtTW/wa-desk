//! The Status screen: the list with the 24-hour rings, the viewer with its
//! segmented progress bar, and the "Share statuses" pane.

use crate::app::{Icon, WaDesk, icon_button, status_as_message};
use crate::model::Command;
use crate::widgets::{avatar_color, hex_colour, initials, status_send_button, status_share_tile};

impl WaDesk {
    /// The Status screen: the official list on the left (a 64 px header, the
    /// "My status" row, then Recent/Viewed) and the "Share statuses" pane on the
    /// right with the Text / Photo-and-video tiles.
    /// The Status screen: the official list on the left (the "My status" row,
    /// then Recent/Viewed) and the "Share statuses" pane on the right.
    pub(crate) fn status_panel(&mut self, ctx: &egui::Context) {
        let palette = self.palette.clone();
        if self.model.viewer.is_some() {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.status_viewer(ui, ctx);
            });
            return;
        }

        // Right: the share pane, 320 px, the official empty-state card.
        egui::SidePanel::right("status-share")
            .resizable(false)
            .exact_width(320.0)
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                ui.add_space(22.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Share statuses")
                                .size(20.0)
                                .color(palette.text),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "Share photos, videos and text that disappear after 24 hours.",
                            )
                            .size(13.5)
                            .color(palette.secondary),
                        );
                        ui.add_space(18.0);
                        // Two tiles: text first, then photo/video, as official.
                        if status_share_tile(ui, &palette, Icon::Edit, "Text status") {
                            self.status_compose = Some(String::new());
                        }
                        ui.add_space(10.0);
                        if status_share_tile(ui, &palette, Icon::Document, "Photo and video")
                            && let Some(path) = rfd::FileDialog::new()
                                .add_filter("Photo or video", &["jpg", "jpeg", "png", "mp4"])
                                .pick_file()
                        {
                            self.send(Command::PostStatusMedia {
                                path: path.to_string_lossy().into_owned(),
                                caption: String::new(),
                            });
                        }
                        if self.status_compose.is_some() {
                            ui.add_space(16.0);
                            // The official composer row: Emoji, Font, Palette,
                            // Send, and a close control.
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Text status")
                                        .size(13.0)
                                        .color(palette.accent),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if icon_button(ui, Icon::Close, &palette)
                                            .on_hover_text("Close status posting view")
                                            .clicked()
                                        {
                                            self.status_compose = None;
                                        }
                                        if icon_button(ui, Icon::Emoji, &palette)
                                            .on_hover_text("Emoji")
                                            .clicked()
                                        {
                                            self.model.emoji_open = !self.model.emoji_open;
                                        }
                                    },
                                );
                            });
                            // The box takes the chosen background, like the
                            // official composer, and the palette below is the
                            // official 20-colour array from `wa-ui`.
                            let swatches = wa_ui::composer::palette();
                            let background = swatches
                                .backgrounds
                                .get(self.status_compose_background)
                                .cloned()
                                .unwrap_or_else(|| "#A62C71".to_string());
                            let fill = hex_colour(&background);
                            let mut draft = self.status_compose.clone().unwrap_or_default();
                            egui::Frame::new()
                                .fill(fill)
                                .corner_radius(egui::CornerRadius::same(10))
                                .inner_margin(egui::Margin::symmetric(10, 8))
                                .show(ui, |ui| {
                                    ui.add_sized(
                                        [240.0, 64.0],
                                        egui::TextEdit::multiline(&mut draft)
                                            .frame(false)
                                            .hint_text("Type a status"),
                                    );
                                });
                            self.status_compose = Some(draft.clone());
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
                                for (index, hex) in swatches.backgrounds.iter().enumerate() {
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(20.0, 20.0),
                                        egui::Sense::click(),
                                    );
                                    ui.painter().circle_filled(
                                        rect.center(),
                                        10.0,
                                        hex_colour(hex),
                                    );
                                    if index == self.status_compose_background {
                                        ui.painter().circle_stroke(
                                            rect.center(),
                                            9.0,
                                            egui::Stroke::new(2.0_f32, palette.text),
                                        );
                                    }
                                    if response.clicked() {
                                        self.status_compose_background = index;
                                    }
                                }
                            });
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                for (index, name) in swatches.font_names.iter().enumerate() {
                                    let selected = index == self.status_compose_font;
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(name).size(12.5).color(
                                                    if selected {
                                                        palette.accent
                                                    } else {
                                                        palette.secondary
                                                    },
                                                ),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        self.status_compose_font = index;
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            if status_send_button(ui, &palette) && !draft.trim().is_empty() {
                                let font = swatches
                                    .fonts
                                    .get(self.status_compose_font)
                                    .cloned()
                                    .unwrap_or_else(|| "system".to_string());
                                self.send(Command::PostStatusText {
                                    text: draft.trim().to_string(),
                                    background: wa_ui::composer::argb(&background),
                                    font,
                                });
                                self.status_compose = None;
                            }
                        }
                    });
                });
            });

        // Left: the list.
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 12))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Status").size(20.0).color(palette.text));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Back to chats")
                                        .clicked()
                                    {
                                        self.show_status = false;
                                    }
                                    if icon_button(ui, Icon::Edit, &palette)
                                        .on_hover_text("Text status")
                                        .clicked()
                                    {
                                        self.status_compose = Some(String::new());
                                    }
                                },
                            );
                        });
                    });

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // My status: the own avatar with a dashed "add" ring.
                        let own = self.model.own_name();
                        self.ensure_avatar(ui.ctx(), &self.model.me_jid.clone());
                        let avatar = self.avatar_for(&self.model.me_jid.clone()).cloned();
                        if status_row(ui, &palette, &own, "", avatar.as_ref(), false, true) {
                            self.status_compose = Some(String::new());
                        }
                        let (recent, viewed) = self.model.status_sections();
                        if recent.is_empty() && viewed.is_empty() {
                            ui.add_space(26.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new("No recent updates")
                                        .size(14.0)
                                        .color(palette.secondary),
                                );
                            });
                            return;
                        }
                        for (heading, groups) in [("Recent", &recent), ("Viewed", &viewed)] {
                            if groups.is_empty() {
                                continue;
                            }
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(
                                    egui::RichText::new(heading)
                                        .size(13.0)
                                        .color(palette.accent),
                                );
                            });
                            ui.add_space(2.0);
                            for group in groups.iter() {
                                self.ensure_avatar(ui.ctx(), &group.jid);
                                let avatar = self.avatar_for(&group.jid).cloned();
                                let stamp = format!("{} update(s)", group.count);
                                if status_row(
                                    ui,
                                    &palette,
                                    &group.name,
                                    &stamp,
                                    avatar.as_ref(),
                                    group.unseen,
                                    false,
                                ) {
                                    let jid = group.jid.clone();
                                    self.model.open_status(&jid);
                                }
                            }
                        }
                    });
            });
    }

    /// One story frame: progress segments, the body, and prev/next/close.
    /// The 24-hour viewer: segments, author over timestamp, the body, and the
    /// reply/react/delete row.
    pub(crate) fn status_viewer(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(update) = self.model.current_status().cloned() else {
            self.model.close_status();
            return;
        };
        let Some(chrome) = self.model.status_chrome() else {
            self.model.close_status();
            return;
        };
        let palette = self.palette.clone();

        // Segments: everything before the frame is full, the active one fills.
        let (group, frame) = self.model.viewer.unwrap_or((0, 0));
        let groups = self.model.status_groups();
        let frames = groups.get(group).map(|row| row.count).unwrap_or(1);
        let ratio = self.model.status_progress() as f32;
        ui.horizontal(|ui| {
            for index in 0..frames {
                let fill = if index < frame {
                    1.0
                } else if index == frame {
                    ratio.clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let width = (ui.available_width() - (frames as f32) * 4.0) / frames.max(1) as f32;
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(width, 4.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 2.0, egui::Color32::from_gray(70));
                let mut done = rect;
                done.set_width(rect.width() * fill);
                ui.painter()
                    .rect_filled(done, 2.0, egui::Color32::from_gray(220));
            }
        });
        ui.add_space(8.0);

        // The live viewer stacks the author over the timestamp, top-left
        // (sampled 2026-11-05: "Daffa" / "Yesterday at 10:29 PM") instead of
        // running them together on one line.
        ui.horizontal(|ui| {
            // The live chrome carries the author avatar in the header.
            self.ensure_avatar(ctx, &update.sender);
            let avatar = self.avatar_for(&update.sender).cloned();
            crate::widgets::paint_avatar(ui, &update.name, 34.0, avatar.as_ref());
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&update.name).size(14.5).strong());
                ui.label(
                    egui::RichText::new(self.model.bubble_stamp(&status_as_message(&update)))
                        .size(12.5)
                        .color(egui::Color32::from_gray(170)),
                );
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("✕").clicked() {
                    self.model.close_status();
                }
            });
        });

        let bg = update.background.trim_start_matches('#');
        let color = u32::from_str_radix(bg, 16)
            .map(|value| {
                egui::Color32::from_rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
            })
            .unwrap_or(egui::Color32::from_rgb(36, 38, 38));
        // The body takes what is left above the bottom bar: a
        // `centered_and_justified` inside an unconstrained frame swallows the
        // whole window and pushed the reply bar off screen.
        let body_height = (ui.available_height() - 96.0).max(200.0);

        // Image and video stories paint the media: the status message's
        // protobuf is cached by `wa-core`, so the same `FetchMedia` path the
        // chat bubbles use works with the status id.
        let media = matches!(update.kind.as_str(), "image" | "video");
        if media
            && self.model.media_path(&update.id).is_none()
            && self.model.request_media(&update.id)
        {
            self.send(Command::FetchMedia {
                id: update.id.clone(),
            });
        }
        let texture = media
            .then(|| {
                crate::bubbles::texture_at(
                    ui.ctx(),
                    &mut self.textures,
                    &update.id,
                    self.model.media_path(&update.id),
                    crate::bubbles::PHOTO_PX,
                )
            })
            .flatten();

        let (area, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), body_height),
            egui::Sense::hover(),
        );
        let painter = ui.painter();
        painter.rect_filled(area, egui::CornerRadius::same(10), color);
        match texture {
            Some(texture) => {
                let size = texture.size_vec2();
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                // The blurred backdrop: the same frame stretched to cover and
                // dimmed, exactly how the official stage fills the sides.
                let cover = (area.width() / size.x.max(1.0)).max(area.height() / size.y.max(1.0));
                painter.with_clip_rect(area).image(
                    texture.id(),
                    egui::Rect::from_center_size(area.center(), size * cover),
                    uv,
                    egui::Color32::from_gray(80),
                );
                // The story itself, letterboxed inside the stage.
                let fit = (area.width() / size.x.max(1.0)).min(area.height() / size.y.max(1.0));
                painter.image(
                    texture.id(),
                    egui::Rect::from_center_size(area.center(), size * fit),
                    uv,
                    egui::Color32::WHITE,
                );
            }
            None => {
                let body = if update.text.trim().is_empty() {
                    wa_ui::chat::media_placeholder(&update.kind).to_string()
                } else {
                    update.text.clone()
                };
                painter.text(
                    area.center(),
                    egui::Align2::CENTER_CENTER,
                    body,
                    egui::FontId::proportional(22.0),
                    egui::Color32::WHITE,
                );
            }
        }
        // The caption sits over the bottom of the media, like the official.
        if !update.text.trim().is_empty() && media {
            painter.text(
                egui::pos2(area.center().x, area.bottom() - 14.0),
                egui::Align2::CENTER_BOTTOM,
                &update.text,
                egui::FontId::proportional(15.0),
                egui::Color32::WHITE,
            );
        }

        ui.add_space(8.0);
        // The official bottom bar: the side arrows, then either the reply field
        // with its heart reaction (someone else's status) or the view count and
        // Delete (your own).
        let mut reply: Option<String> = None;
        let mut react: Option<String> = None;
        let mut delete = false;
        ui.horizontal(|ui| {
            if !chrome.prev_hidden && ui.button("◀").clicked() {
                self.model.step_status(false);
            }
            if !chrome.next_hidden && ui.button("▶").clicked() {
                self.model.step_status(true);
            }
            if chrome.own_story {
                ui.label(
                    egui::RichText::new(format!("{} view(s)", chrome.view_count_label))
                        .small()
                        .weak(),
                );
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Delete")
                                .size(12.5)
                                .color(palette_danger()),
                        )
                        .frame(false),
                    )
                    .on_hover_text("Delete this status")
                    .clicked()
                {
                    delete = true;
                }
            } else {
                // The live bar starts with an emoji control; ours opens a
                // compact strip that appends to the same draft.
                // The Material glyph, not the emoji font: the text path
                // paints colour emoji as a grey blob.
                if icon_button(ui, Icon::Emoji, &palette)
                    .on_hover_text("Emoji")
                    .clicked()
                {
                    self.status_emoji_open = !self.status_emoji_open;
                }
                if icon_button(ui, Icon::Sticker, &palette)
                    .on_hover_text("Sticker")
                    .clicked()
                {
                    self.status_sticker_open = !self.status_sticker_open;
                }
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut self.model.composer)
                        .hint_text("Type a reply…")
                        .desired_width(240.0),
                );
                let enter =
                    edit.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if (ui.button("Send").clicked() || enter) && !self.model.composer.trim().is_empty()
                {
                    reply = Some(self.model.composer.trim().to_string());
                }
                if ui
                    .button("❤")
                    .on_hover_text("React to this status")
                    .clicked()
                {
                    react = Some("❤".to_string());
                }
            }
        });
        if self.status_sticker_open && !chrome.own_story {
            // The account's stickers, newest first: a tap uploads the file and
            // sends it as the status answer.
            let stickers = self.model.sticker_collection();
            let mut chosen: Option<String> = None;
            ui.horizontal(|ui| {
                for (id, path) in stickers.iter().take(10) {
                    let texture = crate::bubbles::texture_at(
                        ui.ctx(),
                        &mut self.textures,
                        &format!("status-sticker-{id}"),
                        Some(path.clone()),
                        crate::bubbles::STICKER_PX,
                    );
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::click());
                    if response.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(8),
                            egui::Color32::from_white_alpha(20),
                        );
                    }
                    if let Some(texture) = texture {
                        let size = texture.size_vec2();
                        let scale = (40.0 / size.x.max(1.0)).min(40.0 / size.y.max(1.0));
                        ui.painter().image(
                            texture.id(),
                            egui::Rect::from_center_size(rect.center(), size * scale),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            egui::Color32::WHITE,
                        );
                    }
                    if response.clicked() {
                        chosen = Some(path.clone());
                    }
                }
                if stickers.is_empty() {
                    ui.label(
                        egui::RichText::new("No stickers cached yet")
                            .size(12.5)
                            .color(egui::Color32::from_gray(170)),
                    );
                }
            });
            if let Some(path) = chosen {
                use base64::Engine as _;
                if let Ok(bytes) = std::fs::read(&path) {
                    self.send(Command::ReplyStatusSticker {
                        sender: update.sender.clone(),
                        id: update.id.clone(),
                        payload: base64::engine::general_purpose::STANDARD.encode(bytes),
                    });
                    self.status_sticker_open = false;
                }
            }
        }
        if self.status_emoji_open && !chrome.own_story {
            ui.horizontal(|ui| {
                for emoji in [
                    "\u{1F44D}",
                    "\u{2764}\u{FE0F}",
                    "\u{1F602}",
                    "\u{1F62E}",
                    "\u{1F622}",
                    "\u{1F64F}",
                ] {
                    if ui.button(emoji).clicked() {
                        self.model.composer.push_str(emoji);
                        self.status_emoji_open = false;
                    }
                }
            });
        }
        if let Some(text) = reply {
            self.model.composer.clear();
            self.send(Command::ReplyStatus {
                sender: update.sender.clone(),
                id: update.id.clone(),
                text,
            });
        }
        if let Some(emoji) = react {
            self.send(Command::ReactStatus {
                sender: update.sender.clone(),
                id: update.id.clone(),
                emoji,
            });
        }
        if delete {
            self.send(Command::DeleteStatus {
                id: update.id.clone(),
            });
            self.model.close_status();
        }

        // A frame that runs out advances on its own, like the phone.
        if ratio >= 1.0 {
            self.model.step_status(true);
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

fn palette_danger() -> egui::Color32 {
    egui::Color32::from_rgb(251, 80, 97)
}

/// One 72 px status row: ring avatar, name, meta line.
fn status_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    name: &str,
    meta: &str,
    avatar: Option<&egui::TextureHandle>,
    unseen: bool,
    mine: bool,
) -> bool {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 72.0), egui::Sense::click());
    let card = rect.shrink2(egui::vec2(8.0, 0.0));
    if response.hovered() {
        ui.painter()
            .rect_filled(card, egui::CornerRadius::same(12), palette.hover);
    }
    let center = egui::pos2(rect.left() + 44.0, rect.center().y);
    let ring = if mine {
        palette.secondary
    } else if unseen {
        palette.accent
    } else {
        egui::Color32::from_gray(120)
    };
    ui.painter()
        .circle_stroke(center, 26.0, egui::Stroke::new(2.5_f32, ring));
    if let Some(texture) = avatar {
        ui.painter().image(
            texture.id(),
            egui::Rect::from_center_size(center, egui::vec2(46.0, 46.0)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().circle_filled(center, 23.0, avatar_color(name));
        ui.painter().text(
            center,
            egui::Align2::CENTER_CENTER,
            initials(name),
            egui::FontId::proportional(16.0),
            egui::Color32::WHITE,
        );
    }
    ui.painter().text(
        egui::pos2(rect.left() + 82.0, rect.top() + 24.0),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(15.0),
        palette.text,
    );
    let meta_text = if mine && meta.is_empty() {
        "Click to add status update"
    } else {
        meta
    };
    ui.painter().text(
        egui::pos2(rect.left() + 82.0, rect.top() + 46.0),
        egui::Align2::LEFT_CENTER,
        meta_text,
        egui::FontId::proportional(13.0),
        palette.secondary,
    );
    response.clicked()
}
