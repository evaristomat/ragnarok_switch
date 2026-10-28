pub mod colors;
pub mod dialog_container;
pub mod dropdown;
pub mod fallback;
pub mod format;
pub mod head_board;
pub mod scrollbar;
pub mod tab_control;
pub mod window_chrome;

use nostalre_ui::frame::CheckboxTextures;

pub const CHECKBOX: CheckboxTextures = CheckboxTextures {
    off: nostalre_resources::ui::CHECKBOX_0,
    on: nostalre_resources::ui::CHECKBOX_1,
};

/// The keys' pick on a classic window's button (a controller in use): a blue that shows on white and grey
pub const FOCUS_RING: [f32; 4] = [0.25, 0.5, 0.95, 1.0];

/// A 2-unit ring just outside `r`
pub fn focus_ring(ui: &mut nostalre_ui::frame::UiFrame, r: nostalre_ui::rect::Rect) {
    use nostalre_ui::draw::{self, DrawCall, TextureRef};
    for (x, y, w, h) in [
        (r.x - 2.0, r.y - 2.0, r.w + 4.0, 2.0),
        (r.x - 2.0, r.y + r.h, r.w + 4.0, 2.0),
        (r.x - 2.0, r.y, 2.0, r.h),
        (r.x + r.w, r.y, 2.0, r.h),
    ] {
        let (v, i) = draw::quad_vertices(x, y, w, h, FOCUS_RING);
        ui.draw_calls.push(DrawCall {
            vertices: v.into(),
            indices: i.into(),
            texture: TextureRef::White,
        });
    }
}
