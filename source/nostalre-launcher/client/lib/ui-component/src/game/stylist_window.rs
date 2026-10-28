//! The stylist (rAthena's stylist UI, opened by a stylist NPC): kRO's 2022 Style Shop. The player's look with every
//! pick in its place (the preview the scene draws in), hairstyle and clothes colour by arrows, hair colour by
//! swatches, headgear from a grid (mailed, not worn), what each pick costs, and Buy behind a confirmation. Layout
//! and art as kRO's client draws it (`styleshop/`, the user's rig shots).

use crate::helper::window_chrome::{draw_footer, draw_textured_quad, draw_titlebar, text_color};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::event::GameEvent;
use nostalre_game::stylist::{RESET_COLOR, StyleChoice, StyleGoods, StyleSlot, StylingShopTable};
use nostalre_resources::ui::styleshop as art;
use nostalre_ui::draw;
use nostalre_ui::frame::{ButtonTextures, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const STYLIST_WINDOW_ID: WidgetId = WidgetId(6100);
const TAB_BASE: u32 = 6101;
const ARROW_BASE: u32 = 6105;
const ROTATE_L_ID: WidgetId = WidgetId(6109);
const ROTATE_R_ID: WidgetId = WidgetId(6110);
const RESET_ID: WidgetId = WidgetId(6111);
const SWATCH_BASE: u32 = 6112;
const LINE_BASE: u32 = 6122;
const BUY_ID: WidgetId = WidgetId(6130);
const MESSAGE_ID: WidgetId = WidgetId(6131);
const MESSAGE_OK_ID: WidgetId = WidgetId(6132);
const MESSAGE_CANCEL_ID: WidgetId = WidgetId(6133);
const GRID_BASE: u32 = 6140;

const WIN_W: f32 = 450.0;
const WIN_H: f32 = 280.0;
const TITLE_H: f32 = 17.0;
const FOOTER_H: f32 = 30.0;
const COLUMN_W: f32 = 48.0;
const COLUMN_COLOR: [f32; 4] = [0.83, 0.86, 0.91, 1.0];

const TAB_X: f32 = 5.0;
const TAB_Y: f32 = 23.0;
const TAB_STEP: f32 = 45.0;
const TAB_W: f32 = 37.0;
const TAB_H: f32 = 40.0;

/// The preview's feet, and the arrows either side of it (hairstyle, then clothes colour)
const FEET: (f32, f32) = (112.0, 128.0);
const ARROW_L_X: f32 = 66.0;
const ARROW_R_X: f32 = 156.0;
const ARROW_Y: [f32; 2] = [52.0, 84.0];
const ARROW_W: f32 = 8.0;
const ARROW_H: f32 = 12.0;
const RESET: Rect = Rect {
    x: 56.0,
    y: 132.0,
    w: 16.0,
    h: 16.0,
};
/// The rotate arc under the shadow: its two halves meet
const ROTATE_Y: f32 = 141.0;
const ROTATE_L_X: f32 = 93.0;
const ROTATE_R_X: f32 = ROTATE_L_X + ROTATE_W;
const ROTATE_W: f32 = 21.0;

/// Hair colours: the panel, the "none" swatch, eight swatches in two rows
const PALETTE: (f32, f32) = (54.0, 162.0);
const NONE_SWATCH: (f32, f32) = (8.0, 24.0);
const SWATCH_X: f32 = 34.0;
const SWATCH_Y: f32 = 6.0;
const SWATCH_STEP: (f32, f32) = (19.0, 20.0);
const SWATCH: f32 = 14.0;

/// The right pane: its divider, its message, its rows
const PANE_X: f32 = 182.0;
const PANE_ROW_X: f32 = 210.0;
const PANE_ROW_Y: f32 = 70.0;
const PANE_ROW_STEP: f32 = 25.0;
const PANE_ROWS: usize = 7;
/// The headgear grid
const GRID: (f32, f32) = (200.0, 30.0);
const GRID_STEP: (f32, f32) = (42.0, 35.0);
const GRID_COLUMNS: usize = 6;
const ICON: f32 = 24.0;
/// The cost list under the preview while the grid has the pane
const LEFT_LINES: (f32, f32) = (54.0, 162.0);
const LEFT_LINE_STEP: f32 = 16.0;

const BUY: Rect = Rect {
    x: 357.0,
    y: 255.0,
    w: 80.0,
    h: 20.0,
};

/// The message box over the window, as kRO's "Message": a titled window sized to its text, OK (and cancel to a
/// question) at its foot
const MESSAGE_W: f32 = 312.0;
const MESSAGE_TOP: f32 = 74.0;
const MESSAGE_PAD: f32 = 12.0;
const MESSAGE_LINE: f32 = 18.0;
const MESSAGE_FOOT: f32 = 30.0;
const BUTTON: (f32, f32) = (42.0, 20.0);
const OK_TEX: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_OK,
    hover: nostalre_resources::ui::BTN_OK_A,
    pressed: nostalre_resources::ui::BTN_OK_B,
};
const CANCEL_TEX: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CANCEL,
    hover: nostalre_resources::ui::BTN_CANCEL_A,
    pressed: nostalre_resources::ui::BTN_CANCEL_B,
};

