use super::input_dialog::{InputDialog, InputDialogConfig, InputDialogLayout, InputDialogResult};
use super::item_info_window::ITEM_INFO_WINDOW_ID;
use crate::helper::CHECKBOX;
use crate::helper::dialog_container::DialogContainer;
use crate::helper::format::format_thousands;
use crate::helper::window_chrome::{
    FOOTER_TEX, ITEMWIN_MID_TEX, SYS_BASE_OFF_TEX, SYS_BASE_ON_TEX, TITLEBAR_TEX, draw_container,
    draw_footer, draw_titlebar, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::data_table::DataTable;
use nostalre_game::event::GameEvent;
use nostalre_game::npc_shop::{NpcShopData, NpcShopMode, ShopKind};
use nostalre_ui::context::PhysicalKeyCode as KeyCode;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{ButtonTextures, RESIZE_HANDLE_TEX, UiFrame, WidgetId, WindowOrder};
use nostalre_ui::rect::Rect;

pub const INPUT_WIN_ID: WidgetId = WidgetId(701);
pub const OUTPUT_WIN_ID: WidgetId = WidgetId(702);
const BUY_SELL_BTN_ID: WidgetId = WidgetId(703);
const CANCEL_BTN_ID: WidgetId = WidgetId(704);
const QTY_INPUT_ID: WidgetId = WidgetId(709);
const SCROLL_UP_ID: WidgetId = WidgetId(707);
const SCROLL_DOWN_ID: WidgetId = WidgetId(708);
const SCROLL_THUMB_ID: WidgetId = WidgetId(710);
const OUT_SCROLL_UP_ID: WidgetId = WidgetId(711);
const OUT_SCROLL_DOWN_ID: WidgetId = WidgetId(712);
const OUT_SCROLL_THUMB_ID: WidgetId = WidgetId(713);
const INPUT_RESIZE_ID: WidgetId = WidgetId(714);
const DRAG_ALL_CHECK_ID: WidgetId = WidgetId(715);
const ITEM_BASE_ID: u32 = 720;
const BASKET_BASE_ID: u32 = 780;

const WIN_W: f32 = 280.0;
const WIN_GAP: f32 = 10.0;
const TITLE_H: f32 = 17.0;
const CONTAINER_PAD_LEFT: f32 = 16.0;
const CONTAINER_PAD_RIGHT: f32 = 3.0;
const CONTAINER_PAD_Y: f32 = 5.0;
const ITEM_ROW_H: f32 = 32.0;
const FOOTER_H: f32 = 27.0;
const INPUT_MIN_ROWS: usize = 2;
const INPUT_MAX_ROWS: usize = 9;
const INPUT_DEFAULT_ROWS: usize = 8;
const OUTPUT_VISIBLE_ROWS: usize = 2;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;
use crate::helper::scrollbar::{self, SCROLLBAR_W, ScrollbarIds};
const ICON_SIZE: f32 = 24.0;
const ICON_OFFSET_X: f32 = 4.0;
const ICON_OFFSET_Y: f32 = 2.0;

const BUY_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_BUY,
    hover: nostalre_resources::ui::BTN_BUY_A,
    pressed: nostalre_resources::ui::BTN_BUY_B,
};
const SELL_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_SELL,
    hover: nostalre_resources::ui::BTN_SELL_A,
    pressed: nostalre_resources::ui::BTN_SELL_B,
};
const CANCEL_BTN_TEX: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CANCEL,
    hover: nostalre_resources::ui::BTN_CANCEL_A,
    pressed: nostalre_resources::ui::BTN_CANCEL_B,
};

const RESIZE_SIZE: f32 = 13.0;
const CHECK_SIZE: f32 = 12.0;
const CHECK_X: f32 = 12.0;
const CHECK_BOTTOM: f32 = 20.0;
const MSI_NOQUESTION_ITEMCOUNT: u16 = 295;

// A barter shop, as the rig draws Dietrich's: wider, five two-row entries in both windows: the item and its name,
// then what it costs in items (amounts under them) and its zeny
const BARTER_WIN_W: f32 = 364.0;
const BARTER_ROWS: usize = 5;
const BARTER_ICON_X: f32 = 44.0;
const BARTER_NAME_X: f32 = 76.0;
const BARTER_COST_X: f32 = 76.0;
const BARTER_COST_STEP: f32 = 45.0;
/// The cost slots an empty basket entry shows
const BARTER_BASKET_COSTS: usize = 2;

/// Size and rows of the two windows for the open shop's kind
struct ListLayout {
    win_w: f32,
    row_h: f32,
    input_rows: usize,
    output_rows: usize,
}
const NOQUESTION_ITEMCOUNT_FALLBACK: &str = "Toggle Item Amount.";

/// Which side of the basket the open quantity popup is answering.
#[derive(Clone, Copy, PartialEq, Debug)]
enum QtyTarget {
    AddToBasket { item_index: usize },
    RemoveFromBasket { basket_index: usize },
}

pub struct NpcShop {
    pub has_grf_textures: bool,
    pub shop: NpcShopData,
    qty_popup: Option<(QtyTarget, InputDialog)>,
    btn_size: (f32, f32),
    scroll_offset: usize,
    output_scroll_offset: usize,
    input_visible_rows: usize,
    resize_start_rows: Option<usize>,
    container: DialogContainer,
    drag_all: bool,
    /// Where the keys are ([`NpcShop::keys`])
    focus: Focus,
    /// The basket line the keys picked
    basket_pick: usize,
}

/// The basket line the keys picked, while a controller is in use
const PICK_TINT: [f32; 4] = [0.20, 0.42, 0.88, 0.50];

/// The side of a shop the keys work
#[derive(Clone, Copy, PartialEq, Debug)]
enum Focus {
    List,
    Basket,
    /// The footer's Buy (or Sell), or its Cancel
    Buttons {
        cancel: bool,
    },
}

impl Default for NpcShop {
    fn default() -> Self {
        Self::new()
    }
}

impl NpcShop {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            shop: NpcShopData::new(),
            qty_popup: None,
            btn_size: (FALLBACK_BTN_W, FALLBACK_BTN_H),
            scroll_offset: 0,
            output_scroll_offset: 0,
            input_visible_rows: INPUT_DEFAULT_ROWS,
            resize_start_rows: None,
            container: DialogContainer::new(),
            drag_all: true,
            focus: Focus::List,
            basket_pick: 0,
        }
    }

    pub fn drag_all(&self) -> bool {
        self.drag_all
    }

    pub fn set_drag_all(&mut self, value: bool) {
        self.drag_all = value;
    }
}

impl Window for NpcShop {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn set_texture_sizes(&mut self, size_fn: &dyn Fn(&str) -> Option<(u32, u32)>) {
        if let Some((w, h)) = size_fn(BUY_BTN.normal) {
            self.btn_size = (w as f32, h as f32);
        }
        self.container.has_grf_textures = true;
        self.container.set_texture_sizes(size_fn);
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = DialogContainer::grf_texture_paths();
        paths.extend_from_slice(&[
            BUY_BTN.normal,
            BUY_BTN.hover,
            BUY_BTN.pressed,
            SELL_BTN.normal,
            SELL_BTN.hover,
            SELL_BTN.pressed,
            CANCEL_BTN_TEX.normal,
            CANCEL_BTN_TEX.hover,
            CANCEL_BTN_TEX.pressed,
            TITLEBAR_TEX,
            ITEMWIN_MID_TEX,
            FOOTER_TEX,
            SYS_BASE_OFF_TEX,
            SYS_BASE_ON_TEX,
            RESIZE_HANDLE_TEX,
            CHECKBOX.off,
            CHECKBOX.on,
        ]);
        paths.extend(scrollbar::grf_texture_paths());
        paths
    }
}

impl InGameWindow for NpcShop {
    fn owns_keyboard(&self, _ctx: &BuildCtx) -> bool {
        self.shop.is_open()
    }

