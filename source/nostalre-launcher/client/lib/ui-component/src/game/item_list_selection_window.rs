use super::input_dialog::{InputDialog, InputDialogConfig, InputDialogLayout, InputDialogResult};
use crate::Window;
use crate::helper::dialog_container::DialogContainer;
use crate::helper::scrollbar::{self, SCROLLBAR_W, ScrollbarIds};
use crate::helper::window_chrome::{
    SYS_BASE_OFF_TEX, SYS_BASE_ON_TEX, TITLEBAR_TEX, draw_container, draw_footer, draw_titlebar,
    text_color,
};
use nostalre_game::event::GameEvent;
use nostalre_game::skill::SkillEnum;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{ButtonTextures, UiFrame, WidgetId, WindowOrder};
use nostalre_ui::rect::Rect;

pub const ITEM_LIST_SELECTION_WINDOW_ID: WidgetId = WidgetId(2100);
const OK_ID: WidgetId = WidgetId(2101);
const CANCEL_ID: WidgetId = WidgetId(2102);
const OVERLAY_ID: WidgetId = WidgetId(2103);
const SCROLL_UP_ID: WidgetId = WidgetId(2104);
const SCROLL_DOWN_ID: WidgetId = WidgetId(2105);
const SCROLL_THUMB_ID: WidgetId = WidgetId(2106);
const RESIZE_ID: WidgetId = WidgetId(2107);
const ROW_BASE_ID: u32 = 2110;
const AMOUNT_DIALOG_BASE: u32 = 2190; // uses 2190..2193

const OK_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_OK,
    hover: nostalre_resources::ui::BTN_OK_A,
    pressed: nostalre_resources::ui::BTN_OK_B,
};
const CANCEL_BTN: ButtonTextures = ButtonTextures {
    normal: nostalre_resources::ui::BTN_CANCEL,
    hover: nostalre_resources::ui::BTN_CANCEL_A,
    pressed: nostalre_resources::ui::BTN_CANCEL_B,
};

const WIN_W: f32 = 200.0;
const TITLE_H: f32 = 17.0;
const ROW_H: f32 = 20.0;
const ICON_SIZE: f32 = 18.0;
const PAD: f32 = 6.0;
const FOOTER_H: f32 = 28.0;
const DEFAULT_VISIBLE_ROWS: usize = 4;
const MIN_VISIBLE_ROWS: usize = 2;
const MAX_VISIBLE_ROWS: usize = 16;
const GRIP_W: f32 = 40.0;
const GRIP_H: f32 = 6.0;
const FALLBACK_BTN_W: f32 = 42.0;
const FALLBACK_BTN_H: f32 = 20.0;
const SELECTED_COLOR: [f32; 4] = [0.3, 0.3, 0.5, 0.5];

#[derive(Clone, PartialEq)]
pub enum ListContext {
    MakingArrow,
    ElementalConverter,
    WeaponRefine,
    RepairWeapon {
        target_aid: u32,
    },
    Identify,
    AutoSpell,
    /// A 0x0442 menu (Auto Shadow Spell): answered with 0x0443 and its `why`
    SkillSelect {
        why: i32,
    },
    SelectPetEgg,
    /// The item list window (0x07e3, its kind: 0 Change Material, 1 or 2 Four Spirit Analysis): every row takes an
    /// amount, and OK answers with all of them (0x07e4)
    ItemListWin {
        kind: i32,
    },
}

#[derive(Clone, Default)]
pub struct ListRow {
    pub name: String,
    pub icon: Option<String>,
    pub index: i16,
    pub item_id: u32,
    pub refine: u8,
    pub cards: [u32; 4],
    pub skill: Option<SkillEnum>,
    /// How many the player has (`ItemListWin`: the most an amount can be)
    pub owned: u16,
    /// The amount chosen (`ItemListWin`)
    pub amount: u16,
}

#[derive(Default)]
pub struct ItemListSelectionWindow {
    has_grf_textures: bool,
    open: bool,
    title: String,
    context: Option<ListContext>,
    rows: Vec<ListRow>,
    selected: usize,
    scroll_offset: usize,
    visible_rows: usize,
    resizing: bool,
    resize_start_mouse: f32,
    resize_start_rows: usize,
    btn_size: (f32, f32),
    container: DialogContainer,
    /// The row an amount is being typed for (`ItemListWin`)
    amount_dialog: Option<(usize, InputDialog)>,
}

