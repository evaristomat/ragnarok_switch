//! Small native-atlas text and understated surfaces for the handheld controller UI.
pub(crate) use super::style::{ACCENT, DIM, EXP, HP, JOB_EXP, SP, TEXT, bar, fill};
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;

const SCALE: f32 = 0.82;
pub(crate) fn line(ui: &UiFrame) -> f32 {
    ui.atlas.line_height * SCALE
}
pub(crate) fn baseline(ui: &UiFrame, top: f32) -> f32 {
    top + ui.atlas.ascent * SCALE
}
pub(crate) fn measure(ui: &UiFrame, s: &str) -> f32 {
    ui.atlas.measure_text(s) * SCALE
}
pub(crate) fn text(ui: &mut UiFrame, x: f32, y: f32, s: &str, color: [f32; 4]) {
    // One shadow and one glyph batch, using the original font atlas at a smaller size.
    for (dx, dy, color) in [(0.7, 0.7, [0.0, 0.0, 0.0, 0.7]), (0.0, 0.0, color)] {
        let (vertices, indices) =
            draw::text_vertices_scaled(s, x + dx, y + dy, color, ui.atlas, SCALE);
        ui.draw_calls.push(DrawCall {
            vertices: vertices.into(),
            indices: indices.into(),
            texture: TextureRef::FontAtlas,
        });
    }
}
pub(crate) fn right(ui: &mut UiFrame, x: f32, y: f32, s: &str, c: [f32; 4]) {
    text(ui, x - measure(ui, s), y, s, c);
}
pub(crate) fn center(ui: &mut UiFrame, x: f32, y: f32, s: &str, c: [f32; 4]) {
    text(ui, x - measure(ui, s) * 0.5, y, s, c);
}
pub(crate) fn fit(ui: &UiFrame, s: &str, width: f32) -> String {
    let mut out = String::new();
    if measure(ui, s) <= width {
        return s.into();
    }
    for ch in s.chars() {
        if measure(ui, &out) + measure(ui, &ch.to_string()) + measure(ui, "...") > width {
            break;
        }
        out.push(ch);
    }
    out.push_str("...");
    out
}
pub(crate) fn wrap(ui: &UiFrame, s: &str, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in s.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let next = if line.is_empty() {
                word.into()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(ui, &next) > width {
                lines.push(line);
                line = word.into();
            } else {
                line = next;
            }
        }
        lines.push(fit(ui, &line, width));
    }
    lines
}
pub(crate) fn rule(ui: &mut UiFrame, x: f32, y: f32, w: f32) {
    fill(ui, Rect::new(x, y, w, 0.7), [0.65, 0.76, 0.84, 0.24]);
}
pub(crate) fn panel(ui: &mut UiFrame, r: Rect) {
    super::style::push_white(
        ui,
        draw::rounded_rect(r.x, r.y, r.w, r.h, 4.0, [0.35, 0.46, 0.53, 0.85]),
    );
    super::style::push_white(
        ui,
        draw::rounded_rect(
            r.x + 0.7,
            r.y + 0.7,
            r.w - 1.4,
            r.h - 1.4,
            3.5,
            [0.055, 0.09, 0.12, 0.95],
        ),
    );
}
pub(crate) fn slot(ui: &mut UiFrame, r: Rect, picked: bool) {
    let border = if picked {
        [0.65, 0.84, 0.93, 1.0]
    } else {
        [0.42, 0.53, 0.60, 0.35]
    };
    fill(ui, r, border);
    let p = if picked { 1.4 } else { 0.7 };
    fill(
        ui,
        Rect::new(r.x + p, r.y + p, r.w - 2.0 * p, r.h - 2.0 * p),
        [0.035, 0.065, 0.09, 0.85],
    );
}
