//! The emoji and sticker pickers: the square glyph cells, the grid, the recents,
//! the tabs and the GIF tab.

use crate::app::{Icon, WaDesk, draw_icon, emoji_category};
use crate::bubbles::QUOTE_PX;
use crate::bubbles::{STICKER_PX, texture_at};
use crate::model::Command;
use crate::settings_ui::settings_link;

impl WaDesk {
    /// The emoji picker: search, recents, and a wrapped grid that inserts into
    /// the composer.
    /// The emoji picker: the official panel - category tabs, a rounded search
    /// field, colour emoji in a nine-column grid and the Emoji / GIF / Stickers
    /// footer. Glyphs come from the system colour font (`emoji_glyph`), so they
    /// look like the phone's rather than egui's monochrome outlines.
    pub(crate) fn emoji_window(&mut self, ctx: &egui::Context) {
        if !self.model.emoji_open {
            return;
        }
        let palette = self.palette.clone();
        let groups = wa_ui::emoji::groups();
        let mut clicked: Option<String> = None;
        let mut gif_send: Option<String> = None;
        let mut stickers = false;

        egui::Area::new(egui::Id::new("emoji-panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-96.0, -84.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(364.0);
                        // Category tabs: recents first, then the catalogue.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 1.0;
                            for (index, group) in groups.iter().enumerate() {
                                let glyph = group.icon.chars().next().unwrap_or('?');
                                let texture = self.emoji_texture(ui.ctx(), glyph, 20);
                                if emoji_category(
                                    ui,
                                    &palette,
                                    &group.icon,
                                    texture,
                                    self.emoji_group == index && !self.emoji_gif,
                                )
                                .clicked()
                                {
                                    self.emoji_group = index;
                                    self.emoji_gif = false;
                                }
                            }
                        });
                        ui.add_space(8.0);
                        // Rounded search field.
                        egui::Frame::new()
                            .fill(palette.panel)
                            .stroke(egui::Stroke::new(1.0_f32, palette.accent))
                            .corner_radius(egui::CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(16.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Search, palette.secondary);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.model.emoji_query)
                                            .frame(false)
                                            .hint_text("Search emoji")
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            });
                        ui.add_space(6.0);

                        let query = self.model.emoji_query.trim().to_string();
                        let height = (ui.ctx().screen_rect().height() * 0.42).clamp(180.0, 320.0);
                        egui::ScrollArea::vertical()
                            .max_height(height)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                                if self.emoji_gif {
                                    // GIFs of this chat, newest first: WhatsApp's
                                    // picker sends a recent one again.
                                    let gifs = self.model.gif_items();
                                    if gifs.is_empty() {
                                        ui.add_space(10.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "No GIFs in this chat yet - a GIF someone \
                                                 sends shows up here.",
                                            )
                                            .size(12.5)
                                            .color(palette.secondary),
                                        );
                                    } else {
                                        ui.label(
                                            egui::RichText::new("GIFs in this chat")
                                                .size(11.5)
                                                .strong()
                                                .color(palette.secondary),
                                        );
                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                            for (id, path) in &gifs {
                                                let (rect, response) = ui.allocate_exact_size(
                                                    egui::vec2(108.0, 108.0),
                                                    egui::Sense::click(),
                                                );
                                                if response.hovered() {
                                                    ui.painter().rect_filled(
                                                        rect,
                                                        egui::CornerRadius::same(10),
                                                        palette.hover,
                                                    );
                                                }
                                                let key = format!("gif:{id}");
                                                if let Some(texture) = texture_at(
                                                    ui.ctx(),
                                                    &mut self.textures,
                                                    &key,
                                                    (!path.is_empty()).then(|| path.clone()),
                                                    QUOTE_PX,
                                                ) {
                                                    let size = texture.size_vec2();
                                                    let scale = (100.0_f32 / size.x.max(1.0))
                                                        .min(100.0 / size.y.max(1.0));
                                                    ui.painter().image(
                                                        texture.id(),
                                                        egui::Rect::from_center_size(
                                                            rect.center(),
                                                            size * scale,
                                                        ),
                                                        egui::Rect::from_min_max(
                                                            egui::pos2(0.0, 0.0),
                                                            egui::pos2(1.0, 1.0),
                                                        ),
                                                        egui::Color32::WHITE,
                                                    );
                                                } else {
                                                    ui.painter().text(
                                                        rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "GIF",
                                                        egui::FontId::proportional(14.0),
                                                        palette.secondary,
                                                    );
                                                }
                                                if response.clicked() {
                                                    gif_send = Some(id.clone());
                                                }
                                            }
                                        });
                                    }
                                } else if !query.is_empty() {
                                    let items = wa_ui::emoji::search(&query, 160);
                                    let chars: Vec<String> =
                                        items.iter().map(|item| item.char.clone()).collect();
                                    clicked = self.emoji_grid(ui, &palette, &chars);
                                } else if self.emoji_group == 0
                                    && !self.model.emoji_recents.is_empty()
                                {
                                    let recents = self.model.emoji_recents.clone();
                                    clicked = self.emoji_grid(ui, &palette, &recents);
                                } else {
                                    // Recents with nothing remembered yet show the
                                    // first catalogue group, like the phone does.
                                    let index = self.emoji_group.saturating_sub(1);
                                    if let Some(group) = groups.get(index) {
                                        ui.label(
                                            egui::RichText::new(if group.label == "Smileys" {
                                                "Smileys & People"
                                            } else {
                                                group.label.as_str()
                                            })
                                            .size(11.5)
                                            .strong()
                                            .color(palette.secondary),
                                        );
                                        ui.add_space(4.0);
                                        let chars: Vec<String> = group
                                            .items
                                            .iter()
                                            .map(|item| item.char.clone())
                                            .collect();
                                        clicked = self.emoji_grid(ui, &palette, &chars);
                                    }
                                }
                            });

                        ui.add_space(4.0);
                        ui.separator();
                        // The live footer is three icon tabs with the selected one underlined;
                        // there is no Close button (the smiley toggles the panel).
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 26.0;
                            ui.add_space(10.0);
                            let active = |selected: bool| {
                                if selected {
                                    palette.accent
                                } else {
                                    palette.secondary
                                }
                            };
                            let (emoji_rect, emoji_response) = ui
                                .allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            draw_icon(ui, emoji_rect, Icon::Emoji, active(!self.emoji_gif));
                            if !self.emoji_gif {
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(emoji_rect.left(), emoji_rect.bottom() + 6.0),
                                        egui::pos2(emoji_rect.right(), emoji_rect.bottom() + 6.0),
                                    ],
                                    egui::Stroke::new(2.0_f32, palette.accent),
                                );
                            }
                            if emoji_response.clicked() {
                                self.emoji_gif = false;
                            }
                            let gif = ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("GIF")
                                            .size(13.0)
                                            .strong()
                                            .color(active(self.emoji_gif)),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("GIF");
                            if self.emoji_gif {
                                let rect = gif.rect;
                                ui.painter().line_segment(
                                    [
                                        egui::pos2(rect.left(), rect.bottom() + 4.0),
                                        egui::pos2(rect.right(), rect.bottom() + 4.0),
                                    ],
                                    egui::Stroke::new(2.0_f32, palette.accent),
                                );
                            }
                            if gif.clicked() {
                                self.emoji_gif = true;
                            }
                            let (sticker_rect, sticker_response) = ui
                                .allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            draw_icon(ui, sticker_rect, Icon::Sticker, palette.secondary);
                            if sticker_response.clicked() {
                                stickers = true;
                            }
                        });
                    });
            });

        if let Some(emoji) = clicked {
            self.model.insert_emoji(&emoji);
        }
        if let Some(id) = gif_send
            && let Some(chat) = self.model.active.clone()
        {
            self.send(Command::ForwardMessage { chat, id });
            self.model.emoji_open = false;
        }
        if stickers {
            self.model.emoji_open = false;
            self.model.stickers_open = true;
            self.send(Command::FavoriteStickers);
        }
    }

    /// The sticker picker: the account's favourites, then the stickers the
    /// session already downloaded from the chats. Clicking sends one.
    /// The sticker picker: the official panel - Recents / Favourites / Packs
    /// tabs, a store search field, a grid of stickers, the Create tile and the
    /// shared Emoji / GIF / Stickers footer.
    pub(crate) fn stickers_window(&mut self, ctx: &egui::Context) {
        if !self.model.stickers_open {
            return;
        }
        let palette = self.palette.clone();
        let chat = self.model.active.clone().unwrap_or_default();
        let favorites = self.model.favorites.clone();
        let error = self.model.favorites_error.clone();
        let recent = self.model.sticker_collection();
        let mut send: Option<Command> = None;
        // The sticker the user just tapped, so its bubble can appear before
        // the upload finishes.
        let mut sticker_preview: Option<(String, String)> = None;
        let mut emoji_tab = false;
        let mut gif_tab = false;

        egui::Area::new(egui::Id::new("sticker-panel"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-96.0, -84.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(palette.elevated)
                    .corner_radius(egui::CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(364.0);
                        // Tabs: recents, favourites, packs.
                        // The live tab row is icons with the selected one underlined:
                        // clock (recents), star (favourites), packs, then `+`.
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 22.0;
                            let recent = self.emoji_texture(ui.ctx(), '\u{1F553}', 20);
                            for (index, glyph) in [(0usize, "recent"), (1, "fav"), (2, "packs")] {
                                let selected = self.sticker_tab == index;
                                let colour = if selected {
                                    palette.accent
                                } else {
                                    palette.secondary
                                };
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(22.0, 22.0),
                                    egui::Sense::click(),
                                );
                                match glyph {
                                    "recent" => {
                                        if let Some(texture) = recent.clone() {
                                            let size = texture.size_vec2();
                                            let scale = (20.0 / size.x.max(1.0))
                                                .min(20.0 / size.y.max(1.0));
                                            ui.painter().image(
                                                texture.id(),
                                                egui::Rect::from_center_size(
                                                    rect.center(),
                                                    size * scale,
                                                ),
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                egui::Color32::WHITE,
                                            );
                                        }
                                    }
                                    "fav" => draw_icon(ui, rect, Icon::Star, colour),
                                    _ => draw_icon(ui, rect, Icon::Sticker, colour),
                                }
                                if selected {
                                    ui.painter().line_segment(
                                        [
                                            egui::pos2(rect.left(), rect.bottom() + 5.0),
                                            egui::pos2(rect.right(), rect.bottom() + 5.0),
                                        ],
                                        egui::Stroke::new(2.0_f32, palette.accent),
                                    );
                                }
                                if response.clicked() {
                                    self.sticker_tab = index;
                                    if index == 2
                                        && self.model.packs.is_empty()
                                        && !self.packs_requested
                                    {
                                        self.packs_requested = true;
                                        self.send(Command::StickerPacks);
                                    }
                                }
                            }
                            // The live row ends with a `+` that adds a pack from the store.
                            let (plus, plus_response) =
                                ui.allocate_exact_size(egui::vec2(22.0, 22.0), egui::Sense::click());
                            draw_icon(ui, plus, Icon::Add, palette.secondary);
                            if plus_response
                                .on_hover_text("Add a sticker pack")
                                .clicked()
                            {
                                self.notice = Some((
                                    "Adding packs is done from the store on the phone".to_string(),
                                    std::time::Instant::now(),
                                ));
                            }
                        });
                        ui.add_space(8.0);
                        egui::Frame::new()
                            .fill(palette.panel)
                            .stroke(egui::Stroke::new(1.0_f32, palette.accent))
                            .corner_radius(egui::CornerRadius::same(12))
                            .inner_margin(egui::Margin::symmetric(10, 5))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let (rect, _) = ui.allocate_exact_size(
                                        egui::vec2(16.0, 16.0),
                                        egui::Sense::hover(),
                                    );
                                    draw_icon(ui, rect, Icon::Search, palette.secondary);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.sticker_query)
                                            .frame(false)
                                            .hint_text("Search via WhatsApp sticker store")
                                            .desired_width(f32::INFINITY),
                                    );
                                });
                            });
                        ui.add_space(6.0);

                        if chat.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("Open a chat to send a sticker")
                                    .size(12.5)
                                    .color(palette.secondary),
                            );
                        }
                        if let Some(err) = error {
                            ui.label(
                                egui::RichText::new(format!("Favourites: {err}"))
                                    .size(11.5)
                                    .color(palette.danger),
                            );
                        }

                        let query = self.sticker_query.trim().to_lowercase();
                        let mut rows: Vec<(String, String, bool)> = Vec::new();
                        match self.sticker_tab {
                            1 => {
                                for favorite in &favorites {
                                    rows.push((favorite.id.clone(), favorite.path.clone(), true));
                                }
                            }
                            2 => {}
                            _ => {
                                for (id, path) in &recent {
                                    rows.push((id.clone(), path.clone(), false));
                                }
                            }
                        }
                        if !query.is_empty() {
                            rows.retain(|(_, path, _)| path.to_lowercase().contains(&query));
                        }

                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                if self.sticker_tab == 2 {
                                    // Packs shared in a chat: WhatsApp never syncs the
                                    // account's pack list to linked devices.
                                    if self.model.packs.is_empty() {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "No sticker packs yet - a pack shows up \
                                                 here when someone shares one in a chat.",
                                            )
                                            .size(12.5)
                                            .color(palette.secondary),
                                        );
                                        ui.add_space(6.0);
                                        if settings_link(ui, &palette, "Check again")
                                            && !self.packs_requested
                                        {
                                            self.packs_requested = true;
                                            self.send(Command::StickerPacks);
                                        }
                                    } else {
                                        let packs = self.model.packs.clone();
                                        let active = self.model.active_pack.clone();
                                        ui.add_space(6.0);
                                        for pack in &packs {
                                            let selected =
                                                active.as_deref() == Some(pack.id.as_str());
                                            let label = format!(
                                                "{} - {} stickers",
                                                if pack.publisher.is_empty() {
                                                    pack.name.clone()
                                                } else {
                                                    format!("{} ({})", pack.name, pack.publisher)
                                                },
                                                pack.stickers.len()
                                            );
                                            let (rect, response) = ui.allocate_exact_size(
                                                egui::vec2(ui.available_width(), 30.0),
                                                egui::Sense::click(),
                                            );
                                            if selected || response.hovered() {
                                                ui.painter().rect_filled(
                                                    rect,
                                                    egui::CornerRadius::same(8),
                                                    if selected {
                                                        palette.selected
                                                    } else {
                                                        palette.hover
                                                    },
                                                );
                                            }
                                            ui.painter().text(
                                                egui::pos2(rect.left() + 8.0, rect.center().y),
                                                egui::Align2::LEFT_CENTER,
                                                &label,
                                                egui::FontId::proportional(12.5),
                                                palette.text,
                                            );
                                            if response.clicked() {
                                                self.model.active_pack = Some(pack.id.clone());
                                                self.send(Command::StickerPackItems {
                                                    id: pack.id.clone(),
                                                });
                                            }
                                        }
                                        ui.add_space(6.0);
                                        let stickers = self.model.active_pack_stickers();
                                        if stickers.iter().all(|item| item.path.is_empty()) {
                                            ui.label(
                                                egui::RichText::new("Downloading the pack...")
                                                    .size(12.0)
                                                    .color(palette.secondary),
                                            );
                                        } else {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.spacing_mut().item_spacing =
                                                    egui::vec2(6.0, 6.0);
                                                for item in &stickers {
                                                    if item.path.is_empty() {
                                                        continue;
                                                    }
                                                    let key = format!("pack:{}", item.path);
                                                    let Some(texture) = texture_at(
                                                        ui.ctx(),
                                                        &mut self.sticker_textures,
                                                        &key,
                                                        Some(item.path.clone()),
                                                        STICKER_PX,
                                                    ) else {
                                                        continue;
                                                    };
                                                    let (rect, response) = ui.allocate_exact_size(
                                                        egui::vec2(74.0, 74.0),
                                                        egui::Sense::click(),
                                                    );
                                                    if response.hovered() {
                                                        ui.painter().rect_filled(
                                                            rect,
                                                            egui::CornerRadius::same(10),
                                                            palette.hover,
                                                        );
                                                    }
                                                    let size = texture.size_vec2();
                                                    let scale = (66.0_f32 / size.x.max(1.0))
                                                        .min(66.0 / size.y.max(1.0));
                                                    ui.painter().image(
                                                        texture.id(),
                                                        egui::Rect::from_center_size(
                                                            rect.center(),
                                                            size * scale,
                                                        ),
                                                        egui::Rect::from_min_max(
                                                            egui::pos2(0.0, 0.0),
                                                            egui::pos2(1.0, 1.0),
                                                        ),
                                                        egui::Color32::WHITE,
                                                    );
                                                    if response.clicked() && !chat.is_empty() {
                                                        use base64::Engine as _;
                                                        sticker_preview =
                                                            Some((chat.clone(), item.path.clone()));
                                                        let bytes = std::fs::read(&item.path)
                                                            .unwrap_or_default();
                                                        send = Some(Command::SendSticker {
                                                            chat: chat.clone(),
                                                            payload:
                                                                base64::engine::general_purpose::STANDARD
                                                                    .encode(bytes),
                                                        });
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
                                    if self.sticker_tab == 0 {
                                        // The Create tile leads the recents grid.
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(74.0, 74.0),
                                            egui::Sense::click(),
                                        );
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(10),
                                                palette.hover,
                                            );
                                        }
                                        draw_icon(
                                            ui,
                                            egui::Rect::from_center_size(
                                                rect.center(),
                                                egui::vec2(26.0, 26.0),
                                            ),
                                            Icon::Add,
                                            palette.secondary,
                                        );
                                        if response.clicked() {
                                            // The official Create tile turns a picture
                                            // into a 512x512 WebP and sends it.
                                            if let Some(path) = rfd::FileDialog::new()
                                                .add_filter(
                                                    "Image",
                                                    &["png", "jpg", "jpeg", "webp"],
                                                )
                                                .pick_file()
                                            {
                                                match crate::sticker::make_sticker(&path) {
                                                    Ok(bytes) => {
                                                        use base64::Engine as _;
                                                        if !chat.is_empty() {
                                                            send = Some(Command::SendSticker {
                                                                chat: chat.clone(),
                                                                payload:
                                                                    base64::engine::general_purpose::STANDARD
                                                                        .encode(bytes),
                                                            });
                                                        }
                                                    }
                                                    Err(err) => {
                                                        self.notice = Some((
                                                            err,
                                                            std::time::Instant::now(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    for (id, path, favorite) in &rows {
                                        let key = format!("sticker:{id}");
                                        let Some(texture) = texture_at(
                                            ui.ctx(),
                                            &mut self.sticker_textures,
                                            &key,
                                            Some(path.clone()),
                                            STICKER_PX,
                                        ) else {
                                            continue;
                                        };
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(74.0, 74.0),
                                            egui::Sense::click(),
                                        );
                                        if response.hovered() {
                                            ui.painter().rect_filled(
                                                rect,
                                                egui::CornerRadius::same(10),
                                                palette.hover,
                                            );
                                        }
                                        let size = texture.size_vec2();
                                        let scale =
                                            (66.0_f32 / size.x.max(1.0)).min(66.0 / size.y.max(1.0));
                                        ui.painter().image(
                                            texture.id(),
                                            egui::Rect::from_center_size(
                                                rect.center(),
                                                size * scale,
                                            ),
                                            egui::Rect::from_min_max(
                                                egui::pos2(0.0, 0.0),
                                                egui::pos2(1.0, 1.0),
                                            ),
                                            egui::Color32::WHITE,
                                        );
                                        if response.clicked() && !chat.is_empty() {
                                            sticker_preview =
                                                Some((chat.clone(), path.clone()));
                                            send = Some(if *favorite {
                                                Command::SendFavoriteSticker {
                                                    chat: chat.clone(),
                                                    id: id.clone(),
                                                }
                                            } else {
                                                Command::ForwardSticker {
                                                    chat: chat.clone(),
                                                    message_id: id.clone(),
                                                }
                                            });
                                        }
                                    }
                                });
                                if rows.is_empty() && self.sticker_tab != 2 {
                                    ui.add_space(10.0);
                                    ui.label(
                                        egui::RichText::new(if self.sticker_tab == 1 {
                                            "No favourite stickers yet."
                                        } else {
                                            "Send or receive a sticker and it shows up here."
                                        })
                                        .size(12.5)
                                        .color(palette.secondary),
                                    );
                                }
                            });

                        ui.add_space(4.0);
                        ui.separator();
                        // Same icon footer as the emoji panel: emoji, GIF, sticker
                        // (selected, underlined), no Close - the sticker control
                        // in the composer toggles the panel.
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.spacing_mut().item_spacing.x = 26.0;
                            let (emoji_rect, emoji_response) = ui.allocate_exact_size(
                                egui::vec2(22.0, 22.0),
                                egui::Sense::click(),
                            );
                            draw_icon(ui, emoji_rect, Icon::Emoji, palette.secondary);
                            if emoji_response.clicked() {
                                emoji_tab = true;
                            }
                            let gif = ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("GIF")
                                            .size(13.0)
                                            .strong()
                                            .color(palette.secondary),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("GIF");
                            if gif.clicked() {
                                gif_tab = true;
                            }
                            let (sticker_rect, _) = ui.allocate_exact_size(
                                egui::vec2(22.0, 22.0),
                                egui::Sense::hover(),
                            );
                            draw_icon(ui, sticker_rect, Icon::Sticker, palette.accent);
                            ui.painter().line_segment(
                                [
                                    egui::pos2(sticker_rect.left(), sticker_rect.bottom() + 6.0),
                                    egui::pos2(sticker_rect.right(), sticker_rect.bottom() + 6.0),
                                ],
                                egui::Stroke::new(2.0_f32, palette.accent),
                            );
                        });
                    });
            });

        if let Some(command) = send {
            if let Some((chat, path)) = sticker_preview {
                self.model.add_own_sticker(&chat, &path);
            }
            self.send(command);
            // One tap sends one sticker; the panel closes immediately like the
            // official picker instead of letting a slow upload double-send.
            self.model.stickers_open = false;
        }
        if emoji_tab {
            self.model.stickers_open = false;
            self.model.emoji_open = true;
            self.emoji_gif = false;
        }
        if gif_tab {
            self.model.stickers_open = false;
            self.model.emoji_open = true;
            self.emoji_gif = true;
        }
    }

    /// A colour texture for one emoji glyph.
    fn emoji_texture(
        &mut self,
        ctx: &egui::Context,
        ch: char,
        px: u32,
    ) -> Option<egui::TextureHandle> {
        crate::emoji_glyph::texture(ctx, &mut self.emoji_textures, ch, px)
    }

    /// One emoji cell: the colour glyph, a hover pill, click to insert.
    fn emoji_cell(
        &mut self,
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
        emoji: &str,
    ) -> bool {
        // 40 px cell with a 30 px glyph: the official grid breathes.
        let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
        if response.hovered() {
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(8), palette.hover);
        }
        let glyph = emoji.chars().next().unwrap_or('?');
        if let Some(texture) = self.emoji_texture(ui.ctx(), glyph, 30) {
            let size = texture.size_vec2();
            let scale = (30.0 / size.x.max(1.0)).min(30.0 / size.y.max(1.0));
            ui.painter().image(
                texture.id(),
                egui::Rect::from_center_size(rect.center(), size * scale),
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                emoji,
                egui::FontId::proportional(19.0),
                palette.text,
            );
        }
        response.clicked()
    }

    /// The nine-column grid of emoji.
    fn emoji_grid(
        &mut self,
        ui: &mut egui::Ui,
        palette: &crate::theme::Palette,
        chars: &[String],
    ) -> Option<String> {
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(2.0, 2.0);
            for emoji in chars {
                if self.emoji_cell(ui, palette, emoji) {
                    clicked = Some(emoji.clone());
                }
            }
        });
        clicked
    }
}
