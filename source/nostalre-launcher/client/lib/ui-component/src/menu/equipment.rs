//! Equipment: what you wear, slot by slot, and beside it what in your bag fits the picked slot.
use super::ROW_BASE_ID;
use super::frame::{self, COLUMN_GAP, Hint, ICON, Key, Keys, ROW_H, TABS_H, Textures};
use super::item_detail::{ItemDetail, display_name, icon, item_row};
use crate::BuildCtx;
use crate::hud::style;
use nostalre_game::event::GameEvent;
use nostalre_game::inventory::{EquipmentLocation, InventoryData};
use nostalre_game::item::Item;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const GEAR: [(&str, EquipmentLocation); 11] = [
    (crate::pt!("Upper head", "Cabeça: topo"), EquipmentLocation::HeadTop),
    (crate::pt!("Mid head", "Cabeça: meio"), EquipmentLocation::HeadMid),
    (crate::pt!("Lower head", "Cabeça: baixo"), EquipmentLocation::HeadLow),
    (crate::pt!("Armour", "Armadura"), EquipmentLocation::Armor),
    (crate::pt!("Weapon", "Arma"), EquipmentLocation::HandRight),
    (crate::pt!("Shield", "Escudo"), EquipmentLocation::HandLeft),
    (crate::pt!("Garment", "Capa"), EquipmentLocation::Garment),
    (crate::pt!("Shoes", "Calçado"), EquipmentLocation::Shoes),
    (crate::pt!("Accessory L", "Acessório E"), EquipmentLocation::AccessoryLeft),
    (crate::pt!("Accessory R", "Acessório D"), EquipmentLocation::AccessoryRight),
    (crate::pt!("Ammunition", "Munição"), EquipmentLocation::Ammo),
];
const COSTUME: [(&str, EquipmentLocation); 10] = [
    (crate::pt!("Upper head", "Cabeça: topo"), EquipmentLocation::CostumeHeadTop),
    (crate::pt!("Mid head", "Cabeça: meio"), EquipmentLocation::CostumeHeadMid),
    (crate::pt!("Lower head", "Cabeça: baixo"), EquipmentLocation::CostumeHeadLow),
    (crate::pt!("Garment", "Capa"), EquipmentLocation::CostumeGarment),
    (crate::pt!("Shadow armour", "Armadura sombria"), EquipmentLocation::ShadowArmor),
    (crate::pt!("Shadow weapon", "Arma sombria"), EquipmentLocation::ShadowWeapon),
    (crate::pt!("Shadow shield", "Escudo sombrio"), EquipmentLocation::ShadowShield),
    (crate::pt!("Shadow shoes", "Calçado sombrio"), EquipmentLocation::ShadowShoes),
    (crate::pt!("Shadow acc. L", "Acessório sombrio E"), EquipmentLocation::ShadowAccL),
    (crate::pt!("Shadow acc. R", "Acessório sombrio D"), EquipmentLocation::ShadowAccR),
];
const TABS: [&str; 3] = [crate::pt!("Gear", "Equipamento"), crate::pt!("Costume", "Visual"), crate::pt!("More", "Mais")];
const MORE_TAB: usize = 2;
/// What the More tab opens: the equipment window's other pages
const MORE: [&str; 2] = [crate::pt!("Titles and damage skins", "Títulos e aparência do dano"), crate::pt!("Attributes", "Atributos")];
const MORE_HELP: [&str; 2] = [
    crate::pt!("The equipment window: titles, damage skins and what others see.", "Janela de equipamentos: títulos, aparência do dano e visualização por outros."),
    crate::pt!("Equipment Attributes: what your gear adds up to.", "Atributos dos equipamentos: bônus totais."),
];
const MORE_W: f32 = 420.0;
const SLOT_LABEL_W: f32 = 140.0;
const SLOTS_SHARE: f32 = 0.48;
/// The right column: the picked item on top, the bag under it
const DETAIL_SHARE: f32 = 0.5;
/// The bag's rows start after the slots' ids
const BAG_ROW_OFFSET: u32 = 20;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Focus {
    #[default]
    Slots,
    Bag,
}

#[derive(Default)]
pub(super) struct Equipment {
    tab: usize,
    slot: usize,
    focus: Focus,
    bag: usize,
    bag_offset: usize,
    detail: ItemDetail,
}

fn slots(tab: usize) -> &'static [(&'static str, EquipmentLocation)] {
    match tab {
        0 => &GEAR,
        1 => &COSTUME,
        _ => &[],
    }
}

/// What in the bag can go on `location`
fn fits(inv: &InventoryData, location: EquipmentLocation) -> Vec<&Item> {
    let mask = InventoryData::slot_mask(location);
    inv.all_items()
        .iter()
        .filter(|i| !i.is_equipped() && i.is_identified && i.equip_location() & mask != 0)
        .collect()
}

