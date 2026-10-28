use crate::helper::window_chrome::{
    SYS_BASE_OFF_TEX, SYS_BASE_ON_TEX, TITLEBAR_TEX, draw_container, draw_sys_button,
    draw_titlebar, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::event::GameEvent;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const CART_SELECT_WINDOW_ID: WidgetId = WidgetId(1850);
const CART_SELECT_CLOSE_BTN_ID: WidgetId = WidgetId(1851);
const CART_SELECT_ROW_BASE_ID: u32 = 1855;

const CLOSE_OFF_TEX: &str = nostalre_resources::ui::basic::SYS_CLOSE_OFF;
const CLOSE_ON_TEX: &str = nostalre_resources::ui::basic::SYS_CLOSE_ON;

const TITLE_H: f32 = 17.0;
const ROW_H: f32 = 44.0;
const PAD: f32 = 6.0;
const WIN_W: f32 = 170.0;
const MINI_BTN_SIZE: f32 = 11.0;
const PREVIEW_W: f32 = 44.0;
/// Cart Decoration's list as the rig draws its Cart List (2026-10-17): one wide row of carts, no labels
const LIST_W: f32 = 410.0;
const LIST_CELL_W: f32 = 70.0;
const LIST_LEFT: f32 = 24.0;
/// Its carts at about their own size (half the player's height on the rig), so a taller row
const LIST_ROW_H: f32 = 56.0;
const LIST_PREVIEW_SCALE: f32 = 0.8;
const PREVIEW_SCALE: f32 = 0.4;

const CART_MODELS: [(u8, u16); 5] = [(1, 0), (2, 40), (3, 65), (4, 80), (5, 90)];

/// Cart Decoration's list (0x097f): the carts the server offers, under the account id the answer repeats
struct Decoration {
    identity: u32,
    /// The offered carts as rows: (cart, no level needed)
    rows: Vec<(u8, u16)>,
    title: String,
}

/// The window's size: Change Cart's column of rows, or Cart Decoration's one row of carts
fn layout_size(decoration: &Option<Decoration>) -> (f32, f32) {
    match decoration {
        Some(_) => (LIST_W, TITLE_H + PAD + LIST_ROW_H + PAD),
        None => (
            WIN_W,
            TITLE_H + PAD + CART_MODELS.len() as f32 * ROW_H + PAD,
        ),
    }
}

/// The rows a window shows, (cart, base level it needs), and its title: Cart Decoration's offer, else Change Cart's five
fn rows_of(decoration: &Option<Decoration>) -> (&[(u8, u16)], &str) {
    match decoration {
        Some(d) => (&d.rows, &d.title),
        None => (&CART_MODELS, "Change Cart"),
    }
}

/// Where row `i` sits from the body's top left: down a column for Change Cart, along one row for the Cart List
fn cell_rect(i: usize, across: bool, x: f32, body_y: f32) -> Rect {
    if across {
        Rect::new(
            x + LIST_LEFT + i as f32 * LIST_CELL_W,
            body_y + PAD,
            LIST_CELL_W - 2.0,
            LIST_ROW_H - 2.0,
        )
    } else {
        Rect::new(
            x + PAD,
            body_y + PAD + i as f32 * ROW_H,
            WIN_W - PAD * 2.0,
            ROW_H - 2.0,
        )
    }
}

pub struct CartSelectWindow {
    pub has_grf_textures: bool,
    open: bool,
    /// Set while the window shows Cart Decoration's list rather than Change Cart's five
    decoration: Option<Decoration>,
    model_previews: Vec<(u8, [f32; 2])>,
    preview_insert_index: Option<usize>,
}

impl Default for CartSelectWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl CartSelectWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            open: false,
            decoration: None,
            model_previews: Vec::new(),
            preview_insert_index: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn model_previews(&self) -> &[(u8, [f32; 2])] {
        &self.model_previews
    }

    /// How large the previews draw: the Cart List's carts near their own size, Change Cart's small beside a label
    pub fn preview_scale(&self) -> f32 {
        if self.decoration.is_some() {
            LIST_PREVIEW_SCALE
        } else {
            PREVIEW_SCALE
        }
    }

    pub fn preview_insert_index(&self) -> Option<usize> {
        self.preview_insert_index
    }

    pub fn open(&mut self) {
        self.open = true;
        self.decoration = None;
    }

    /// Cart Decoration's list (0x097f), each pick answered with 0x0980 under `identity`
    pub fn open_decoration(&mut self, identity: u32, carts: Vec<u8>, title: impl Into<String>) {
        self.open = !carts.is_empty();
        self.decoration = Some(Decoration {
            identity,
            rows: carts.into_iter().map(|c| (c, 0)).collect(),
            title: title.into(),
        });
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

impl Window for CartSelectWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn window_size(&self) -> (f32, f32) {
        layout_size(&self.decoration)
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        vec![
            TITLEBAR_TEX,
            SYS_BASE_OFF_TEX,
            SYS_BASE_ON_TEX,
            CLOSE_OFF_TEX,
            CLOSE_ON_TEX,
        ]
    }
}

impl InGameWindow for CartSelectWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.close();
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        let character = &mut *ctx.character;
        let _data = ctx.data;
        self.model_previews.clear();
        self.preview_insert_index = None;
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();

        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;
        let text_color = text_color(grf);

        // the field borrowed alone, so the rows need no copy while the previews fill
        let (rows, title) = rows_of(&self.decoration);
        let across = self.decoration.is_some();
        let (win_w, win_h) = layout_size(&self.decoration);
        let win = ui.window_at(CART_SELECT_WINDOW_ID, win_w, win_h, TITLE_H, 200.0, 160.0);
        ui.interact(CART_SELECT_WINDOW_ID, Rect::new(win.x, win.y, win_w, win_h));

        draw_titlebar(ui, win.x, win.y, win_w, TITLE_H, grf);
        ui.text(win.x + 17.0, win.y + TITLE_H - 3.0, title, text_color);

        let close_rect = Rect::new(
            win.x + win_w - MINI_BTN_SIZE - 3.0,
            win.y + (TITLE_H - MINI_BTN_SIZE) / 2.0,
            MINI_BTN_SIZE,
            MINI_BTN_SIZE,
        );
        let close_resp = ui.interact(CART_SELECT_CLOSE_BTN_ID, close_rect);
        if close_resp.hovered() {
            ui.any_interactive_hovered = true;
        }
        draw_sys_button(
            ui,
            close_rect,
            (MINI_BTN_SIZE, MINI_BTN_SIZE),
            close_resp.hovered(),
            grf,
            CLOSE_ON_TEX,
            CLOSE_OFF_TEX,
            Some('x'),
        );
        if close_resp.clicked() {
            self.open = false;
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let body_y = win.y + TITLE_H;
        draw_container(ui, win.x, body_y, win_w, win_h - TITLE_H, grf);

        for (i, (num, req_level)) in rows.iter().enumerate() {
            let row_rect = cell_rect(i, across, win.x, body_y);
            let unlocked = character.base_level >= *req_level;
            let resp = ui.interact(WidgetId(CART_SELECT_ROW_BASE_ID + i as u32), row_rect);
            if unlocked && resp.hovered() {
                ui.any_interactive_hovered = true;
            }

            if unlocked && resp.hovered() {
                let hover_bg = if grf {
                    [0.85, 0.85, 0.8, 0.5]
                } else {
                    [0.3, 0.3, 0.4, 0.3]
                };
                let (v, idx) =
                    draw::quad_vertices(row_rect.x, row_rect.y, row_rect.w, row_rect.h, hover_bg);
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: idx.into(),
                    texture: TextureRef::White,
                });
            }

            if unlocked {
                let anchor_x = if across {
                    row_rect.x + row_rect.w / 2.0
                } else {
                    row_rect.x + PREVIEW_W / 2.0
                };
                let anchor_y = row_rect.y + row_rect.h - 6.0;
                self.model_previews.push((*num, [anchor_x, anchor_y]));
            }

            if !across {
                let label = if unlocked {
                    format!("Cart {num}")
                } else {
                    format!("Cart {num} (Lv {req_level})")
                };
                let label_color = if unlocked {
                    text_color
                } else {
                    [0.5, 0.5, 0.5, 1.0]
                };
                let text_x = row_rect.x + PREVIEW_W + 4.0;
                ui.text(text_x, row_rect.y + ROW_H / 2.0 + 1.0, &label, label_color);
            }

            if unlocked && resp.clicked() {
                events.push(match &self.decoration {
                    Some(d) => GameEvent::RequestSelectCart {
                        identity: d.identity,
                        cart: *num,
                    },
                    None => GameEvent::RequestChangeCart {
                        num: i16::from(*num),
                    },
                });
                self.open = false;
            }
        }

        self.preview_insert_index = Some(ui.draw_calls.len());

        ui.has_grf_textures = prev_grf;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cart_decoration_shows_the_offered_carts_and_change_cart_its_five() {
        let mut win = CartSelectWindow::new();
        win.open_decoration(2_000_000, vec![10, 11, 12], "Cart List");
        assert!(win.is_open());
        let (rows, title) = rows_of(&win.decoration);
        assert_eq!(rows, vec![(10, 0), (11, 0), (12, 0)], "no level needed");
        assert_eq!(title, "Cart List");
        assert_eq!(
            layout_size(&win.decoration),
            (LIST_W, TITLE_H + PAD + LIST_ROW_H + PAD),
            "one row"
        );
        assert_eq!(win.preview_scale(), LIST_PREVIEW_SCALE);
        let (a, b) = (cell_rect(0, true, 0.0, 0.0), cell_rect(1, true, 0.0, 0.0));
        assert_eq!((b.x - a.x, a.y), (LIST_CELL_W, b.y), "side by side");

        win.open();
        let (rows, title) = rows_of(&win.decoration);
        assert_eq!(rows.len(), 5);
        assert_eq!((rows[4], title), ((5, 90), "Change Cart"));

        win.open_decoration(2_000_000, Vec::new(), "Cart List");
        assert!(!win.is_open(), "nothing offered: nothing shown");
    }
}
