//! Full-window screens split out of `app.rs`.

use crate::app::{Icon, WaDesk, icon_button};
use crate::widgets::{fit_text, paint_avatar, status_share_tile};

impl WaDesk {
    /// The Channels screen.
    pub(crate) fn channels_panel(&mut self, ctx: &egui::Context) {
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
                                egui::RichText::new("Channels")
                                    .size(20.0)
                                    .color(palette.text),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // The live drawer's only header control is
                                    // the circle-plus that creates a channel.
                                    if icon_button(ui, Icon::Add, &palette)
                                        .on_hover_text("Create channel")
                                        .clicked()
                                    {
                                        self.notice = Some((
                                            "Creating channels needs the phone".to_string(),
                                            std::time::Instant::now(),
                                        ));
                                    }
                                },
                            );
                        });
                    });

                // The live drawer's search field.
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
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
                                crate::app::draw_icon(ui, rect, Icon::Search, palette.secondary);
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.channels_query)
                                        .frame(false)
                                        .hint_text("Search")
                                        .desired_width(ui.available_width() - 8.0),
                                );
                            });
                        });
                    ui.add_space(12.0);
                });

                // "Discover channels" - the official empty-state copy.
                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    ui.add_space(20.0);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("Discover channels")
                                .size(17.0)
                                .strong()
                                .color(palette.text),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Entertainment, sports, news, lifestyle, people and more. \
                                 Follow the channels that interest you.",
                            )
                            .size(13.0)
                            .color(palette.secondary),
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if status_share_tile(ui, &palette, Icon::Add, "Create channel") {
                                self.notice = Some((
                                    "Creating channels needs the phone".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                            if status_share_tile(ui, &palette, Icon::Search, "Explore channels") {
                                self.notice = Some((
                                    "Browsing the channel directory is not in this build"
                                        .to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        });
                    });
                });

                let rows = self.model.channel_rows();
                if rows.is_empty() {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            egui::RichText::new("Find channels to follow")
                                .size(12.5)
                                .strong()
                                .color(palette.secondary),
                        );
                    });
                    ui.add_space(26.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No followed channels yet")
                                .size(13.5)
                                .color(palette.secondary),
                        );
                    });
                    return;
                }

                ui.add_space(16.0);
                ui.horizontal_top(|ui| {
                    // The channel rail.
                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        ui.set_width(240.0);
                        for (row, followers) in &rows {
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let card = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 64.0),
                                egui::Sense::click(),
                            );
                            let (rect, response) = card;
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    rect.shrink2(egui::vec2(8.0, 0.0)),
                                    egui::CornerRadius::same(12),
                                    if selected {
                                        palette.selected
                                    } else {
                                        palette.hover
                                    },
                                );
                            }
                            paint_avatar(ui, &row.name, 40.0, None);
                            ui.painter().text(
                                egui::pos2(rect.left() + 58.0, rect.center().y - 8.0),
                                egui::Align2::LEFT_CENTER,
                                fit_text(
                                    ui.painter(),
                                    &row.name,
                                    egui::FontId::proportional(14.5),
                                    palette.text,
                                    rect.width() - 70.0,
                                ),
                                egui::FontId::proportional(14.5),
                                palette.text,
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 58.0, rect.center().y + 10.0),
                                egui::Align2::LEFT_CENTER,
                                followers.clone(),
                                egui::FontId::proportional(12.5),
                                palette.secondary,
                            );
                            if response.clicked() {
                                self.model.active = Some(row.jid.clone());
                            }
                            ui.add_space(2.0);
                        }
                    });
                    ui.separator();

                    // Posts, read-only: no composer for a channel.
                    ui.vertical(|ui| {
                        let Some(jid) = self.model.active.clone() else {
                            ui.add_space(20.0);
                            ui.label(
                                egui::RichText::new("Pick a channel")
                                    .size(13.0)
                                    .color(palette.secondary),
                            );
                            return;
                        };
                        let posts = self.model.channel_posts(&jid);
                        egui::ScrollArea::vertical()
                            .stick_to_bottom(true)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for post in &posts {
                                    let header = format!(
                                        "{}  \u{00b7}  {}",
                                        self.model.title(
                                            &self
                                                .model
                                                .chats
                                                .get(&jid)
                                                .cloned()
                                                .unwrap_or_default()
                                        ),
                                        self.model.bubble_stamp(post)
                                    );
                                    ui.label(egui::RichText::new(header).small().weak());
                                    let body = self.model.bubble_body(post);
                                    egui::Frame::new()
                                        .fill(palette.bubble_in)
                                        .corner_radius(egui::CornerRadius::same(8))
                                        .inner_margin(egui::Margin::symmetric(10, 8))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(body)
                                                    .size(14.0)
                                                    .color(palette.text),
                                            );
                                        });
                                    ui.add_space(8.0);
                                }
                            });
                    });
                });
            });
    }
}