/// The request to wear `item` on that slot: the slot alone picks which of two an accessory or a second
/// weapon takes (rAthena's `pc_equipitem` keeps `req_pos & EQP_ACC`)
fn equip(item: &Item, location: EquipmentLocation) -> GameEvent {
    GameEvent::RequestEquipItem {
        index: item.index,
        location: item.equip_location() & InventoryData::slot_mask(location),
    }
}

impl Equipment {
    /// Escape from the bag goes back to the slots
    pub fn back(&mut self) -> bool {
        let was_bag = self.focus == Focus::Bag;
        self.focus = Focus::Slots;
        was_bag
    }

    pub fn footer(&self, ctx: &BuildCtx) -> (String, Vec<Hint>) {
        if self.tab == MORE_TAB {
            let hints = vec![Hint::MOVE, Hint::TABS, Hint::OPEN, Hint::BACK];
            return (MORE_HELP[self.slot.min(MORE.len() - 1)].to_string(), hints);
        }
        let inv = &ctx.character.inventory;
        let (label, location) = slots(self.tab)[self.slot.min(slots(self.tab).len() - 1)];
        let name = |item: &Item| display_name(ctx.data, &ctx.character.char_names, item);
        match self.focus {
            Focus::Slots => {
                let worn = inv.equipped_in_slot(location);
                let help = worn.map_or_else(|| crate::pt_format!("{label}: nothing", "{label}: nada"), name);
                let mut hints = vec![Hint::MOVE, Hint::TABS];
                if !fits(inv, location).is_empty() {
                    hints.push(Hint::tap("Enter", crate::pt!("choose", "escolher"), Key::Enter));
                }
                if worn.is_some() {
                    hints.push(Hint::tap("Del", crate::pt!("remove", "remover"), Key::Remove));
                }
                hints.push(Hint::BACK);
                (help, hints)
            }
            Focus::Bag => {
                let help = fits(inv, location)
                    .get(self.bag)
                    .map_or_else(String::new, |item| name(item));
                (
                    help,
                    vec![
                        Hint::MOVE,
                        Hint::tap("Enter", crate::pt!("equip", "equipar"), Key::Enter),
                        Hint::BACK,
                    ],
                )
            }
        }
    }

    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        let clicked = frame::tabs(ui, area.x, area.y, &TABS, self.tab);
        let tab = frame::switch_tab(keys, self.tab, TABS.len(), clicked);
        if tab != self.tab {
            *self = Self {
                tab,
                ..Self::default()
            };
        }
        if self.tab == MORE_TAB {
            return self.more(ui, area, keys, events);
        }
        let table = slots(self.tab);
        let inv = &ctx.character.inventory;
        let names = |item: &Item| display_name(ctx.data, &ctx.character.char_names, item);

        if self.focus == Focus::Slots {
            self.slot = frame::step(keys, self.slot, table.len());
        }
        let location = table[self.slot].1;
        let bag = fits(inv, location);
        let before = self.bag;
        if self.focus == Focus::Bag {
            self.bag = frame::step(keys, self.bag, bag.len());
        }
        self.bag = self.bag.min(bag.len().saturating_sub(1));
        // the last fitting item went on, or was used up: back to the slots
        if bag.is_empty() {
            self.focus = Focus::Slots;
        }

        let top = area.y + TABS_H;
        let left_w = ((area.w - COLUMN_GAP) * SLOTS_SHARE).floor();
        let worn_in = |loc| inv.equipped_in_slot(loc);
        let mut remove = None;
        for (i, &(label, loc)) in table.iter().enumerate() {
            let rect = Rect::new(area.x, top + i as f32 * ROW_H, left_w, ROW_H);
            let picked = i == self.slot;
            let resp = frame::row(ui, WidgetId(ROW_BASE_ID + i as u32), rect, picked);
            if resp.clicked() {
                self.slot = i;
                self.focus = Focus::Slots;
            }
            let baseline = frame::row_baseline(ui, rect);
            let x = rect.x + frame::ROW_TEXT;
            style::text(ui, x, baseline, label, style::DIM);
            let name_x = x + SLOT_LABEL_W;
            let max_w = rect.x + rect.w - name_x - 8.0;
            let worn = worn_in(loc);
            let weapon_mask = InventoryData::slot_mask(EquipmentLocation::HandRight);
            // a two-handed weapon holds the shield's slot too
            let weapon_in_shield = loc == EquipmentLocation::HandLeft
                && worn.is_some_and(|w| w.wear_state & weapon_mask != 0);
            match worn {
                Some(_) if weapon_in_shield => {
                    style::text(ui, name_x, baseline, crate::pt!("two-handed", "duas mãos"), style::DIM_MORE)
                }
                Some(item) => {
                    let icon_y = rect.y + ((ROW_H - ICON) / 2.0).floor();
                    icon(ui, item, Rect::new(name_x, icon_y, ICON, ICON));
                    let name = style::fit(ui, &names(item), max_w - ICON - 8.0);
                    style::text(ui, name_x + ICON + 8.0, baseline, &name, style::TEXT);
                }
                None => style::text(ui, name_x, baseline, crate::pt!("none", "nenhum"), style::DIM_MORE),
            }
            if resp.double_clicked() {
                remove = worn;
            }
        }
        if keys.remove && self.focus == Focus::Slots {
            remove = worn_in(location);
        }
        if let Some(item) = remove {
            events.push(GameEvent::RequestUnequipItem { index: item.index });
        }