impl ItemListSelectionWindow {
    pub fn new() -> Self {
        Self {
            visible_rows: DEFAULT_VISIBLE_ROWS,
            btn_size: (FALLBACK_BTN_W, FALLBACK_BTN_H),
            ..Default::default()
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn open(&mut self, title: impl Into<String>, context: ListContext, rows: Vec<ListRow>) {
        self.title = title.into();
        self.context = Some(context);
        self.open = !rows.is_empty();
        self.rows = rows;
        self.selected = 0;
        self.scroll_offset = 0;
    }

    fn close(&mut self) {
        self.open = false;
        self.rows.clear();
        self.context = None;
        self.amount_dialog = None;
    }

    /// Every row given an amount, as (inventory index, amount) (`ItemListWin`)
    fn chosen_amounts(&self) -> Vec<(u16, u16)> {
        self.rows
            .iter()
            .filter(|r| r.amount > 0)
            .map(|r| (r.index as u16, r.amount))
            .collect()
    }

    fn takes_amounts(&self) -> bool {
        matches!(self.context, Some(ListContext::ItemListWin { .. }))
    }

    /// Asks how many of the selected row to put in (`ItemListWin`), up to what the player has
    fn ask_amount(&mut self) {
        let Some(row) = self.rows.get(self.selected) else {
            return;
        };
        let mut dialog = InputDialog::new(
            InputDialogConfig {
                layout: InputDialogLayout::ItemCount {
                    item_name: row.name.clone(),
                },
                escape_cancels: true,
                default_value: row.owned.to_string(),
                max_len: 6,
                numeric_only: true,
                max_value: Some(i32::from(row.owned)),
            },
            WidgetId(AMOUNT_DIALOG_BASE),
        );
        dialog.init_container(&self.container);
        self.amount_dialog = Some((self.selected, dialog));
    }

    /// The amount prompt's frame: a submitted amount goes on its row, at most what the player has
    fn build_amount_dialog(&mut self, ui: &mut UiFrame) {
        let Some((row, dialog)) = self.amount_dialog.as_mut() else {
            return;
        };
        match dialog.build(ui) {
            InputDialogResult::Submitted => {
                let amount = dialog.value_i32().unwrap_or(0).max(0);
                if let Some(row) = self.rows.get_mut(*row) {
                    row.amount = u16::try_from(amount).unwrap_or(u16::MAX).min(row.owned);
                }
                self.amount_dialog = None;
            }
            InputDialogResult::Cancel => self.amount_dialog = None,
            InputDialogResult::None => {}
        }
    }

    /// The server locks the character (menuskill) while the list is open and only
    /// clears it on a reply. Closing must still answer with an invalid sentinel so
    /// the server's cancel path runs without performing the action.
    pub fn cancel(&mut self, events: &mut Vec<GameEvent>) {
        // Escape while an amount is asked for takes back the prompt alone
        if self.amount_dialog.take().is_some() {
            return;
        }
        if let Some(context) = self.context.clone() {
            let event = match context {
                ListContext::MakingArrow | ListContext::ElementalConverter => {
                    GameEvent::RequestMakingArrow { item_id: 0 }
                }
                ListContext::WeaponRefine => GameEvent::RequestWeaponRefine { index: 0 },
                ListContext::RepairWeapon { .. } => GameEvent::RequestRepairItem {
                    index: -1,
                    item_id: 0,
                    refine: 0,
                    cards: [0; 4],
                },
                ListContext::Identify => GameEvent::RequestIdentifyItem { index: -1 },
                ListContext::AutoSpell => GameEvent::RequestSelectAutoSpell { skill: None },
                ListContext::SkillSelect { why } => {
                    GameEvent::RequestSelectSkill { why, skill: None }
                }
                // Hatching is an item-use, not a menuskill; cancelling needs no reply.
                ListContext::SelectPetEgg => {
                    self.close();
                    return;
                }
                ListContext::ItemListWin { kind } => GameEvent::RequestItemListWindow {
                    kind,
                    confirmed: false,
                    items: Vec::new(),
                },
            };
            events.push(event);
        }
        self.close();
    }

    fn confirm(&mut self, events: &mut Vec<GameEvent>) {
        let (row, context) = match (self.rows.get(self.selected), self.context.clone()) {
            (Some(row), Some(context)) => (row, context),
            _ => {
                self.close();
                return;
            }
        };
        let event = match context {
            ListContext::MakingArrow | ListContext::ElementalConverter => {
                GameEvent::RequestMakingArrow {
                    item_id: row.item_id,
                }
            }
            ListContext::WeaponRefine => GameEvent::RequestWeaponRefine {
                index: row.index as i32,
            },
            ListContext::RepairWeapon { .. } => GameEvent::RequestRepairItem {
                index: row.index,
                item_id: row.item_id,
                refine: row.refine,
                cards: row.cards,
            },
            ListContext::Identify => GameEvent::RequestIdentifyItem { index: row.index },
            ListContext::AutoSpell => GameEvent::RequestSelectAutoSpell { skill: row.skill },
            ListContext::SkillSelect { why } => GameEvent::RequestSelectSkill {
                why,
                skill: row.skill,
            },
            ListContext::SelectPetEgg => GameEvent::RequestSelectPetEgg {
                index: row.index as u16,
            },
            ListContext::ItemListWin { kind } => GameEvent::RequestItemListWindow {
                kind,
                confirmed: true,
                items: self.chosen_amounts(),
            },
        };
        events.push(event);
        self.close();
    }

    pub fn build(&mut self, ui: &mut UiFrame) -> Vec<GameEvent> {
        if !self.open {
            return Vec::new();
        }
        let mut events = Vec::new();
        let asking = self.amount_dialog.is_some();
        if let Some((_, dialog)) = &self.amount_dialog {
            ui.set_modal(&[dialog.win_id()]);
        }

        if !asking && ui.ctx.key_up && self.selected > 0 {
            self.selected -= 1;
        }
        if !asking && ui.ctx.key_down && self.selected + 1 < self.rows.len() {
            self.selected += 1;
        }
        if !asking && ui.ctx.key_enter {
            if self.takes_amounts() {
                self.ask_amount();
            } else {
                self.confirm(&mut events);
                return events;
            }
        }
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let grf = self.has_grf_textures;
        let tc = text_color(grf);
        let (btn_w, btn_h) = self.btn_size;

        let visible_rows = self.visible_rows.clamp(MIN_VISIBLE_ROWS, MAX_VISIBLE_ROWS);
        let max_scroll = self.rows.len().saturating_sub(visible_rows);
        let list_h = visible_rows as f32 * ROW_H;
        let win_h = TITLE_H + PAD + list_h + PAD + FOOTER_H;
        let dx = ((ui.ctx.screen_width - WIN_W) / 2.0).max(0.0).floor();
        let dy = ((ui.ctx.screen_height - win_h) / 2.0).max(0.0).floor();

        let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
        ui.interact(OVERLAY_ID, screen);
        ui.ensure_in_z_order_with(ITEM_LIST_SELECTION_WINDOW_ID, WindowOrder::Foreground);
        let win = ui.window_fixed(ITEM_LIST_SELECTION_WINDOW_ID, WIN_W, win_h, dx, dy);
        ui.interact(ITEM_LIST_SELECTION_WINDOW_ID, win);

        draw_titlebar(ui, dx, dy, WIN_W, TITLE_H, grf);
        ui.text(dx + 17.0, dy + TITLE_H - 3.0, &self.title, tc);

        let body_y = dy + TITLE_H;
        let body_h = PAD + list_h + PAD;
        draw_container(ui, dx, body_y, WIN_W, body_h, grf);
        draw_footer(ui, dx, dy + win_h - FOOTER_H, WIN_W, FOOTER_H, grf);

        let has_scroll = max_scroll > 0;
        let list_w = WIN_W - PAD * 2.0 - if has_scroll { SCROLLBAR_W } else { 0.0 };
        let list_x = dx + PAD;
        let list_y = body_y + PAD;

        let content_rect = Rect::new(list_x, list_y, WIN_W - PAD * 2.0, list_h);
        if has_scroll {
            self.scroll_offset = scrollbar::scrollbar(
                ui,
                ScrollbarIds {
                    up: SCROLL_UP_ID,
                    down: SCROLL_DOWN_ID,
                    thumb: SCROLL_THUMB_ID,
                },
                self.scroll_offset,
                visible_rows,
                max_scroll,
                content_rect,
                dx + WIN_W - PAD - SCROLLBAR_W,
                list_y,
                list_h,
            );
        } else {
            self.scroll_offset = 0;
        }

        let mut clicked = None;
        let mut confirm_now = false;
        for vis in 0..visible_rows {
            let idx = self.scroll_offset + vis;
            let Some(row) = self.rows.get(idx) else {
                break;
            };
            let row_y = list_y + vis as f32 * ROW_H;
            let row_rect = Rect::new(list_x, row_y, list_w, ROW_H);
            let resp = ui.interact(WidgetId(ROW_BASE_ID + vis as u32), row_rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                clicked = Some(idx);
            }
            if resp.double_clicked() {
                clicked = Some(idx);
                confirm_now = true;
            }
            if idx == self.selected {
                let (v, i) = draw::quad_vertices(
                    row_rect.x,
                    row_rect.y,
                    row_rect.w,
                    row_rect.h,
                    SELECTED_COLOR,
                );
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: i.into(),
                    texture: TextureRef::White,
                });
            }
            let mut text_x = row_rect.x + 2.0;
            if let Some(icon) = &row.icon {
                let iy = row_y + (ROW_H - ICON_SIZE) / 2.0;
                let (v, i) = draw::quad_vertices(text_x, iy, ICON_SIZE, ICON_SIZE, [1.0; 4]);
                ui.draw_calls.push(DrawCall {
                    vertices: v.into(),
                    indices: i.into(),
                    texture: TextureRef::Named(icon.clone().into()),
                });
                text_x += ICON_SIZE + 4.0;
            }
            // only a row given an amount builds a label: the rest draw their name as it is
            if row.amount > 0 {
                let label = format!("{} x {}", row.name, row.amount);
                ui.text(text_x, row_y + ROW_H - 5.0, &label, tc);
            } else {
                ui.text(text_x, row_y + ROW_H - 5.0, &row.name, tc);
            }
        }
        if let Some(idx) = clicked {
            self.selected = idx;
        }
        if confirm_now && self.takes_amounts() {
            self.ask_amount();
        } else if confirm_now {
            self.confirm(&mut events);
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let win_rect = Rect::new(dx, dy, WIN_W, win_h);
        let btns = win_rect.buttons_bottom_right(2, btn_w, btn_h, 5.0, 5.0, 3.0);
        let cancel = ui.button(CANCEL_ID, btns[0], &CANCEL_BTN, crate::pt!("Cancel", "Cancelar"));
        let ok = ui.button(OK_ID, btns[1], &OK_BTN, "OK");
        if ok.clicked() {
            self.confirm(&mut events);
        } else if cancel.clicked() {
            self.cancel(&mut events);
        }

        let grip_rect = Rect::new(dx + PAD, dy + win_h - GRIP_H, GRIP_W, GRIP_H);
        ui.interact(RESIZE_ID, grip_rect);
        let grip_hovered = grip_rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y);
        if grip_hovered {
            ui.any_interactive_hovered = true;
        }
        if grip_hovered && ui.ctx.mouse_clicked {
            self.resizing = true;
            self.resize_start_mouse = ui.ctx.mouse_y;
            self.resize_start_rows = visible_rows;
        }
        if !ui.ctx.mouse_down {
            self.resizing = false;
        }
        if self.resizing {
            let delta = ((ui.ctx.mouse_y - self.resize_start_mouse) / ROW_H).round() as i32;
            self.visible_rows = (self.resize_start_rows as i32 + delta)
                .clamp(MIN_VISIBLE_ROWS as i32, MAX_VISIBLE_ROWS as i32)
                as usize;
        }
        let grip_color = if grip_hovered || self.resizing {
            [0.6, 0.6, 0.7, 0.9]
        } else {
            [0.5, 0.5, 0.6, 0.8]
        };
        let (gv, gi) = draw::quad_vertices(grip_rect.x, grip_rect.y + 2.0, GRIP_W, 2.0, grip_color);
        ui.draw_calls.push(DrawCall {
            vertices: gv.into(),
            indices: gi.into(),
            texture: TextureRef::White,
        });

        // A prompt opened this frame starts on the next: the key or click that opened it must not answer it too
        if asking {
            self.build_amount_dialog(ui);
        }
        ui.has_grf_textures = prev_grf;
        events
    }
}

