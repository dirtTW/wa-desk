//! The window: a chat list on the left, the open conversation on the right.
//!
//! Painting is all this module does. Every string it shows comes from
//! `crate::model`, which in turn asks `wa-ui`.

use std::sync::mpsc::Sender;

use tokio::sync::mpsc::UnboundedReceiver;
use wa_types::{MessageView, UiEvent};

use crate::model::{ChatRow, Command, Model};

/// What `main` assembles before the window exists.
pub struct NativeApp {
    pub events: UnboundedReceiver<UiEvent>,
    pub commands: Sender<Command>,
    pub runtime: tokio::runtime::Handle,
}

impl NativeApp {
    pub fn new(
        events: UnboundedReceiver<UiEvent>,
        commands: Sender<Command>,
        runtime: tokio::runtime::Handle,
    ) -> Self {
        Self {
            events,
            commands,
            runtime,
        }
    }
}

pub struct WaDesk {
    model: Model,
    events: UnboundedReceiver<UiEvent>,
    commands: Sender<Command>,
    runtime: tokio::runtime::Handle,
    typing_sent: bool,
}

impl WaDesk {
    pub fn new(_cc: &eframe::CreationContext<'_>, native: NativeApp) -> Self {
        Self {
            model: Model::new(),
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        while let Ok(event) = self.events.try_recv() {
            self.model.apply(&event);
            changed = true;
        }
        if changed {
            ctx.request_repaint();
        }
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn sidebar(&mut self, ctx: &egui::Context) {
        egui::SidePanel::left("chats")
            .resizable(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.heading("Chats");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(&self.model.status).small().weak());
                    });
                });
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.model.search)
                        .hint_text("Search")
                        .desired_width(f32::INFINITY),
                );
                ui.separator();

                let rows: Vec<ChatRow> = self.model.visible_chats().into_iter().cloned().collect();

                let previews: Vec<(String, String, String, i64)> = rows
                    .iter()
                    .map(|row| {
                        (
                            self.model.title(row),
                            self.model.row_stamp(row),
                            self.model.row_preview(row),
                            row.unread,
                        )
                    })
                    .collect();

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (row, (title, stamp, preview, unread)) in rows.iter().zip(previews) {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let text = format!(
                                "{}{}\n{}{}",
                                if selected { "▶ " } else { "" },
                                title,
                                preview,
                                if stamp.is_empty() {
                                    String::new()
                                } else {
                                    format!("   · {stamp}")
                                }
                            );
                            let label = if unread > 0 {
                                format!("{text}   ({unread})")
                            } else {
                                text
                            };
                            if ui
                                .add_sized(
                                    [ui.available_width(), 46.0],
                                    egui::Button::new(label).selected(selected),
                                )
                                .clicked()
                            {
                                let jid = row.jid.clone();
                                self.model.open_chat(&jid);
                                self.send(Command::MarkRead { chat: jid.clone() });
                            }
                        }
                    });
            });
    }

    fn conversation(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(jid) = self.model.active.clone() else {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Pick a chat").weak());
                });
                return;
            };
            let Some(row) = self.model.chats.get(&jid).cloned() else {
                return;
            };

            ui.horizontal(|ui| {
                ui.heading(self.model.title(&row));
                if row.jid.ends_with("@g.us") {
                    ui.label(egui::RichText::new("group").small().weak());
                }
            });
            ui.separator();

            let height = ui.available_height() - 54.0;
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(height.max(120.0))
                .show(ui, |ui| {
                    for message in &row.messages {
                        bubble(ui, &self.model, message);
                    }
                });

            ui.separator();
            ui.horizontal(|ui| {
                let send_width = 64.0;
                let edit = ui.add_sized(
                    [ui.available_width() - send_width, 30.0],
                    egui::TextEdit::singleline(&mut self.model.composer)
                        .hint_text("Type a message"),
                );
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
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui
                    .add_sized([send_width, 30.0], egui::Button::new("Send"))
                    .clicked()
                    || enter
                {
                    let text = self.model.composer.trim().to_string();
                    if !text.is_empty() {
                        self.send(Command::SendText {
                            chat: jid.clone(),
                            text: text.clone(),
                        });
                        self.model.composer.clear();
                        self.send(Command::Typing {
                            chat: jid.clone(),
                            typing: false,
                        });
                        self.typing_sent = false;
                    }
                }
            });
        });
    }
}

fn bubble(ui: &mut egui::Ui, model: &Model, message: &MessageView) {
    let align = if message.from_me {
        egui::Align::Max
    } else {
        egui::Align::Min
    };
    ui.with_layout(egui::Layout::top_down(align), |ui| {
        let mut header = String::new();
        if let Some(name) = message.sender_name.as_ref().filter(|_| !message.from_me) {
            header.push_str(name);
            header.push_str(" · ");
        }
        header.push_str(&model.bubble_stamp(message));
        ui.label(egui::RichText::new(header).small().weak());

        let body = if message.text.is_empty() {
            format!("[{}]", message.kind)
        } else {
            message.text.clone()
        };
        let fill = if message.from_me {
            egui::Color32::from_rgb(20, 77, 55)
        } else {
            egui::Color32::from_rgb(36, 38, 38)
        };
        egui::Frame::new()
            .fill(fill)
            .inner_margin(egui::Margin::symmetric(8, 5))
            .corner_radius(egui::CornerRadius::same(8))
            .show(ui, |ui| {
                ui.label(egui::RichText::new(body).color(egui::Color32::from_gray(240)));
            });
        ui.add_space(3.0);
    });
}

impl eframe::App for WaDesk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.model.now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.drain_events(ctx);
        self.sidebar(ctx);
        self.conversation(ctx);
        let _ = &self.runtime;
    }
}