    fn setup_modal(&self, ui: &mut UiFrame) {
        if !self.shop.is_open() {
            return;
        }
        let mut modal_ids = vec![INPUT_WIN_ID, OUTPUT_WIN_ID, ITEM_INFO_WINDOW_ID];
        if let Some((_, ref dialog)) = self.qty_popup {
            modal_ids.push(dialog.win_id());
        }
        ui.set_modal(&modal_ids);
    }

    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.shop.is_open()
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if self.qty_popup.is_some() {
            self.qty_popup = None;
            return Vec::new();
        }
        vec![GameEvent::RequestNpcShopClose]
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.shop.is_open() {
            return Vec::new();
        }

        let mut events = Vec::new();
        // the amount popup takes the keys while it is up; the Enter that opened it is not its answer too
        if self.qty_popup.is_none() {
            self.keys(ui, ctx, &mut events);
            if self.qty_popup.is_some() {
                ui.ctx.key_enter = false;
            }
        }

        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;

        let input_default_x = 100.0;
        let input_default_y = 100.0;
        let output_default_x = input_default_x + self.layout().win_w + WIN_GAP;

        let (input_win_h, output_win_h) = self.window_heights();

        let output_default_y = input_default_y + input_win_h - output_win_h;

        let input_rect = self.build_input_window(
            ui,
            &mut events,
            ctx,
            input_default_x,
            input_default_y,
            input_win_h,
        );
        if let Some((source_id, basket_index)) = ui.drop_zone(input_rect)
            && source_id == OUTPUT_WIN_ID
        {
            self.drop_on_input(basket_index, ctx);
        }

        let output_rect = self.build_output_window(
            ui,
            &mut events,
            ctx,
            output_default_x,
            output_default_y,
            output_win_h,
        );

        if let Some((source_id, item_idx)) = ui.drop_zone(output_rect)
            && source_id == INPUT_WIN_ID
        {
            self.drop_on_output(item_idx, ctx);
        }

        if self.qty_popup.is_some() {
            self.build_quantity_popup(ui);
        }

        ui.has_grf_textures = prev_grf;
        events
    }
}

impl NpcShop {
    fn layout(&self) -> ListLayout {
        if self.shop.is_barter() {
            return ListLayout {
                win_w: BARTER_WIN_W,
                row_h: 2.0 * ITEM_ROW_H,
                input_rows: BARTER_ROWS,
                output_rows: BARTER_ROWS,
            };
        }
        ListLayout {
            win_w: WIN_W,
            row_h: ITEM_ROW_H,
            input_rows: self.input_visible_rows.max(INPUT_MIN_ROWS),
            output_rows: OUTPUT_VISIBLE_ROWS
                .max(self.shop.basket.len().min(5))
                .max(2),
        }
    }

    fn window_heights(&self) -> (f32, f32) {
        let layout = self.layout();
        let height = |rows: usize| {
            TITLE_H + CONTAINER_PAD_Y + rows as f32 * layout.row_h + CONTAINER_PAD_Y + FOOTER_H
        };
        (height(layout.input_rows), height(layout.output_rows))
    }

    /// The input (item list) and output (basket) windows with their current
    /// sizes, for a gallery/packer to lay out. Empty while the shop is closed.
    pub fn gallery_windows(&self) -> Vec<(WidgetId, (f32, f32))> {
        if !self.shop.is_open() {
            return Vec::new();
        }
        let (input_h, output_h) = self.window_heights();
        let w = self.layout().win_w;
        vec![(INPUT_WIN_ID, (w, input_h)), (OUTPUT_WIN_ID, (w, output_h))]
    }

    fn build_input_window(
        &mut self,
        ui: &mut UiFrame,
        events: &mut Vec<GameEvent>,
        ctx: &BuildCtx,
        default_x: f32,
        default_y: f32,
        win_h: f32,
    ) -> Rect {
        let layout = self.layout();
        let barter = self.shop.is_barter();
        let win_w = layout.win_w;
        let title_h = TITLE_H;
        let footer_h = FOOTER_H;
        let row_h = layout.row_h;
        let pad_left = CONTAINER_PAD_LEFT;
        let pad_right = CONTAINER_PAD_RIGHT;
        let pad_y = CONTAINER_PAD_Y;
        let scrollbar_w = SCROLLBAR_W;

        let visible_indices = self.list_indices();
        let item_count = visible_indices.len();
        let visible = layout.input_rows;

        ui.ensure_in_z_order_with(INPUT_WIN_ID, WindowOrder::Foreground);
        let win = ui.window_at(INPUT_WIN_ID, win_w, win_h, title_h, default_x, default_y);
        ui.interact(INPUT_WIN_ID, Rect::new(win.x, win.y, win_w, win_h));

        let grf = self.has_grf_textures;
        let text_color = text_color(grf);

        draw_titlebar(ui, win.x, win.y, win_w, title_h, grf);
        let title = match self.shop.mode {
            Some(NpcShopMode::Buy) => "Shop Items",
            Some(NpcShopMode::Sell) => "Available Items for selling",
            None => "",
        };
        ui.text(win.x + (17.0), win.y + title_h - (3.0), title, text_color);

        let container_y = win.y + title_h;
        let container_h = win_h - title_h - footer_h;
        draw_container(ui, win.x, container_y, win_w, container_h, grf);

        let max_scroll = item_count.saturating_sub(visible);

        let list_y = container_y + pad_y;
        let icon_size = ICON_SIZE;
        let name_x = win.x + pad_left + (ICON_OFFSET_X) + icon_size + (4.0);
        let row_content_w = win_w - pad_left - pad_right - scrollbar_w;

        let icon_x = win.x + pad_left + ICON_OFFSET_X;
        for i in 0..visible {
            let ry = list_y + i as f32 * row_h;
            let Some(&item_idx) = visible_indices.get(self.scroll_offset + i) else {
                if grf {
                    let x = if barter {
                        win.x + BARTER_ICON_X
                    } else {
                        icon_x
                    };
                    draw_slot(ui, x, ry + ICON_OFFSET_Y);
                }
                continue;
            };

            let row_rect = Rect::new(win.x + pad_left, ry, row_content_w, row_h);
            let widget_id = WidgetId(ITEM_BASE_ID + i as u32);
            let response = ui.interact(widget_id, row_rect);
            let is_selected = self.shop.selected_index == Some(item_idx);

            if barter {
                let name = self.item_display_name(item_idx, ctx);
                let entry = BarterEntry {
                    x: win.x,
                    y: ry,
                    w: win_w - scrollbar_w,
                    index: item_idx,
                    under_item: self.shop.stock_left(item_idx).map(i64::from),
                    times: 1,
                    cost_slots: 0,
                    selected: is_selected,
                };
                draw_barter_entry(ui, ctx.data, &self.shop, &entry, &name, grf, text_color);
            } else {
                if grf {
                    draw_slot(ui, icon_x, ry + ICON_OFFSET_Y);
                }
                if let Some(icon_path) = self.shop.item_icon_path(item_idx) {
                    let identified = self.shop.item_is_identified(item_idx);
                    draw_item_icon(ui, icon_path, icon_x, ry + ICON_OFFSET_Y, identified);
                }

                if is_selected {
                    let (v, idx) = draw::quad_vertices(
                        row_rect.x,
                        row_rect.y,
                        row_rect.w,
                        row_rect.h,
                        [0.20, 0.42, 0.88, 0.50],
                    );
                    ui.draw_calls.push(DrawCall {
                        vertices: v.into(),
                        indices: idx.into(),
                        texture: TextureRef::White,
                    });
                }

                // Under the icon: what is left to sell, or a market's stock (kRO draws both like a stack's count)
                let left = match self.shop.mode {
                    Some(NpcShopMode::Sell) => {
                        Some(i64::from(self.shop.sell_item_remaining(item_idx)))
                    }
                    _ => self.shop.stock_left(item_idx).map(i64::from),
                };
                if let Some(left) = left {
                    draw_count(ui, icon_x, ry + ICON_OFFSET_Y, left, text_color);
                }

                let name = self.item_display_name(item_idx, ctx);
                let text_y = ry + row_h - (8.0);
                ui.text(name_x, text_y, &name, text_color);

                let price_right = win.x + win_w - pad_right - scrollbar_w - (8.0);
                draw_shop_price(
                    ui,
                    price_right,
                    text_y,
                    self.shop.item_base_price(item_idx),
                    self.shop.item_price(item_idx),
                    text_color,
                );
            }

            if response.clicked() {
                self.shop.selected_index = Some(item_idx);
                let drag_icon = self.shop.item_icon_path(item_idx);
                ui.drag_source(INPUT_WIN_ID, item_idx, drag_icon, (icon_size, icon_size));
            }
            if response.double_clicked() {
                self.drop_on_output(item_idx, ctx);
            }
            if response.right_clicked()
                && let Some(item) = self.shop.item_at(item_idx)
            {
                events.push(GameEvent::ShowItemInfoDirect {
                    item: Box::new(item.clone()),
                });
            }
        }

        if item_count > visible {
            let sb_x = win.x + win_w - scrollbar_w - (1.0);
            let content_rect = Rect::new(win.x, container_y, win_w, container_h);
            self.scroll_offset = scrollbar::scrollbar(
                ui,
                ScrollbarIds {
                    up: SCROLL_UP_ID,
                    down: SCROLL_DOWN_ID,
                    thumb: SCROLL_THUMB_ID,
                },
                self.scroll_offset,
                visible,
                max_scroll,
                content_rect,
                sb_x,
                container_y,
                container_h,
            );
        }

        let footer_y = win.y + win_h - footer_h;
        draw_footer(ui, win.x, footer_y, win_w, footer_h, grf);

        // kRO offers the toggle only while selling
        if self.shop.mode == Some(NpcShopMode::Sell) {
            let check_rect = Rect::new(
                win.x + CHECK_X,
                win.y + win_h - CHECK_BOTTOM,
                CHECK_SIZE,
                CHECK_SIZE,
            );
            let label = ctx
                .data
                .msg_string
                .as_ref()
                .and_then(|table| table.get(MSI_NOQUESTION_ITEMCOUNT))
                .unwrap_or(NOQUESTION_ITEMCOUNT_FALLBACK);
            let mut checked = self.drag_all;
            if ui
                .checkbox(DRAG_ALL_CHECK_ID, check_rect, &mut checked, &CHECKBOX)
                .clicked()
            {
                self.drag_all = checked;
            }
            ui.text(
                check_rect.x + CHECK_SIZE + 4.0,
                check_rect.y + CHECK_SIZE - 2.0,
                label,
                text_color,
            );
        }

        // a barter keeps its five entries
        if barter {
            return win;
        }
        let resize_rect = Rect::new(
            win.x + win_w - RESIZE_SIZE,
            footer_y + footer_h - RESIZE_SIZE,
            RESIZE_SIZE,
            RESIZE_SIZE,
        );
        let resize = ui.resize_handle(INPUT_RESIZE_ID, resize_rect);
        if resize.started {
            self.resize_start_rows = Some(self.input_visible_rows);
        }
        if resize.dragging
            && let Some(start_rows) = self.resize_start_rows
        {
            let new = (start_rows as f32 + resize.delta_y / row_h).round() as i32;
            self.input_visible_rows =
                new.clamp(INPUT_MIN_ROWS as i32, INPUT_MAX_ROWS as i32) as usize;
        }

        win
    }

