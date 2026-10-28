//! Items: the bag by kind, a list on the left, the picked item's description beside it.
use super::ROW_BASE_ID;
use super::bar_picker::ToBar;
use super::frame::{self, COLUMN_GAP, Hint, Key, Keys, ROW_H, TABS_H, Textures};
use super::item_detail::{ItemDetail, display_name, item_row};
use crate::BuildCtx;
use crate::game::hotkey_bar::item_fits;
use crate::game::inventory_window::activate;
use crate::helper::format::format_thousands;
use crate::hud::style;
use nostalre_game::character::Character;
use nostalre_game::event::GameEvent;
use nostalre_game::inventory::InventoryData;
use nostalre_game::item::{InventoryTab, Item};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const TABS: [(&str, InventoryTab); 3] = [
    (crate::pt!("Usable", "Consumíveis"), InventoryTab::Usable),
    (crate::pt!("Equipment", "Equipamentos"), InventoryTab::Equip),
    (crate::pt!("Etc", "Diversos"), InventoryTab::Etc),
];
const LIST_SHARE: f32 = 0.52;
const EMPTY: &str = crate::pt!(
    "Nothing of this kind in your bag.",
    "Nenhum item deste tipo no inventário."
);

#[derive(Default)]
pub(super) struct Items {
    pub(super) tab: usize,
    preview: Option<((f32, f32), usize)>,
    compact_detail_key: u16,
    compact_detail_offset: usize,
    picked: usize,
    offset: usize,
    detail: ItemDetail,
    /// How many of a stack to drop, while the player says
    dropping: Option<DropPrompt>,
}

/// The drop question: for the item at `index`, as long as it is still that item
struct DropPrompt {
    index: u16,
    item_id: u32,
    count: i16,
    /// The digits typed so far, which replace the count
    typed: Option<i32>,
}

impl DropPrompt {
    /// The asked item as it now stands; `None` once it is gone or another item took its place
    fn item<'a>(&self, inv: &'a InventoryData) -> Option<&'a Item> {
        inv.get_item(self.index)
            .filter(|i| i.item_id == self.item_id && i.count > 0)
    }

    fn update(&mut self, keys: &Keys, typed: &[char], max: i16) {
        let step = if keys.right {
            1
        } else if keys.left {
            -1
        } else if keys.up {
            10
        } else if keys.down {
            -10
        } else {
            0
        };
        self.count = (self.count + step).clamp(1, max);
        for digit in typed.iter().filter_map(|c| c.to_digit(10)) {
            let number = (self.typed.unwrap_or(0) * 10 + digit as i32).min(i32::from(max));
            self.typed = Some(number);
            self.count = number.clamp(1, i32::from(max)) as i16;
        }
        self.count = self.count.min(max);
    }
}

/// The bag's weight, as "used / most"
pub(super) fn weight(inv: &InventoryData) -> String {
    format!(
        "{} / {}",
        format_thousands(i64::from(inv.weight)),
        format_thousands(i64::from(inv.max_weight))
    )
}

/// The bag's slots, as "used / all"
pub(super) fn slots(inv: &InventoryData) -> String {
    format!("{} / {}", inv.all_items().len(), inv.slots)
}

/// The top line's note: weight, slots and zeny
pub(super) fn summary(c: &Character) -> String {
    let inv = &c.inventory;
    crate::pt_format!(
        "Weight {} · {} slots · {} z",
        "Peso {} · {} espaços · {} z",
        weight(inv),
        slots(inv),
        format_thousands(i64::from(inv.zeny))
    )
}

/// What Enter does with the item, as `activate` decides it
fn action(c: &Character, item: &Item) -> &'static str {
    if c.refine.is_open() {
        crate::pt!("refine", "refinar")
    } else if c.storage.is_open() {
        crate::pt!("store", "guardar")
    } else if item.is_equipment() || item.is_ammunition() {
        crate::pt!("equip", "equipar")
    } else if item.is_card() {
        crate::pt!("insert", "inserir")
    } else {
        crate::pt!("use", "usar")
    }
}

