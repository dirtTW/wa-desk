//! Full-window screens split out of `app.rs`.

use crate::app::{Icon, WaDesk, draw_icon, icon_button};
use crate::model::Command;
use crate::widgets::{avatar_color, fit_text, initials, status_share_tile};

impl WaDesk {
    /// The Communities screen.
    pub(crate) fn communities_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette.clone();
        // The right pane first (panels reserve space in order): the live
        egui::Panel::left("communities")
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
                            ui.label(egui::RichText::new("Communities").size(26.0).strong());
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // The live drawer's header control.
                                    if icon_button(ui, Icon::Add, &palette)
                                        .on_hover_text("New community")
                                        .clicked()
                                    {
                                        self.notice = Some((
                                            "Creating a community needs the phone".to_string(),
                                            std::time::Instant::now(),
                                        ));
                                    }
                                },
                            );
                        });
                    });

                ui.add_space(6.0);
                let (new_rect, new_response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 60.0),
                    egui::Sense::click(),
                );
                if new_response.hovered() {
                    ui.painter().rect_filled(
                        new_rect.shrink2(egui::vec2(6.0, 0.0)),
                        egui::CornerRadius::same(12),
                        palette.hover,
                    );
                }
                let disc = egui::Rect::from_center_size(
                    egui::pos2(new_rect.left() + 36.0, new_rect.center().y),
                    egui::vec2(40.0, 40.0),
                );
                ui.painter()
                    .rect_filled(disc, egui::CornerRadius::same(10), palette.accent);
                crate::app::draw_icon(
                    ui,
                    disc.shrink(9.0),
                    Icon::Groups,
                    egui::Color32::from_rgb(11, 20, 26),
                );
                ui.painter().text(
                    egui::pos2(new_rect.left() + 66.0, new_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    "New community",
                    egui::FontId::proportional(15.5),
                    palette.text,
                );
                if new_response.clicked() {
                    self.notice = Some((
                        "Creating a community needs the phone".to_string(),
                        std::time::Instant::now(),
                    ));
                }

                let communities = self
                    .model
                    .communities()
                    .into_iter()
                    .map(|(parent, children)| {
                        (
                            parent.clone(),
                            children.into_iter().cloned().collect::<Vec<_>>(),
                        )
                    })
                    .collect::<Vec<_>>();

                if communities.is_empty() {
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("Create communities")
                                    .size(17.0)
                                    .color(palette.text),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Bring members together in topic-based groups and easily send \
                                     them admin announcements. Your personal messages in \
                                     communities are end-to-end encrypted",
                                )
                                .size(13.0)
                                .color(palette.secondary),
                            );
                            ui.add_space(10.0);
                            if status_share_tile(ui, &palette, Icon::Groups, "New community") {
                                self.notice = Some((
                                    "Creating a community needs the phone".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        });
                    });
                    return;
                }

                // The official screen opens the first community, so its groups
                // (announcements first) are visible without a click.
                if self.communities_active.is_none() {
                    self.communities_active = communities.first().map(|(p, _)| p.jid.clone());
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(8.0);
                        let _ = &self.communities_active;
                        for (parent, children) in &communities {
                            self.ensure_avatar(ui.ctx(), &parent.jid);
                            let avatar = self.avatar_for(&parent.jid).cloned();
                            let selected =
                                self.communities_active.as_deref() == Some(parent.jid.as_str());
                            let (rect, response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), 72.0),
                                egui::Sense::click(),
                            );
                            let card = rect.shrink2(egui::vec2(8.0, 0.0));
                            if selected || response.hovered() {
                                ui.painter().rect_filled(
                                    card,
                                    egui::CornerRadius::same(12),
                                    if selected {
                                        palette.selected
                                    } else {
                                        palette.hover
                                    },
                                );
                            }
                            // Painted (not allocated): allocating inside the row
                            // pushed the avatar onto the next line.
                            let centre = egui::pos2(rect.left() + 44.0, rect.center().y);
                            if let Some(texture) = avatar.as_ref() {
                                ui.painter().image(
                                    texture.id(),
                                    egui::Rect::from_center_size(centre, egui::vec2(48.0, 48.0)),
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            } else {
                                ui.painter().circle_filled(
                                    centre,
                                    24.0,
                                    avatar_color(&parent.name),
                                );
                                ui.painter().text(
                                    centre,
                                    egui::Align2::CENTER_CENTER,
                                    initials(&parent.name),
                                    egui::FontId::proportional(16.0),
                                    egui::Color32::WHITE,
                                );
                            }
                            ui.painter().text(
                                egui::pos2(rect.left() + 82.0, rect.center().y - 9.0),
                                egui::Align2::LEFT_CENTER,
                                fit_text(
                                    ui.painter(),
                                    &parent.name,
                                    egui::FontId::proportional(15.0),
                                    palette.text,
                                    rect.width() - 100.0,
                                ),
                                egui::FontId::proportional(15.0),
                                palette.text,
                            );
                            let count = children.len();
                            ui.painter().text(
                                egui::pos2(rect.left() + 82.0, rect.center().y + 11.0),
                                egui::Align2::LEFT_CENTER,
                                if count == 1 {
                                    "1 group".to_string()
                                } else {
                                    format!("{count} groups")
                                },
                                egui::FontId::proportional(12.5),
                                palette.secondary,
                            );
                            if response.clicked() {
                                self.communities_active = Some(parent.jid.clone());
                                if self.model.request_group(&parent.jid) {
                                    self.send(Command::GroupInfo {
                                        chat: parent.jid.clone(),
                                    });
                                }
                            }

                            // The selected community opens its groups below it.
                            if selected {
                                ui.add_space(2.0);
                                let shown: Vec<&crate::model::ChatRow> = if self.communities_all {
                                    children.iter().collect()
                                } else {
                                    children.iter().take(4).collect()
                                };
                                for child in shown {
                                    let (row, hit) = ui.allocate_exact_size(
                                        egui::vec2(ui.available_width(), 56.0),
                                        egui::Sense::click(),
                                    );
                                    if hit.hovered() {
                                        ui.painter().rect_filled(
                                            row.shrink2(egui::vec2(8.0, 0.0)),
                                            egui::CornerRadius::same(12),
                                            palette.hover,
                                        );
                                    }
                                    self.ensure_avatar(ui.ctx(), &child.jid);
                                    let avatar = self.avatar_for(&child.jid).cloned();
                                    let dot = egui::pos2(row.left() + 26.0, row.center().y);
                                    if let Some(texture) = avatar.as_ref() {
                                        ui.painter().image(
                                            texture.id(),
                                            egui::Rect::from_center_size(
                                                dot,
                                                egui::vec2(36.0, 36.0),
                                            ),
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                    } else {
                                        ui.painter().circle_filled(
                                            dot,
                                            18.0,
                                            avatar_color(&child.name),
                                        );
                                        ui.painter().text(
                                            dot,
                                            egui::Align2::CENTER_CENTER,
                                            initials(&child.name),
                                            egui::FontId::proportional(13.0),
                                            egui::Color32::WHITE,
                                        );
                                    }
                                    // The official row is name + last message
                                    // + stamp, not just the name.
                                    let (preview, stamp) = self
                                        .model
                                        .chats
                                        .get(&child.jid)
                                        .map(|chat| {
                                            (
                                                self.model.row_preview(chat),
                                                self.model.row_stamp(chat),
                                            )
                                        })
                                        .unwrap_or_default();
                                    ui.painter().text(
                                        egui::pos2(row.left() + 70.0, row.center().y - 9.0),
                                        egui::Align2::LEFT_CENTER,
                                        fit_text(
                                            ui.painter(),
                                            &child.name,
                                            egui::FontId::proportional(14.5),
                                            palette.text,
                                            row.width() - 130.0,
                                        ),
                                        egui::FontId::proportional(14.5),
                                        palette.text,
                                    );
                                    if !preview.is_empty() {
                                        ui.painter().text(
                                            egui::pos2(row.left() + 70.0, row.center().y + 10.0),
                                            egui::Align2::LEFT_CENTER,
                                            fit_text(
                                                ui.painter(),
                                                &crate::widgets::single_line(&preview),
                                                egui::FontId::proportional(12.5),
                                                palette.secondary,
                                                row.width() - 130.0,
                                            ),
                                            egui::FontId::proportional(12.5),
                                            palette.secondary,
                                        );
                                    }
                                    if !stamp.is_empty() {
                                        ui.painter().text(
                                            egui::pos2(row.right() - 22.0, row.center().y - 9.0),
                                            egui::Align2::RIGHT_CENTER,
                                            stamp,
                                            egui::FontId::proportional(11.0),
                                            palette.secondary,
                                        );
                                    }
                                    // The live row marks the announcement
                                    // group with the crossed bell.
                                    if child.announce {
                                        draw_icon(
                                            ui,
                                            egui::Rect::from_center_size(
                                                egui::pos2(row.right() - 74.0, row.center().y),
                                                egui::vec2(13.0, 13.0),
                                            ),
                                            Icon::BellOff,
                                            palette.secondary,
                                        );
                                    }
                                    if hit.clicked() {
                                        let jid = child.jid.clone();
                                        self.show_communities = false;
                                        self.model.open_chat(&jid);
                                    }
                                }
                            }
                            if !self.communities_all && children.len() > 4 {
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(46.0);
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("View all")
                                                    .size(13.0)
                                                    .color(palette.accent),
                                            )
                                            .frame(false),
                                        )
                                        .clicked()
                                    {
                                        self.communities_all = true;
                                    }
                                });
                            }
                        }
                    });
            });

        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.28).clamp(24.0, 240.0));
                    let (icon, _) =
                        ui.allocate_exact_size(egui::vec2(56.0, 56.0), egui::Sense::hover());
                    crate::app::draw_icon(ui, icon, Icon::Groups, egui::Color32::from_gray(72));
                    ui.add_space(12.0);
                    ui.label(
                        egui::RichText::new("Create communities")
                            .size(23.0)
                            .strong()
                            .color(palette.text),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(
                            "Bring members together in topic-based groups and easily send them \
                             admin announcements.",
                        )
                        .size(13.5)
                        .color(palette.secondary),
                    );
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        let (lock, _) =
                            ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        crate::app::draw_icon(ui, lock, Icon::Lock, palette.secondary);
                        ui.label(
                            egui::RichText::new(
                                "Your personal messages in communities are end-to-end encrypted",
                            )
                            .size(11.5)
                            .color(palette.secondary),
                        );
                    });
                });
            });
    }
}