const BUY_TEX: ButtonTextures = ButtonTextures {
    normal: art::BUY,
    hover: art::BUY_OVER,
    pressed: art::BUY_PRESS,
};
const ARROW_TEX: [ButtonTextures; 2] = [
    ButtonTextures {
        normal: art::ARROW_L,
        hover: art::ARROW_L_OVER,
        pressed: art::ARROW_L_OVER,
    },
    ButtonTextures {
        normal: art::ARROW_R,
        hover: art::ARROW_R_OVER,
        pressed: art::ARROW_R_OVER,
    },
];
const ROTATE_TEX: [ButtonTextures; 2] = [
    ButtonTextures {
        normal: art::ROTATE_L,
        hover: art::ROTATE_L_OVER,
        pressed: art::ROTATE_L_PRESS,
    },
    ButtonTextures {
        normal: art::ROTATE_R,
        hover: art::ROTATE_R_OVER,
        pressed: art::ROTATE_R_PRESS,
    },
];
const RESET_TEX: ButtonTextures = ButtonTextures {
    normal: art::RESET,
    hover: art::RESET_OVER,
    pressed: art::RESET_PRESS,
};
const SWATCHES: [[&str; 3]; 8] = [
    [art::COLOR_1, art::COLOR_1_ON, art::COLOR_1_OVER],
    [art::COLOR_2, art::COLOR_2_ON, art::COLOR_2_OVER],
    [art::COLOR_3, art::COLOR_3_ON, art::COLOR_3_OVER],
    [art::COLOR_4, art::COLOR_4_ON, art::COLOR_4_OVER],
    [art::COLOR_5, art::COLOR_5_ON, art::COLOR_5_OVER],
    [art::COLOR_6, art::COLOR_6_ON, art::COLOR_6_OVER],
    [art::COLOR_7, art::COLOR_7_ON, art::COLOR_7_OVER],
    [art::COLOR_8, art::COLOR_8_ON, art::COLOR_8_OVER],
];

const TEXT: [f32; 4] = [0.15, 0.15, 0.2, 1.0];
const MUTED: [f32; 4] = [0.6, 0.6, 0.65, 1.0];

// msgstringtable lines
const MSG_TITLE: u16 = 2813;
const MSG_CONFIRM: u16 = 2818;
const MSG_CANNOT_DYE: u16 = 2834;
const MSG_CHOOSE: u16 = 2835;
const MSG_DONE: u16 = 2836;
/// A refused style, in its box as kRO's exe shows it (0x0a47 at 0x7f6830)
const MSG_FAILED: u16 = 1575;
const MSG_MESSAGE: u16 = 3477;

/// The window's tabs, top to bottom
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Hair,
    Accessory,
    Costume,
    Favorite,
}

const TABS: [(Tab, [&str; 3]); 4] = [
    (
        Tab::Hair,
        [art::TAB_HAIR_OFF, art::TAB_HAIR_ON, art::TAB_HAIR_OVER],
    ),
    (
        Tab::Accessory,
        [
            art::TAB_ACCESSORIES_OFF,
            art::TAB_ACCESSORIES_ON,
            art::TAB_ACCESSORIES_OVER,
        ],
    ),
    (
        Tab::Costume,
        // not offered yet: drawn as it rests, and a click does nothing
        [
            art::TAB_COSTUME_OFF,
            art::TAB_COSTUME_OFF,
            art::TAB_COSTUME_OFF,
        ],
    ),
    (
        Tab::Favorite,
        [
            art::TAB_FAVORITE_OFF,
            art::TAB_FAVORITE_ON,
            art::TAB_FAVORITE_OVER,
        ],
    ),
];

