//! The menus and dialogs: the sidebar kebab, the chat's ⋮ menu, the attach menu,
//! the new-chat/contact/group pickers, the starred list, the forward picker and
//! the delete question.

use crate::app::{
    ContactDraft, Icon, LocationDraft, PollDraft, WaDesk, draw_icon, forward_checkbox,
};
use crate::bubbles::menu_row;
use crate::model::Command;
use crate::widgets::fit_text;

impl WaDesk {
    /// The Chats kebab menu: a small card with the actions the official client
    /// keeps there.
    pub(crate) fn chat_menu(&mut self, ctx: &egui::Context) {
        if !self.show_chat_menu {
            return;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.show_chat_menu = false;
            return;
        }
        let palette = self.palette.clone();
        egui::Window::new("chats-menu")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(58.0 + 300.0, 54.0))
            .frame(
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .inner_margin(egui::Margin::symmetric(6, 6)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(230.0);
                // The official Chats menu: New group, Starred messages, Select
                // chats, Mark all as read, then App lock and Log out.
                if menu_row(ui, &palette, Icon::GroupAdd, "New group") {
                    self.show_new_group = true;
                    self.send(Command::Contacts);
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Star, "Starred messages") {
                    self.show_starred = true;
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::SelectChats, "Select chats") {
                    self.notice = Some((
                        "Selecting several chats is not supported yet.".to_string(),
                        std::time::Instant::now(),
                    ));
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::MarkRead, "Mark all as read") {
                    let unread: Vec<String> = self
                        .model
                        .chats
                        .values()
                        .filter(|row| row.unread > 0)
                        .map(|row| row.jid.clone())
                        .collect();
                    for chat in unread {
                        self.send(Command::MarkRead { chat });
                    }
                    self.show_chat_menu = false;
                }
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                if menu_row(ui, &palette, Icon::Lock, "App lock") {
                    self.notice = Some((
                        "App lock is not supported yet.".to_string(),
                        std::time::Instant::now(),
                    ));
                    self.show_chat_menu = false;
                }
                if menu_row(ui, &palette, Icon::Logout, "Log out") {
                    self.send(Command::Relink);
                    self.show_chat_menu = false;
                }
            });
    }

    /// The open chat's actions menu (mute, pin, unread, clear, delete).
    pub(crate) fn chat_actions_menu(&mut self, ctx: &egui::Context) {
        if !self.show_chat_actions {
            return;
        }
        let Some(chat) = self.model.active.clone() else {
            self.show_chat_actions = false;
            return;
        };
        if let Some(action) = self.chat_actions_window(ctx, &chat, chat.ends_with("@g.us")) {
            if action == "info" {
                self.show_group = true;
                if self.model.request_group(&chat) {
                    self.send(Command::GroupInfo { chat: chat.clone() });
                }
            } else if action == "favourite" {
                let favourited = self.model.toggle_favourite(&chat);
                self.notice = Some((
                    if favourited {
                        "Added to favourites".to_string()
                    } else {
                        "Removed from favourites".to_string()
                    },
                    std::time::Instant::now(),
                ));
            } else if action == "import" {
                self.import_chat_export(&chat);
            } else if action == "voice" || action == "video" {
                self.start_call(action == "video");
            } else {
                self.send(Command::ChatAction {
                    chat: chat.clone(),
                    action,
                });
            }
            self.show_chat_actions = false;
        }
    }

    pub(crate) fn chat_actions_window(
        &mut self,
        ctx: &egui::Context,
        chat: &str,
        is_group: bool,
    ) -> Option<String> {
        let palette = self.palette.clone();
        let mut chosen = None;
        egui::Window::new("chat-actions")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(ctx.content_rect().width() - 300.0, 62.0))
            .frame(
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .inner_margin(egui::Margin::symmetric(6, 6)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(220.0);
                if is_group && menu_row(ui, &palette, Icon::Groups, "Group info") {
                    chosen = Some("info".to_string());
                }
                // The official menu leads with the calls.
                if !is_group {
                    if menu_row(ui, &palette, Icon::Phone, "Voice call") {
                        chosen = Some("voice".to_string());
                    }
                    if menu_row(ui, &palette, Icon::VideoCall, "Video call") {
                        chosen = Some("video".to_string());
                    }
                }
                // Official order: favourite first, then the mutes and the rest.
                let favourite_label = if self.model.is_favourite(chat) {
                    "Remove from favourites"
                } else {
                    "Add to favourites"
                };
                if menu_row(ui, &palette, Icon::Star, favourite_label) {
                    chosen = Some("favourite".to_string());
                }
                for (label, action) in [
                    ("Mute for 8 hours", "mute8h"),
                    ("Mute for 1 week", "mute1w"),
                    ("Unmute", "unmute"),
                    ("Pin chat", "pin"),
                    ("Unpin chat", "unpin"),
                    ("Mark as unread", "unread"),
                    ("Archive chat", "archive"),
                    ("Clear chat", "clear"),
                    ("Delete chat", "delete"),
                    ("Import chat export…", "import"),
                ] {
                    if menu_row(ui, &palette, Icon::Document, label) {
                        chosen = Some(action.to_string());
                    }
                }
            });
        let _ = chat;
        chosen
    }

    /// The attach menu: the official entries (Photos & videos, Camera,
    /// Document, Audio, Poll, Location, Contact). The ones this client can send
    /// open the file picker; the rest say where they live.
    pub(crate) fn attach_window(&mut self, ctx: &egui::Context) {
        if !self.show_attach {
            return;
        }
        let palette = self.palette.clone();
        let mut chosen: Option<&'static str> = None;
        egui::Window::new("attach-menu")
            .title_bar(false)
            .resizable(false)
            .fixed_pos(egui::pos2(
                ctx.content_rect().left() + 74.0,
                ctx.content_rect().bottom() - 470.0,
            ))
            .frame(
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(12))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 8],
                        blur: 24,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(120),
                    })
                    .inner_margin(egui::Margin::symmetric(3, 6)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(228.0);
                // Order sampled from the live client's attach menu on
                // 2026-11-05: Photos & videos, Camera, Document, Audio,
                // Contact, Poll. Location is this client's extra, kept last.
                for (icon, label, action) in [
                    (Icon::Document, "Photos & videos", "photos"),
                    (Icon::VideoCall, "Camera", "camera"),
                    (Icon::Document, "Document", "document"),
                    (Icon::Volume, "Audio", "audio"),
                    (Icon::PersonAdd, "Contact", "contact"),
                    (Icon::SelectChats, "Poll", "poll"),
                    (Icon::Status, "Location", "location"),
                ] {
                    if menu_row(ui, &palette, icon, label) {
                        chosen = Some(action);
                    }
                }
            });
        let Some(action) = chosen else { return };
        self.show_attach = false;
        match action {
            "photos" => self.pick_attachment(),
            "document" => self.pick_attachment(),
            "audio" => self.pick_attachment(),
            "camera" => {
                self.notice = Some((
                    "The camera capture is not in this build".to_string(),
                    std::time::Instant::now(),
                ));
            }
            "poll" => {
                self.poll_draft = Some(PollDraft {
                    question: String::new(),
                    options: [String::new(), String::new(), String::new()],
                    multiple: false,
                });
            }
            "contact" => {
                self.contact_draft = Some(ContactDraft {
                    name: String::new(),
                    phone: String::new(),
                });
            }
            "location" => {
                self.location_draft = Some(LocationDraft {
                    name: String::new(),
                    lat: String::new(),
                    lng: String::new(),
                });
            }
            _ => {}
        }
    }

    /// The new-chat picker: the address book with a search field.
    pub(crate) fn new_chat_window(&mut self, ctx: &egui::Context) {
        if !self.show_new_chat {
            return;
        }
        let palette = self.palette.clone();
        let mut open = true;
        egui::Window::new("New chat")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                // The official picker leads with the two "create" rows.
                ui.horizontal(|ui| {
                    if menu_row(ui, &palette, Icon::GroupAdd, "New group") {
                        self.show_new_group = true;
                        self.send(Command::Contacts);
                        self.show_new_chat = false;
                    }
                    if menu_row(ui, &palette, Icon::PersonAdd, "New contact") {
                        self.show_new_contact = true;
                        self.show_new_chat = false;
                    }
                });
                ui.separator();
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_chat_search)
                        .hint_text("Search contacts")
                        .desired_width(f32::INFINITY),
                );
                let needle = self.new_chat_search.trim().to_lowercase();
                let rows: Vec<(String, String)> = self
                    .contacts
                    .iter()
                    .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
                    .take(80)
                    .cloned()
                    .collect();
                if rows.is_empty() {
                    ui.label(
                        egui::RichText::new("No contacts yet - sync the address book first")
                            .small()
                            .weak(),
                    );
                }
                egui::ScrollArea::vertical()
                    .max_height(380.0)
                    .show(ui, |ui| {
                        for (jid, name) in rows {
                            if ui
                                .add(egui::Button::new(name).frame(false))
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                self.model.open_chat(&jid);
                                self.send(Command::MarkRead { chat: jid.clone() });
                                self.send(Command::ChatHistory { chat: jid });
                                self.show_new_chat = false;
                            }
                        }
                    });
            });
        let _ = palette;
        if !open {
            self.show_new_chat = false;
        }
    }

    /// The starred-messages screen, gathered across chats.
    pub(crate) fn starred_window(&mut self, ctx: &egui::Context) {
        if !self.show_starred {
            return;
        }
        let palette = self.palette.clone();
        let rows = self.model.starred_messages();
        let mut open = true;
        egui::Window::new("Starred messages")
            .open(&mut open)
            .resizable(true)
            .default_width(420.0)
            .show(ctx, |ui| {
                if rows.is_empty() {
                    ui.label(egui::RichText::new("No starred messages").weak());
                    return;
                }
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        for (chat, message) in &rows {
                            let name = self
                                .model
                                .chats
                                .get(chat)
                                .map(|row| self.model.title(row))
                                .unwrap_or_else(|| chat.clone());
                            let body = self.model.bubble_body(message);
                            let label = format!(
                                "{name}  ·  {}\n{}",
                                self.model.bubble_stamp(message),
                                body
                            );
                            if ui.add(egui::Button::new(label).frame(false)).clicked() {
                                self.model.open_chat(chat);
                                self.send(Command::MarkRead { chat: chat.clone() });
                                self.show_starred = false;
                            }
                            ui.separator();
                        }
                    });
            });
        let _ = palette;
        if !open {
            self.show_starred = false;
        }
    }

    /// Create a group: a name and a contact checklist.
    pub(crate) fn new_group_window(&mut self, ctx: &egui::Context) {
        if !self.show_new_group {
            return;
        }
        let mut open = true;
        let contacts = self.contacts.clone();
        egui::Window::new("New group")
            .open(&mut open)
            .resizable(false)
            .default_width(380.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_group_subject)
                        .hint_text("Group name")
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} selected", self.new_group_members.len()))
                        .small()
                        .weak(),
                );
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (jid, name) in contacts {
                            let mut checked = self.new_group_members.contains(&jid);
                            if ui.checkbox(&mut checked, name).changed() {
                                if checked {
                                    self.new_group_members.insert(jid);
                                } else {
                                    self.new_group_members.remove(&jid);
                                }
                            }
                        }
                    });
                let ready =
                    !self.new_group_subject.trim().is_empty() && !self.new_group_members.is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new("Create group"))
                    .clicked()
                {
                    self.send(Command::CreateGroup {
                        subject: self.new_group_subject.trim().to_string(),
                        members: self.new_group_members.iter().cloned().collect(),
                    });
                    self.new_group_subject.clear();
                    self.new_group_members.clear();
                    self.show_new_group = false;
                }
            });
        if !open {
            self.show_new_group = false;
        }
    }

    /// Add a saved contact (name + international number).
    pub(crate) fn new_contact_window(&mut self, ctx: &egui::Context) {
        if !self.show_new_contact {
            return;
        }
        let mut open = true;
        egui::Window::new("New contact")
            .open(&mut open)
            .resizable(false)
            .default_width(320.0)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_contact_name)
                        .hint_text("Name")
                        .desired_width(f32::INFINITY),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.new_contact_phone)
                        .hint_text("628123456789")
                        .desired_width(f32::INFINITY),
                );
                let ready = !self.new_contact_name.trim().is_empty()
                    && !self.new_contact_phone.trim().is_empty();
                if ui.add_enabled(ready, egui::Button::new("Save")).clicked() {
                    self.send(Command::NewContact {
                        phone: self.new_contact_phone.trim().to_string(),
                        name: self.new_contact_name.trim().to_string(),
                    });
                    self.new_contact_phone.clear();
                    self.new_contact_name.clear();
                    self.show_new_contact = false;
                }
            });
        if !open {
            self.show_new_contact = false;
        }
    }

    /// Add members to the open group.
    pub(crate) fn group_add_window(&mut self, ctx: &egui::Context) {
        if !self.group_add_open {
            return;
        }
        let mut open = true;
        let contacts = self.contacts.clone();
        let chat = self.model.active.clone().unwrap_or_default();
        egui::Window::new("Add members")
            .open(&mut open)
            .resizable(false)
            .default_width(360.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for (jid, name) in contacts {
                            let mut checked = self.group_add_members.contains(&jid);
                            if ui.checkbox(&mut checked, name).changed() {
                                if checked {
                                    self.group_add_members.insert(jid);
                                } else {
                                    self.group_add_members.remove(&jid);
                                }
                            }
                        }
                    });
                let count = self.group_add_members.len();
                if ui
                    .add_enabled(
                        count > 0,
                        egui::Button::new(format!("Add {count} member(s)")),
                    )
                    .clicked()
                {
                    let members: Vec<String> = self.group_add_members.iter().cloned().collect();
                    for member in members {
                        self.send(Command::GroupMember {
                            chat: chat.clone(),
                            member,
                            action: "add".into(),
                        });
                    }
                    self.group_add_members.clear();
                    self.group_add_open = false;
                }
            });
        if !open {
            self.group_add_open = false;
        }
    }

    /// The delete dialog: for me, and for everyone when `wa-ui` allows it.
    pub(crate) fn delete_window(&mut self, ctx: &egui::Context) {
        if self.model.delete_target.is_none() {
            return;
        }
        let (title, can_everyone) = self.model.delete_question();
        let target = self.model.delete_target.clone();
        let mut open = true;
        egui::Window::new("Delete message")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.label(title);
                let Some(target) = target.clone() else { return };
                if ui.button("Delete for me").clicked() {
                    self.send(Command::DeleteForMe {
                        chat: target.chat.clone(),
                        id: target.id.clone(),
                        from_me: target.from_me,
                        participant: if target.from_me {
                            None
                        } else {
                            target.sender.clone()
                        },
                    });
                    self.model.remove_message(&target.chat, &target.id);
                    self.model.delete_target = None;
                }
                if can_everyone && ui.button("Delete for everyone").clicked() {
                    self.send(Command::DeleteForEveryone {
                        chat: target.chat.clone(),
                        id: target.id.clone(),
                    });
                    self.model.delete_target = None;
                }
                if ui.button("Cancel").clicked() {
                    self.model.delete_target = None;
                }
            });
        if !open {
            self.model.delete_target = None;
        }
    }

    /// The forward picker, in the official shape: a floating card with the close
    /// control and the title on top, the send action pinned right, a rounded
    /// search field, the "My status" row, and the recent chats with a square
    /// checkbox each.
    pub(crate) fn forward_window(&mut self, ctx: &egui::Context) {
        let Some(target) = self.model.forward_target.clone() else {
            return;
        };
        // Snapshot the mutable bits: the card paints and collects actions, the
        // model is only touched once the card is closed.
        let palette = self.palette.clone();
        let mut search = self.model.forward_search.clone();
        let mut chosen = self.model.forward_chosen.clone();
        let mut toggle: Option<String> = None;
        let mut send = false;
        let mut close = false;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.model.forward_target = None;
            return;
        }

        let viewport = ctx.content_rect();
        let width = 400.0_f32.min(viewport.width() - 40.0);
        let height = (viewport.height() * 0.74).min(640.0);

        // Avatars are fetched while the rows are collected, so the card itself
        // only paints (a `&mut self` inside the area closure would fight the
        // model borrows).
        let jids: Vec<String> = self
            .model
            .forward_rows()
            .iter()
            .map(|row| row.jid.clone())
            .collect();
        let mut rows: Vec<(String, String, String, Option<egui::TextureHandle>)> = Vec::new();
        for jid in jids {
            self.ensure_avatar(ctx, &jid);
            let (title, preview) = match self.model.chats.get(&jid) {
                Some(row) => (self.model.row_title(row), self.model.row_preview(row)),
                None => (jid.clone(), String::new()),
            };
            let avatar = self.avatar_for(&jid).cloned();
            rows.push((jid, title, preview, avatar));
        }

        egui::Area::new(egui::Id::unique("forward-card"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(
                viewport.center().x - width / 2.0,
                viewport.center().y - height / 2.0,
            ))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .stroke(egui::Stroke::new(1.0_f32, palette.border))
                    .corner_radius(egui::CornerRadius::same(16))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 12],
                        blur: 32,
                        spread: 0,
                        color: egui::Color32::from_black_alpha(150),
                    })
                    .inner_margin(egui::Margin::symmetric(18, 14))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.set_height(height);

                        // Header: close, title, send.
                        ui.horizontal(|ui| {
                            let (rect, response) = ui
                                .allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::click());
                            if response.hovered() {
                                ui.painter()
                                    .circle_filled(rect.center(), 16.0, palette.hover);
                            }
                            draw_icon(
                                ui,
                                egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0)),
                                Icon::Close,
                                palette.text,
                            );
                            if response
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .clicked()
                            {
                                close = true;
                            }
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Forward message to")
                                    .size(16.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let ready = !chosen.is_empty();
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(32.0, 32.0),
                                        egui::Sense::click(),
                                    );
                                    if ready && response.hovered() {
                                        ui.painter().circle_filled(
                                            rect.center(),
                                            16.0,
                                            palette.hover,
                                        );
                                    }
                                    draw_icon(
                                        ui,
                                        egui::Rect::from_center_size(
                                            rect.center(),
                                            egui::vec2(20.0, 20.0),
                                        ),
                                        Icon::Send,
                                        if ready {
                                            palette.accent
                                        } else {
                                            palette.secondary
                                        },
                                    );
                                    if ready
                                        && response
                                            .on_hover_cursor(egui::CursorIcon::PointingHand)
                                            .clicked()
                                    {
                                        send = true;
                                    }
                                },
                            );
                        });
                        ui.add_space(12.0);

                        // The search field: rounded, accent hairline, magnifier
                        // inside like the official box.
                        egui::Frame::new()
                            .fill(palette.panel)
                            .stroke(egui::Stroke::new(1.0_f32, palette.accent))
                            .corner_radius(egui::CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(14, 7))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(18.0, 18.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Search, palette.accent);
                                    ui.add_space(4.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut search)
                                            .frame(egui::Frame::NONE)
                                            .hint_text("Search name, number or @username")
                                            .desired_width(ui.available_width()),
                                    );
                                });
                            });
                        ui.add_space(10.0);

                        // "My status": the official second row. This client
                        // posts statuses from the Status screen, so the row is
                        // shown but not selectable here.
                        let (rect, response) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 58.0),
                            egui::Sense::hover(),
                        );
                        if response.hovered() {
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(10),
                                palette.hover,
                            );
                        }
                        forward_checkbox(ui, &palette, rect.left() + 12.0, rect.center().y, false);
                        let disc = egui::pos2(rect.left() + 52.0, rect.center().y);
                        ui.painter().circle_filled(disc, 20.0, palette.accent);
                        draw_icon(
                            ui,
                            egui::Rect::from_center_size(disc, egui::vec2(20.0, 20.0)),
                            Icon::Status,
                            egui::Color32::from_rgb(11, 20, 26),
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 82.0, rect.center().y - 9.0),
                            egui::Align2::LEFT_CENTER,
                            "My status",
                            egui::FontId::proportional(14.5),
                            palette.text,
                        );
                        ui.painter().text(
                            egui::pos2(rect.left() + 82.0, rect.center().y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            "Contacts excluded",
                            egui::FontId::proportional(12.5),
                            palette.secondary,
                        );

                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new("Recent chats")
                                .size(13.0)
                                .color(palette.secondary),
                        );
                        ui.add_space(4.0);

                        let list_height = (ui.available_height() - 34.0).max(120.0);
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .max_height(list_height)
                            .scroll_bar_visibility(
                                egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                            )
                            .show(ui, |ui| {
                                for (jid, name, preview, avatar) in &rows {
                                    let picked = chosen.contains(jid);
                                    let (rect, response) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 60.0),
                                        egui::Sense::click(),
                                    );
                                    if response.hovered() {
                                        ui.painter().rect_filled(
                                            rect,
                                            egui::CornerRadius::same(10),
                                            palette.hover,
                                        );
                                    }
                                    forward_checkbox(
                                        ui,
                                        &palette,
                                        rect.left() + 12.0,
                                        rect.center().y,
                                        picked,
                                    );
                                    let disc = egui::pos2(rect.left() + 54.0, rect.center().y);
                                    if let Some(texture) = avatar {
                                        ui.painter().image(
                                            texture.id(),
                                            egui::Rect::from_center_size(
                                                disc,
                                                egui::vec2(40.0, 40.0),
                                            ),
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        let clean = name.trim_start_matches('~').trim();
                                        ui.painter().circle_filled(
                                            disc,
                                            20.0,
                                            crate::widgets::avatar_color(clean),
                                        );
                                        ui.painter().text(
                                            disc,
                                            egui::Align2::CENTER_CENTER,
                                            crate::widgets::initials(clean),
                                            egui::FontId::proportional(14.0),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                    ui.painter().text(
                                        egui::pos2(rect.left() + 84.0, rect.center().y - 9.0),
                                        egui::Align2::LEFT_CENTER,
                                        fit_text(
                                            ui.painter(),
                                            name,
                                            egui::FontId::proportional(14.5),
                                            palette.text,
                                            rect.width() - 100.0,
                                        ),
                                        egui::FontId::proportional(14.5),
                                        palette.text,
                                    );
                                    ui.painter().text(
                                        egui::pos2(rect.left() + 84.0, rect.center().y + 10.0),
                                        egui::Align2::LEFT_CENTER,
                                        fit_text(
                                            ui.painter(),
                                            &crate::widgets::single_line(preview),
                                            egui::FontId::proportional(12.5),
                                            palette.secondary,
                                            rect.width() - 100.0,
                                        ),
                                        egui::FontId::proportional(12.5),
                                        palette.secondary,
                                    );
                                    if response
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        toggle = Some(jid.clone());
                                    }
                                }
                            });

                        if let Some(jid) = &toggle {
                            if let Some(index) = chosen.iter().position(|item| item == jid) {
                                chosen.remove(index);
                            } else {
                                chosen.push(jid.clone());
                            }
                        }
                    });
            });

        self.model.forward_search = search;
        self.model.forward_chosen = chosen;
        if send {
            self.send(Command::Forward {
                chats: self.model.forward_chosen.clone(),
                id: target.id.clone(),
            });
            self.model.forward_target = None;
        } else if close {
            self.model.forward_target = None;
        }
    }
}
