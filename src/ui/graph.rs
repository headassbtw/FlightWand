use eframe::epaint::{Color32, Rounding, Stroke};
use egui::{FontId, Painter, Rect, pos2, vec2};

const NUM_WIDTH: f32 = 50.0;

#[profiling::function]
fn draw_line(
    left: f32,
    right: f32,
    idx: usize,
    advance: f32,
    last: bool,
    color: Color32,
    painter: &Painter,
    rect: Rect,
) {
    let value = right;
    let left = ((left + 1.0) / 2.0) * rect.height();
    let right = ((value + 1.0) / 2.0) * rect.height();

    let left_x = rect.min.x + (advance * (idx - 1) as f32);
    let right_x = rect.min.x + (advance * idx as f32);

    let right_point = pos2(right_x, right + rect.min.y);

    painter.line_segment([pos2(left_x, left + rect.min.y), right_point], Stroke::new(2.0, color));

    if last && value < 2.0 && value > -2.0 {
        profiling::scope!("text");
        let galley = painter.layout(format!("{:2.3}", value), FontId::monospace(12.0), color, NUM_WIDTH);

        let lo_y = right_point.y - galley.rect.height() / 2.0;
        let hi_y = right_point.y + galley.rect.height() / 2.0;

        let lo_y = if lo_y < rect.min.y {
            rect.min.y
        } else if hi_y > rect.max.y {
            rect.max.y - galley.rect.height()
        } else {
            lo_y
        };

        painter.galley(pos2(right_x, lo_y), galley, color);
    }
}

#[profiling::function]
pub fn graph(
    items: &[[f32; 4]],
    up: [f32; 3],
    ui: &mut egui::Ui,
    modifier: impl Fn(&[f32; 4], [f32; 3]) -> [f32; 4],
) -> egui::Response {
    let height = ui.spacing().interact_size.y * 10.0;

    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_rect_before_wrap().width(), height), egui::Sense::hover());

    // this limits the rect the graph can draw out, so outrageously big values don't go outside of it
    let painter = ui.painter_at(rect);

    let width = rect.width() - NUM_WIDTH;
    let advance = width / (items.len() - 1) as f32;

    painter.line_segment(
        [rect.right_top() - vec2(NUM_WIDTH, 0.0), rect.right_bottom() - vec2(NUM_WIDTH, 0.0)],
        ui.visuals().noninteractive().bg_stroke,
    );
    painter.rect_filled(response.rect.with_max_x(response.rect.max.x - NUM_WIDTH), Rounding::ZERO, Color32::BLACK);
    painter.line_segment(
        [rect.left_center(), rect.right_center() - vec2(NUM_WIDTH, 0.0)],
        Stroke::new(2.0, Color32::GRAY),
    );

    let mut i: usize = 1;
    while i < items.len() {
        profiling::scope!(&format!("item {}", i));
        let item0 = modifier(&items[i - 1], up);
        let item1 = modifier(&items[i], up);
        let last = i == items.len() - 1;

        draw_line(item0[0], item1[0], i, advance, last, Color32::RED, &painter, rect);
        draw_line(item0[1], item1[1], i, advance, last, Color32::GREEN, &painter, rect);
        draw_line(item0[2], item1[2], i, advance, last, Color32::BLUE, &painter, rect);
        draw_line(item0[3], item1[3], i, advance, last, Color32::YELLOW, &painter, rect);

        i += 1;
    }

    ui.painter().rect(response.rect, Rounding::ZERO, Color32::TRANSPARENT, ui.visuals().noninteractive().bg_stroke);
    response
}