/// Where the scene draws the preview: the feet, the facing, and the draw call it goes before (under the dialogs)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylistPreview {
    pub feet: [f32; 2],
    pub direction: u8,
    pub insert_index: usize,
}

/// One thing a pick costs, as its line says it
struct CostLine {
    slot: StyleSlot,
    label: String,
    icon: Option<String>,
    zeny: u32,
    payable: bool,
}

pub struct StylistWindow {
    pub has_grf_textures: bool,
    tab: Tab,
    /// The preview's facing, 0 to 7 (kRO's directions)
    direction: u8,
    /// Picks the player untick are left out of the purchase
    unticked: [bool; 7],
    /// The open message box: what it is for (the purchase, or the server's answer) and what it says
    message: Option<(Asking, String)>,
    preview: Option<StylistPreview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asking {
    Buy,
    Answer { success: bool },
}

impl Default for StylistWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl StylistWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            tab: Tab::Hair,
            direction: 0,
            unticked: [false; 7],
            message: None,
            preview: None,
        }
    }

    /// Where the scene draws the preview, while the window is open
    pub fn preview(&self) -> Option<StylistPreview> {
        self.preview
    }

    fn reset_view(&mut self) {
        *self = Self {
            has_grf_textures: self.has_grf_textures,
            ..Self::new()
        };
    }

    fn close(&mut self, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        ctx.character.stylist.close();
        self.reset_view();
        vec![GameEvent::RequestStylistClose]
    }

    /// What the purchase sends: every ticked pick
    fn purchase(&self, choice: StyleChoice) -> StyleChoice {
        let mut ticked = choice;
        for (pick, unticked) in ticked.0.iter_mut().zip(self.unticked) {
            if unticked {
                *pick = 0;
            }
        }
        ticked
    }
}

/// The entries of a list a player may pick, by their place in it from 1: a Doram's own lists whole, the human
/// lists' entries marked for Doram too
fn offered(table: &StylingShopTable, slot: StyleSlot, doram: bool) -> Vec<(i16, &StyleGoods)> {
    let Some(list) = table.lists.get(slot.list(doram)) else {
        return Vec::new();
    };
    let own_list = slot.list(doram) != slot.list(false);
    list.iter()
        .enumerate()
        .filter(|(_, goods)| !doram || own_list || goods.doram_too)
        .filter_map(|(i, goods)| Some((i16::try_from(i + 1).ok()?, goods)))
        .collect()
}

/// The next pick an arrow gives: through the offered places and back to "no change" (0)
fn step(offered: &[(i16, &StyleGoods)], current: i16, forward: bool) -> i16 {
    let picks: Vec<i16> = std::iter::once(0)
        .chain(offered.iter().map(|(place, _)| *place))
        .collect();
    let at = picks.iter().position(|&p| p == current).unwrap_or(0);
    let next = if forward {
        (at + 1) % picks.len()
    } else {
        (at + picks.len() - 1) % picks.len()
    };
    picks[next]
}

fn fill(ui: &mut UiFrame, rect: Rect, color: [f32; 4]) {
    crate::helper::window_chrome::fill(ui, rect.x, rect.y, rect.w, rect.h, color);
}

/// What each pick costs, and whether the player can pay it now
fn cost_lines(ctx: &BuildCtx, doram: bool) -> Vec<CostLine> {
    let session = &ctx.character.stylist;
    let table = &ctx.data.styling_shop;
    let inventory = &ctx.character.inventory;
    let mut lines = Vec::new();
    for slot in StyleSlot::ALL {
        let Some(goods) = session.picked(table, slot, doram) else {
            continue;
        };
        if goods.zeny > 0 {
            lines.push(CostLine {
                slot,
                label: format!("{} Zeny", goods.zeny),
                icon: Some(art::ICON_ZENY.to_string()),
                zeny: goods.zeny,
                payable: i64::from(inventory.zeny) >= i64::from(goods.zeny),
            });
        }
        if let Some(item) = goods.item {
            let owned = |id: Option<u32>| id.is_some_and(|id| inventory.count_of(id) > 0);
            lines.push(CostLine {
                slot,
                label: ctx.data.item_name_for(item, true),
                icon: ctx.data.item_icon_path(item),
                zeny: 0,
                payable: owned(Some(item)) || owned(goods.box_item),
            });
        }
    }
    lines
}

