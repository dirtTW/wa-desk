//! The window: a chat list on the left, the open conversation on the right.
//!
//! Painting is all this module does. Every string it shows comes from
//! `crate::model`, which in turn asks `wa-ui`.

use std::sync::mpsc::Sender;

use tokio::sync::mpsc::UnboundedReceiver;
use wa_types::{MessageView, UiEvent};

use crate::login::LoginState;
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
    /// Rasterised QR code, keyed by the SVG it came from so a rotating code
    /// is decoded once per rotation.
    qr: Option<(String, egui::TextureHandle)>,
}

impl WaDesk {
    pub fn new(_cc: &eframe::CreationContext<'_>, native: NativeApp) -> Self {
        Self {
            model: Model::new(),
            events: native.events,
            commands: native.commands,
            runtime: native.runtime,
            typing_sent: false,
            qr: None,
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

impl WaDesk {
    /// The login screen: the QR (or pairing code), the hint line and the
    /// actions that get a fresh code.
    fn gate(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.heading("WA Desk");
                ui.label(egui::RichText::new(self.model.login.hint()).weak());

                match self.model.login.clone() {
                    LoginState::Qr(svg) => {
                        if let Some(texture) = self.qr_texture(ctx, &svg) {
                            let side = 280.0;
                            ui.add_space(12.0);
                            ui.image((texture.id(), egui::vec2(side, side)));
                        } else {
                            ui.label(
                                egui::RichText::new("could not draw the QR code")
                                    .color(egui::Color32::RED),
                            );
                        }
                    }
                    LoginState::PairCode(code) => {
                        ui.add_space(16.0);
                        ui.label(egui::RichText::new(code).size(30.0).monospace());
                        ui.label(
                            egui::RichText::new("Linked devices → Link with phone number")
                                .small()
                                .weak(),
                        );
                    }
                    LoginState::Expired | LoginState::Waiting => {}
                    LoginState::Linked => {}
                }

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if crate::login::can_restart(&self.model.login)
                        && ui.button("Request a new code").clicked()
                    {
                        self.model.login_restarted();
                        self.send(Command::Restart);
                    }
                });

                ui.add_space(24.0);
                ui.separator();
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Or link with a phone number").small());
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 90.0).max(120.0);
                    ui.add_sized(
                        [width, 28.0],
                        egui::TextEdit::singleline(&mut self.model.pair_phone)
                            .hint_text("628123456789"),
                    );
                    if ui
                        .add_sized([80.0, 28.0], egui::Button::new("Get code"))
                        .clicked()
                    {
                        let phone: String = self
                            .model
                            .pair_phone
                            .chars()
                            .filter(|c| c.is_ascii_digit())
                            .collect();
                        if !phone.is_empty() {
                            self.send(Command::PairCode { phone });
                        }
                    }
                });
                ui.add_space(10.0);
                ui.label(egui::RichText::new(&self.model.status).small().weak());
            });
        });
    }

    /// Rasterise the QR SVG once per code.
    fn qr_texture(&mut self, ctx: &egui::Context, svg: &str) -> Option<egui::TextureHandle> {
        if let Some((_, texture)) = self.qr.as_ref().filter(|(cached, _)| cached == svg) {
            return Some(texture.clone());
        }
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).ok()?;
        let size = tree.size().to_int_size();
        let mut pixmap = resvg::tiny_skia::Pixmap::new(size.width().max(1), size.height().max(1))?;
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        let image = egui::ColorImage::from_rgba_premultiplied(
            [size.width() as usize, size.height() as usize],
            pixmap.data(),
        );
        let texture = ctx.load_texture("login-qr", image, egui::TextureOptions::NEAREST);
        self.qr = Some((svg.to_string(), texture.clone()));
        Some(texture)
    }
}

impl eframe::App for WaDesk {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.model.now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        self.drain_events(ctx);
        if !self.model.login.is_linked() {
            self.gate(ctx);
        } else {
            self.sidebar(ctx);
            self.conversation(ctx);
        }
        let _ = &self.runtime;
    }
}
