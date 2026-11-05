//! The login gate's artwork and cards: the demo QR payload, the card shell, the
//! numbered step, the link line, the WhatsApp mark and the laptop illustration.
//! Pure painters over a `Ui` and the palette, so they sit outside `app.rs`.

/// A dummy QR bitmap for `--preview` (`WA_DESK_LOGIN=qr`): it looks like a
/// code without being one, which is all a layout screenshot needs.
pub(crate) fn demo_qr_svg() -> String {
    let modules = 25;
    let mut path = String::new();
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    for y in 0..modules {
        for x in 0..modules {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            if seed % 100 < 46 {
                path.push_str(&format!("M{x} {y}h1v1h-1z"));
            }
        }
    }
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"300\" height=\"300\" \
         viewBox=\"0 0 {modules} {modules}\">\
         <rect width=\"{modules}\" height=\"{modules}\" fill=\"#ffffff\"/>\
         <path d=\"{path}\" fill=\"#111b21\"/></svg>"
    )
}

/// One card of the login page: rounded, bordered, generous padding.
pub(crate) fn gate_card(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    width: f32,
    contents: impl FnOnce(&mut egui::Ui),
) {
    let margin = 26.0;
    ui.allocate_ui(egui::vec2(width, 0.0), |ui| {
        egui::Frame::new()
            .fill(palette.elevated)
            .stroke(egui::Stroke::new(
                1.0_f32,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 22),
            ))
            .corner_radius(egui::CornerRadius::same(24))
            .inner_margin(egui::Margin::same(margin as i8))
            .show(ui, |ui| {
                ui.set_width(width - margin * 2.0);
                contents(ui);
            });
    });
}

/// One numbered step of the scan card: the outlined disc and its text.
pub(crate) fn gate_step(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    number: i32,
    contents: impl FnOnce(&mut egui::Ui, &crate::theme::Palette),
) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(24.0, 24.0), egui::Sense::hover());
        ui.painter().circle_stroke(
            rect.center(),
            11.0,
            egui::Stroke::new(1.2_f32, palette.secondary),
        );
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            number.to_string(),
            egui::FontId::proportional(12.5),
            palette.secondary,
        );
        ui.add_space(8.0);
        contents(ui, palette);
    });
}

/// A green underlined link, the way the official page draws them.
pub(crate) fn gate_link(
    ui: &mut egui::Ui,
    palette: &crate::theme::Palette,
    text: &str,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        text.to_string(),
        egui::FontId::proportional(14.0),
        palette.accent,
    );
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(galley.size().x, galley.size().y + 2.0),
        egui::Sense::click(),
    );
    let color = if response.hovered() {
        palette.accent.gamma_multiply(0.85)
    } else {
        palette.accent
    };
    let underline_y = rect.bottom() - 1.0;
    ui.painter().line_segment(
        [
            egui::pos2(rect.left(), underline_y),
            egui::pos2(rect.right(), underline_y),
        ],
        egui::Stroke::new(1.0_f32, color),
    );
    ui.painter()
        .galley(egui::pos2(rect.left(), rect.top()), galley, color);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The WhatsApp glyph: a speech bubble outline with a handset inside.
pub(crate) fn draw_whatsapp_mark(ui: &egui::Ui, rect: egui::Rect, color: egui::Color32) {
    let radius = rect.width() * 0.42;
    ui.painter()
        .circle_stroke(rect.center(), radius, egui::Stroke::new(1.6_f32, color));
    // The tail: a small triangle at the bottom left.
    let center = rect.center();
    ui.painter().add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(center.x - radius * 0.72, center.y + radius * 0.68),
            egui::pos2(center.x - radius * 0.30, center.y + radius * 0.74),
            egui::pos2(center.x - radius * 0.62, center.y + radius * 1.16),
        ],
        color,
        egui::Stroke::NONE,
    ));
    // The handset: a short thick stroke on a diagonal inside the disc.
    ui.painter().line_segment(
        [
            egui::pos2(center.x - radius * 0.34, center.y + radius * 0.30),
            egui::pos2(center.x + radius * 0.36, center.y - radius * 0.34),
        ],
        egui::Stroke::new(rect.width() * 0.22, color),
    );
}

/// The laptop drawing of the download card, in the accent green.
pub(crate) fn draw_laptop(ui: &egui::Ui, rect: egui::Rect, palette: &crate::theme::Palette) {
    let painter = ui.painter();
    let green = palette.accent;
    let screen = egui::Rect::from_min_size(
        egui::pos2(
            rect.left() + rect.width() * 0.13,
            rect.top() + rect.height() * 0.06,
        ),
        egui::vec2(rect.width() * 0.74, rect.height() * 0.62),
    );
    painter.rect_stroke(
        screen,
        egui::CornerRadius::same(6),
        egui::Stroke::new(2.0_f32, green),
        egui::StrokeKind::Inside,
    );
    let bar = egui::Rect::from_min_size(
        screen.min + egui::vec2(6.0, 6.0),
        egui::vec2(screen.width() * 0.34, screen.height() - 12.0),
    );
    painter.rect_filled(bar, egui::CornerRadius::same(3), green);
    let phone = egui::Rect::from_min_size(
        egui::pos2(screen.right() - 26.0, screen.top() + 10.0),
        egui::vec2(18.0, 30.0),
    );
    painter.rect_filled(phone, egui::CornerRadius::same(4), green);
    let base = egui::Rect::from_min_size(
        egui::pos2(rect.left() + rect.width() * 0.06, screen.bottom() + 3.0),
        egui::vec2(rect.width() * 0.88, 5.0),
    );
    painter.rect_filled(base, egui::CornerRadius::same(3), green);
}