impl Window for StylistWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }
    fn window_size(&self) -> (f32, f32) {
        (WIN_W, WIN_H)
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = art::ALL.to_vec();
        paths.extend([
            crate::helper::window_chrome::TITLEBAR_TEX,
            crate::helper::window_chrome::FOOTER_TEX,
            OK_TEX.normal,
            OK_TEX.hover,
            OK_TEX.pressed,
            CANCEL_TEX.normal,
            CANCEL_TEX.hover,
            CANCEL_TEX.pressed,
        ]);
        paths
    }
}

impl InGameWindow for StylistWindow {
    fn wants_escape(&self, ctx: &BuildCtx) -> bool {
        ctx.character.stylist.is_open()
    }

    fn on_escape(&mut self, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if matches!(self.message, Some((Asking::Buy, _))) {
            self.message = None;
            return Vec::new();
        }
        self.close(ctx)
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.preview = None;
        if !ctx.character.stylist.is_open() {
            if self.message.is_some() || self.tab != Tab::Hair {
                self.reset_view();
            }
            return Vec::new();
        }
        let mut events = Vec::new();
        let doram = nostalre_game::sprite_path::is_doram(ctx.job_class);
        ctx.character
            .stylist
            .drop_current_picks(&ctx.data.styling_shop, doram);
        if let Some(success) = ctx.character.stylist.take_result() {
            let text = if success {
                ctx.data
                    .msg_or(MSG_DONE, &[], "You have successfully changed your style.")
            } else {
                ctx.data.msg_or(MSG_FAILED, &[], "Failed.")
            };
            self.message = Some((Asking::Answer { success }, text));
        }

        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;
        let win = ui.window_at(STYLIST_WINDOW_ID, WIN_W, WIN_H, TITLE_H, 260.0, 120.0);
        let (ox, oy) = (win.x, win.y);
        ui.interact(STYLIST_WINDOW_ID, Rect::new(ox, oy, WIN_W, WIN_H));
        draw_titlebar(ui, ox, oy, WIN_W, TITLE_H, grf);
        let title = ctx.data.msg_or(MSG_TITLE, &[], "Style Shop");
        ui.text(ox + 18.0, oy + TITLE_H - 4.0, &title, text_color(grf));
        let body = Rect::new(ox, oy + TITLE_H, WIN_W, WIN_H - TITLE_H - FOOTER_H);
        fill(ui, body, [1.0; 4]);
        fill(ui, Rect::new(ox, body.y, COLUMN_W, body.h), COLUMN_COLOR);
        let at = |x: f32, y: f32| (ox + x, oy + y);

        // The tabs; the second costume is not offered here yet (rAthena: third jobs only)
        for (i, (tab, tex)) in TABS.iter().enumerate() {
            let (x, y) = at(TAB_X, TAB_Y + i as f32 * TAB_STEP);
            let rect = Rect::new(x, y, TAB_W, TAB_H);
            let response = ui.interact(WidgetId(TAB_BASE + i as u32), rect);
            let state = if self.tab == *tab {
                1
            } else if response.hovered() {
                2
            } else {
                0
            };
            draw_textured_quad(ui, x, y, TAB_W, TAB_H, tex[state]);
            if response.clicked() && *tab != Tab::Costume {
                self.tab = *tab;
            }
        }

        // The preview: its shadow, then the character the scene draws in here
        let (fx, fy) = at(FEET.0, FEET.1);
        draw_textured_quad(ui, fx - 27.0, fy - 18.0, 54.0, 35.0, art::SHADOW_CHR);
        self.preview = Some(StylistPreview {
            feet: [fx, fy],
            direction: self.direction,
            insert_index: ui.draw_calls.len(),
        });

        let table = &ctx.data.styling_shop;
        // Hairstyle and clothes colour by arrows, on the hair tab
        if self.tab == Tab::Hair {
            for (row, slot) in [StyleSlot::HeadStyle, StyleSlot::BodyPalette]
                .into_iter()
                .enumerate()
            {
                // the arrows pass over the look the player has: picking it would only cost
                let mut offered = offered(table, slot, doram);
                offered.retain(|(_, goods)| !ctx.character.stylist.is_current(slot, goods));
                for (side, x) in [ARROW_L_X, ARROW_R_X].into_iter().enumerate() {
                    let (x, y) = at(x, ARROW_Y[row]);
                    let id = WidgetId(ARROW_BASE + (row * 2 + side) as u32);
                    let rect = Rect::new(x, y, ARROW_W, ARROW_H);
                    if ui.button(id, rect, &ARROW_TEX[side], "").clicked() && !offered.is_empty() {
                        let current = ctx.character.stylist.choice().get(slot);
                        ctx.character
                            .stylist
                            .pick(slot, step(&offered, current, side == 1));
                    }
                }
            }
        }
        for (i, (x, id)) in [(ROTATE_L_X, ROTATE_L_ID), (ROTATE_R_X, ROTATE_R_ID)]
            .into_iter()
            .enumerate()
        {
            let (x, y) = at(x, ROTATE_Y);
            if ui
                .button(id, Rect::new(x, y, ROTATE_W, 9.0), &ROTATE_TEX[i], "")
                .clicked()
            {
                self.direction = if i == 0 {
                    (self.direction + 1) % 8
                } else {
                    (self.direction + 7) % 8
                };
            }
        }
        let (rx, ry) = at(RESET.x, RESET.y);
        if ui
            .button(
                RESET_ID,
                Rect::new(rx, ry, RESET.w, RESET.h),
                &RESET_TEX,
                "",
            )
            .clicked()
        {
            for slot in StyleSlot::ALL {
                ctx.character.stylist.pick(slot, 0);
            }
            self.unticked = [false; 7];
        }

        let lines = cost_lines(ctx, doram);
        match self.tab {
            Tab::Hair => {
                self.build_palette(ui, ctx, doram, at(PALETTE.0, PALETTE.1));
                self.build_pane_lines(ui, ctx, &lines, at(PANE_X, 0.0).0, oy);
            }
            Tab::Accessory => {
                self.build_grid(ui, ctx, doram, at(GRID.0, GRID.1));
                self.build_left_lines(ui, &lines, at(LEFT_LINES.0, LEFT_LINES.1));
            }
            Tab::Costume | Tab::Favorite => {
                // Favourites are the client's own, kept by kRO in its settings: not kept here yet
                for column in 0..3 {
                    let x = ox + PANE_X - 120.0 + column as f32 * 120.0;
                    draw_textured_quad(ui, x, oy + TITLE_H + 2.0, 2.0, 230.0, art::LINE_COLUMN);
                }
            }
        }

        // The footer: what the player has, and Buy
        let footer_y = oy + WIN_H - FOOTER_H;
        draw_footer(ui, ox, footer_y, WIN_W, FOOTER_H, grf);
        let zeny = crate::helper::format::format_thousands(i64::from(ctx.character.inventory.zeny));
        ui.text(
            ox + 10.0,
            footer_y + 19.0,
            &format!("On Hand  {zeny} z"),
            TEXT,
        );
        // Buy takes the ticked picks (a free one too: back to colour 0 costs nothing) when all of them can be paid
        let ticked: Vec<&CostLine> = lines
            .iter()
            .filter(|l| !self.unticked[l.slot as usize])
            .collect();
        let zeny: i64 = ticked.iter().map(|l| i64::from(l.zeny)).sum();
        let affordable = !self.purchase(ctx.character.stylist.choice()).is_empty()
            && ticked.iter().all(|l| l.payable)
            && i64::from(ctx.character.inventory.zeny) >= zeny;
        let (bx, by) = at(BUY.x, BUY.y);
        let buy = Rect::new(bx, by, BUY.w, BUY.h);
        let waiting = ctx.character.stylist.is_waiting();
        if affordable && !waiting && self.message.is_none() {
            if ui.button(BUY_ID, buy, &BUY_TEX, crate::pt!("Buy", "Comprar")).clicked() {
                let text = ctx.data.msg_or(
                    MSG_CONFIRM,
                    &[],
                    "Item cannot be restored, are you sure want to apply it?",
                );
                self.message = Some((Asking::Buy, text));
            }
        } else {
            draw_textured_quad(ui, bx, by, BUY.w, BUY.h, art::BUY_DISABLE);
        }

        // The message box over the window
        let message_title = ctx.data.msg_or(MSG_MESSAGE, &[], "Message");
        if let Some((asking, ok)) = self.build_message(ui, &message_title, grf, (ox, oy)) {
            self.message = None;
            match (asking, ok) {
                (Asking::Buy, true) => {
                    let choice = self.purchase(ctx.character.stylist.choice());
                    ctx.character.stylist.start();
                    events.push(GameEvent::RequestStylistBuy { choice });
                }
                (Asking::Answer { success: true }, _) => events.extend(self.close(ctx)),
                _ => {}
            }
        }

        ui.has_grf_textures = prev_grf;
        events
    }
}

