//! The Channels drawer: "Channels" + circle-plus header, a Search field, the
//! followed-channel rows, the "Find channels to follow" section and a "Discover
//! more" footer. The right pane shows the open channel's posts read-only, or the
//! centred "Discover channels" empty state.
//!
//! Sampled from web.whatsapp.com on 2026-11-05 (`target/live_channels.jpg`).

use crate::app::{Icon, WaDesk, draw_icon, icon_button};
use crate::widgets::{fit_text, paint_avatar};

impl WaDesk {
    pub(crate) fn channels_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette.clone();
        let rows = self.model.channel_rows();

        // ---- the drawer, in the chat list's column -------------------------
        egui::Panel::left("channels")
            .resizable(true)
            .default_size(420.0)
            .size_range(320.0..=560.0)
            .frame(egui::Frame::new().fill(palette.elevated))
            .show(ui, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .show(ui, |ui| {
                        ui.set_min_height(44.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Channels").size(26.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // The live drawer's only header control.
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

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add_space(10.0);
                    egui::Frame::new()
                        .fill(palette.panel)
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
                                    egui::TextEdit::singleline(&mut self.channels_query)
                                        .frame(egui::Frame::NONE)
                                        .hint_text("Search")
                                        .desired_width(ui.available_width() - 8.0),
                                );
                            });
                        });
                    ui.add_space(10.0);
                });
                ui.add_space(4.0);

                let needle = self.channels_query.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (row, followers) in &rows {
                            if !needle.is_empty() && !row.name.to_lowercase().contains(&needle) {
                                continue;
                            }
                            let selected = self.model.active.as_deref() == Some(row.jid.as_str());
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 72.0),
                                egui::Sense::click(),
                            );
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    rect.shrink2(egui::vec2(6.0, 0.0)),
                                    egui::CornerRadius::same(12),
                                    if selected {
                                        palette.selected
                                    } else {
                                        palette.hover
                                    },
                                );
                            }
                            let center = egui::pos2(rect.left() + 40.0, rect.center().y);
                            paint_avatar(ui, &row.name, 48.0, None);
                            ui.painter().text(
                                egui::pos2(rect.left() + 74.0, rect.top() + 26.0),
                                egui::Align2::LEFT_CENTER,
                                fit_text(
                                    ui.painter(),
                                    &row.name,
                                    egui::FontId::proportional(15.5),
                                    palette.text,
                                    rect.width() - 90.0,
                                ),
                                egui::FontId::proportional(15.5),
                                palette.text,
                            );
                            ui.painter().text(
                                egui::pos2(rect.left() + 74.0, rect.top() + 47.0),
                                egui::Align2::LEFT_CENTER,
                                followers.clone(),
                                egui::FontId::proportional(12.5),
                                palette.secondary,
                            );
                            let _ = center;
                            if response.clicked() {
                                self.model.active = Some(row.jid.clone());
                            }
                        }

                        if rows.is_empty() {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(14.0);
                                ui.label(
                                    egui::RichText::new("No followed channels yet")
                                        .size(13.0)
                                        .color(palette.secondary),
                                );
                            });
                        }

                        // The live drawer's suggestion section. Rows with a
                        // Follow pill need the channel directory, which a linked
                        // device cannot reach; the section keeps the live shape -
                        // heading, muted line, divider, "Discover more" with its
                        // chevron - so nothing is faked.
                        ui.add_space(14.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                egui::RichText::new("Find channels to follow")
                                    .size(12.5)
                                    .strong()
                                    .color(palette.secondary),
                            );
                        });
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            ui.label(
                                egui::RichText::new(
                                    "Follow channels on the phone; they show up here.",
                                )
                                .size(12.5)
                                .color(palette.secondary),
                            );
                        });
                        ui.add_space(10.0);
                        // The hairline the live footer sits under, inset on both
                        // sides so it never crosses the drawer's edge.
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            let (line, _) = ui.allocate_exact_size(
                                egui::vec2((ui.available_width() - 14.0).max(0.0), 1.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                line,
                                egui::CornerRadius::ZERO,
                                palette.border,
                            );
                        });
                        ui.add_space(6.0);
                        let (footer, footer_hit) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), 38.0),
                            egui::Sense::click(),
                        );
                        if footer_hit.hovered() {
                            ui.painter().rect_filled(
                                footer.shrink2(egui::vec2(8.0, 0.0)),
                                egui::CornerRadius::same(8),
                                palette.hover,
                            );
                        }
                        draw_icon(
                            ui,
                            egui::Rect::from_center_size(
                                egui::pos2(footer.left() + 22.0, footer.center().y),
                                egui::vec2(16.0, 16.0),
                            ),
                            Icon::Groups,
                            palette.accent,
                        );
                        ui.painter().text(
                            egui::pos2(footer.left() + 40.0, footer.center().y),
                            egui::Align2::LEFT_CENTER,
                            "Discover more",
                            egui::FontId::proportional(13.0),
                            palette.accent,
                        );
                        draw_icon(
                            ui,
                            egui::Rect::from_center_size(
                                egui::pos2(footer.right() - 28.0, footer.center().y),
                                egui::vec2(14.0, 14.0),
                            ),
                            Icon::ChevronRight,
                            palette.secondary,
                        );
                        if footer_hit.clicked() {
                            self.notice = Some((
                                "The channel directory lives on the phone".to_string(),
                                std::time::Instant::now(),
                            ));
                        }
                    });
            });

        // ---- the right pane -------------------------------------------------
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ui, |ui| {
                // Only a channel JID belongs here: `active` may still point at
                // the chat the user was reading (the demo group, for instance).
                let Some(jid) = self
                    .model
                    .active
                    .clone()
                    .filter(|jid| jid.ends_with("@newsletter"))
                else {
                    discover_pane(ui, &palette);
                    return;
                };
                let posts = self.model.channel_posts(&jid);
                let title = self
                    .model
                    .chats
                    .get(&jid)
                    .map(|row| self.model.title(row))
                    .unwrap_or_default();
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add_space(18.0);
                    ui.label(egui::RichText::new(title).size(17.0).strong());
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(6.0);
                        for post in &posts {
                            ui.horizontal(|ui| {
                                ui.add_space(18.0);
                                ui.vertical(|ui| {
                                    ui.set_width((ui.available_width() - 60.0).min(620.0));
                                    ui.label(
                                        egui::RichText::new(self.model.bubble_stamp(post))
                                            .small()
                                            .weak(),
                                    );
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
                                });
                            });
                        }
                    });
            });
    }
}

/// The live empty state: the channel glyph, the copy and the two round actions.
fn discover_pane(ui: &mut egui::Ui, palette: &crate::theme::Palette) {
    ui.vertical_centered(|ui| {
        ui.add_space((ui.available_height() * 0.24).clamp(20.0, 220.0));
        let (icon, _) = ui.allocate_exact_size(egui::vec2(54.0, 54.0), egui::Sense::hover());
        crate::app::draw_icon(ui, icon, Icon::Channels, egui::Color32::from_gray(72));
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("Discover channels")
                .size(23.0)
                .strong()
                .color(palette.text),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(
                "Entertainment, sports, news, lifestyle, people and more. Follow the \
                 channels that interest you",
            )
            .size(13.5)
            .color(palette.secondary),
        );
        ui.add_space(18.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 26.0;
            let total = 2.0 * 72.0 + 26.0;
            ui.add_space(((ui.available_width() - total) / 2.0).max(0.0));
            let _ = crate::app::action_button(ui, palette, Icon::Add, "Create channel");
            let _ = crate::app::action_button(ui, palette, Icon::Search, "Explore channels");
        });
    });
}