    fn build_output_window(
        &mut self,
        ui: &mut UiFrame,
        events: &mut Vec<GameEvent>,
        ctx: &BuildCtx,
        default_x: f32,
        default_y: f32,
        win_h: f32,
    ) -> Rect {
        let layout = self.layout();
        let barter = self.shop.is_barter();
        let win_w = layout.win_w;
        let title_h = TITLE_H;
        let footer_h = FOOTER_H;
        let row_h = layout.row_h;
        let pad_left = CONTAINER_PAD_LEFT;
        let pad_right = CONTAINER_PAD_RIGHT;
        let pad_y = CONTAINER_PAD_Y;
        let scrollbar_w = SCROLLBAR_W;
        let (btn_w, btn_h) = self.btn_size;

        ui.ensure_in_z_order_with(OUTPUT_WIN_ID, WindowOrder::Foreground);
        let win = ui.window_at(OUTPUT_WIN_ID, win_w, win_h, title_h, default_x, default_y);
        ui.interact(OUTPUT_WIN_ID, Rect::new(win.x, win.y, win_w, win_h));

        let grf = self.has_grf_textures;
        let text_color = text_color(grf);

        draw_titlebar(ui, win.x, win.y, win_w, title_h, grf);
        let title = match self.shop.mode {
            _ if barter => "Items you want to exchange",
            Some(NpcShopMode::Buy) => "Buying Items",
            Some(NpcShopMode::Sell) => "Selling Items",
            None => "",
        };
        ui.text(win.x + (17.0), win.y + title_h - (3.0), title, text_color);

        let container_y = win.y + title_h;
        let container_h = win_h - title_h - footer_h;
        draw_container(ui, win.x, container_y, win_w, container_h, grf);

        let list_y = container_y + pad_y;
        let icon_size = ICON_SIZE;
        let name_x = win.x + pad_left + (ICON_OFFSET_X) + icon_size + (4.0);
        let basket_count = self.shop.basket.len();
        let visible = layout.output_rows;
        let has_scrollbar = basket_count > visible;
        let row_content_w =
            win_w - pad_left - pad_right - if has_scrollbar { scrollbar_w } else { 0.0 };

        let max_scroll = basket_count.saturating_sub(visible);
        let mut double_clicked_row = None;

        let icon_x = win.x + pad_left + ICON_OFFSET_X;
        for i in 0..visible {
            let ry = list_y + i as f32 * row_h;
            let ci = self.output_scroll_offset + i;
            let Some(basket_item) = self.shop.basket.get(ci) else {
                if grf && barter {
                    draw_empty_barter_entry(ui, win.x, ry);
                } else if grf {
                    draw_slot(ui, icon_x, ry + ICON_OFFSET_Y);
                }
                continue;
            };
            let row_rect = Rect::new(win.x + pad_left, ry, row_content_w, row_h);
            let widget_id = WidgetId(BASKET_BASE_ID + i as u32);
            let response = ui.interact(widget_id, row_rect);
            if response.hovered() {
                ui.any_interactive_hovered = true;
            }
            let picked = self.focus == Focus::Basket && ui.ctx.pad_labels && ci == self.basket_pick;
            if picked && !barter {
                let (v, idx) =
                    draw::quad_vertices(row_rect.x, row_rect.y, row_rect.w, row_rect.h, PICK_TINT);
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: idx.into(),
                    texture: TextureRef::White,
                });
            }

            if barter {
                let name = self.item_display_name(basket_item.source_index, ctx);
                let entry = BarterEntry {
                    x: win.x,
                    y: ry,
                    w: row_content_w + pad_left,
                    index: basket_item.source_index,
                    under_item: Some(i64::from(basket_item.quantity)),
                    times: u32::try_from(basket_item.quantity).unwrap_or(0),
                    cost_slots: BARTER_BASKET_COSTS,
                    selected: picked,
                };
                draw_barter_entry(ui, ctx.data, &self.shop, &entry, &name, grf, text_color);
            } else {
                if grf {
                    draw_slot(ui, icon_x, ry + ICON_OFFSET_Y);
                }
                if let Some(icon_path) = self.shop.item_icon_path(basket_item.source_index) {
                    let identified = self.shop.item_is_identified(basket_item.source_index);
                    draw_item_icon(ui, icon_path, icon_x, ry + ICON_OFFSET_Y, identified);
                }

                if response.hovered() {
                    let (v, idx) = draw::quad_vertices(
                        row_rect.x,
                        row_rect.y,
                        row_rect.w,
                        row_rect.h,
                        [0.88, 0.20, 0.20, 0.15],
                    );
                    ui.draw_calls.push(DrawCall {
                        vertices: v.into(),
                        indices: idx.into(),
                        texture: TextureRef::White,
                    });
                }

                let text_y = ry + row_h - (8.0);

                let qty_str = basket_item.quantity.to_string();
                let qty_w = ui.atlas.measure_text(&qty_str);
                let qty_x = win.x + pad_left + (ICON_OFFSET_X) + icon_size - qty_w;
                ui.text(qty_x, ry + icon_size, &qty_str, text_color);

                let name = self.item_display_name(basket_item.source_index, ctx);
                ui.text(name_x, text_y, &name, text_color);

                let price = self.shop.item_price(basket_item.source_index);
                let subtotal = price as i64 * basket_item.quantity as i64;
                let price_str = format_zeny(subtotal as i32);
                let z_x = win.x + win_w
                    - pad_right
                    - if has_scrollbar { scrollbar_w } else { 0.0 }
                    - (10.0);
                let price_w = ui.atlas.measure_text(&price_str);
                let price_x = z_x - (2.0) - price_w;
                ui.text(price_x, text_y, &price_str, text_color);
                ui.text(z_x, text_y, "Z", text_color);
            }