impl StylistWindow {
    /// The open message box, a window of its own over the stylist (the pointer is its alone): its title, its text
    /// wrapped, OK, and cancel when it asks. What was answered, and whether with OK, once a button is pressed.
    fn build_message(
        &mut self,
        ui: &mut UiFrame,
        title: &str,
        grf: bool,
        (ox, oy): (f32, f32),
    ) -> Option<(Asking, bool)> {
        let (asking, text) = self.message.clone()?;
        let lines = draw::word_wrap(
            &text,
            MESSAGE_W - 2.0 * MESSAGE_PAD,
            |t| ui.atlas.measure_text(t),
            false,
        );
        let h = TITLE_H + MESSAGE_PAD + lines.len() as f32 * MESSAGE_LINE + MESSAGE_FOOT;
        ui.ensure_in_z_order_with(MESSAGE_ID, nostalre_ui::frame::WindowOrder::Foreground);
        ui.bring_to_front(MESSAGE_ID);
        let default = (ox + (WIN_W - MESSAGE_W) / 2.0, oy + MESSAGE_TOP);
        let win = ui.window_at(MESSAGE_ID, MESSAGE_W, h, TITLE_H, default.0, default.1);
        ui.interact(MESSAGE_ID, win);
        draw_titlebar(ui, win.x, win.y, MESSAGE_W, TITLE_H, grf);
        ui.text(win.x + 18.0, win.y + TITLE_H - 4.0, title, text_color(grf));
        fill(
            ui,
            Rect::new(win.x, win.y + TITLE_H, MESSAGE_W, h - TITLE_H),
            [1.0; 4],
        );
        for (i, line) in lines.iter().enumerate() {
            let y = win.y + TITLE_H + MESSAGE_PAD + (i as f32 + 1.0) * MESSAGE_LINE - 4.0;
            ui.text(win.x + MESSAGE_PAD, y, line, TEXT);
        }
        let by = win.y + h - BUTTON.1 - 6.0;
        let cancel_x = win.x + MESSAGE_W - BUTTON.0 - 8.0;
        let asks = asking == Asking::Buy;
        let ok_x = if asks {
            cancel_x - BUTTON.0 - 4.0
        } else {
            cancel_x
        };
        if ui
            .button(
                MESSAGE_OK_ID,
                Rect::new(ok_x, by, BUTTON.0, BUTTON.1),
                &OK_TEX,
                "OK",
            )
            .clicked()
        {
            return Some((asking, true));
        }
        if asks
            && ui
                .button(
                    MESSAGE_CANCEL_ID,
                    Rect::new(cancel_x, by, BUTTON.0, BUTTON.1),
                    &CANCEL_TEX,
                    crate::pt!("Cancel", "Cancelar"),
                )
                .clicked()
        {
            return Some((asking, false));
        }
        None
    }

