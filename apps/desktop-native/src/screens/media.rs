//! The Media screen: the official Media / Docs / Links panel - tabs, a search
//! box, sort and select, the close control, and a grid of the media across the
//! chats with the sender's caption under each tile.

use crate::app::{Icon, WaDesk, draw_icon, icon_button};
use crate::bubbles::{PHOTO_PX, texture_at};
use wa_types::MessageView;

impl WaDesk {
    /// The Media screen, sampled from the live client on 2026-11-05: the dialog
    /// with `Media · Docs · Links`, `Search`, `Sort by`, `Select`, `Close` and a
    /// grid of tiles whose captions start "Image from …".
    pub(crate) fn media_panel(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(palette.panel))
            .show(ui, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.set_min_height(40.0);
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Media")
                                        .size(15.0)
                                        .strong()
                                        .color(palette.text),
                                );
                                ui.label(
                                    egui::RichText::new("Media from all chats")
                                        .size(11.5)
                                        .color(palette.secondary),
                                );
                            });
                            ui.add_space(24.0);
                            for (index, label) in ["Media", "Docs", "Links"].into_iter().enumerate()
                            {
                                let selected = self.media_tab == index;
                                let colour = if selected {
                                    palette.accent
                                } else {
                                    palette.secondary
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(label).size(14.0).color(colour),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    self.media_tab = index;
                                }
                                ui.add_space(10.0);
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if icon_button(ui, Icon::Close, &palette)
                                        .on_hover_text("Close")
                                        .clicked()
                                    {
                                        self.show_media = false;
                                    }
                                    ui.add_space(6.0);
                                    ui.label(
                                        egui::RichText::new("Select")
                                            .size(13.0)
                                            .color(palette.secondary),
                                    );
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new("Sort by")
                                            .size(13.0)
                                            .color(palette.secondary),
                                    );
                                    ui.add_space(10.0);
                                    if icon_button(ui, Icon::Search, &palette)
                                        .on_hover_text("Search")
                                        .clicked()
                                    {
                                        self.media_search_open = !self.media_search_open;
                                    }
                                },
                            );
                        });
                    });

                if self.media_search_open {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(16.0);
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                        draw_icon(ui, rect, Icon::Search, palette.secondary);
                        ui.add(
                            egui::TextEdit::singleline(&mut self.media_search)
                                .hint_text("Search media")
                                .desired_width(320.0),
                        );
                    });
                }

                let needle = self.media_search.trim().to_lowercase();
                let rows: Vec<(String, wa_types::MessageView)> = match self.media_tab {
                    1 => self.model.document_gallery(),
                    2 => self.model.link_gallery(),
                    _ => self.model.media_gallery(),
                };
                let rows: Vec<(String, wa_types::MessageView)> = rows
                    .into_iter()
                    .filter(|(_, message)| {
                        needle.is_empty()
                            || message.text.to_lowercase().contains(&needle)
                            || message
                                .sender_name
                                .as_deref()
                                .unwrap_or_default()
                                .to_lowercase()
                                .contains(&needle)
                    })
                    .collect();

                ui.add_space(10.0);
                if rows.is_empty() {
                    ui.add_space(40.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("No media yet")
                                .size(14.0)
                                .color(palette.secondary),
                        );
                    });
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // The live grid: four columns, a hairline gap, a day
                        // heading per run and the sender's name under each tile.
                        let cols = 4usize;
                        let gap = 3.0;
                        let tile = ((ui.available_width() - gap * (cols as f32 - 1.0))
                            / cols as f32)
                            .max(80.0);
                        let mut groups: Vec<(String, Vec<&(String, MessageView)>)> = Vec::new();
                        for row in &rows {
                            let day = wa_ui::labels::day_of(row.1.timestamp, crate::app::now_ms());
                            match groups.last_mut() {
                                Some((last, items)) if *last == day => items.push(row),
                                _ => groups.push((day, vec![row])),
                            }
                        }
                        for (day, items) in groups {
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new(day).size(12.5).color(palette.secondary));
                            ui.add_space(2.0);
                            for chunk in items.chunks(cols) {
                                ui.horizontal(|ui| {
                                    ui.spacing_mut().item_spacing.x = gap;
                                    for (chat, message) in chunk {
                                        let texture = texture_at(
                                            ui.ctx(),
                                            &mut self.textures,
                                            &format!("media-{}", message.id),
                                            self.model.media_path(&message.id),
                                            PHOTO_PX,
                                        );
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(tile, tile),
                                            egui::Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            egui::CornerRadius::ZERO,
                                            palette.hover,
                                        );
                                        if let Some(texture) = texture {
                                            let size = texture.size_vec2();
                                            let scale = (rect.width() / size.x.max(1.0))
                                                .max(rect.height() / size.y.max(1.0));
                                            let drawn = egui::Rect::from_center_size(
                                                rect.center(),
                                                size * scale,
                                            );
                                            ui.painter().with_clip_rect(rect).image(
                                                texture.id(),
                                                drawn,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                egui::Color32::WHITE,
                                            );
                                        } else {
                                            draw_icon(
                                                ui,
                                                egui::Rect::from_center_size(
                                                    rect.center(),
                                                    egui::vec2(26.0, 26.0),
                                                ),
                                                if message.kind == "video" {
                                                    Icon::Play
                                                } else {
                                                    Icon::Document
                                                },
                                                palette.secondary,
                                            );
                                        }
                                        if message.kind == "video" {
                                            // The live tile carries a film badge
                                            // in its top-left corner.
                                            let badge = egui::Rect::from_min_size(
                                                egui::pos2(rect.left() + 4.0, rect.top() + 4.0),
                                                egui::vec2(16.0, 13.0),
                                            );
                                            ui.painter().rect_filled(
                                                badge,
                                                egui::CornerRadius::same(3),
                                                egui::Color32::from_black_alpha(140),
                                            );
                                            draw_icon(
                                                ui,
                                                badge.shrink(2.0),
                                                Icon::Play,
                                                egui::Color32::from_gray(235),
                                            );
                                        }
                                        if response.clicked() {
                                            let target = chat.clone();
                                            self.model.open_chat(&target);
                                            self.show_media = false;
                                            self.model.open_viewer(&message.id);
                                        }
                                        // The sender's name sits under the tile,
                                        // like the live grid.
                                        let who = message
                                            .sender_name
                                            .clone()
                                            .unwrap_or_else(|| "you".to_string());
                                        ui.vertical(|ui| {
                                            ui.add_space(2.0);
                                            ui.label(
                                                egui::RichText::new(who)
                                                    .size(11.0)
                                                    .color(palette.secondary),
                                            );
                                        });
                                    }
                                });
                            }
                        }
                    });
            });
    }
}