            if response.right_clicked()
                && let Some(item) = self.shop.item_at(basket_item.source_index)
            {
                events.push(GameEvent::ShowItemInfoDirect {
                    item: Box::new(item.clone()),
                });
            }

            if response.clicked() {
                let drag_icon = self.shop.item_icon_path(basket_item.source_index);
                ui.drag_source(OUTPUT_WIN_ID, ci, drag_icon, (icon_size, icon_size));
            }
            if response.double_clicked() {
                double_clicked_row = Some(ci);
            }
        }

        if let Some(ci) = double_clicked_row {
            self.drop_on_input(ci, ctx);
        }

        if has_scrollbar {
            let sb_x = win.x + win_w - scrollbar_w - (1.0);
            let content_rect = Rect::new(win.x, container_y, win_w, container_h);
            self.output_scroll_offset = scrollbar::scrollbar(
                ui,
                ScrollbarIds {
                    up: OUT_SCROLL_UP_ID,
                    down: OUT_SCROLL_DOWN_ID,
                    thumb: OUT_SCROLL_THUMB_ID,
                },
                self.output_scroll_offset,
                visible,
                max_scroll,
                content_rect,
                sb_x,
                container_y,
                container_h,
            );
        }

        let footer_y = win.y + win_h - footer_h;
        draw_footer(ui, win.x, footer_y, win_w, footer_h, grf);

        // a barter's footer has only its buttons
        if !barter {
            let total = self.shop.basket_total();
            let total_label = format!("Total : {} Zeny", format_thousands(total));
            ui.text(
                win.x + (10.0),
                footer_y + footer_h - (10.0),
                &total_label,
                text_color,
            );
        }

        let btn_y = footer_y + (4.0);
        let cancel_x = win.x + win_w - (15.0) - btn_w;
        let action_x = cancel_x - (5.0) - btn_w;

        let (action_tex, action_label) = match self.shop.mode {
            Some(NpcShopMode::Sell) => (&SELL_BTN, crate::pt!("Sell", "Vender")),
            _ => (&BUY_BTN, crate::pt!("Buy", "Comprar")),
        };
        let action_rect = Rect::new(action_x, btn_y, btn_w, btn_h);
        let cancel_rect = Rect::new(cancel_x, btn_y, btn_w, btn_h);
        let action_btn = ui.button(BUY_SELL_BTN_ID, action_rect, action_tex, action_label);
        let cancel_btn = ui.button(CANCEL_BTN_ID, cancel_rect, &CANCEL_BTN_TEX, crate::pt!("Cancel", "Cancelar"));
        if ui.ctx.pad_labels
            && let Focus::Buttons { cancel } = self.focus
        {
            crate::helper::focus_ring(ui, if cancel { cancel_rect } else { action_rect });
        }

        if action_btn.clicked() {
            self.confirm(ctx, events);
        }

        if cancel_btn.clicked() {
            events.push(GameEvent::RequestNpcShopClose);
        }

        win
    }

    /// The list's lines: what is for sale, or what can be sold
    fn list_indices(&self) -> Vec<usize> {
        match self.shop.mode {
            Some(NpcShopMode::Sell) => self.shop.visible_sell_indices(),
            _ => (0..self.shop.item_count()).collect(),
        }
    }

    /// The keys, a controller's D-pad and buttons. Up/Down walk the list or the basket, Left/Right go between
    /// them, Down off the basket's end (or Right from the list with nothing in it) reaches the footer's buttons;
    /// Enter (A) moves the picked line across (asking the amount as a drag does) or presses the button. A
    /// controller's Start (Space while one is in use, so a keyboard's stray Space buys nothing) buys or sells
    /// from anywhere
    fn keys(&mut self, ui: &UiFrame, ctx: &BuildCtx, events: &mut Vec<GameEvent>) {
        let c = &ui.ctx;
        let step = isize::from(c.key_down) - isize::from(c.key_up);
        let basket_len = self.shop.basket.len();
        // a line taken out since: the pick stays on the basket's last
        self.basket_pick = self.basket_pick.min(basket_len.saturating_sub(1));
        if self.focus == Focus::Basket && basket_len == 0 {
            self.focus = Focus::List;
        }
        let layout = self.layout();
        match self.focus {
            Focus::List if c.key_right => {
                self.focus = if basket_len > 0 {
                    Focus::Basket
                } else {
                    Focus::Buttons { cancel: false }
                };
            }
            Focus::List => {
                let lines = self.list_indices();
                let at = self
                    .shop
                    .selected_index
                    .and_then(|index| lines.iter().position(|&l| l == index));
                // the first key on a list nothing is picked in picks its first line
                let at = match at {
                    Some(at) => at
                        .saturating_add_signed(step)
                        .min(lines.len().saturating_sub(1)),
                    None if !lines.is_empty() && (step != 0 || c.key_enter) => 0,
                    None => usize::MAX,
                };
                if let Some(&index) = lines.get(at) {
                    self.shop.selected_index = Some(index);
                    keep_in_view(&mut self.scroll_offset, at, layout.input_rows);
                    if c.key_enter {
                        self.drop_on_output(index, ctx);
                    }
                }
            }
            Focus::Basket if c.key_left => self.focus = Focus::List,
            Focus::Basket if c.key_down && self.basket_pick + 1 >= basket_len => {
                self.focus = Focus::Buttons { cancel: false };
            }
            Focus::Basket => {
                self.basket_pick = self
                    .basket_pick
                    .saturating_add_signed(step)
                    .min(basket_len - 1);
                keep_in_view(
                    &mut self.output_scroll_offset,
                    self.basket_pick,
                    layout.output_rows,
                );
                if c.key_enter {
                    self.drop_on_input(self.basket_pick, ctx);
                }
            }
            Focus::Buttons { cancel } => {
                if c.key_left {
                    self.focus = if cancel {
                        Focus::Buttons { cancel: false }
                    } else {
                        Focus::List
                    };
                } else if c.key_right {
                    self.focus = Focus::Buttons { cancel: true };
                } else if c.key_up {
                    self.focus = if basket_len > 0 {
                        Focus::Basket
                    } else {
                        Focus::List
                    };
                } else if c.key_enter && cancel {
                    events.push(GameEvent::RequestNpcShopClose);
                } else if c.key_enter {
                    self.confirm(ctx, events);
                }
            }
        }
        if c.pad_labels && c.pressed_codes.contains(&KeyCode::Space) {
            self.confirm(ctx, events);
        }
    }

    /// Buys or sells what is in the basket: the footer's button, or Space (a controller's Start)
    fn confirm(&mut self, ctx: &BuildCtx, events: &mut Vec<GameEvent>) {
        if self.shop.basket.is_empty() {
            return;
        }
        match self.shop.mode {
            Some(NpcShopMode::Buy) => {
                let items: Vec<(i16, u32)> = self
                    .shop
                    .basket
                    .iter()
                    .map(|c| {
                        let item_id = self.shop.buy_items[c.source_index].item.item_id;
                        (c.quantity, item_id)
                    })
                    .collect();
                // a market or a barter stays open: the basket empties now, so a second click buys nothing
                // twice, and the answer takes what was bought off the stock
                match self.shop.kind {
                    ShopKind::Market => {
                        events.push(GameEvent::RequestNpcMarketBuy { items });
                        self.shop.basket.clear();
                    }
                    ShopKind::Barter { extended } => {
                        let items = self.shop.take_barter_purchase(&ctx.character.inventory);
                        events.push(GameEvent::RequestNpcBarterBuy { extended, items });
                    }
                    ShopKind::Plain => {
                        events.push(GameEvent::RequestNpcShopBuy { items });
                        self.close();
                    }
                }
            }
            Some(NpcShopMode::Sell) => {
                let items: Vec<(i16, i16)> = self
                    .shop
                    .basket
                    .iter()
                    .map(|c| {
                        let index = self.shop.sell_items[c.source_index].item.index as i16;
                        (index, c.quantity)
                    })
                    .collect();
                events.push(GameEvent::RequestNpcShopSell { items });
                self.close();
            }
            None => {}
        }
    }

    fn item_display_name(&self, index: usize, ctx: &BuildCtx) -> String {
        self.shop
            .item_display_name(index, ctx.data, &ctx.character.char_names)
    }

    fn open_qty_popup(&mut self, target: QtyTarget, name: &str) {
        let available = match target {
            QtyTarget::AddToBasket { item_index } => match self.shop.mode {
                Some(NpcShopMode::Sell) => {
                    Some(self.shop.sell_item_remaining(item_index)).filter(|left| *left > 0)
                }
                _ => self
                    .shop
                    .stock_left(item_index)
                    .map(|left| left.min(i16::MAX as u32) as i16),
            },
            QtyTarget::RemoveFromBasket { basket_index } => {
                Some(self.shop.basket_quantity(basket_index)).filter(|left| *left > 0)
            }
        };
        // buying starts empty, even with a market's stock as the limit
        let buying = matches!(target, QtyTarget::AddToBasket { .. })
            && self.shop.mode == Some(NpcShopMode::Buy);
        let default_qty = available
            .filter(|_| !buying)
            .map(|remaining| remaining.to_string())
            .unwrap_or_default();
        let mut dialog = InputDialog::new(
            InputDialogConfig {
                layout: InputDialogLayout::ItemCount {
                    item_name: name.to_string(),
                },
                escape_cancels: false,
                default_value: default_qty,
                max_len: 6,
                numeric_only: true,
                max_value: available.map(|remaining| remaining as i32),
            },
            WidgetId(QTY_INPUT_ID.0),
        );
        dialog.init_container(&self.container);
        self.qty_popup = Some((target, dialog));
    }

    /// An item-list row dropped on the basket. The drag-all box only short-circuits
    /// the prompt while selling; buying always asks for a stackable.
    fn drop_on_output(&mut self, item_index: usize, ctx: &BuildCtx) {
        if !self.shop.needs_quantity_prompt(item_index) {
            self.shop.add_to_basket(item_index, 1);
            return;
        }
        if self.drag_all && self.shop.mode == Some(NpcShopMode::Sell) {
            let remaining = self.shop.sell_item_remaining(item_index);
            if remaining > 0 {
                self.shop.add_to_basket(item_index, remaining);
            }
            return;
        }
        let name = self.item_display_name(item_index, ctx);
        self.open_qty_popup(QtyTarget::AddToBasket { item_index }, &name);
    }

    /// A basket row dropped back on the item list: the original removes the whole
    /// line when the drag-all box is ticked or only one is staged, and otherwise
    /// asks how many to take out.
    fn drop_on_input(&mut self, basket_index: usize, ctx: &BuildCtx) {
        let staged = self.shop.basket_quantity(basket_index);
        if staged <= 0 {
            return;
        }
        if self.drag_all || staged == 1 {
            self.shop.remove_from_basket(basket_index, staged);
            return;
        }
        let Some(line) = self.shop.basket.get(basket_index) else {
            return;
        };
        let name = self.item_display_name(line.source_index, ctx);
        self.open_qty_popup(QtyTarget::RemoveFromBasket { basket_index }, &name);
    }

    fn build_quantity_popup(&mut self, ui: &mut UiFrame) {
        let (target, dialog) = self.qty_popup.as_mut().unwrap();
        let target = *target;
        dialog.init_container(&self.container);

        match dialog.build(ui) {
            InputDialogResult::Submitted => {
                let qty: i16 = dialog.value_i16().unwrap_or(0);
                if qty > 0 {
                    match target {
                        QtyTarget::AddToBasket { item_index } => {
                            self.shop.add_to_basket(item_index, qty)
                        }
                        QtyTarget::RemoveFromBasket { basket_index } => {
                            self.shop.remove_from_basket(basket_index, qty)
                        }
                    }
                }
                self.qty_popup = None;
            }
            InputDialogResult::Cancel => {
                self.qty_popup = None;
            }
            InputDialogResult::None => {}
        }
    }

    pub fn close(&mut self) {
        self.shop.close();
        self.qty_popup = None;
        self.scroll_offset = 0;
        self.output_scroll_offset = 0;
        self.input_visible_rows = INPUT_DEFAULT_ROWS;
        self.resize_start_rows = None;
        self.focus = Focus::List;
        self.basket_pick = 0;
    }
}