    /// The hair colours: "none" (back to colour 0), then the shop's eight; none when the hairstyle takes no colour
    fn build_palette(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        doram: bool,
        (px, py): (f32, f32),
    ) {
        draw_textured_quad(ui, px, py, 110.0, 42.0, art::PALETTE_BG);
        draw_textured_quad(ui, px + 4.0, py + 2.0, 23.0, 21.0, art::PALETTE_ICON);
        let table = &ctx.data.styling_shop;
        let dyeable = ctx
            .character
            .stylist
            .picked(table, StyleSlot::HeadStyle, doram)
            .is_none_or(|style| style.dyeable);
        let current = ctx.character.stylist.choice().get(StyleSlot::HeadPalette);
        let none = Rect::new(px + NONE_SWATCH.0, py + NONE_SWATCH.1, 16.0, 16.0);
        let response = ui.interact(WidgetId(SWATCH_BASE), none);
        let tex = if current == RESET_COLOR {
            art::COLOR_NONE_ON
        } else {
            art::COLOR_NONE
        };
        draw_textured_quad(ui, none.x, none.y, none.w, none.h, tex);
        if response.clicked() && dyeable {
            let pick = if current == RESET_COLOR {
                0
            } else {
                RESET_COLOR
            };
            ctx.character.stylist.pick(StyleSlot::HeadPalette, pick);
        }
        let offered = offered(table, StyleSlot::HeadPalette, doram);
        for (i, swatch) in SWATCHES.iter().enumerate() {
            // swatch n shows hair colour n: the entry of the list with that look
            let Some(&(place, goods)) = offered.iter().find(|(_, g)| g.goods == i as u32 + 1)
            else {
                continue;
            };
            // the colour the hair has shows chosen and takes no click
            let worn = ctx
                .character
                .stylist
                .is_current(StyleSlot::HeadPalette, goods);
            let x = px + SWATCH_X + (i % 4) as f32 * SWATCH_STEP.0;
            let y = py + SWATCH_Y + (i / 4) as f32 * SWATCH_STEP.1;
            let rect = Rect::new(x, y, SWATCH, SWATCH);
            let response = ui.interact(WidgetId(SWATCH_BASE + 1 + i as u32), rect);
            let state = if current == place || (current == 0 && worn) {
                1
            } else if response.hovered() && dyeable && !worn {
                2
            } else {
                0
            };
            draw_textured_quad(ui, x, y, SWATCH, SWATCH, swatch[state]);
            if response.hovered() && !dyeable {
                ui.any_interactive_hovered = true;
                let text = ctx
                    .data
                    .msg_or(MSG_CANNOT_DYE, &[], "This hairstyle cannot be dyed.");
                ui.tooltip(x, y, &text);
            }
            if response.clicked() && dyeable && !worn {
                let pick = if current == place { 0 } else { place };
                ctx.character.stylist.pick(StyleSlot::HeadPalette, pick);
            }
        }
    }