impl Items {
    pub fn character_preview(&self) -> Option<((f32, f32), usize)> {
        self.preview
    }

    fn shown<'a>(&self, inv: &'a InventoryData) -> Vec<&'a Item> {
        let tab = TABS[self.tab].1;
        inv.all_items()
            .iter()
            .filter(|i| i.tab() == tab && !i.is_equipped())
            .collect()
    }

    /// Whether the drop question is up: the screen under it takes no taps
    pub fn asking(&self) -> bool {
        self.dropping.is_some()
    }

    /// Escape out of the drop question
    pub fn back(&mut self) -> bool {
        self.dropping.take().is_some()
    }

    pub fn footer(&self, ctx: &BuildCtx) -> (String, Vec<Hint>) {
        let c = &*ctx.character;
        if let Some((prompt, item)) = self
            .dropping
            .as_ref()
            .and_then(|p| Some((p, p.item(&c.inventory)?)))
        {
            let help = if item.count == 1 {
                crate::pt_format!("Drop {}?", "Jogar fora {}?", item.name)
            } else {
                crate::pt_format!(
                    "Drop how many {}?  {} of {}  (arrows or D-pad, or type it)",
                    "Jogar fora quantos {}?  {} de {}  (direcional ou digite)",
                    item.name,
                    prompt.count,
                    item.count
                )
            };
            return (
                help,
                vec![
                    Hint::tap("Enter", crate::pt!("drop", "jogar fora"), Key::Enter),
                    Hint::CANCEL,
                ],
            );
        }
        let shown = self.shown(&c.inventory);
        let Some(item) = shown.get(self.picked) else {
            return (EMPTY.into(), vec![Hint::TABS, Hint::BACK]);
        };
        let mut hints = vec![
            Hint::MOVE,
            Hint::TABS,
            Hint::tap("Enter", action(c, item), Key::Enter),
        ];
        if item_fits(item) {
            hints.push(Hint::TO_BAR);
        }
        hints.push(Hint::tap(
            "Del",
            crate::pt!("drop", "jogar fora"),
            Key::Remove,
        ));
        hints.push(Hint::BACK);
        (display_name(ctx.data, &c.char_names, item), hints)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        to_bar: &mut Option<ToBar>,
        events: &mut Vec<GameEvent>,
    ) {
        let asked = self.ask(ui, &ctx.character.inventory, keys, events);
        // while it asks, the keys are the question's
        let keys = &if asked { Keys::default() } else { *keys };
        let labels = TABS.map(|(label, _)| label);
        let clicked = frame::tabs(ui, area.x, area.y, &labels, self.tab);
        let tab = frame::switch_tab(keys, self.tab, TABS.len(), clicked);
        if tab != self.tab {
            self.tab = tab;
            self.picked = 0;
            self.offset = 0;
        }

        let character = &mut *ctx.character;
        let shown = self.shown(&character.inventory);
        let before = self.picked;
        self.picked = frame::step(keys, self.picked, shown.len());
        let list = Rect::new(
            area.x,
            area.y + TABS_H,
            (area.w * LIST_SHARE).floor(),
            area.h - TABS_H,
        );
        let visible = (list.h / ROW_H).floor() as usize;
        let follow = before != self.picked;
        frame::scroll(
            ui,
            list,
            &mut self.offset,
            self.picked,
            shown.len(),
            visible,
            follow,
        );

        let mut use_index = keys.enter.then_some(self.picked);
        let rows = shown.iter().enumerate().skip(self.offset).take(visible);
        for (slot, (i, item)) in rows.enumerate() {
            let rect = Rect::new(list.x, list.y + slot as f32 * ROW_H, list.w, ROW_H);
            let id = WidgetId(ROW_BASE_ID + slot as u32);
            let name = display_name(ctx.data, &character.char_names, item);
            let count = (item.count > 1).then(|| format!("× {}", item.count));
            let resp = item_row(
                ui,
                id,
                rect,
                i == self.picked,
                item,
                &name,
                count.as_deref(),
            );
            if resp.clicked() {
                self.picked = i;
            }
            if resp.double_clicked() {
                use_index = Some(i);
            }
        }
        if shown.is_empty() {
            let baseline = style::baseline(ui, list.y);
            style::text(ui, list.x + frame::ROW_TEXT, baseline, EMPTY, style::DIM);
        }

        if let Some(item) = shown.get(self.picked) {
            let x = list.x + list.w + COLUMN_GAP;
            let detail = Rect::new(x, list.y, area.x + area.w - x, list.h);
            let producers = &character.char_names;
            self.detail
                .build(ui, ctx.data, producers, item, detail, textures, events);
        }
        if let Some(item) = use_index.and_then(|i| shown.get(i)) {
            let storage_open = character.storage.is_open();
            events.extend(activate(item, &mut character.refine, storage_open));
        }
        let picked = shown.get(self.picked);
        if keys.to_bar
            && let Some(item) = picked.filter(|i| item_fits(i))
        {
            *to_bar = Some(ToBar::Item {
                index: item.index,
                item_id: item.item_id,
            });
        }
        // always asked: one stray press of a controller's Y must not drop anything
        if keys.remove
            && let Some(item) = picked
        {
            self.dropping = Some(DropPrompt {
                index: item.index,
                item_id: item.item_id,
                count: item.count.max(1),
                typed: None,
            });
        }
    }

    /// The drop question's keys; true while it asks. It goes away by itself once the item is gone
    fn ask(
        &mut self,
        ui: &UiFrame,
        inv: &InventoryData,
        keys: &Keys,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        let Some(prompt) = &mut self.dropping else {
            return false;
        };
        let Some(item) = prompt.item(inv) else {
            self.dropping = None;
            return true;
        };
        prompt.update(keys, &ui.ctx.typed_chars, item.count);
        if keys.enter {
            events.push(GameEvent::RequestDropItem {
                index: prompt.index,
                count: prompt.count,
            });
            self.dropping = None;
        }
        true
    }
}

