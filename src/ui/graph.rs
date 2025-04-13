use eframe::epaint::{Color32, Rounding, Stroke};
use egui::{pos2, vec2};

pub fn graph(items: &[[f32; 4]], up: [f32; 3], ui: &mut egui::Ui, modifier: impl Fn(&[f32; 4], [f32; 3]) -> [f32; 4]) -> egui::Response {
    let height = ui.spacing().interact_size.y * 10.0;

    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_rect_before_wrap().width(), height), egui::Sense::hover());

    let rect = rect.shrink(ui.visuals().noninteractive().bg_stroke.width);
    let height = height - (ui.visuals().noninteractive().bg_stroke.width * 2.0);
    let adv = rect.width() / (items.len() - 1) as f32;

    ui.painter().rect_filled(response.rect, Rounding::ZERO, Color32::BLACK);
    ui.painter().line_segment([rect.left_center(), rect.right_center()], Stroke::new(2.0, Color32::GRAY));

    let mut i: usize = 1;
    while i < items.len() {
        let item0 = modifier(&items[i - 1], up);
        let item1 = modifier(&items[i], up);
        let x0 = ((item0[0] + 1.0) / 2.0) * height;
        let y0 = ((item0[1] + 1.0) / 2.0) * height;
        let z0 = ((item0[2] + 1.0) / 2.0) * height;
        let w0 = ((item0[3] + 1.0) / 2.0) * height;

        let x1 = ((item1[0] + 1.0) / 2.0) * height;
        let y1 = ((item1[1] + 1.0) / 2.0) * height;
        let z1 = ((item1[2] + 1.0) / 2.0) * height;
        let w1 = ((item1[3] + 1.0) / 2.0) * height;

        let left = rect.min.x + adv * ((i - 1) as f32);
        let right = rect.min.x + adv * (i as f32);

        let min = rect.min.y;
        ui.painter().line_segment([pos2(left, min + x0), pos2(right, min + x1)], Stroke::new(2.0, Color32::RED));
        ui.painter().line_segment([pos2(left, min + y0), pos2(right, min + y1)], Stroke::new(2.0, Color32::GREEN));
        ui.painter().line_segment([pos2(left, min + z0), pos2(right, min + z1)], Stroke::new(2.0, Color32::BLUE));
        ui.painter().line_segment([pos2(left, min + w0), pos2(right, min + w1)], Stroke::new(2.0, Color32::YELLOW));

        i += 1;
    }

    ui.painter().rect(response.rect, Rounding::ZERO, Color32::TRANSPARENT, ui.visuals().noninteractive().bg_stroke);
    response
}