    /// The headgear the shop offers, top, middle then lower; a click picks one and a second click lets it go
    fn build_grid(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        doram: bool,
        (gx, gy): (f32, f32),
    ) {
        let table = &ctx.data.styling_shop;
        let slots = [
            StyleSlot::TopAccessory,
            StyleSlot::MidAccessory,
            StyleSlot::BottomAccessory,
        ];
        let entries: Vec<(StyleSlot, i16, u32)> = slots
            .into_iter()
            .flat_map(|slot| {
                offered(table, slot, doram)
                    .into_iter()
                    .map(move |(place, goods)| (slot, place, goods.goods))
            })
            .collect();
        for (n, (slot, place, item)) in entries.into_iter().enumerate() {
            let x = gx + (n % GRID_COLUMNS) as f32 * GRID_STEP.0;
            let y = gy + (n / GRID_COLUMNS) as f32 * GRID_STEP.1;
            let picked = ctx.character.stylist.choice().get(slot) == place;
            let shadow = if picked {
                art::SHADOW_ITEM_ON
            } else {
                art::SHADOW_ITEM
            };
            draw_textured_quad(ui, x - 4.0, y + ICON - 10.0, 32.0, 18.0, shadow);
            if let Some(icon) = ctx.data.item_icon_path(item) {
                draw_textured_quad(ui, x, y, ICON, ICON, icon);
            }
            let response = ui.interact(WidgetId(GRID_BASE + n as u32), Rect::new(x, y, ICON, ICON));
            if response.hovered() {
                ui.any_interactive_hovered = true;
                ui.tooltip(x + ICON / 2.0, y, &ctx.data.item_name_for(item, true));
            }
            if response.clicked() {
                ctx.character
                    .stylist
                    .pick(slot, if picked { 0 } else { place });
            }
        }
    }

    /// The right pane on the hair tab: what each pick costs, ticked to buy; or how to start
    fn build_pane_lines(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        lines: &[CostLine],
        pane_x: f32,
        oy: f32,
    ) {
        let x = pane_x - PANE_X + PANE_ROW_X;
        draw_textured_quad(ui, pane_x, oy + TITLE_H + 2.0, 2.0, 230.0, art::LINE_COLUMN);
        for row in 0..PANE_ROWS {
            let y = oy + PANE_ROW_Y + row as f32 * PANE_ROW_STEP;
            draw_textured_quad(ui, x, y, 174.0, 1.0, art::LINE_ROW);
        }
        if lines.is_empty() {
            let text = ctx
                .data
                .msg_or(MSG_CHOOSE, &[], "Press the arrows to choose a new style.");
            ui.text_centered(pane_x, oy + 50.0, WIN_W - PANE_X, &text, TEXT);
            return;
        }
        for (i, line) in lines.iter().enumerate().take(PANE_ROWS) {
            let y = oy + PANE_ROW_Y - 22.0 + i as f32 * PANE_ROW_STEP;
            self.build_line(ui, line, i, x, y, true);
        }
    }

