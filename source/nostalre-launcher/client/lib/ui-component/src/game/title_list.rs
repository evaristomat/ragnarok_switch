//! The equipment window's Title tab list (`UIListBoxTitle`, the 2025 exe's 0x8360c0): "Disable Title" then the
//! character's titles, 15-pixel rows on 0xe1e1e1, one click picks a row, a double-click wears it (0x858840,
//! 0x858430: the window then sends 0x0a2e with the row's title, 0 for none)
use crate::helper::scrollbar::{self, SCROLLBAR_W, ScrollbarIds};
use crate::helper::window_chrome::fill;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The first row (MSI_TAKEOFF_TITLE, 2686)
const DISABLE_TITLE: &str = "Disable Title";
const ROW_H: f32 = 15.0;
/// The picked row's band, a little short of the row (0x854360)
const PICKED_H: f32 = 14.0;
const TEXT_X: f32 = 3.0;
const BACKGROUND: [f32; 4] = [225.0 / 255.0, 225.0 / 255.0, 225.0 / 255.0, 1.0];
/// `colorchip.bmp` at (3 × 4 + 2, 2 × 4 + 2), the list base's pick colour (0x835ef0, 0x7a6d70)
const PICKED: [f32; 4] = [202.0 / 255.0, 223.0 / 255.0, 251.0 / 255.0, 1.0];
const TEXT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

#[derive(Default)]
pub struct TitleList {
    picked: Option<usize>,
    scroll: usize,
}

impl TitleList {
    /// As kRO refills the list on showing the tab: nothing picked, at the top (0x8bc4a0)
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Draws the list in `rect` over the character's `owned` titles, named by `name`; returns the title a
    /// double-click asks to wear (0 for none)
    pub fn build<'a>(
        &mut self,
        ui: &mut UiFrame,
        base_id: u32,
        rect: Rect,
        owned: &[i32],
        name: impl Fn(i32) -> &'a str,
    ) -> Option<i32> {
        fill(ui, rect.x, rect.y, rect.w, rect.h, BACKGROUND);
        let rows = owned.len() + 1;
        let visible = (rect.h / ROW_H).floor() as usize;
        let max_scroll = rows.saturating_sub(visible);
        let row_w = if max_scroll > 0 {
            rect.w - SCROLLBAR_W
        } else {
            rect.w
        };
        self.scroll = scrollbar::scrollbar(
            ui,
            ScrollbarIds {
                up: WidgetId(base_id),
                down: WidgetId(base_id + 1),
                thumb: WidgetId(base_id + 2),
            },
            self.scroll,
            visible,
            max_scroll,
            rect,
            rect.x + rect.w - SCROLLBAR_W,
            rect.y,
            rect.h,
        );
        let mut wear = None;
        for (slot, row) in (self.scroll..rows.min(self.scroll + visible)).enumerate() {
            let (label, title) = match row {
                0 => (DISABLE_TITLE, 0),
                _ => (name(owned[row - 1]), owned[row - 1]),
            };
            let row_y = rect.y + slot as f32 * ROW_H;
            if self.picked == Some(row) {
                fill(ui, rect.x, row_y, row_w, PICKED_H, PICKED);
            }
            ui.text(rect.x + TEXT_X, row_y + ui.atlas.ascent, label, TEXT);
            let response = ui.interact(
                WidgetId(base_id + 3 + slot as u32),
                Rect::new(rect.x, row_y, row_w, ROW_H),
            );
            if response.clicked() {
                self.picked = Some(row);
            }
            if response.double_clicked() {
                wear = Some(title);
            }
        }
        wear
    }
}