/// The pale oval kRO draws under every icon place of a shop list, filled or not
/// `offset` moved so line `at` is among the `rows` shown
fn keep_in_view(offset: &mut usize, at: usize, rows: usize) {
    if at < *offset {
        *offset = at;
    } else if at >= *offset + rows {
        *offset = at + 1 - rows;
    }
}

fn draw_slot(ui: &mut UiFrame, x: f32, y: f32) {
    draw_quad(ui, x, y, [1.0; 4], ITEMWIN_MID_TEX.to_string());
}

/// An item's icon, greyed while unidentified
fn draw_item_icon(ui: &mut UiFrame, path: String, x: f32, y: f32, identified: bool) {
    let tint = if identified {
        [1.0; 4]
    } else {
        [0.67, 0.67, 0.67, 1.0]
    };
    draw_quad(ui, x, y, tint, path);
}

fn draw_quad(ui: &mut UiFrame, x: f32, y: f32, tint: [f32; 4], texture: String) {
    let (v, idx) = draw::quad_vertices(x, y, ICON_SIZE, ICON_SIZE, tint);
    ui.draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: idx.into(),
        texture: TextureRef::Named(texture.into()),
    });
}

/// A count under an icon's right edge, as kRO draws a stack's
fn draw_count(ui: &mut UiFrame, icon_x: f32, icon_y: f32, count: i64, color: [f32; 4]) {
    let text = count.to_string();
    let w = ui.atlas.measure_text(&text);
    let x = icon_x + ICON_SIZE - w + 2.0;
    ui.text(x, icon_y - ICON_OFFSET_Y + ITEM_ROW_H - 2.0, &text, color);
}

/// Where and what one barter entry shows: `under_item` (its stock, or the amount in the basket) and its costs
/// `times` over, with at least `cost_slots` slots
struct BarterEntry {
    x: f32,
    y: f32,
    w: f32,
    index: usize,
    under_item: Option<i64>,
    times: u32,
    cost_slots: usize,
    selected: bool,
}