    /// The cost list under the preview, while the grid has the pane
    fn build_left_lines(&mut self, ui: &mut UiFrame, lines: &[CostLine], (lx, ly): (f32, f32)) {
        for (i, line) in lines.iter().enumerate() {
            self.build_line(ui, line, i, lx, ly + i as f32 * LEFT_LINE_STEP, false);
        }
    }

    /// One cost: its tick (none when it cannot be paid), its icon on the wide pane, its label, struck when unpaid
    fn build_line(
        &mut self,
        ui: &mut UiFrame,
        line: &CostLine,
        i: usize,
        x: f32,
        y: f32,
        wide: bool,
    ) {
        let ticked = line.payable && !self.unticked[line.slot as usize];
        let check = if !line.payable {
            art::CHECK_DISABLE
        } else if ticked {
            art::CHECK_ON
        } else {
            art::CHECK_OFF
        };
        let tick = Rect::new(x, y + 3.0, 17.0, 13.0);
        draw_textured_quad(ui, tick.x, tick.y, tick.w, tick.h, check);
        if ui.interact(WidgetId(LINE_BASE + i as u32), tick).clicked() && line.payable {
            let slot = line.slot as usize;
            self.unticked[slot] = !self.unticked[slot];
        }
        let mut text_x = x + 22.0;
        if wide && let Some(icon) = &line.icon {
            draw_textured_quad(ui, text_x, y - 2.0, ICON, ICON, icon.clone());
            text_x += ICON + 6.0;
        }
        let color = if line.payable { TEXT } else { MUTED };
        ui.text(text_x, y + 14.0, &line.label, color);
        if !line.payable {
            let w = ui.atlas.measure_text(&line.label);
            fill(ui, Rect::new(text_x, y + 9.0, w, 1.0), MUTED);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn goods(goods: u32, doram_too: bool) -> StyleGoods {
        StyleGoods {
            goods,
            zeny: 100_000,
            item: None,
            box_item: None,
            doram_too,
            dyeable: true,
        }
    }

    #[test]
    fn arrows_step_through_the_offered_places_and_back_to_no_change() {
        let list = [goods(1, true), goods(2, false), goods(3, true)];
        let offered: Vec<(i16, &StyleGoods)> = list
            .iter()
            .enumerate()
            .map(|(i, g)| (i as i16 + 1, g))
            .collect();
        assert_eq!(step(&offered, 0, true), 1);
        assert_eq!(
            step(&offered, 3, true),
            0,
            "past the last, back to no change"
        );
        assert_eq!(step(&offered, 0, false), 3);
    }

    #[test]
    fn a_doram_is_offered_its_own_lists_whole_and_the_human_ones_it_may_take() {
        let mut table = StylingShopTable::default();
        table
            .lists
            .insert("HeadPalette".into(), vec![goods(1, true), goods(2, false)]);
        table
            .lists
            .insert("DoramHeadStyle".into(), vec![goods(5, false)]);
        let places = |slot, doram| {
            offered(&table, slot, doram)
                .iter()
                .map(|(place, _)| *place)
                .collect::<Vec<_>>()
        };
        assert_eq!(places(StyleSlot::HeadPalette, false), [1, 2]);
        assert_eq!(
            places(StyleSlot::HeadPalette, true),
            [1],
            "only those marked for Doram"
        );
        assert_eq!(
            places(StyleSlot::HeadStyle, true),
            [1],
            "its own list, whole"
        );
    }

    #[test]
    fn an_unticked_pick_is_left_out_of_the_purchase() {
        let mut window = StylistWindow::new();
        window.unticked[StyleSlot::MidAccessory as usize] = true;
        let choice = StyleChoice([1, 2, 0, 0, 5, 0, 0]);
        assert_eq!(window.purchase(choice).0, [1, 2, 0, 0, 0, 0, 0]);
    }
}