        let x = area.x + left_w + COLUMN_GAP;
        let w = area.x + area.w - x;
        let detail_h = (area.h - TABS_H) * DETAIL_SHARE;
        let detail = Rect::new(x, top, w, detail_h);
        let shown = match self.focus {
            Focus::Slots => worn_in(location),
            Focus::Bag => bag.get(self.bag).copied(),
        };
        match shown {
            Some(item) => {
                let producers = &ctx.character.char_names;
                self.detail
                    .build(ui, ctx.data, producers, item, detail, textures, events);
            }
            None => {
                let note = crate::pt_format!("Nothing on {}.", "Nada em {}.", table[self.slot].0.to_lowercase());
                style::text(ui, x, style::baseline(ui, top), &note, style::DIM);
            }
        }
        let y = frame::section(ui, x, top + detail_h + 12.0, w, crate::pt!("In your bag", "No inventário"));
        let list = Rect::new(x, y, w, area.y + area.h - y);
        let visible = (list.h / ROW_H).floor() as usize;
        frame::scroll(
            ui,
            list,
            &mut self.bag_offset,
            self.bag,
            bag.len(),
            visible,
            before != self.bag,
        );
        if bag.is_empty() {
            style::text(
                ui,
                x,
                style::baseline(ui, y),
                crate::pt!("Nothing that fits.", "Nenhum item compatível."),
                style::DIM,
            );
        }
        let mut wear = None;
        for (row, (i, item)) in bag
            .iter()
            .enumerate()
            .skip(self.bag_offset)
            .take(visible)
            .enumerate()
        {
            let rect = Rect::new(x, y + row as f32 * ROW_H, w, ROW_H);
            let picked = self.focus == Focus::Bag && i == self.bag;
            let id = WidgetId(ROW_BASE_ID + BAG_ROW_OFFSET + row as u32);
            let resp = item_row(ui, id, rect, picked, item, &names(item), None);
            if resp.clicked() {
                self.focus = Focus::Bag;
                self.bag = i;
            }
            if resp.double_clicked() {
                wear = Some(*item);
            }
        }

        match self.focus {
            Focus::Slots if (keys.enter || keys.right) && !bag.is_empty() => {
                self.focus = Focus::Bag;
            }
            Focus::Bag if keys.left => self.focus = Focus::Slots,
            Focus::Bag if keys.enter => wear = bag.get(self.bag).copied(),
            _ => {}
        }
        if let Some(item) = wear {
            events.push(equip(item, location));
            self.focus = Focus::Slots;
        }
        false
    }

    /// The More tab: a list of the windows with the rest; true once one opens
    fn more(
        &mut self,
        ui: &mut UiFrame,
        area: Rect,
        keys: &Keys,
        events: &mut Vec<GameEvent>,
    ) -> bool {
        let list = Rect::new(area.x, area.y + TABS_H, area.w, area.h - TABS_H);
        let Some(i) = frame::list(ui, list, MORE_W, &MORE, &mut self.slot, keys) else {
            return false;
        };
        let window = if i == 0 {
            GameEvent::ToggleEquipment
        } else {
            GameEvent::ToggleEquipmentProperties
        };
        events.push(GameEvent::OpenWindow(Box::new(window)));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accessory(index: u16) -> Item {
        let mut item = Item::from_tables(
            2601,
            models::enums::item::ItemType::Armor,
            &nostalre_game::data_table::DataTable::new(),
        );
        item.index = index;
        item.location = 136;
        item.is_identified = true;
        item
    }

    #[test]
    fn an_accessory_goes_on_the_slot_picked() {
        let ring = accessory(3);
        assert!(matches!(
            equip(&ring, EquipmentLocation::AccessoryLeft),
            GameEvent::RequestEquipItem {
                index: 3,
                location: 8
            }
        ));
        assert!(matches!(
            equip(&ring, EquipmentLocation::AccessoryRight),
            GameEvent::RequestEquipItem {
                index: 3,
                location: 128
            }
        ));
    }
}