impl Window for ItemListSelectionWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
        self.container.has_grf_textures = value;
    }
    fn set_texture_sizes(&mut self, size_fn: &dyn Fn(&str) -> Option<(u32, u32)>) {
        if let Some((w, h)) = size_fn(OK_BTN.normal) {
            self.btn_size = (w as f32, h as f32);
        }
        self.container.set_texture_sizes(size_fn);
    }
    fn grf_texture_paths() -> Vec<&'static str> {
        let mut paths = vec![
            TITLEBAR_TEX,
            SYS_BASE_OFF_TEX,
            SYS_BASE_ON_TEX,
            OK_BTN.normal,
            OK_BTN.hover,
            OK_BTN.pressed,
            CANCEL_BTN.normal,
            CANCEL_BTN.hover,
            CANCEL_BTN.pressed,
        ];
        paths.extend(scrollbar::grf_texture_paths());
        paths.extend(InputDialog::grf_texture_paths());
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    fn row(item_id: u32, index: i16) -> ListRow {
        ListRow {
            name: format!("Item {item_id}"),
            icon: None,
            index,
            item_id,
            refine: 0,
            cards: [0; 4],
            skill: None,
            ..Default::default()
        }
    }

    #[test]
    fn enter_confirms_selected_row_by_context() {
        let mut win = ItemListSelectionWindow::new();
        win.open(
            "Identify",
            ListContext::Identify,
            vec![row(0, 3), row(0, 7)],
        );
        win.selected = 1;

        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);

        assert!(!win.is_open());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::RequestIdentifyItem { index: 7 }))
        );
    }

    #[test]
    fn escape_sends_cancel_reply_to_unlock_menuskill() {
        let mut win = ItemListSelectionWindow::new();
        win.open("Make Arrow", ListContext::MakingArrow, vec![row(1750, 0)]);

        let mut events = Vec::new();
        win.cancel(&mut events);

        assert!(!win.is_open());
        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::RequestMakingArrow { item_id: 0 }))
        );
    }

    #[test]
    fn arrow_context_emits_making_arrow_with_item_id() {
        let mut win = ItemListSelectionWindow::new();
        win.open("Make Arrow", ListContext::MakingArrow, vec![row(1750, 0)]);

        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);

        assert!(
            events
                .iter()
                .any(|e| matches!(e, GameEvent::RequestMakingArrow { item_id: 1750 }))
        );
    }

    fn stone(item_id: u32, index: i16, owned: u16) -> ListRow {
        ListRow {
            owned,
            ..row(item_id, index)
        }
    }

    #[test]
    fn the_item_list_window_asks_amounts_and_answers_with_every_chosen_row() {
        let mut win = ItemListSelectionWindow::new();
        win.open(
            "Four Spirit Analysis",
            ListContext::ItemListWin { kind: 1 },
            vec![stone(994, 4, 30), stone(995, 6, 12), stone(996, 9, 5)],
        );
        let mut state = StateCache::new();
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_enter = true;
        let mut ui = test_frame(&mut ctx, &mut state);
        let events = win.build(&mut ui);
        assert!(
            events.is_empty(),
            "Enter asks for the selected row's amount"
        );
        assert!(win.is_open() && win.amount_dialog.is_some());

        // The amounts as the prompt leaves them: at most what the player has
        win.amount_dialog = None;
        win.rows[0].amount = 30;
        win.rows[2].amount = 5;
        let mut events = Vec::new();
        win.confirm(&mut events);
        assert!(!win.is_open());
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestItemListWindow { kind: 1, confirmed: true, items }]
                if items == &vec![(4, 30), (9, 5)]
        ));
    }

    #[test]
    fn escape_takes_back_the_amount_prompt_then_cancels_the_item_list_window() {
        let mut win = ItemListSelectionWindow::new();
        win.open(
            "Change Material",
            ListContext::ItemListWin { kind: 0 },
            vec![stone(940, 4, 45)],
        );
        win.ask_amount();
        let mut events = Vec::new();
        win.cancel(&mut events);
        assert!(events.is_empty() && win.is_open(), "the prompt alone goes");
        win.cancel(&mut events);
        assert!(!win.is_open());
        assert!(matches!(
            events.as_slice(),
            [GameEvent::RequestItemListWindow { kind: 0, confirmed: false, items }] if items.is_empty()
        ));
    }
}
