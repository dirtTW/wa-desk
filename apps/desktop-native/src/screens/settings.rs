//! The Settings screen: the section list (Profile, Account, Privacy, Chats,
//! Notifications, Keyboard shortcuts, AI assistant, Updates, Help) and every
//! detail page behind it. The widgets they are built from live in
//! `settings_ui.rs`.

use crate::app::{Icon, WaDesk, icon_button};
use crate::model::Command;
use crate::settings_ui::{
    settings_card, settings_field, settings_link, settings_list_row, settings_radio_row,
    settings_row, settings_switch,
};
use crate::updates::UpdateState;
use crate::widgets::paint_avatar;

impl WaDesk {
    /// The left column: account header, the section rows and Log out.
    pub(crate) fn settings_sections(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        let width = (ui.available_width() * 0.42).clamp(230.0, 330.0);
        let column_height = (ui.ctx().screen_rect().bottom() - ui.cursor().top() - 4.0).max(140.0);
        // An explicit-width child: a ScrollArea created straight inside a
        // horizontal layout has no width to give its rows (the row then asked
        // for `available_width() - 10` and egui panicked on -10).
        ui.allocate_ui_with_layout(
            egui::vec2(width, column_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings-sections")
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .max_height(column_height)
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.add_space(20.0);
                        ui.add_space(20.0);
                        let me = self.model.me_jid.clone();
                        self.ensure_avatar(ui.ctx(), &me);
                        let avatar = self.avatar_for(&me).cloned();
                        ui.horizontal(|ui| {
                            ui.add_space(18.0);
                            paint_avatar(ui, &self.model.own_name(), 56.0, avatar.as_ref());
                            ui.add_space(4.0);
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new(self.model.own_name())
                                        .size(17.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.label(
                                    egui::RichText::new(&self.model.status)
                                        .size(12.5)
                                        .color(palette.secondary),
                                );
                            });
                        });
                        ui.add_space(12.0);
                        // The live "You" pane carries a search field and the two profile
                        // links above the section list.
                        ui.horizontal(|ui| {
                            ui.add_space(18.0);
                            if icon_button(ui, Icon::Search, palette).clicked() {
                                self.settings_search_open = !self.settings_search_open;
                            }
                            if self.settings_search_open || !self.settings_search.is_empty() {
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.settings_search)
                                        .hint_text("Search")
                                        .desired_width(width - 80.0),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("Search")
                                        .size(13.5)
                                        .color(palette.secondary),
                                );
                            }
                        });
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.add_space(22.0);
                            if settings_link(ui, palette, "View profile") {
                                self.settings_section = 0;
                            }
                            ui.add_space(14.0);
                            if settings_link(ui, palette, "View about") {
                                self.settings_section = 0;
                            }
                        });
                        ui.add_space(12.0);
                        // The official list, in its order and with its subtitles (sampled
                        // from the "You" drawer of web.whatsapp.com), plus this client's own
                        // AI and Updates pages.
                        let sections = [
                            (Icon::Status, "Profile", "Name, profile picture, status"),
                            (
                                Icon::Lock,
                                "Account",
                                "Security notifications, account info",
                            ),
                            (
                                Icon::SelectChats,
                                "Privacy",
                                "Blocked contacts, disappearing messages",
                            ),
                            (Icon::Chats, "Chats", "Theme, wallpaper, chat settings"),
                            (Icon::Phone, "Notifications", "Messages, groups, sounds"),
                            (
                                Icon::Edit,
                                "Keyboard shortcuts",
                                "Keys this window answers to",
                            ),
                            (Icon::Emoji, "AI assistant", "Auto-reply with your key"),
                            (Icon::Document, "Updates", "Version and releases"),
                            (
                                Icon::Groups,
                                "Help and feedback",
                                "Project, changelog, privacy",
                            ),
                        ];
                        let needle = self.settings_search.trim().to_lowercase();
                        for (index, (icon, title, subtitle)) in sections.iter().enumerate() {
                            if !needle.is_empty()
                                && !title.to_lowercase().contains(&needle)
                                && !subtitle.to_lowercase().contains(&needle)
                            {
                                continue;
                            }
                            let selected = self.settings_section == index;
                            let response = settings_list_row(
                                ui,
                                palette,
                                *icon,
                                title,
                                Some(subtitle),
                                selected,
                                palette.secondary,
                            );
                            if response.clicked() {
                                self.settings_section = index;
                            }
                        }
                        ui.add_space(6.0);
                        let response = settings_list_row(
                            ui,
                            palette,
                            Icon::Logout,
                            "Log out",
                            None,
                            false,
                            palette.danger,
                        );
                        if response.clicked() {
                            self.send(Command::Relink);
                            self.settings_note =
                                "Unlinked - scan the QR with the phone to link again.".to_string();
                        }
                    });
            },
        );
    }

    /// The page title and one-line description at the top of the detail pane.
    pub(crate) fn settings_header(
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
        title: &str,
        subtitle: &str,
    ) {
        ui.label(
            egui::RichText::new(title)
                .size(22.0)
                .strong()
                .color(palette.text),
        );
        ui.add_space(2.0);
        ui.label(
            egui::RichText::new(subtitle)
                .size(13.0)
                .color(palette.secondary),
        );
    }

    /// Profile: how the account appears to everyone else.
    pub(crate) fn profile_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Profile",
            "This is how you appear to your contacts.",
        );
        let me = self.model.me_jid.clone();
        self.ensure_avatar(ui.ctx(), &me);
        let avatar = self.avatar_for(&me).cloned();
        let name = self.model.own_name();
        let status = self.model.status.clone();
        settings_card(ui, palette, "PROFILE", |ui| {
            ui.horizontal(|ui| {
                ui.add_space(14.0);
                paint_avatar(ui, &name, 64.0, avatar.as_ref());
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(&name).size(18.0).color(palette.text));
                    ui.label(
                        egui::RichText::new(&status)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            });
            ui.add_space(6.0);
        });
        settings_card(ui, palette, "NAME", |ui| {
            settings_row(
                ui,
                palette,
                "Your name",
                "Taken from the account's push name; change it on the phone.",
                |ui| {
                    ui.label(
                        egui::RichText::new(&name)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Profile photo",
                "Fetched from WhatsApp and cached beside the session.",
                |ui| {
                    ui.label(
                        egui::RichText::new(if avatar.is_some() {
                            "Photo"
                        } else {
                            "Initials"
                        })
                        .size(13.0)
                        .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "ABOUT", |ui| {
            settings_row(
                ui,
                palette,
                "About",
                "Read-only: the phone owns this field.",
                |ui| {
                    ui.label(egui::RichText::new("—").size(13.0).color(palette.secondary));
                },
            );
        });
    }

    /// Account: the linked identity and what the session is doing.
    pub(crate) fn account_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Account",
            "Security notifications and this linked device.",
        );
        let me = self.model.me_jid.clone();
        let status = self.model.status.clone();
        let chats = self.model.chats.len();
        settings_card(ui, palette, "ACCOUNT INFO", |ui| {
            settings_row(
                ui,
                palette,
                "Phone number",
                "The JID this device is linked to.",
                |ui| {
                    let number = wa_ui::text::format_phone(&wa_ui::text::phone_from_jid(&me));
                    ui.label(
                        egui::RichText::new(number)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Session",
                "Live state of the WhatsApp connection.",
                |ui| {
                    let color = if status.eq_ignore_ascii_case("connected") {
                        palette.accent
                    } else {
                        palette.danger
                    };
                    ui.label(egui::RichText::new(&status).size(13.0).color(color));
                },
            );
            settings_row(
                ui,
                palette,
                "Chats on this device",
                "Conversations stored in the local cache.",
                |ui| {
                    ui.label(
                        egui::RichText::new(chats.to_string())
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "LINKED DEVICES", |ui| {
            settings_row(
                ui,
                palette,
                "This device",
                "The session in this window; the phone owns the full device list.",
                |ui| {
                    ui.label(
                        egui::RichText::new("WA Desk")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Manage devices",
                "Remove a linked device from the phone: \
                 Settings -> Linked devices.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        // The live Account page (sampled 2026-11-05) lists exactly these three:
        // Security notifications, Request account info, How to delete my account.
        settings_card(ui, palette, "ACCOUNT", |ui| {
            settings_row(
                ui,
                palette,
                "Request account info",
                "The phone builds the report; this device can only ask.",
                |ui| {
                    if ui.button("Request").clicked() {
                        self.settings_note =
                            "Requesting account info is done from the phone".to_string();
                    }
                },
            );
            settings_row(
                ui,
                palette,
                "How to delete my account",
                "Deleting is a phone action; this link only explains it.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "SECURITY", |ui| {
            settings_row(
                ui,
                palette,
                "Security notifications",
                "A device is linked from the phone:                  Linked devices -> Link a device.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Two-step verification",
                "Managed by WhatsApp on the phone; not exposed to linked devices.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "STARTUP AND CLOSE", |ui| {
            let mut launch = crate::autostart::enabled();
            let mut changed = false;
            settings_row(
                ui,
                palette,
                "Launch WA Desk at login",
                "Starts with the account's Run key; nothing is written outside it.",
                |ui| {
                    if settings_switch(ui, palette, launch).clicked() {
                        launch = !launch;
                        changed = true;
                    }
                },
            );
            if changed {
                let note = match crate::autostart::set(launch) {
                    Ok(()) => format!(
                        "Launch at login {}.",
                        if launch { "enabled" } else { "disabled" }
                    ),
                    Err(err) => err,
                };
                self.settings_note = note;
            }
        });
        settings_card(ui, palette, "LOG OUT", |ui| {
            settings_row(
                ui,
                palette,
                "Unlink this device",
                "The phone keeps working; this session is removed.",
                |ui| {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new("Log out").color(palette.danger))
                                .frame(false),
                        )
                        .clicked()
                    {
                        self.send(Command::Relink);
                        self.settings_note =
                            "Unlinked - scan the QR with the phone to link again.".to_string();
                    }
                },
            );
        });
    }

    /// Privacy, in the official order: who can see what, disappearing messages,
    /// the block list and the advanced switches. Visibility is owned by the
    /// phone (a linked device can read it, not set it), so the values say so
    /// instead of inventing a local setting.
    pub(crate) fn privacy_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Privacy",
            "Who can see my personal info, disappearing messages and blocked contacts.",
        );
        // The rows the live client lists under "Who can see my personal info",
        // sampled 2026-11-05.
        settings_card(ui, palette, "WHO CAN SEE MY PERSONAL INFO", |ui| {
            for (title, note) in [
                (
                    "Last seen and online",
                    "Decided on the phone; this device only reads chats.",
                ),
                (
                    "Profile picture",
                    "The phone's setting applies to the linked device too.",
                ),
                ("About", "Owned by the phone."),
                ("Status", "Status privacy is set when a story is posted."),
            ] {
                settings_row(ui, palette, title, note, |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            }
            settings_row(
                ui,
                palette,
                "Read receipts",
                "A linked device always acknowledges what it displays; group receipts are always sent.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "DISAPPEARING MESSAGES", |ui| {
            settings_row(
                ui,
                palette,
                "Default message timer",
                "Set per chat on the phone; the bubble list shows it.",
                |ui| {
                    ui.label(
                        egui::RichText::new("24 hours")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Groups",
                "Older bodies cannot be requested back once the timer passes.",
                |ui| {
                    ui.label(
                        egui::RichText::new("My contacts")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "CALLS AND CONTACTS", |ui| {
            settings_row(
                ui,
                palette,
                "Blocked contacts",
                "The phone owns the blocked list; this client does not mirror it.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "App lock",
                "Require a password to open WhatsApp on this device.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "ADVANCED", |ui| {
            for (title, note) in [
                (
                    "Block unknown account messages",
                    "The phone blocks messages from unknown accounts past a volume.",
                ),
                (
                    "Protect IP address in calls",
                    "Calls are relayed through WhatsApp when the phone enables it.",
                ),
                (
                    "Turn off link previews",
                    "Previews are generated by the sender's client, not by this device.",
                ),
            ] {
                settings_row(ui, palette, title, note, |ui| {
                    ui.label(
                        egui::RichText::new("Phone")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            }
        });
    }

    /// Keyboard shortcuts this window answers to.
    pub(crate) fn shortcuts_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Keyboard shortcuts",
            "Keys this window answers to.",
        );
        let rows = [
            ("Ctrl + V", "Paste an image or file into the composer"),
            ("Enter", "Send the message"),
            ("Shift + Enter", "New line in the composer"),
            ("Esc", "Close search, the viewer or the emoji picker"),
            ("Arrow keys", "Previous / next media in the viewer"),
            ("Space", "Play or pause the open clip"),
            ("Ctrl + F", "Search inside the open chat"),
        ];
        settings_card(ui, palette, "SHORTCUTS", |ui| {
            for (keys, what) in rows {
                settings_row(ui, palette, keys, what, |ui| {
                    ui.label("");
                });
            }
        });
    }

    /// The About / Storage card at the bottom of the Help page.
    pub(crate) fn about_card(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        settings_card(ui, palette, "ABOUT", |ui| {
            settings_row(
                ui,
                palette,
                "Version",
                "Unofficial WhatsApp client in pure Rust.",
                |ui| {
                    ui.label(
                        egui::RichText::new(env!("CARGO_PKG_VERSION"))
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Storage",
                "Chat cache, media and settings live here:",
                |_ui| {},
            );
            ui.label(
                egui::RichText::new("%APPDATA%\\io.github.dirttw.wadesk-native")
                    .size(12.0)
                    .monospace()
                    .color(palette.secondary),
            );
        });
    }

    /// Notifications, in the official order: Messages, Groups, Status, Calls,
    /// then the preview/sound/background switches. Only the first is a real
    /// setting here (the toast switch); the rest report what this client does.
    pub(crate) fn notifications_detail(
        &mut self,
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
    ) {
        Self::settings_header(
            ui,
            palette,
            "Notifications",
            "Messages, groups, sounds, previews and background sync.",
        );
        settings_card(ui, palette, "NOTIFICATIONS", |ui| {
            settings_row(
                ui,
                palette,
                "Messages",
                "A toast only for messages that arrive while the window is unfocused - never for the open chat.",
                |ui| {
                    settings_switch(ui, palette, self.settings.notifications);
                },
            );
            settings_row(
                ui,
                palette,
                "Groups",
                "Group messages follow the same rule as messages.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Status",
                "Status updates never raise a toast; they land in the Status panel.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Off")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Calls",
                "An incoming call opens the call sheet instead of a toast.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "PREVIEWS AND SOUND", |ui| {
            settings_row(
                ui,
                palette,
                "Show previews",
                "The toast carries the sender and the message text.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Play sound for outgoing messages",
                "This client sends silently.",
                |ui| {
                    ui.label(
                        egui::RichText::new("Off")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Background sync",
                "The linked session keeps syncing while the window is in the background.",
                |ui| {
                    ui.label(
                        egui::RichText::new("On")
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(
                ui,
                palette,
                "Test notification",
                "Posts one toast so the OS path can be checked.",
                |ui| {
                    ui.label(egui::RichText::new("Send").size(13.0).color(palette.accent));
                },
            );
        });
    }

    /// Chats: theme, chat history and the sync actions.
    pub(crate) fn chats_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Chats",
            "Appearance and what the phone sends us.",
        );
        let system_dark = ui.ctx().style().visuals.dark_mode;
        let current = self.settings.theme.clone();
        let mut chosen: Option<String> = None;
        settings_card(ui, palette, "THEME", |ui| {
            for choice in ["light", "dark", "system"] {
                if settings_radio_row(
                    ui,
                    palette,
                    wa_ui::settings::theme_label(choice),
                    current == choice,
                )
                .clicked()
                {
                    chosen = Some(choice.to_string());
                }
            }
        });
        if let Some(choice) = chosen {
            self.settings.theme = choice;
            let _ = self.settings.save(&self.settings_path);
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "{} · showing {}",
                self.settings.theme_label(),
                self.settings.resolved_theme(system_dark)
            ))
            .size(12.0)
            .color(palette.secondary),
        );

        let mut notifications = self.settings.notifications;
        let mut toggled = false;
        settings_card(ui, palette, "NOTIFICATIONS", |ui| {
            settings_row(
                ui,
                palette,
                "Desktop notifications",
                "A toast when a message arrives while WA Desk is in the background.",
                |ui| {
                    if settings_switch(ui, palette, notifications).clicked() {
                        notifications = !notifications;
                        toggled = true;
                    }
                },
            );
        });
        if toggled {
            self.settings.notifications = notifications;
            let _ = self.settings.save(&self.settings_path);
            self.settings_note = if notifications {
                "Desktop notifications are on.".to_string()
            } else {
                "Desktop notifications are off.".to_string()
            };
        }

        let mut action: Option<&'static str> = None;
        settings_card(ui, palette, "CHATS", |ui| {
            // The live Chats pane (sampled 2026-11-05) lists these five under
            // the theme choices; ours reports what this client actually does.
            for (title, note, value) in [
                (
                    "Media upload quality",
                    "Photos and videos are uploaded untouched.",
                    "Original",
                ),
                (
                    "Media auto-download",
                    "A chat's media is fetched when it is opened.",
                    "On open",
                ),
                ("Spell check", "egui has no spell checker.", "Off"),
                (
                    "Replace text with emoji",
                    "Typed shortcuts are sent as typed.",
                    "Off",
                ),
                ("Enter is send", "Enter sends the message.", "On"),
            ] {
                settings_row(ui, palette, title, note, |ui| {
                    ui.label(
                        egui::RichText::new(value)
                            .size(13.0)
                            .color(palette.secondary),
                    );
                });
            }
        });
        ui.add_space(12.0);
        settings_card(ui, palette, "CHAT HISTORY", |ui| {
            ui.label(
                egui::RichText::new(
                    "Phones only push the full chat list to a newly linked device, so this asks \
                     the linked session for one batch at a time (eight chats).",
                )
                .size(12.5)
                .color(palette.secondary),
            );
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 22.0;
                for (label, key) in [
                    ("Sync chat history", "history"),
                    ("Sync more chats", "more"),
                    ("Sync groups", "groups"),
                    ("Refresh statuses", "status"),
                ] {
                    if settings_link(ui, palette, label) {
                        action = Some(key);
                    }
                }
            });
        });
        if let Some(action) = action {
            match action {
                "history" => {
                    self.send(Command::InitialSync);
                    self.send(Command::SyncChatState);
                    self.settings_note = "Asking the phone for history…".to_string();
                }
                "more" => {
                    self.send(Command::SyncMore);
                    self.settings_note = "Asking for the next chats…".to_string();
                }
                "groups" => {
                    self.send(Command::SyncGroups);
                    self.settings_note = "Asking for participating groups…".to_string();
                }
                _ => {
                    self.send(Command::StatusHistory);
                    self.settings_note = "Asking for recent statuses…".to_string();
                }
            }
        }
    }

    /// AI assistant: the encrypted key and the model configuration.
    pub(crate) fn ai_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "AI assistant",
            "Bring your own OpenAI-compatible endpoint.",
        );
        let hint = self.settings.key_hint();
        settings_card(ui, palette, "BEHAVIOUR", |ui| {
            settings_row(
                ui,
                palette,
                "AI mode",
                "Answer saved contacts, and groups that mention you.",
                |ui| {
                    let mut on = self.settings.ai.enabled;
                    if settings_switch(ui, palette, on).clicked() {
                        on = !on;
                    }
                    self.settings.ai.enabled = on;
                },
            );
            settings_row(
                ui,
                palette,
                "Require /ai trigger",
                "Only reply when a message asks with /ai.",
                |ui| {
                    let mut on = self.settings.ai.require_trigger;
                    if settings_switch(ui, palette, on).clicked() {
                        on = !on;
                    }
                    self.settings.ai.require_trigger = on;
                },
            );
        });

        settings_card(ui, palette, "CONNECTION", |ui| {
            ui.label(
                egui::RichText::new(hint.clone())
                    .size(12.0)
                    .color(palette.secondary),
            );
            ui.add_space(8.0);
            settings_field(
                ui,
                palette,
                "Base URL",
                "https://opencode.ai/zen/go/v1",
                &mut self.settings.ai.base_url,
                false,
            );
            settings_field(
                ui,
                palette,
                "Model",
                "the chat model id",
                &mut self.settings.ai.model,
                false,
            );
            settings_field(
                ui,
                palette,
                "Vision model",
                "optional - the chat model when empty",
                &mut self.settings.ai.vision_model,
                false,
            );
            settings_field(
                ui,
                palette,
                "API key",
                "leave blank to keep the stored key",
                &mut self.settings.ai.api_key,
                true,
            );
        });

        settings_card(ui, palette, "PERSONA", |ui| {
            ui.label(
                egui::RichText::new("System prompt")
                    .size(13.0)
                    .color(palette.text),
            );
            ui.add_space(6.0);
            ui.add(
                egui::TextEdit::multiline(&mut self.settings.ai.system_prompt)
                    .desired_width(f32::INFINITY)
                    .desired_rows(4),
            );
        });

        ui.add_space(14.0);
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Save")
                        .size(13.5)
                        .color(egui::Color32::WHITE),
                )
                .fill(palette.accent)
                .corner_radius(egui::CornerRadius::same(18))
                .min_size(egui::vec2(96.0, 34.0)),
            )
            .clicked()
        {
            let plan = self.settings.ai_plan();
            if !plan.ok {
                self.settings_note = plan.error.unwrap_or_default();
            } else {
                match self.settings.save(&self.settings_path) {
                    Ok(()) => {
                        if !self.settings.ai.api_key.is_empty() {
                            // The key is encrypted at rest (AES-256-GCM under a
                            // random file key); only the masked hint stays in
                            // settings.json.
                            match crate::ai::keys::save(
                                &crate::settings::NativeSettings::data_dir(),
                                &self.settings.ai.api_key,
                            ) {
                                Ok(()) => self.settings.ai.has_key = true,
                                Err(err) => {
                                    self.settings_note = format!("Could not store the key: {err}");
                                    return;
                                }
                            }
                        }
                        self.settings.ai.api_key.clear();
                        self.settings_note = plan.toast;
                    }
                    Err(err) => self.settings_note = err,
                }
            }
        }
    }

    /// Updates: the GitHub pre-release channel.
    pub(crate) fn updates_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(ui, palette, "Updates", "Version and GitHub releases.");
        let version = env!("CARGO_PKG_VERSION");
        let state = self.update.clone();
        let busy = matches!(state, UpdateState::Checking | UpdateState::Downloading(_));
        settings_card(ui, palette, "THIS INSTALL", |ui| {
            settings_row(
                ui,
                palette,
                "Installed version",
                "Every WA Desk release is a pre-release.",
                |ui| {
                    ui.label(
                        egui::RichText::new(format!("v{version}"))
                            .size(13.0)
                            .color(palette.secondary),
                    );
                },
            );
            settings_row(ui, palette, "Channel", "dirtTW/wa-desk", |ui| {
                if !busy && settings_link(ui, palette, "Check now") {
                    self.check_updates();
                }
            });
        });

        settings_card(ui, palette, "STATUS", |ui| match state {
            UpdateState::Idle => {
                ui.label(
                    egui::RichText::new("Not checked yet.")
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::Checking => {
                ui.label(
                    egui::RichText::new("Checking the release list…")
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::UpToDate => {
                ui.label(
                    egui::RichText::new("You are on the newest release.")
                        .size(13.0)
                        .color(palette.secondary),
                );
            }
            UpdateState::Available(release) => {
                let megabytes = release.size as f64 / 1_048_576.0;
                ui.label(
                    egui::RichText::new(format!(
                        "{} is available ({megabytes:.1} MB)",
                        release.tag
                    ))
                    .size(13.0)
                    .color(palette.text),
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 20.0;
                    if settings_link(ui, palette, "Download & install") {
                        self.download_update(release.clone());
                    }
                    if !release.notes_url.is_empty() && settings_link(ui, palette, "Release notes")
                    {
                        ui.ctx()
                            .open_url(egui::OpenUrl::new_tab(&release.notes_url));
                    }
                });
            }
            UpdateState::Downloading(release) => {
                // The button becomes the progress bar: the download streams in
                // 64 kB chunks and publishes its byte counters.
                let (done, total) = crate::updates::download_bytes();
                let megabytes = |bytes: u64| bytes as f64 / 1_048_576.0;
                let caption = if total > 0 {
                    format!(
                        "{} - {:.1} of {:.1} MB",
                        release.tag,
                        megabytes(done),
                        megabytes(total)
                    )
                } else {
                    format!("{} - starting", release.tag)
                };
                ui.label(egui::RichText::new(caption).size(13.0).color(palette.text));
                ui.add_space(8.0);
                crate::settings_ui::download_progress_button(
                    ui,
                    palette,
                    crate::updates::download_fraction(),
                );
                // Keep repainting while the bytes arrive.
                ui.ctx()
                    .request_repaint_after(std::time::Duration::from_millis(150));
            }
            UpdateState::Ready(path) => {
                ui.label(
                    egui::RichText::new("Downloaded - restarting finishes the update.")
                        .size(13.0)
                        .color(palette.secondary),
                );
                ui.add_space(8.0);
                if settings_link(ui, palette, "Restart now") {
                    match crate::updates::install(&path) {
                        Ok(()) => std::process::exit(0),
                        Err(err) => self.update = UpdateState::Failed(err),
                    }
                }
            }
            UpdateState::Failed(err) => {
                ui.label(egui::RichText::new(err).size(13.0).color(palette.danger));
            }
        });
    }

    /// Help and feedback: the project links.
    pub(crate) fn help_detail(&mut self, ui: &mut egui::Ui, palette: &crate::theme::Palette) {
        Self::settings_header(
            ui,
            palette,
            "Help and feedback",
            "WA Desk is unofficial and open.",
        );
        settings_card(ui, palette, "PROJECT", |ui| {
            for (label, url) in [
                ("Source code", "https://github.com/dirtTW/wa-desk"),
                (
                    "Change log",
                    "https://github.com/dirtTW/wa-desk/blob/main/CHANGELOG.md",
                ),
                (
                    "Report an issue",
                    "https://github.com/dirtTW/wa-desk/issues",
                ),
            ] {
                if settings_link(ui, palette, label) {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(url));
                }
                ui.add_space(8.0);
            }
        });
        settings_card(ui, palette, "ABOUT", |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "WA Desk v{} · MIT licensed · not affiliated with WhatsApp or Meta.",
                    env!("CARGO_PKG_VERSION")
                ))
                .size(12.5)
                .color(palette.secondary),
            );
        });

        ui.add_space(12.0);
        self.about_card(ui, palette);
    }
}