/// A barter entry over two rows: the item and its name, then each item it costs with its amount, and its zeny
fn draw_barter_entry(
    ui: &mut UiFrame,
    data: &DataTable,
    shop: &NpcShopData,
    entry: &BarterEntry,
    name: &str,
    grf: bool,
    color: [f32; 4],
) {
    let icon_x = entry.x + BARTER_ICON_X;
    let icon_y = entry.y + ICON_OFFSET_Y;
    let cost_y = icon_y + ITEM_ROW_H;
    if entry.selected {
        let x = entry.x + BARTER_NAME_X - 4.0;
        let (v, idx) = draw::quad_vertices(
            x,
            entry.y,
            entry.x + entry.w - x,
            2.0 * ITEM_ROW_H - 4.0,
            [0.20, 0.42, 0.88, 0.50],
        );
        ui.draw_calls.push(DrawCall {
            vertices: v.into(),
            indices: idx.into(),
            texture: TextureRef::White,
        });
    }
    if grf {
        draw_slot(ui, icon_x, icon_y);
    }
    if let Some(path) = shop.item_icon_path(entry.index) {
        draw_item_icon(ui, path, icon_x, icon_y, true);
    }
    if let Some(count) = entry.under_item {
        draw_count(ui, icon_x, icon_y, count, color);
    }
    ui.text(
        entry.x + BARTER_NAME_X,
        entry.y + ITEM_ROW_H - 8.0,
        name,
        color,
    );

    let costs = shop.barter.get(entry.index).map_or(&[][..], |b| &b.costs);
    for k in 0..costs.len().max(entry.cost_slots) {
        let x = entry.x + BARTER_COST_X + k as f32 * BARTER_COST_STEP;
        if grf {
            draw_slot(ui, x, cost_y);
        }
        let Some(cost) = costs.get(k) else {
            continue;
        };
        if let Some(path) = data.item_icon_path(cost.item_id) {
            draw_item_icon(ui, path, x, cost_y, true);
        }
        let amount = i64::from(cost.amount) * i64::from(entry.times);
        draw_count(ui, x, cost_y, amount, color);
        // the refine an item must have, over its icon, and its name on hover
        if cost.refine > 0 {
            ui.text(x, cost_y + 10.0, &format!("+{}", cost.refine), color);
        }
        let icon = Rect::new(x, cost_y, ICON_SIZE, ICON_SIZE);
        if icon.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) && ui.pointer_available() {
            let name = data.item_name_for(cost.item_id, true);
            ui.tooltip(
                x + ICON_SIZE / 2.0,
                cost_y,
                &cost_label(&name, cost.refine, amount),
            );
        }
    }

    // kRO writes a barter's zeny plain: "90000 z"
    let zeny = i64::from(shop.item_price(entry.index)) * i64::from(entry.times);
    let text = format!("{zeny} z");
    let w = ui.atlas.measure_text(&text);
    ui.text(
        entry.x + entry.w - 12.0 - w,
        entry.y + 2.0 * ITEM_ROW_H - 8.0,
        &text,
        color,
    );
}

/// A barter cost's tooltip: "+4 Blade x 1"
fn cost_label(name: &str, refine: u16, amount: i64) -> String {
    let refine = if refine > 0 {
        format!("+{refine} ")
    } else {
        String::new()
    };
    format!("{refine}{name} x {amount}")
}

/// An empty basket entry of a barter: the item's slot and two cost slots
fn draw_empty_barter_entry(ui: &mut UiFrame, x: f32, y: f32) {
    draw_slot(ui, x + BARTER_ICON_X, y + ICON_OFFSET_Y);
    for k in 0..BARTER_BASKET_COSTS {
        let cost_x = x + BARTER_COST_X + k as f32 * BARTER_COST_STEP;
        draw_slot(ui, cost_x, y + ICON_OFFSET_Y + ITEM_ROW_H);
    }
}

/// Right-aligned shop price. When `base` differs from `final_` (Discount/Overcharge
/// skill in play) it renders `base -> final_`, the base in `muted` and the final in the
/// digit-bucket price color.
fn draw_shop_price(
    ui: &mut UiFrame,
    right_x: f32,
    y: f32,
    base: i32,
    final_: i32,
    muted: [f32; 4],
) {
    let final_str = format!("{} Z", format_thousands(final_ as i64));
    if base != final_ {
        let base_str = format!("{} -> {}", format_thousands(base as i64), final_str);
        let base_w = ui.atlas.measure_text(&base_str);
        ui.text(right_x - base_w, y, &base_str, muted);
    } else {
        let x = right_x - ui.atlas.measure_text(&final_str);
        ui.text(x, y, &final_str, muted);
    }
}