impl Items {
    #[allow(clippy::too_many_arguments)]
    pub fn build_compact(
        &mut self,
        ui: &mut UiFrame,
        ctx: &mut BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        to_bar: &mut Option<ToBar>,
        events: &mut Vec<GameEvent>,
    ) {
        use super::item_detail::icon;
        use crate::hud::compact as s;
        use nostalre_game::inventory::EquipmentLocation;
        let asked = self.ask(ui, &ctx.character.inventory, keys, events);
        let keys = if asked { Keys::default() } else { *keys };
        let mut clicked = None;
        let mut tx = area.x;
        s::text(ui, tx, s::baseline(ui, area.y), "L", s::DIM);
        tx += 17.0;
        for (i, (label, _)) in TABS.iter().enumerate() {
            let width = s::measure(ui, label);
            let r = Rect::new(tx, area.y, width, 20.0);
            if ui
                .interact(WidgetId(super::TAB_BASE_ID + i as u32), r)
                .clicked()
            {
                clicked = Some(i);
            }
            s::text(
                ui,
                tx,
                s::baseline(ui, area.y),
                label,
                if i == self.tab { s::TEXT } else { s::DIM },
            );
            if i == self.tab {
                s::fill(ui, Rect::new(tx, area.y + 17.0, width, 1.3), s::ACCENT);
            }
            tx += width + 12.0;
        }
        s::text(ui, tx, s::baseline(ui, area.y), "R", s::DIM);
        s::rule(ui, area.x, area.y + 23.0, area.w);
        let tab = frame::switch_tab(&keys, self.tab, TABS.len(), clicked);
        if tab != self.tab {
            self.tab = tab;
            self.picked = 0;
            self.offset = 0;
        }
        let c = &mut *ctx.character;
        let shown = self.shown(&c.inventory);
        let body = Rect::new(area.x, area.y + 34.0, area.w, area.h - 34.0);
        let left = 72.0;
        let detail_w = (body.w * 0.27).clamp(126.0, 190.0);
        let grid = Rect::new(
            body.x + left + 12.0,
            body.y,
            body.w - left - detail_w - 24.0,
            body.h - 20.0,
        );
        let detail = Rect::new(body.x + body.w - detail_w, body.y, detail_w, body.h);
        s::fill(
            ui,
            Rect::new(grid.x - 7.0, body.y, 0.7, body.h),
            [0.55, 0.7, 0.8, 0.25],
        );
        s::fill(
            ui,
            Rect::new(detail.x - 8.0, body.y, 0.7, body.h),
            [0.55, 0.7, 0.8, 0.25],
        );
        self.preview = Some(((body.x + left * 0.5, body.y + 70.0), ui.draw_calls.len()));
        s::text(
            ui,
            body.x,
            s::baseline(ui, body.y + 90.0),
            "Equipado",
            s::DIM,
        );
        for (i, loc) in [
            EquipmentLocation::HandRight,
            EquipmentLocation::Armor,
            EquipmentLocation::HeadTop,
            EquipmentLocation::HandLeft,
        ]
        .into_iter()
        .enumerate()
        {
            let y = body.y + 107.0 + i as f32 * 20.0;
            if y + 18.0 > body.y + body.h {
                break;
            }
            if let Some(item) = c.inventory.equipped_in_slot(loc) {
                let r = Rect::new(body.x, y, 16.0, 16.0);
                if let Some(path) = item.icon_path() {
                    textures.want(&path);
                }
                icon(ui, item, r);
                let name = display_name(ctx.data, &c.char_names, item);
                s::text(
                    ui,
                    body.x + 20.0,
                    s::baseline(ui, y + 2.0),
                    &s::fit(ui, &name, left - 20.0),
                    s::TEXT,
                );
                if r.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
                    ui.tooltip(r.x, r.y + 18.0, &name);
                }
            }
        }
        let cols = 6;
        let cell = ((grid.w - 5.0 * 4.0) / 6.0).min(36.0).max(20.0);
        let pitch = cell + 4.0;
        let rows = ((grid.h + 4.0) / pitch).floor().max(1.0) as usize;
        let visible = rows * cols;
        let before = self.picked;
        self.picked = grid_step(&keys, self.picked, shown.len(), cols);
        let selected_row = self.picked / cols;
        let mut first_row = self.offset / cols;
        if selected_row < first_row {
            first_row = selected_row;
        }
        if selected_row >= first_row + rows {
            first_row = selected_row + 1 - rows;
        }
        let total_rows = shown.len().div_ceil(cols);
        if grid.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) && ui.ctx.scroll_delta != 0.0 {
            first_row = if ui.ctx.scroll_delta > 0.0 {
                first_row.saturating_sub(1)
            } else {
                first_row + 1
            };
            first_row = first_row.min(total_rows.saturating_sub(rows));
        }
        self.offset = first_row * cols;
        let start_x = grid.x + (grid.w - (cell * 6.0 + 20.0)) * 0.5;
        let mut use_index = keys.enter.then_some(self.picked);
        for slot in 0..visible {
            let i = self.offset + slot;
            let r = Rect::new(
                start_x + (slot % cols) as f32 * pitch,
                grid.y + (slot / cols) as f32 * pitch,
                cell,
                cell,
            );
            s::slot(ui, r, i == self.picked && !shown.is_empty());
            let resp = ui.interact(WidgetId(ROW_BASE_ID + slot as u32), r);
            if let Some(item) = shown.get(i) {
                if let Some(path) = item.icon_path() {
                    textures.want(&path);
                }
                let icon_size = 24.0_f32.min(cell - 4.0);
                icon(
                    ui,
                    item,
                    Rect::new(
                        r.x + (cell - icon_size) * 0.5,
                        r.y + 2.0,
                        icon_size,
                        icon_size,
                    ),
                );
                if item.count > 1 {
                    s::right(
                        ui,
                        r.x + r.w - 2.0,
                        r.y + r.h - 2.0,
                        &item.count.to_string(),
                        s::TEXT,
                    );
                }
                if resp.clicked() {
                    self.picked = i;
                }
                if resp.double_clicked() {
                    use_index = Some(i);
                }
                if resp.hovered() {
                    ui.tooltip(
                        r.x,
                        r.y + r.h + 3.0,
                        &display_name(ctx.data, &c.char_names, item),
                    );
                }
            }
        }
        let label = crate::pt_format!("{} slots", "{} espaços", slots(&c.inventory));
        s::text(
            ui,
            grid.x,
            s::baseline(ui, body.y + body.h - 13.0),
            &label,
            s::DIM,
        );
        let ratio = style::ratio(
            c.inventory.weight.max(0) as u32,
            c.inventory.max_weight.max(0) as u32,
        );
        s::right(
            ui,
            grid.x + grid.w,
            s::baseline(ui, body.y + body.h - 13.0),
            &format!("Peso {:.0}%", ratio * 100.0),
            s::DIM,
        );
        if shown.is_empty() {
            s::text(
                ui,
                detail.x,
                s::baseline(ui, detail.y),
                &s::fit(ui, EMPTY, detail.w),
                s::DIM,
            );
        }
        if let Some(item) = shown.get(self.picked) {
            let name = display_name(ctx.data, &c.char_names, item);
            let mut y = detail.y;
            for row in s::wrap(ui, &name, detail.w).iter().take(2) {
                s::text(ui, detail.x, s::baseline(ui, y), row, s::TEXT);
                y += s::line(ui);
            }
            let compare = comparison_item(&c.inventory, item);
            let compared_h = if compare.is_some() { 58.0 } else { 0.0 };
            y += 7.0;
            let description = ctx
                .data
                .item_description
                .as_ref()
                .and_then(|t| t.get(item.item_id, item.is_identified))
                .map(|lines| nostalre_ui::draw::strip_color_codes(&lines.join("\n")))
                .unwrap_or_default();
            let lines = s::wrap(ui, &description, detail.w);
            let count = ((detail.y + detail.h - compared_h - y - 12.0) / s::line(ui))
                .floor()
                .max(1.0) as usize;
            if self.compact_detail_key != item.index || self.picked != before {
                self.compact_detail_key = item.index;
                self.compact_detail_offset = 0;
            }
            // ZL/ZR feed Home/End; wheel scrolls only while over the description.
            let scroll_up = ui
                .ctx
                .pressed_codes
                .contains(&nostalre_ui::context::PhysicalKeyCode::Home);
            let scroll_down = ui
                .ctx
                .pressed_codes
                .contains(&nostalre_ui::context::PhysicalKeyCode::End);
            if scroll_up {
                self.compact_detail_offset = self.compact_detail_offset.saturating_sub(1);
            }
            if scroll_down {
                self.compact_detail_offset += 1;
            }
            if detail.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) && ui.ctx.scroll_delta != 0.0 {
                self.compact_detail_offset = if ui.ctx.scroll_delta > 0.0 {
                    self.compact_detail_offset.saturating_sub(2)
                } else {
                    self.compact_detail_offset + 2
                };
            }
            self.compact_detail_offset = self
                .compact_detail_offset
                .min(lines.len().saturating_sub(count));
            for row in lines.iter().skip(self.compact_detail_offset).take(count) {
                s::text(ui, detail.x, s::baseline(ui, y), row, s::DIM);
                y += s::line(ui);
            }
            if lines.len() > count {
                s::right(
                    ui,
                    detail.x + detail.w,
                    s::baseline(ui, detail.y + detail.h - compared_h - 12.0),
                    "ZL/ZR  Descrição",
                    s::ACCENT,
                );
            }
            if let Some(worn) = compare {
                let y = detail.y + detail.h - 53.0;
                s::rule(ui, detail.x, y, detail.w);
                s::text(
                    ui,
                    detail.x,
                    s::baseline(ui, y + 7.0),
                    "Equipamento atual",
                    s::DIM,
                );
                let name = display_name(ctx.data, &c.char_names, worn);
                s::text(
                    ui,
                    detail.x,
                    s::baseline(ui, y + 23.0),
                    &s::fit(ui, &name, detail.w),
                    s::TEXT,
                );
                let slot_count = |i: &Item| {
                    ctx.data
                        .item_slot_count
                        .as_ref()
                        .map_or(0, |t| t.get_slot_count(i.item_id))
                };
                s::text(
                    ui,
                    detail.x,
                    s::baseline(ui, y + 39.0),
                    &format!("Slots  {} → {}", slot_count(worn), slot_count(item)),
                    s::DIM,
                );
            }
        }
        if let Some(item) = use_index.and_then(|i| shown.get(i)) {
            let storage_open = c.storage.is_open();
            events.extend(activate(item, &mut c.refine, storage_open));
        }
        let picked = shown.get(self.picked);
        if keys.to_bar
            && let Some(item) = picked.filter(|i| item_fits(i))
        {
            *to_bar = Some(ToBar::Item {
                index: item.index,
                item_id: item.item_id,
            });
        }
        if keys.remove
            && let Some(item) = picked
        {
            self.dropping = Some(DropPrompt {
                index: item.index,
                item_id: item.item_id,
                count: item.count.max(1),
                typed: None,
            });
        }
    }
}

