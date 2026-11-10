//! The QR gate: the "Scan to log in" card, the steps, the demo QR and the
//! pairing-code form, plus the QR texture cache.

use crate::app::{Icon, WaDesk, draw_icon};
use crate::login::LoginState;
use crate::login_ui::{draw_laptop, draw_whatsapp_mark, gate_card, gate_link, gate_step};
use crate::model::Command;

impl WaDesk {
    /// The login screen: the QR (or pairing code), the hint line and the
    /// actions that get a fresh code.
    /// The login gate: WhatsApp Web's "scan to log in" page, painted with the
    /// dark palette (wordmark, download card, the scan card with its three
    /// numbered steps, the QR and the phone-number link).
    pub(crate) fn gate(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(self.palette.panel))
            .show(ui, |ui| {
                let palette = self.palette.clone();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Top-left wordmark, like the web page.
                        ui.add_space(18.0);
                        ui.horizontal(|ui| {
                            ui.add_space(26.0);
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
                            draw_whatsapp_mark(ui, rect, palette.accent);
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("WhatsApp")
                                    .size(15.0)
                                    .color(palette.text)
                                    .strong(),
                            );
                        });
                        ui.add_space(46.0);

                        let width = (ui.available_width() - 96.0).clamp(420.0, 860.0);
                        let mut go_phone = false;
                        let mut new_code = false;

                        ui.vertical_centered(|ui| {
                            // ---- Download card -------------------------------------
                            gate_card(ui, &palette, width, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(120.0, 84.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_laptop(ui, rect, &palette);
                                    ui.add_space(18.0);
                                    ui.vertical(|ui| {
                                        ui.label(
                                            egui::RichText::new("Download WA Desk for Windows")
                                                .size(17.0)
                                                .color(palette.text)
                                                .strong(),
                                        );
                                        ui.add_space(3.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "Get extra features like voice and video calling, \
                                                 screen sharing and more.",
                                            )
                                            .size(13.5)
                                            .color(palette.secondary),
                                        );
                                    });
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(148.0, 44.0),
                                                egui::Sense::click(),
                                            );
                                            let fill = if response.hovered() {
                                                palette.accent.gamma_multiply(0.88)
                                            } else {
                                                palette.accent
                                            };
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(24),
                                                fill,
                                            );
                                            let label = ui.painter().layout_no_wrap(
                                                "Download".to_string(),
                                                egui::FontId::proportional(15.0),
                                                egui::Color32::from_rgb(11, 20, 26),
                                            );
                                            let icon = egui::Rect::from_center_size(
                                                egui::pos2(
                                                    rect.right() - 26.0,
                                                    rect.center().y,
                                                ),
                                                egui::vec2(18.0, 18.0),
                                            );
                                            ui.painter().galley(
                                                egui::pos2(
                                                    rect.left() + 22.0,
                                                    rect.center().y - label.size().y / 2.0,
                                                ),
                                                label,
                                                egui::Color32::from_rgb(11, 20, 26),
                                            );
                                            draw_icon(
                                                ui,
                                                icon,
                                                Icon::Download,
                                                egui::Color32::from_rgb(11, 20, 26),
                                            );
                                            if response.clicked() {
                                                self.notice = Some((
                                                    "The installer is the WA-Desk-Setup.exe \
                                                     release download"
                                                        .to_string(),
                                                    std::time::Instant::now(),
                                                ));
                                            }
                                        },
                                    );
                                });
                            });

                            ui.add_space(18.0);

                            // ---- Scan card -----------------------------------------
                            gate_card(ui, &palette, width, |ui| {
                                ui.horizontal_top(|ui| {
                                    // Left: title, steps, links.
                                    ui.vertical(|ui| {
                                        ui.set_width((width * 0.5 - 60.0).max(240.0));
                                        ui.label(
                                            egui::RichText::new("Scan to log in")
                                                .size(27.0)
                                                .color(palette.text),
                                        );
                                        ui.add_space(16.0);
                                        gate_step(
                                            ui,
                                            &palette,
                                            1,
                                            |ui, palette| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "Scan the QR code with your phone's camera",
                                                    )
                                                    .size(14.0)
                                                    .color(palette.text),
                                                );
                                            },
                                        );
                                        ui.add_space(10.0);
                                        gate_step(
                                            ui,
                                            &palette,
                                            2,
                                            |ui, palette| {
                                                ui.horizontal(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(
                                                            "Tap the link to open WhatsApp",
                                                        )
                                                        .size(14.0)
                                                        .color(palette.text),
                                                    );
                                                    let (rect, _) = ui.allocate_exact_size(
                                                        egui::vec2(15.0, 15.0),
                                                        egui::Sense::hover(),
                                                    );
                                                    draw_whatsapp_mark(ui, rect, palette.accent);
                                                });
                                            },
                                        );
                                        ui.add_space(10.0);
                                        gate_step(
                                            ui,
                                            &palette,
                                            3,
                                            |ui, palette| {
                                                ui.label(
                                                    egui::RichText::new(
                                                        "Scan the QR code again to link your account",
                                                    )
                                                    .size(14.0)
                                                    .color(palette.text),
                                                );
                                            },
                                        );
                                        ui.add_space(14.0);
                                        if gate_link(ui, &palette, "Need help?")
                                            .on_hover_text(
                                                "Open WhatsApp on your phone, then Linked devices \
                                                 → Link a device.",
                                            )
                                            .clicked()
                                        {
                                            self.notice = Some((
                                                "WhatsApp on your phone → Linked devices → \
                                                 Link a device"
                                                    .to_string(),
                                                std::time::Instant::now(),
                                            ));
                                        }
                                    });

                                    // Right: the QR (or the pairing code) and the links.
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Min),
                                        |ui| {
                                            ui.vertical(|ui| {
                                                let side = 268.0;
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Min),
                                                    |ui| {
                                                        ui.allocate_ui(
                                                            egui::vec2(side + 24.0, side + 24.0),
                                                            |ui| {
                                                                let (rect, _) =
                                                                    ui.allocate_exact_size(
                                                                        egui::vec2(
                                                                            side + 24.0,
                                                                            side + 24.0,
                                                                        ),
                                                                        egui::Sense::hover(),
                                                                    );
                                                                ui.painter().rect_filled(
                                                                    rect,
                                                                    egui::CornerRadius::same(16),
                                                                    egui::Color32::WHITE,
                                                                );
                                                                let qr_rect = rect.shrink(12.0);
                                                                match self.model.login.clone() {
                                                                    LoginState::Qr(svg) => {
                                                                        if let Some(texture) =
                                                                            self.qr_texture(ctx, &svg)
                                                                        {
                                                                            ui.painter().image(
                                                                                texture.id(),
                                                                                qr_rect,
                                                                                egui::Rect::from_min_max(
                                                                                    egui::pos2(0.0, 0.0),
                                                                                    egui::pos2(1.0, 1.0),
                                                                                ),
                                                                                egui::Color32::WHITE,
                                                                            );
                                                                        }
                                                                    }
                                                                    LoginState::PairCode(code) => {
                                                                        let galley = ui
                                                                            .painter()
                                                                            .layout_no_wrap(
                                                                                code,
                                                                                egui::FontId::monospace(26.0),
                                                                                egui::Color32::from_rgb(17, 27, 33),
                                                                            );
                                                                        ui.painter().galley(
                                                                            egui::pos2(
                                                                                rect.center().x
                                                                                    - galley.size().x
                                                                                        / 2.0,
                                                                                rect.center().y
                                                                                    - galley.size().y
                                                                                        / 2.0,
                                                                            ),
                                                                            galley,
                                                                            egui::Color32::from_rgb(
                                                                                17, 27, 33,
                                                                            ),
                                                                        );
                                                                    }
                                                                    LoginState::Expired
                                                                    | LoginState::Waiting
                                                                    | LoginState::Linked => {}
                                                                }
                                                            },
                                                        );
                                                    },
                                                );
                                                ui.add_space(14.0);
                                                if self.gate_phone {
                                                    ui.vertical_centered(|ui| {
                                                        let width =
                                                            (ui.available_width()).min(240.0);
                                                        ui.add_sized(
                                                            [width, 30.0],
                                                            egui::TextEdit::singleline(
                                                                &mut self.model.pair_phone,
                                                            )
                                                            .hint_text("628123456789"),
                                                        );
                                                        ui.add_space(6.0);
                                                        let (rect, response) =
                                                            ui.allocate_exact_size(
                                                                egui::vec2(120.0, 30.0),
                                                                egui::Sense::click(),
                                                            );
                                                        ui.painter().rect_filled(
                                                            rect,
                                                            egui::CornerRadius::same(24),
                                                            palette.accent,
                                                        );
                                                        ui.painter().text(
                                                            rect.center(),
                                                            egui::Align2::CENTER_CENTER,
                                                            "Get code",
                                                            egui::FontId::proportional(13.5),
                                                            egui::Color32::from_rgb(11, 20, 26),
                                                        );
                                                        if response.clicked() {
                                                            let phone: String = self
                                                                .model
                                                                .pair_phone
                                                                .chars()
                                                                .filter(|c| c.is_ascii_digit())
                                                                .collect();
                                                            if !phone.is_empty() {
                                                                self.send(Command::PairCode {
                                                                    phone,
                                                                });
                                                            }
                                                        }
                                                    });
                                                } else {
                                                    ui.with_layout(
                                                        egui::Layout::right_to_left(
                                                            egui::Align::Center,
                                                        ),
                                                        |ui| {
                                                            if gate_link(
                                                                ui,
                                                                &palette,
                                                                "Log in with phone number",
                                                            )
                                                            .clicked()
                                                            {
                                                                go_phone = true;
                                                            }
                                                            draw_icon(
                                                                ui,
                                                                egui::Rect::from_center_size(
                                                                    egui::pos2(
                                                                        ui.cursor().left() + 8.0,
                                                                        ui.cursor().top() + 8.0,
                                                                    ),
                                                                    egui::vec2(16.0, 16.0),
                                                                ),
                                                                Icon::ChevronRight,
                                                                palette.accent,
                                                            );
                                                        },
                                                    );
                                                }
                                            });
                                        },
                                    );
                                });

                                ui.add_space(16.0);
                                // The session is stored on disk, so this is a
                                // truthful checked box rather than a switch.
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui
                                        .allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                                    ui.painter().rect_filled(
                                        rect,
                                        egui::CornerRadius::same(4),
                                        palette.accent,
                                    );
                                    draw_icon(
                                        ui,
                                        rect.shrink(3.0),
                                        Icon::Check,
                                        egui::Color32::from_rgb(11, 20, 26),
                                    );
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new("Stay logged in on this device")
                                            .size(13.5)
                                            .color(palette.text),
                                    );
                                    draw_icon(
                                        ui,
                                        egui::Rect::from_min_size(
                                            egui::pos2(ui.cursor().left() + 6.0, ui.cursor().top() + 2.0),
                                            egui::vec2(15.0, 15.0),
                                        ),
                                        Icon::Lock,
                                        palette.secondary,
                                    );
                                    ui.add_space(21.0);
                                    if crate::login::can_restart(&self.model.login)
                                        && gate_link(ui, &palette, "Request a new code").clicked()
                                    {
                                        new_code = true;
                                    }
                                });
                            });

                            ui.add_space(22.0);
                            ui.vertical_centered(|ui| {
                                ui.horizontal(|ui| {
                                    let text = ui.painter().layout_no_wrap(
                                        "Don't have a WhatsApp account?".to_string(),
                                        egui::FontId::proportional(14.0),
                                        palette.text,
                                    );
                                    let link = ui.painter().layout_no_wrap(
                                        "Get started".to_string(),
                                        egui::FontId::proportional(14.0),
                                        palette.accent,
                                    );
                                    let total = text.size().x + 6.0 + link.size().x + 16.0;
                                    ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                                    ui.label(
                                        egui::RichText::new("Don't have a WhatsApp account?")
                                            .size(14.0)
                                            .color(palette.text),
                                    );
                                    ui.add_space(2.0);
                                    ui.label(
                                        egui::RichText::new("Get started")
                                            .size(14.0)
                                            .color(palette.accent)
                                            .underline(),
                                    );
                                });
                                ui.add_space(14.0);
                                ui.horizontal(|ui| {
                                    let galley = ui.painter().layout_no_wrap(
                                        "Your personal messages are end-to-end encrypted"
                                            .to_string(),
                                        egui::FontId::proportional(13.0),
                                        palette.secondary,
                                    );
                                    let total = galley.size().x + 22.0;
                                    ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(15.0, 15.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Lock, palette.secondary);
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new(
                                            "Your personal messages are end-to-end encrypted",
                                        )
                                        .size(13.0)
                                        .color(palette.secondary),
                                    );
                                });
                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new("Terms & Privacy Policy")
                                        .size(12.0)
                                        .color(palette.secondary.gamma_multiply(0.8)),
                                );
                            });
                            ui.add_space(28.0);
                        });

                        if go_phone {
                            self.gate_phone = true;
                        }
                        if new_code {
                            self.model.login_restarted();
                            self.send(Command::Restart);
                        }
                    });
            });
    }

    /// Rasterise the QR SVG once per code.
    pub(crate) fn qr_texture(
        &mut self,
        ctx: &egui::Context,
        svg: &str,
    ) -> Option<egui::TextureHandle> {
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
