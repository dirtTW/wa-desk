//! The conversation: the bubble list, the composer bar with its reply/edit
//! banner, and the attach menu's draft dialogs (poll, contact, location, list).

use crate::app::{
    Icon, WaDesk, action_button, diag, draw_icon, enter_sends, icon_button, paint_wallpaper,
    unread_divider,
};
use crate::bubbles::{bubble, day_chip};
use crate::model::{BubbleAction, Command, Model};
use crate::widgets::{dialog_button, dialog_card, paint_avatar};

impl WaDesk {
    pub(crate) fn conversation(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.wallpaper))
            .show(ui, |ui| {
                // The doodle wallpaper sits behind everything in the panel.
                paint_wallpaper(ui, &palette, ui.max_rect());
                let Some(jid) = self.model.active.clone() else {
                    // The live empty state (`target/live_now.jpg`): a promo card
                    // for calls, then five quick actions and the privacy line.
                    let width = (ui.available_width() - 40.0).clamp(280.0, 420.0);
                    ui.add_space((ui.available_height() * 0.16).clamp(24.0, 140.0));
                    ui.vertical_centered(|ui| {
                        egui::Frame::new()
                            .fill(palette.elevated)
                            .corner_radius(egui::CornerRadius::same(18))
                            .inner_margin(egui::Margin::symmetric(26, 22))
                            .show(ui, |ui| {
                                ui.set_width(width - 52.0);
                                let (art, _) = ui.allocate_exact_size(
                                    egui::vec2(140.0, 96.0),
                                    egui::Sense::hover(),
                                );
                                // A laptop with a phone beside it, in the accent
                                // green the live illustration uses.
                                ui.painter().rect_stroke(
                                    egui::Rect::from_min_size(
                                        egui::pos2(art.left() + 14.0, art.top() + 12.0),
                                        egui::vec2(84.0, 62.0),
                                    ),
                                    egui::CornerRadius::same(5),
                                    egui::Stroke::new(2.0_f32, palette.accent),
                                    egui::StrokeKind::Inside,
                                );
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(art.left(), art.top() + 80.0),
                                        egui::pos2(art.left() + 112.0, art.top() + 80.0),
                                    ],
                                    egui::Stroke::new(4.0_f32, palette.accent),
                                );
                                let phone = egui::Rect::from_min_size(
                                    egui::pos2(art.left() + 96.0, art.top() + 20.0),
                                    egui::vec2(36.0, 62.0),
                                );
                                ui.painter().rect_filled(
                                    phone,
                                    egui::CornerRadius::same(7),
                                    palette.panel,
                                );
                                ui.painter().rect_stroke(
                                    phone,
                                    egui::CornerRadius::same(7),
                                    egui::Stroke::new(2.0_f32, palette.accent),
                                    egui::StrokeKind::Inside,
                                );
                                draw_icon(
                                    ui,
                                    egui::Rect::from_center_size(
                                        phone.center(),
                                        egui::vec2(20.0, 20.0),
                                    ),
                                    Icon::Phone,
                                    palette.accent,
                                );
                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Voice and video calling is now available")
                                        .size(19.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    egui::RichText::new(
                                        "Now you can make and join calls on WhatsApp Web.",
                                    )
                                    .size(13.0)
                                    .color(palette.secondary),
                                );
                                ui.add_space(14.0);
                                let (pill, response) = ui.allocate_exact_size(
                                    egui::vec2(132.0, 34.0),
                                    egui::Sense::click(),
                                );
                                ui.painter().rect_filled(
                                    pill,
                                    egui::CornerRadius::same(17),
                                    if response.hovered() {
                                        palette.accent.gamma_multiply(0.9)
                                    } else {
                                        palette.accent
                                    },
                                );
                                ui.painter().text(
                                    pill.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Go to Calls",
                                    egui::FontId::proportional(13.5),
                                    egui::Color32::from_rgb(11, 20, 26),
                                );
                                if response.clicked() {
                                    self.show_calls = true;
                                }
                            });
                        ui.add_space(26.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 18.0;
                            let total = 5.0 * 72.0 + 4.0 * 18.0;
                            ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                            if action_button(ui, &palette, Icon::Document, "Send document") {
                                self.show_new_chat = true;
                                self.send(Command::Contacts);
                            }
                            if action_button(ui, &palette, Icon::PersonAdd, "Add contact") {
                                self.contact_draft = Some(crate::app::ContactDraft::default());
                            }
                            if action_button(ui, &palette, Icon::Mail, "Invite a friend") {
                                self.notice = Some((
                                    "Invite links are created on the phone".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                            if action_button(ui, &palette, Icon::Phone, "New call") {
                                self.show_calls = true;
                            }
                            if action_button(ui, &palette, Icon::Sparkle, "Ask Meta AI") {
                                self.notice = Some((
                                    "Meta AI needs its own service; AI assistant answers here"
                                        .to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        });
                    });
                    // The live footer line: a lock and the privacy promise.
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        ui.horizontal(|ui| {
                            let (lock, _) = ui
                                .allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            draw_icon(ui, lock, Icon::Lock, palette.secondary);
                            ui.label(
                                egui::RichText::new(
                                    "Your personal messages are end-to-end encrypted",
                                )
                                .size(11.5)
                                .color(palette.secondary),
                            );
                        });
                    });
                    return;
                };
                let Some(row) = self.model.chats.get(&jid).cloned() else {
                    return;
                };

                // The chat header: avatar, name, presence, and the two icons the
                // official client keeps (search and the kebab).
                egui::Frame::new()
                    .fill(palette.panel)
                    .inner_margin(egui::Margin::symmetric(14, 7))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            self.ensure_avatar(ui.ctx(), &row.jid);
                            let chat_avatar = self.avatar_for(&row.jid).cloned();
                            paint_avatar(ui, &self.model.title(&row), 38.0, chat_avatar.as_ref());
                            ui.add_space(2.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(self.model.row_title(&row))
                                        .size(15.5)
                                        .strong(),
                                );
                                let subtitle = if row.jid.ends_with("@g.us") {
                                    match self.model.group.as_ref() {
                                        // The live header lists the participants
                                        // ("Yi, You"), not the member count.
                                        Some(info) if info.jid == row.jid => {
                                            self.model.group_subtitle(info)
                                        }
                                        _ => "Group".to_string(),
                                    }
                                } else if self.model.is_unsaved_person(&row.jid) {
                                    // An unsaved person's header line is the
                                    // number, like the official client.
                                    self.model
                                        .number_of(&row.jid)
                                        .unwrap_or_else(|| "online".to_string())
                                } else {
                                    "online".to_string()
                                };
                                ui.label(
                                    egui::RichText::new(subtitle)
                                        .size(11.5)
                                        .color(palette.secondary),
                                );
                            });
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Right to left: the kebab sits at the right edge,
                                    // then search, then the calls - the official order.
                                    if icon_button(ui, Icon::Menu, &palette)
                                        .on_hover_text("Menu")
                                        .clicked()
                                    {
                                        self.show_chat_menu = false;
                                        self.show_chat_actions = !self.show_chat_actions;
                                    }
                                    if icon_button(ui, Icon::Search, &palette)
                                        .on_hover_text("Search")
                                        .clicked()
                                    {
                                        self.model.search_open = !self.model.search_open;
                                        if !self.model.search_open {
                                            self.model.chat_search.clear();
                                            self.model.search_hit = -1;
                                        }
                                    }
                                    if icon_button(ui, Icon::Phone, &palette)
                                        .on_hover_text("Voice call")
                                        .clicked()
                                    {
                                        self.start_call(false);
                                    }
                                    if icon_button(ui, Icon::VideoCall, &palette)
                                        .on_hover_text("Video call")
                                        .clicked()
                                    {
                                        self.start_call(true);
                                    }
                                },
                            );
                        });
                    });

                // In-chat search: the counter, the stepper, Esc to close.
                if self.model.search_open {
                    ui.horizontal(|ui| {
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut self.model.chat_search)
                                .hint_text("Search this chat")
                                .desired_width(220.0),
                        );
                        if edit.changed() {
                            self.model.search_refresh();
                            self.scrolled_hit = -1;
                        }
                        ui.label(
                            egui::RichText::new(self.model.search_label())
                                .small()
                                .color(self.palette.read),
                        );
                        if ui.button("▲").clicked() {
                            self.model.search_step(false);
                            self.scrolled_hit = -1;
                        }
                        if ui.button("▼").clicked() {
                            self.model.search_step(true);
                            self.scrolled_hit = -1;
                        }
                        if ui.button("✕").clicked() {
                            self.model.search_open = false;
                            self.model.chat_search.clear();
                            self.model.search_hit = -1;
                        }
                    });
                }
                ui.separator();

                // Ask for photos and stickers that are not on disk yet (one
                // request each). Only the newest few: older bubbles were
                // requested when they were on screen.
                let wanted: Vec<String> = row
                    .messages
                    .iter()
                    .rev()
                    .filter(|message| {
                        matches!(
                            message.kind.as_str(),
                            "image" | "video" | "sticker" | "audio"
                        )
                    })
                    .take(24)
                    .map(|message| message.id.clone())
                    .collect();
                diag(&format!(
                    "chat {jid} messages {} media wanted {}",
                    row.messages.len(),
                    wanted.len()
                ));
                // Videos already on disk but without a poster: ask for the
                // frame once, so the bubble stops showing "Loading".
                for message in &row.messages {
                    if message.kind != "video" {
                        continue;
                    }
                    let poster = self.model.poster_of(&message.id);
                    let media = self.model.media_path(&message.id);
                    diag(&format!(
                        "video {} poster {} media {}",
                        message.id,
                        poster.is_some(),
                        media.is_some()
                    ));
                    if poster.is_some() {
                        continue;
                    }
                    let Some(path) = media else {
                        continue;
                    };
                    if self.posters_asked.insert(message.id.clone()) {
                        self.send(Command::VideoPoster {
                            id: message.id.clone(),
                            path,
                        });
                    }
                }
                for id in wanted {
                    if self.model.request_media(&id) {
                        diag(&format!("request media {id}"));
                        self.send(Command::FetchMedia { id });
                    }
                }

                // What the search marks, and which hit to bring into view.
                let hits: Vec<usize> = if self.model.search_open {
                    self.model.search_matches()
                } else {
                    Vec::new()
                };
                let current = self.model.search_current();
                let query = self.model.chat_search.clone();
                // A search hit frames its bubble; `WA_DESK_SCROLL_TO` frames any index the
                // same way, once, for a capture.
                let target = self.scroll_target.take();
                // The list pins itself to the newest message every frame, which undid the
                // jump; skip that pin for the one frame that frames the requested message.
                self.suppress_bottom = target.is_some();
                let focus = current
                    .filter(|_| self.scrolled_hit != self.model.search_hit)
                    .or(target);

                // The composer is painted *after* the list, so its real height
                // is measured for the next frame instead of guessed: a fixed
                // 64 px reserve clipped the field's bottom edge against the
                // window border as soon as the banner or the recording row made
                // the bar taller.
                let composer_height: f32 = ctx.data(|data| {
                    data.get_temp(egui::Id::unique("composer-height"))
                        .unwrap_or(64.0)
                });
                let height = (ui.available_height() - composer_height - 10.0).max(120.0);
                // WhatsApp anchors a short conversation to the composer; the
                // previous frame's content height tells us how much to pad.
                // Keyed by chat: a *global* height padded the new chat with the
                // previous one's for a frame, so the list jumped on every click.
                let height_key = egui::Id::unique(("chat-content-height", jid.as_str()));
                let content_height: f32 = ctx.data(|data| data.get_temp(height_key).unwrap_or(0.0));
                let spare = (height - content_height).max(0.0);
                let mut chosen: Option<BubbleAction> = None;
                let playback = self.audio.state();
                let mut list = egui::ScrollArea::vertical()
                    .stick_to_bottom(focus.is_none())
                    .auto_shrink([false, false])
                    .max_height(height.max(120.0));
                // A freshly opened chat (or a message we just sent) must be
                // visible, even when the history is far taller than the window:
                // `stick_to_bottom` alone only holds once the view is there, so
                // the content asks egui to scroll to its own bottom.
                let want_bottom = self.model.scroll_bottom > 0;
                if want_bottom {
                    self.model.scroll_bottom -= 1;
                    ui.ctx().request_repaint();
                }
                // `WA_DESK_SCROLL_TOP=1` parks the view at the first message so
                // the floating "newest message" disc can be checked.
                if self.preview_scroll_top {
                    list = list.vertical_scroll_offset(0.0);
                }
                let list_output = list.show(ui, |ui| {
                    let ctx = ui.ctx().clone();
                    // Breathing room from the window edge: the official
                    // list never lets a bubble touch the panel border. A
                    // frame margin insets *both* sides, which `add_space`
                    // cannot do for a right-aligned bubble.
                    egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(20, 0))
                        .show(ui, |ui| {
                            let mut index = 0usize;
                            let now = self.model.now;
                            while index < row.messages.len() {
                                let message = &row.messages[index];
                                // The official client separates days with a chip.
                                if Model::starts_new_day(&row.messages, index, now) {
                                    day_chip(
                                        ui,
                                        &palette,
                                        &wa_ui::labels::day_of(message.timestamp, now),
                                    );
                                }
                                // ...and marks where the unread block began.
                                if self
                                    .model
                                    .unread_anchor
                                    .get(&row.jid)
                                    .is_some_and(|id| id == &message.id)
                                {
                                    unread_divider(ui, &palette);
                                }
                                let is_hit = hits.contains(&index);
                                let head = Model::starts_run(&row.messages, index);
                                let avatar = if row.jid.ends_with("@g.us")
                                    && !message.from_me
                                    && head
                                    && let Some(sender) = message.sender.as_deref()
                                {
                                    self.ensure_avatar(ui.ctx(), sender);
                                    self.avatar_for(sender).cloned()
                                } else {
                                    None
                                };
                                // A burst of photos/videos from one sender paints as the official
                                // album grid, one bubble with one caption and one clock.
                                let span = Model::album_span(&row.messages, index);
                                // `WA_DESK_DIAG=1`: log every media index with the
                                // span the album rule produced, so a run that should
                                // group but does not can be traced.
                                if (message.kind == "image" || message.kind == "video")
                                    && crate::app::diag_enabled()
                                {
                                    let mut note = format!(
                                        "media {index} span {span} total {}: {} sender={:?} ts={} me={}",
                                        row.messages.len(),
                                        message.kind,
                                        message.sender,
                                        message.timestamp,
                                        message.from_me
                                    );
                                    for look in index..(index + 3).min(row.messages.len()) {
                                        let item = &row.messages[look];
                                        note.push_str(&format!(
                                            " || {look} {} s={:?} ts={} me={}",
                                            item.kind, item.sender, item.timestamp, item.from_me
                                        ));
                                    }
                                    diag(&note);
                                }
                                if span > 1 {
                                    diag(&format!("album draw at {index} with {span} items"));
                                    let items: Vec<&wa_types::MessageView> =
                                        row.messages[index..index + span].iter().collect();
                                    let tail = Model::ends_run(&row.messages, index + span - 1);
                                    if let Some(action) = crate::bubbles::album_bubble(
                                        ui,
                                        &palette,
                                        &self.model,
                                        &items,
                                        &mut self.textures,
                                        &ctx,
                                        (ui.available_width() * 0.66).max(160.0),
                                        tail,
                                        focus == Some(index),
                                    ) {
                                        chosen = Some(action);
                                    }
                                    index += span;
                                    continue;
                                }
                                if let Some(action) = bubble(
                                    ui,
                                    &palette,
                                    &self.model,
                                    message,
                                    &mut self.textures,
                                    &ctx,
                                    if is_hit { Some(query.as_str()) } else { None },
                                    focus == Some(index),
                                    &playback,
                                    Model::ends_run(&row.messages, index),
                                    avatar.as_ref(),
                                    head,
                                ) {
                                    chosen = Some(action);
                                }
                                index += 1;
                            }
                            ctx.data_mut(|data| {
                                data.insert_temp(
                                    height_key,
                                    (ui.min_rect().height() - spare).max(0.0),
                                );
                            });
                        });
                    // Ask now that the content has a height; egui applies it at
                    // the end of this frame.
                    if want_bottom && !self.preview_scroll_top && !self.suppress_bottom {
                        ui.scroll_to_rect(ui.min_rect(), Some(egui::Align::BOTTOM));
                    }
                });
                // The floating "jump to the newest message" disc the official
                // client shows once you scroll up in a long history: a 44 px
                // circle with a chevron, just above the composer.
                let viewport = list_output.inner_rect;
                let max_offset = (list_output.content_size.y - viewport.height()).max(0.0);
                if max_offset > 1.0 && list_output.state.clamped_offset().y < max_offset - 2.0 {
                    let center = egui::pos2(viewport.right() - 42.0, viewport.bottom() - 30.0);
                    let rect = egui::Rect::from_center_size(center, egui::vec2(44.0, 44.0));
                    let response = ui
                        .interact(
                            rect,
                            egui::Id::unique("scroll-to-latest"),
                            egui::Sense::click(),
                        )
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text("Scroll to the latest message");
                    let fill = if response.hovered() {
                        palette.hover
                    } else {
                        palette.elevated
                    };
                    let painter = ui.painter();
                    painter.circle_filled(center, 22.0, fill);
                    painter.circle_stroke(center, 21.5, egui::Stroke::new(1.0_f32, palette.border));
                    // The icon table has no "expand more", and two strokes stay
                    // crisp at any scale.
                    let chevron = egui::Stroke::new(2.0_f32, palette.text);
                    painter.line_segment(
                        [
                            center + egui::vec2(-6.0, -3.0),
                            center + egui::vec2(0.0, 3.0),
                        ],
                        chevron,
                    );
                    painter.line_segment(
                        [
                            center + egui::vec2(0.0, 3.0),
                            center + egui::vec2(6.0, -3.0),
                        ],
                        chevron,
                    );
                    if response.clicked() {
                        self.model.scroll_bottom = 3;
                        ui.ctx().request_repaint();
                    }
                }
                if let Some(action) = chosen {
                    self.run_action(&row, action);
                }
                if focus.is_some() {
                    self.scrolled_hit = self.model.search_hit;
                }

                // Esc leaves the search bar, like WhatsApp Web.
                if self.model.search_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.model.search_open = false;
                    self.model.chat_search.clear();
                    self.model.search_hit = -1;
                }

                let composer_top = ui.cursor().top();
                ui.separator();

                // The reply / edit card above the composer.
                self.composer_banner(ui);

                egui::Frame::new()
                    .fill(palette.panel)
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .show(ui, |ui| {
                        if let Some((note, at)) = self.attach_note.clone() {
                            if at.elapsed() > std::time::Duration::from_secs(8) {
                                self.attach_note = None;
                            } else {
                                ui.label(
                                    egui::RichText::new(note)
                                        .size(11.5)
                                        .color(palette.secondary),
                                );
                            }
                        }
                        // Ctrl+V with a file on the OS clipboard attaches it.
                        let paste_files = ui.ctx().input(|input| {
                            input.modifiers.command && input.key_pressed(egui::Key::V)
                        });
                        if paste_files && let Some(path) = crate::clipboard::first_file() {
                            let text = self.model.composer.trim().to_string();
                            let caption = (!text.is_empty()).then_some(text);
                            if caption.is_some() {
                                self.model.composer.clear();
                            }
                            self.send_document(&jid, path, caption);
                        }
                        ui.horizontal(|ui| {
                            // Official composer: attach on the left, then the
                            // rounded field with emoji + stickers inside it on
                            // the right, then the green send button.
                            if icon_button(ui, Icon::Attach, &palette)
                                .on_hover_text("Attach")
                                .clicked()
                            {
                                self.show_attach = !self.show_attach;
                            }
                            let send_width = 46.0;
                            let input_width = ui.available_width() - send_width;
                            let field_width = (input_width - 98.0).max(80.0);
                            // The official field is 38 px tall inside a 62 px bar.
                            let field_height = 38.0;
                            let mut edit_response = None;
                            // The single-line field consumes the Enter event
                            // (`consume_key`) while it surrenders focus, so the
                            // key has to be sampled before the widget is drawn.
                            let enter_pressed =
                                ui.input(|input| input.key_pressed(egui::Key::Enter));
                            egui::Frame::new()
                                .fill(palette.raised)
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 6))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        edit_response = Some(
                                            ui.add_sized(
                                                [field_width, field_height],
                                                egui::TextEdit::singleline(
                                                    &mut self.model.composer,
                                                )
                                                .id(egui::Id::unique("composer-input"))
                                                .frame(egui::Frame::NONE)
                                                .hint_text("Type a message"),
                                            ),
                                        );
                                        if icon_button(ui, Icon::Emoji, &palette)
                                            .on_hover_text("Emoji")
                                            .clicked()
                                        {
                                            self.model.emoji_open = !self.model.emoji_open;
                                            self.model.stickers_open = false;
                                        }
                                    });
                                });
                            let edit = edit_response.expect("the composer field is created above");
                            if edit.changed() {
                                let typing = !self.model.composer.trim().is_empty();
                                if typing != self.typing_sent {
                                    self.typing_sent = typing;
                                    self.send(Command::Typing {
                                        chat: jid.clone(),
                                        typing,
                                    });
                                }
                            }
                            // `has_focus` covers the frame before the field
                            // surrenders focus, `lost_focus` the one after it.
                            let enter =
                                enter_sends(enter_pressed, edit.has_focus() || edit.lost_focus());
                            // The official rule: an empty draft shows the
                            // microphone, a typed one shows send.
                            let empty_draft = self.model.composer.trim().is_empty()
                                && self.model.reply_to.is_none()
                                && self.model.editing.is_none();
                            if empty_draft
                                && self.voice.is_none()
                                && icon_button(ui, Icon::Mic, &palette)
                                    .on_hover_text("Record a voice note")
                                    .clicked()
                            {
                                match crate::voicenote::Recording::start() {
                                    Ok(recording) => self.voice = Some(recording),
                                    Err(err) => {
                                        self.notice = Some((err, std::time::Instant::now()))
                                    }
                                }
                            }
                            if let Some(recording) = self.voice.as_ref() {
                                // While recording: the elapsed time, a cancel
                                // button and the send button.
                                let seconds = recording.seconds();
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{:0}:{:02}",
                                        (seconds as u32) / 60,
                                        (seconds as u32) % 60
                                    ))
                                    .size(13.0)
                                    .color(palette.danger),
                                );
                                if icon_button(ui, Icon::Close, &palette)
                                    .on_hover_text("Cancel the recording")
                                    .clicked()
                                {
                                    self.voice = None;
                                }
                                if icon_button(ui, Icon::Send, &palette)
                                    .on_hover_text("Send the voice note")
                                    .clicked()
                                    && let Some(recording) = self.voice.take()
                                {
                                    match recording.finish() {
                                        Ok(note) => {
                                            use base64::Engine as _;
                                            let payload = base64::engine::general_purpose::STANDARD
                                                .encode(&note.ogg);
                                            self.send(Command::SendVoiceNote {
                                                chat: jid.clone(),
                                                payload,
                                                seconds: note.seconds,
                                                waveform: note.waveform,
                                            });
                                        }
                                        Err(err) => {
                                            self.notice = Some((err, std::time::Instant::now()))
                                        }
                                    }
                                }
                            } else if icon_button(ui, Icon::Send, &palette)
                                .on_hover_text(self.model.send_label())
                                .clicked()
                                || (enter && !empty_draft)
                            {
                                let text = self.model.composer.trim().to_string();
                                if !text.is_empty() {
                                    if let Some(editing) = self.model.editing.take() {
                                        self.send(Command::EditMessage {
                                            chat: jid.clone(),
                                            id: editing.id,
                                            text,
                                        });
                                    } else if let Some(reply) = self.model.reply_to.take() {
                                        let quoted = if reply.text.trim().is_empty() {
                                            None
                                        } else {
                                            Some(reply.text.clone())
                                        };
                                        // WhatsApp does not echo our own
                                        // messages back, so the bubble has to
                                        // exist before the stanza leaves.
                                        self.model.add_own_text(&jid, &text, Some(&reply));
                                        self.send(Command::SendReply {
                                            chat: jid.clone(),
                                            text,
                                            reply_id: reply.id,
                                            quoted,
                                        });
                                    } else {
                                        self.model.add_own_text(&jid, &text, None);
                                        self.send(Command::SendText {
                                            chat: jid.clone(),
                                            text,
                                        });
                                    }
                                    self.model.composer.clear();
                                    self.send(Command::Typing {
                                        chat: jid.clone(),
                                        typing: false,
                                    });
                                    self.typing_sent = false;
                                }
                            }
                        });
                        ctx.data_mut(|data| {
                            data.insert_temp(
                                egui::Id::unique("composer-height"),
                                (ui.min_rect().bottom() - composer_top).max(0.0),
                            );
                        });
                    });
            });
    }

    /// The reply / edit card above the composer, with its cancel button.
    pub(crate) fn composer_banner(&mut self, ui: &mut egui::Ui) {
        if let Some(preview) = self.model.reply_preview() {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(32, 34, 34))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new(preview.name)
                                    .small()
                                    .color(egui::Color32::from_rgb(83, 189, 235)),
                            );
                            ui.label(egui::RichText::new(preview.preview).small().weak());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                self.model.reply_to = None;
                            }
                        });
                    });
                });
            return;
        }
        if let Some(editing) = self.model.editing.clone() {
            egui::Frame::new()
                .fill(egui::Color32::from_rgb(32, 34, 34))
                .inner_margin(egui::Margin::symmetric(8, 4))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Editing message")
                                    .small()
                                    .color(egui::Color32::from_rgb(240, 200, 60)),
                            );
                            ui.label(egui::RichText::new(&editing.text).small().weak());
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✕").clicked() {
                                self.model.cancel_composer_mode();
                            }
                        });
                    });
                });
        }
    }

    /// The poll composer: a question and three option rows, like the official
    /// dialog (more options can be added later; WhatsApp needs two).
    pub(crate) fn poll_window(&mut self, ctx: &egui::Context) {
        if self.poll_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.content_rect();
        let mut send = false;
        let mut close = false;
        let draft = self.poll_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "poll-dialog",
            360.0,
            "Create poll",
            &palette,
            viewport,
            |ui| {
                ui.label(
                    egui::RichText::new("QUESTION")
                        .size(11.5)
                        .color(palette.accent),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut draft.question)
                        .hint_text("Ask a question")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("OPTIONS")
                        .size(11.5)
                        .color(palette.accent),
                );
                for option in &mut draft.options {
                    ui.add(
                        egui::TextEdit::singleline(option)
                            .hint_text("Option")
                            .desired_width(f32::INFINITY),
                    );
                }
                ui.add_space(8.0);
                ui.checkbox(&mut draft.multiple, "Allow multiple answers");
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Send", true).clicked() {
                        send = true;
                    }
                });
            },
        );
        if send {
            let draft = self.poll_draft.clone().expect("draft is open");
            let options: Vec<String> = draft
                .options
                .into_iter()
                .map(|option| option.trim().to_string())
                .filter(|option| !option.is_empty())
                .collect();
            if let Some(chat) = self.model.active.clone() {
                if options.len() >= 2 && !draft.question.trim().is_empty() {
                    self.send(Command::SendPoll {
                        chat,
                        question: draft.question.trim().to_string(),
                        options,
                        multiple: draft.multiple,
                    });
                } else {
                    self.notice = Some((
                        "A poll needs a question and two options".to_string(),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.poll_draft = None;
        } else if close {
            self.poll_draft = None;
        }
    }

    /// The contact-card composer: a name and a number.
    pub(crate) fn contact_window(&mut self, ctx: &egui::Context) {
        if self.contact_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.content_rect();
        let mut send = false;
        let mut close = false;
        let draft = self.contact_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "contact-dialog",
            340.0,
            "Share a contact",
            &palette,
            viewport,
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .hint_text("Name")
                        .desired_width(f32::INFINITY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut draft.phone)
                        .hint_text("Phone number")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Send", true).clicked() {
                        send = true;
                    }
                });
            },
        );
        if send {
            let draft = self.contact_draft.clone().expect("draft is open");
            if let Some(chat) = self.model.active.clone() {
                self.send(Command::SendContact {
                    chat,
                    name: draft.name.trim().to_string(),
                    phone: draft.phone.trim().to_string(),
                });
            }
            self.contact_draft = None;
        } else if close {
            self.contact_draft = None;
        }
    }

    /// The location composer: a place name and the coordinates.
    pub(crate) fn location_window(&mut self, ctx: &egui::Context) {
        if self.location_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.content_rect();
        let mut send = false;
        let mut close = false;
        let draft = self.location_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "location-dialog",
            340.0,
            "Share a location",
            &palette,
            viewport,
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut draft.name)
                        .hint_text("Place name")
                        .desired_width(f32::INFINITY),
                );
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.lat)
                            .hint_text("Latitude")
                            .desired_width(120.0),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut draft.lng)
                            .hint_text("Longitude")
                            .desired_width(120.0),
                    );
                });
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Send", true).clicked() {
                        send = true;
                    }
                });
            },
        );
        if send {
            let draft = self.location_draft.clone().expect("draft is open");
            let lat = draft.lat.trim().parse::<f64>().unwrap_or(0.0);
            let lng = draft.lng.trim().parse::<f64>().unwrap_or(0.0);
            if let Some(chat) = self.model.active.clone() {
                if lat != 0.0 || lng != 0.0 {
                    self.send(Command::SendLocation {
                        chat,
                        lat,
                        lng,
                        name: draft.name.trim().to_string(),
                    });
                } else {
                    self.notice = Some((
                        "Type a latitude and longitude".to_string(),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.location_draft = None;
        } else if close {
            self.location_draft = None;
        }
    }

    /// The "New list" dialog: a name, and the open chat joins it.
    pub(crate) fn list_window(&mut self, ctx: &egui::Context) {
        if self.list_draft.is_none() {
            return;
        }
        let palette = self.palette.clone();
        let viewport = ctx.content_rect();
        let mut create = false;
        let mut close = false;
        let draft = self.list_draft.as_mut().expect("draft is open");
        dialog_card(
            ctx,
            "list-dialog",
            320.0,
            "New list",
            &palette,
            viewport,
            |ui| {
                ui.add(
                    egui::TextEdit::singleline(draft)
                        .hint_text("List name")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new("The open chat joins the list; more can be added later.")
                        .size(12.0)
                        .color(palette.secondary),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if dialog_button(ui, &palette, "Cancel", false).clicked() {
                        close = true;
                    }
                    if dialog_button(ui, &palette, "Create", true).clicked() {
                        create = true;
                    }
                });
            },
        );
        if create {
            let name = self.list_draft.clone().unwrap_or_default();
            let chat = self.model.active.clone();
            match self.model.create_list(&name, chat.as_deref()) {
                Some(id) => {
                    self.model.chat_filter = crate::model::ChatFilter::List(id);
                }
                None => {
                    self.notice = Some((
                        "The list needs a name".to_string(),
                        std::time::Instant::now(),
                    ));
                }
            }
            self.list_draft = None;
        } else if close {
            self.list_draft = None;
        }
    }
}
