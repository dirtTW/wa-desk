//! The widgets the Settings pages are built from: the section list rows, the
//! raised cards, label + description rows, the theme radios, the M3 switch, the
//! accent link, a labelled field, and where the native settings file lives.
//! They take a `Ui` and a palette, never the window, so they sit outside
//! `app.rs`.

use crate::app::{Icon, draw_icon};

/// One row of the settings section list.
pub(crate) fn settings_list_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    icon: Icon,
    title: &str,
    subtitle: Option<&str>,
    selected: bool,
    color: egui::Color32,
) -> egui::Response {
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let height = if subtitle.is_some() { 54.0 } else { 44.0 };
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width() - 10.0, height),
            egui::Sense::click(),
        );
        if selected || response.hovered() {
            ui.painter().rect_filled(
                rect,
                egui::CornerRadius::same(10),
                if selected {
                    palette.selected
                } else {
                    palette.hover
                },
            );
        }
        draw_icon(
            ui,
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 24.0, rect.center().y),
                egui::vec2(20.0, 20.0),
            ),
            icon,
            if selected { palette.accent } else { color },
        );
        let text_left = rect.left() + 46.0;
        match subtitle {
            Some(subtitle) => {
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y - 9.0),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(14.0),
                    palette.text,
                );
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y + 10.0),
                    egui::Align2::LEFT_CENTER,
                    subtitle,
                    egui::FontId::proportional(11.5),
                    palette.secondary,
                );
            }
            None => {
                ui.painter().text(
                    egui::pos2(text_left, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    title,
                    egui::FontId::proportional(14.0),
                    color,
                );
            }
        }
        response
    })
    .inner
}

/// A card in the detail pane: raised surface, green uppercase title.
pub(crate) fn settings_card(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    title: &str,
    body: impl FnOnce(&mut egui::Ui),
) {
    ui.add_space(16.0);
    egui::Frame::new()
        .fill(palette.elevated)
        .corner_radius(egui::CornerRadius::same(14))
        .inner_margin(egui::Margin::symmetric(18, 14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                egui::RichText::new(title)
                    .size(11.5)
                    .strong()
                    .color(palette.accent),
            );
            ui.add_space(10.0);
            body(ui);
        });
}

/// A label + description row with an optional trailing control.
pub(crate) fn settings_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    title: &str,
    description: &str,
    control: impl FnOnce(&mut egui::Ui),
) {
    ui.horizontal_top(|ui| {
        // Two fixed columns: egui would otherwise wrap the control onto the
        // next line once the wrapping label had taken the whole width. Both
        // columns scale with the pane, so nothing is clipped when it is narrow.
        let width = ui.available_width();
        let control_width = (width * 0.36).clamp(84.0, 168.0);
        let text_width = (width - control_width - 12.0).max(90.0);
        ui.allocate_ui_with_layout(
            egui::vec2(text_width, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(text_width);
                ui.label(egui::RichText::new(title).size(14.0).color(palette.text));
                if !description.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new(description)
                            .size(12.0)
                            .color(palette.secondary),
                    );
                }
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(control_width, 0.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.set_width(control_width);
                control(ui)
            },
        );
    });
    ui.add_space(10.0);
}

/// A full-width radio row (the theme choices).
pub(crate) fn settings_radio_row(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 34.0), egui::Sense::click());
    if response.hovered() {
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(8), palette.hover);
    }
    let center = egui::pos2(rect.right() - 18.0, rect.center().y);
    ui.painter().circle_stroke(
        center,
        8.0,
        egui::Stroke::new(
            1.8_f32,
            if selected {
                palette.accent
            } else {
                palette.secondary
            },
        ),
    );
    if selected {
        ui.painter().circle_filled(center, 4.4, palette.accent);
    }
    ui.painter().text(
        egui::pos2(rect.left() + 2.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.text,
    );
    response
}

/// A Material 3 switch.
pub(crate) fn settings_switch(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    on: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(40.0, 22.0), egui::Sense::click());
    let track = if on {
        palette.accent
    } else {
        egui::Color32::from_gray(120)
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(11), track);
    let knob_x = if on {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    ui.painter().circle_filled(
        egui::pos2(knob_x, rect.center().y),
        8.0,
        egui::Color32::WHITE,
    );
    response
}

/// An accent-coloured text action.
pub(crate) fn settings_link(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
) -> bool {
    ui.add(
        egui::Button::new(egui::RichText::new(label).size(13.0).color(palette.accent)).frame(false),
    )
    .clicked()
}

/// A labelled single-line field.
pub(crate) fn settings_field(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    label: &str,
    hint: &str,
    value: &mut String,
    password: bool,
) {
    ui.label(
        egui::RichText::new(label)
            .size(12.5)
            .color(palette.secondary),
    );
    ui.add_space(3.0);
    let mut edit = egui::TextEdit::singleline(value)
        .desired_width(f32::INFINITY)
        .hint_text(hint);
    if password {
        edit = edit.password(true);
    }
    ui.add(edit);
    ui.add_space(10.0);
}

/// Where the native app keeps its own settings.
pub(crate) fn settings_file() -> std::path::PathBuf {
    crate::settings::NativeSettings::data_dir().join("settings.json")
}