/// Grid navigation wraps within a row horizontally and preserves the column vertically.
fn grid_step(keys: &Keys, picked: usize, len: usize, cols: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let cols = cols.max(1);
    let picked = picked.min(len - 1);
    let row = picked / cols;
    let col = picked % cols;
    let start = row * cols;
    let row_len = (len - start).min(cols);
    if keys.left {
        start + (col + row_len - 1) % row_len
    } else if keys.right {
        start + (col + 1) % row_len
    } else if keys.up {
        if row == 0 {
            ((len - 1) / cols * cols + col).min(len - 1)
        } else {
            picked - cols
        }
    } else if keys.down {
        if start + cols >= len {
            col.min(len - 1)
        } else {
            (picked + cols).min(len - 1)
        }
    } else {
        picked
    }
}

/// Compare actual server inventory; never simulate ATK/DEF or apply guessed bonuses.
fn comparison_item<'a>(inv: &'a InventoryData, item: &Item) -> Option<&'a Item> {
    (item.is_equipment() && item.is_identified)
        .then(|| {
            inv.all_items().iter().find(|worn| {
                worn.index != item.index && worn.wear_state & item.equip_location() != 0
            })
        })
        .flatten()
}

#[cfg(test)]
mod grid_tests {
    use super::*;
    use nostalre_game::inventory::EquipmentLocation;