fn format_zeny(amount: i32) -> String {
    if amount < 0 {
        return format!("-{}", format_zeny(-amount));
    }
    let s = amount.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InGameWindow;
    use crate::game::minimap_window::MINIMAP_WINDOW_ID;
    use crate::game::skill_tree_window::SKILL_WINDOW_ID;
    use models::enums::item::ItemType;
    use nostalre_game::character::Character;
    use nostalre_game::data_table::DataTable;
    use nostalre_game::item::Item;
    use nostalre_game::npc_shop::{ShopBuyItem, ShopSellItem};

    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    fn buy_line(item_id: u32, name: &str) -> ShopBuyItem {
        ShopBuyItem {
            item: Item {
                grade: 0,
                options: Default::default(),
                index: 0,
                item_id,
                item_type: ItemType::Healing,
                count: 1,
                is_identified: true,
                is_damaged: false,
                refining_level: 0,
                slot: [0; 4],
                location: 0,
                wear_state: 0,
                name: name.into(),
                resource_name: None,
            },
            price: 50,
            discount_price: 50,
        }
    }

    #[test]
    fn keys_pick_a_line_ask_its_amount_and_buy() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_buy(
            100,
            vec![buy_line(501, "Red Potion"), buy_line(502, "Orange Potion")],
        );
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();
        let mut frame = |shop_ui: &mut NpcShop, keys: fn(&mut UiContext)| {
            let mut ui_ctx = UiContext::new(1024.0, 768.0);
            keys(&mut ui_ctx);
            let mut ui = test_frame(&mut ui_ctx, &mut state);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
        };

        frame(&mut shop_ui, |c| c.key_down = true);
        frame(&mut shop_ui, |c| c.key_down = true);
        assert_eq!(
            shop_ui.shop.selected_index,
            Some(1),
            "the first key picks the first line, the next walks on"
        );
        frame(&mut shop_ui, |c| c.key_enter = true);
        assert!(
            shop_ui.qty_popup.is_some(),
            "Enter asks how many, and is not the answer"
        );
        frame(&mut shop_ui, |c| c.key_up = true);
        frame(&mut shop_ui, |c| c.key_up = true);
        frame(&mut shop_ui, |c| c.key_enter = true);
        assert_eq!(shop_ui.shop.basket.len(), 1);
        assert_eq!(
            shop_ui.shop.basket[0].quantity, 2,
            "Up twice from empty is two"
        );

        let events = frame(&mut shop_ui, |c| c.pressed_codes.push(KeyCode::Space));
        assert!(
            events.is_empty(),
            "a keyboard's Space buys nothing, got {events:?}"
        );
        let events = frame(&mut shop_ui, |c| {
            c.pad_labels = true;
            c.pressed_codes.push(KeyCode::Space);
        });
        assert!(
            matches!(events.as_slice(), [GameEvent::RequestNpcShopBuy { items }] if items == &vec![(2, 502)]),
            "Space (Start) buys the basket, got {events:?}"
        );
    }

    #[test]
    fn keys_reach_the_footer_buttons() {
        let mut shop_ui = NpcShop::new();
        shop_ui
            .shop
            .open_buy(100, vec![buy_line(501, "Red Potion")]);
        shop_ui.shop.add_to_basket(0, 3);
        let mut state = StateCache::new();
        let mut character = Character::new();
        let data = DataTable::new();
        let mut frame = |shop_ui: &mut NpcShop, keys: fn(&mut UiContext)| {
            let mut ui_ctx = UiContext::new(1024.0, 768.0);
            keys(&mut ui_ctx);
            let mut ui = test_frame(&mut ui_ctx, &mut state);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
        };

        frame(&mut shop_ui, |c| c.key_right = true);
        assert_eq!(shop_ui.focus, Focus::Basket);
        frame(&mut shop_ui, |c| c.key_down = true);
        assert_eq!(
            shop_ui.focus,
            Focus::Buttons { cancel: false },
            "Down off the basket's end"
        );
        let events = frame(&mut shop_ui, |c| c.key_enter = true);
        assert!(
            matches!(events.as_slice(), [GameEvent::RequestNpcShopBuy { items }] if items == &vec![(3, 501)]),
            "Enter on Buy buys, got {events:?}"
        );

        let mut shop_ui = NpcShop::new();
        shop_ui
            .shop
            .open_buy(100, vec![buy_line(501, "Red Potion")]);
        frame(&mut shop_ui, |c| c.key_right = true);
        frame(&mut shop_ui, |c| c.key_right = true);
        let events = frame(&mut shop_ui, |c| c.key_enter = true);
        assert!(
            matches!(events.as_slice(), [GameEvent::RequestNpcShopClose]),
            "an empty basket: Right reaches Buy, Right again Cancel, got {events:?}"
        );
    }

    #[test]
    fn escape_closes_shop() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 0,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 1,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 50,
                discount_price: 50,
            }],
        );

        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = crate::BuildCtx::test(&mut character, &data);

        assert!(shop_ui.wants_escape(&ctx));
        let events = shop_ui.on_escape(&mut ctx);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], GameEvent::RequestNpcShopClose));
        assert!(shop_ui.shop.is_open());
    }

    #[test]
    fn sell_qty_popup_defaults_to_remaining_stack() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_sell(
            100,
            vec![ShopSellItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 3,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 16,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 25,
                overcharge_price: 25,
            }],
        );

        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");
        assert_eq!(shop_ui.qty_popup.as_ref().unwrap().1.value_i16(), Some(16));

        shop_ui.qty_popup = None;
        shop_ui.shop.add_to_basket(0, 6);
        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");
        assert_eq!(
            shop_ui.qty_popup.as_ref().unwrap().1.value_i16(),
            Some(10),
            "default must follow what is left after the basket"
        );

        shop_ui.qty_popup = None;
        shop_ui.shop.add_to_basket(0, 10);
        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");
        assert_eq!(shop_ui.qty_popup.as_ref().unwrap().1.value_str(), "");
    }

    #[test]
    fn basket_row_drag_or_double_click_asks_how_many_unless_drag_all() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_sell(
            100,
            vec![ShopSellItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 3,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 16,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 25,
                overcharge_price: 25,
            }],
        );
        shop_ui.shop.add_to_basket(0, 10);

        let mut state = StateCache::new();
        let frame = |state: &mut StateCache, shop_ui: &mut NpcShop, mx: f32, my: f32, down: bool| {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(800.0, 600.0);
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            ctx.mouse_down = down;
            let mut ui = test_frame(&mut ctx, state);
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
            ui.draw_drag_icon();
        };

        let (input_h, output_h) = shop_ui.window_heights();
        let basket_row = (
            100.0 + WIN_W + WIN_GAP + CONTAINER_PAD_LEFT + ICON_SIZE,
            100.0 + input_h - output_h + TITLE_H + CONTAINER_PAD_Y + ICON_SIZE / 2.0,
        );
        let list_row = (150.0, 100.0 + TITLE_H + CONTAINER_PAD_Y + ICON_SIZE / 2.0);
        let mut drag_basket_to_list = |state: &mut StateCache, shop_ui: &mut NpcShop| {
            frame(state, shop_ui, basket_row.0, basket_row.1, false);
            {
                let mut character = Character::new();
                let data = DataTable::new();
                let mut ctx = UiContext::new(800.0, 600.0);
                ctx.mouse_x = basket_row.0;
                ctx.mouse_y = basket_row.1;
                ctx.mouse_clicked = true;
                ctx.mouse_down = true;
                let mut ui = test_frame(&mut ctx, state);
                shop_ui.setup_modal(&mut ui);
                let z = ui.get_z_order();
                ui.compute_hovered_window(&z);
                shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
                ui.draw_drag_icon();
            }
            frame(state, shop_ui, list_row.0, list_row.1, true);
            frame(state, shop_ui, list_row.0, list_row.1, false);
        };

        shop_ui.drag_all = false;
        drag_basket_to_list(&mut state, &mut shop_ui);
        let (target, dialog) = shop_ui.qty_popup.as_mut().unwrap();
        assert_eq!(*target, QtyTarget::RemoveFromBasket { basket_index: 0 });
        assert_eq!(dialog.value_str(), "10");
        dialog.set_input_text("4");

        let mut ui_ctx = UiContext::new(800.0, 600.0);
        ui_ctx.key_enter = true;
        {
            let mut ui = test_frame(&mut ui_ctx, &mut state);
            shop_ui.build_quantity_popup(&mut ui);
        }
        assert!(shop_ui.qty_popup.is_none());
        assert_eq!(shop_ui.shop.basket_quantity(0), 6);

        shop_ui.drag_all = true;
        {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(800.0, 600.0);
            ctx.mouse_x = basket_row.0;
            ctx.mouse_y = basket_row.1;
            ctx.mouse_clicked = true;
            ctx.mouse_double_clicked = true;
            ctx.mouse_down = true;
            let mut ui = test_frame(&mut ctx, &mut state);
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        }
        assert!(shop_ui.qty_popup.is_none());
        assert!(shop_ui.shop.basket.is_empty());
    }

    #[test]
    fn buy_qty_popup_starts_empty() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 0,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 1,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 50,
                discount_price: 50,
            }],
        );

        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");
        assert_eq!(shop_ui.qty_popup.as_ref().unwrap().1.value_str(), "");
    }

    #[test]
    fn a_reopened_quantity_prompt_comes_back_on_top() {
        let mut shop_ui = NpcShop::new();
        let mut state = StateCache::new();
        shop_ui.shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 0,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 1,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 50,
                discount_price: 50,
            }],
        );
        // one frame; `raise` comes forward first, as a click on it would bring it
        let mut frame = |shop_ui: &mut NpcShop, raise: Option<WidgetId>| -> Vec<WidgetId> {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(1280.0, 800.0);
            let mut ui = test_frame(&mut ctx, &mut state);
            if let Some(id) = raise {
                ui.bring_to_front(id);
            }
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
            ui.get_z_order()
        };
        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");
        frame(&mut shop_ui, None);
        let prompt = shop_ui.qty_popup.as_ref().unwrap().1.win_id();
        shop_ui.qty_popup = None;
        frame(&mut shop_ui, Some(OUTPUT_WIN_ID));
        let z = frame(&mut shop_ui, None);
        let at = |id| z.iter().position(|&w| w == id);
        assert!(at(OUTPUT_WIN_ID) > at(prompt), "the basket rose over the old prompt: {z:?}");

        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");
        frame(&mut shop_ui, None);
        let z = frame(&mut shop_ui, None);
        assert_eq!(z.last(), Some(&prompt), "{z:?}");
    }

    #[test]
    fn escape_closes_qty_popup_first() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 0,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 1,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 50,
                discount_price: 50,
            }],
        );
        shop_ui.open_qty_popup(QtyTarget::AddToBasket { item_index: 0 }, "Red Potion");

        let mut character = Character::new();
        let data = DataTable::new();
        let mut ctx = crate::BuildCtx::test(&mut character, &data);

        let events = shop_ui.on_escape(&mut ctx);
        assert!(events.is_empty());
        assert!(shop_ui.qty_popup.is_none());
        assert!(shop_ui.shop.is_open());
    }

    #[test]
    fn closed_shop_returns_no_events() {
        let mut shop_ui = NpcShop::new();
        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        let mut ui = test_frame(&mut ctx, &mut state);

        let events = shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(events.is_empty());
    }

    #[test]
    fn shop_windows_claim_the_pointer_over_their_whole_body() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 0,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 1,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 50,
                discount_price: 50,
            }],
        );

        let mut state = StateCache::new();
        // The client builds other windows before the shop, then the shop on top.
        let frame = |state: &mut StateCache, shop_ui: &mut NpcShop, mx: f32, my: f32| -> bool {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(800.0, 600.0);
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            let mut ui = test_frame(&mut ctx, state);
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            ui.window_at(MINIMAP_WINDOW_ID, 200.0, 200.0, 15.0, 600.0, 0.0);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
            ui.any_hovered
        };

        frame(&mut state, &mut shop_ui, 0.0, 0.0);

        let (input_h, output_h) = shop_ui.window_heights();
        // the basket sits beside the list, bottoms aligned
        let basket_y = 100.0 + input_h - output_h;
        assert!(
            frame(&mut state, &mut shop_ui, 150.0, 105.0),
            "title bar must not fall through to the world"
        );
        assert!(
            frame(&mut state, &mut shop_ui, 150.0, 100.0 + input_h - 5.0),
            "footer must not fall through to the world"
        );
        assert!(
            frame(&mut state, &mut shop_ui, 400.0, basket_y + 5.0),
            "basket window title bar must not fall through to the world"
        );
        assert!(
            !frame(&mut state, &mut shop_ui, 600.0, 500.0),
            "the world stays reachable outside the shop"
        );
    }

    #[test]
    fn a_barter_trades_through_its_own_request_and_stays_open() {
        use nostalre_game::event::{BarterCost, BarterItem};
        let mut shop_ui = NpcShop::new();
        let mut state = StateCache::new();
        let terms = BarterItem {
            item_id: 6224,
            item_type: 3,
            shop_index: 4,
            stock: None,
            zeny: 0,
            refine: 0,
            costs: vec![BarterCost {
                item_id: 984,
                refine: 0,
                amount: 3,
            }],
        };
        shop_ui
            .shop
            .apply_barter_list(9, false, vec![terms], &DataTable::new());
        shop_ui.shop.add_to_basket(0, 2);

        let mut frame = |mx: f32, my: f32, clicked: bool| -> Vec<GameEvent> {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(1280.0, 800.0);
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            ctx.mouse_clicked = clicked;
            let mut ui = test_frame(&mut ctx, &mut state);
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
        };
        frame(0.0, 0.0, false);
        // both windows five entries tall, side by side: buy is left of cancel in the basket's footer
        let (_, output_h) = shop_ui_heights_for_barter();
        let basket_x = 100.0 + BARTER_WIN_W + WIN_GAP;
        let buy = (
            basket_x + BARTER_WIN_W - 15.0 - FALLBACK_BTN_W - 5.0 - FALLBACK_BTN_W / 2.0,
            100.0 + output_h - FOOTER_H + 4.0 + FALLBACK_BTN_H / 2.0,
        );
        let events = frame(buy.0, buy.1, true);
        assert!(
            matches!(
                events.as_slice(),
                [GameEvent::RequestNpcBarterBuy { extended: false, items }]
                    if items.len() == 1 && items[0].shop_index == 4 && items[0].amount == 2
            ),
            "expected a barter trade, got {events:?}"
        );
        assert!(frame(buy.0, buy.1, true).is_empty(), "nothing left to trade twice");
    }

    #[test]
    fn a_required_item_names_itself_with_its_refine_on_hover() {
        use nostalre_game::event::{BarterCost, BarterItem};
        assert_eq!(cost_label("Blade", 4, 2), "+4 Blade x 2");
        assert_eq!(cost_label("Oridecon", 0, 3), "Oridecon x 3");

        let mut shop_ui = NpcShop::new();
        let mut state = StateCache::new();
        let terms = BarterItem {
            item_id: 6224,
            item_type: 3,
            shop_index: 0,
            stock: None,
            zeny: 0,
            refine: 0,
            costs: vec![BarterCost {
                item_id: 1108,
                refine: 4,
                amount: 1,
            }],
        };
        shop_ui
            .shop
            .apply_barter_list(9, true, vec![terms], &DataTable::new());
        let mut tooltips_at = |mx: f32, my: f32| -> usize {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(1280.0, 800.0);
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            let mut ui = test_frame(&mut ctx, &mut state);
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
            ui.tooltip_draw_calls.len()
        };
        tooltips_at(0.0, 0.0);
        // the first entry's cost icon: second row of the list, from the window's default corner
        let cost = (
            100.0 + BARTER_COST_X + ICON_SIZE / 2.0,
            100.0 + TITLE_H + CONTAINER_PAD_Y + ICON_OFFSET_Y + ITEM_ROW_H + ICON_SIZE / 2.0,
        );
        assert!(tooltips_at(cost.0, cost.1) > 0, "hovering the cost names it");
        assert_eq!(tooltips_at(cost.0 + 200.0, cost.1), 0);
    }

    /// The barter windows' heights, from the layout alone
    fn shop_ui_heights_for_barter() -> (f32, f32) {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.kind = ShopKind::Barter { extended: false };
        shop_ui.window_heights()
    }

    #[test]
    fn action_button_works_with_another_window_raised_behind() {
        let mut shop_ui = NpcShop::new();
        let mut state = StateCache::new();

        let open_with_basket = |shop_ui: &mut NpcShop| {
            shop_ui.shop.open_buy(
                100,
                vec![ShopBuyItem {
                    item: Item {
                        grade: 0,
                        options: Default::default(),
                        index: 0,
                        item_id: 501,
                        item_type: ItemType::Healing,
                        count: 1,
                        is_identified: true,
                        is_damaged: false,
                        refining_level: 0,
                        slot: [0; 4],
                        location: 0,
                        wear_state: 0,
                        name: "Red Potion".into(),
                        resource_name: None,
                    },
                    price: 50,
                    discount_price: 50,
                }],
            );
            shop_ui.shop.add_to_basket(0, 1);
        };

        let frame = |state: &mut StateCache,
                     shop_ui: &mut NpcShop,
                     mx: f32,
                     my: f32,
                     clicked: bool|
         -> Vec<GameEvent> {
            let mut character = Character::new();
            let data = DataTable::new();
            let mut ctx = UiContext::new(800.0, 600.0);
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            ctx.mouse_clicked = clicked;
            let mut ui = test_frame(&mut ctx, state);
            shop_ui.setup_modal(&mut ui);
            let z = ui.get_z_order();
            ui.compute_hovered_window(&z);
            ui.window_at(SKILL_WINDOW_ID, 300.0, 300.0, 15.0, 500.0, 150.0);
            shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data))
        };

        open_with_basket(&mut shop_ui);
        frame(&mut state, &mut shop_ui, 0.0, 0.0, false);

        // the buy button, in the basket's footer (the basket's bottom meets the list's)
        let (input_h, output_h) = shop_ui.window_heights();
        let buy = (587.0, 100.0 + input_h - output_h + 105.0);

        shop_ui.close();
        frame(&mut state, &mut shop_ui, 750.0, 400.0, true);

        open_with_basket(&mut shop_ui);
        frame(&mut state, &mut shop_ui, 0.0, 0.0, false);

        let events = frame(&mut state, &mut shop_ui, buy.0, buy.1, true);
        assert!(
            matches!(events.as_slice(), [GameEvent::RequestNpcShopBuy { .. }]),
            "expected a buy request, got {events:?}"
        );
        assert!(!shop_ui.shop.is_open());

        // a market buys through its own request and stays open for the answer
        open_with_basket(&mut shop_ui);
        shop_ui.shop.kind = ShopKind::Market;
        shop_ui.shop.stock = vec![Some(10)];
        frame(&mut state, &mut shop_ui, 0.0, 0.0, false);
        let events = frame(&mut state, &mut shop_ui, buy.0, buy.1, true);
        assert!(
            matches!(
                events.as_slice(),
                [GameEvent::RequestNpcMarketBuy { items }] if items == &[(1, 501)]
            ),
            "expected a market buy, got {events:?}"
        );
        assert!(shop_ui.shop.is_open());
        assert!(
            shop_ui.shop.basket.is_empty(),
            "emptied as the buy goes out"
        );
        assert!(
            frame(&mut state, &mut shop_ui, buy.0, buy.1, true).is_empty(),
            "a second click before the answer buys nothing"
        );
    }

    #[test]
    fn right_click_shop_item_shows_info() {
        let mut shop_ui = NpcShop::new();
        shop_ui.shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: Item {
                    grade: 0,
                    options: Default::default(),
                    index: 0,
                    item_id: 501,
                    item_type: ItemType::Healing,
                    count: 1,
                    is_identified: true,
                    is_damaged: false,
                    refining_level: 0,
                    slot: [0; 4],
                    location: 0,
                    wear_state: 0,
                    name: "Red Potion".into(),
                    resource_name: None,
                },
                price: 50,
                discount_price: 50,
            }],
        );

        let mut character = Character::new();
        let data = DataTable::new();
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.mouse_x = 150.0;
        ctx.mouse_y = 138.0;
        ctx.mouse_right_clicked = true;
        let mut ui = test_frame(&mut ctx, &mut state);

        let events = shop_ui.build(&mut ui, &mut crate::BuildCtx::test(&mut character, &data));
        assert!(events.iter().any(|e| matches!(
            e,
            GameEvent::ShowItemInfoDirect { item } if item.item_id == 501
        )));
    }
}