    #[test]
    fn short_rows_empty_grids_and_vertical_wrap_are_safe() {
        let right = Keys {
            right: true,
            ..Keys::default()
        };
        let left = Keys {
            left: true,
            ..Keys::default()
        };
        let down = Keys {
            down: true,
            ..Keys::default()
        };
        let up = Keys {
            up: true,
            ..Keys::default()
        };
        assert_eq!(grid_step(&right, 6, 7, 4), 4);
        assert_eq!(grid_step(&left, 4, 7, 4), 6);
        assert_eq!(grid_step(&down, 3, 7, 4), 6);
        assert_eq!(grid_step(&up, 1, 7, 4), 5);
        assert_eq!(grid_step(&right, 50, 0, 0), 0);
        assert_eq!(grid_step(&down, 0, 1, 4), 0);
    }

    #[test]
    fn comparison_follows_live_wear_state_and_does_not_reveal_unidentified_items() {
        use nostalre_game::data_table::DataTable;
        use nostalre_game::item::ItemType;
        let mask = InventoryData::slot_mask(EquipmentLocation::HandRight);
        let mut inv = InventoryData::new();
        let mut worn = Item::from_tables(1201, ItemType::Weapon, &DataTable::new());
        worn.index = 2;
        worn.location = mask;
        worn.wear_state = mask;
        inv.add_item(worn);
        let mut next = Item::from_tables(1202, ItemType::Weapon, &DataTable::new());
        next.index = 3;
        next.location = mask;
        assert_eq!(comparison_item(&inv, &next).unwrap().index, 2);
        next.is_identified = false;
        assert!(comparison_item(&inv, &next).is_none());
        next.is_identified = true;
        inv.clear_wear_state(2);
        assert!(comparison_item(&inv, &next).is_none());
    }
}
