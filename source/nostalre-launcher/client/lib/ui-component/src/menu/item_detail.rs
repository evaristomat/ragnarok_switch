//! The picked item, as Items and Equipment show it: its art, name and kind, its card slots, its description.
//! Also the item pieces both screens draw in their lists: its icon and full name.
use super::CARD_BASE_ID;
use super::frame::{self, NOT_APPRAISED, ScrolledText, Textures};
use crate::hud::style;
use nostalre_game::char_name::CharNameCache;
use nostalre_game::data_table::DataTable;
use nostalre_game::display_name::format_equipment_display_name;
use nostalre_game::event::GameEvent;
use nostalre_game::item::{Item, ItemType};
use nostalre_ui::draw::strip_color_codes;
use nostalre_ui::frame::{Response, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;
use std::hash::{DefaultHasher, Hash, Hasher};

/// The item's art (`collection/`, 75 by 100 in kRO's data)
const ART_W: f32 = 75.0;
const ART_H: f32 = 100.0;
const ART_PAD: f32 = 6.0;
const CARD: f32 = 30.0;
const CARD_GAP: f32 = 6.0;
const CARD_ICON: f32 = 24.0;
/// An empty card in a slot (rAthena's `card[i]` reads 0; kRO also marks 0xffff)
const SLOT_EMPTY: u32 = 0xffff;
const WHITE: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn type_label(t: ItemType) -> &'static str {
    match t {
        ItemType::Healing => crate::pt!("Healing", "Cura"),
        ItemType::Usable | ItemType::DelayConsume => crate::pt!("Usable", "Consumíveis"),
        ItemType::Cash => crate::pt!("Cash item", "Item de ROPs"),
        ItemType::Armor => crate::pt!("Armour", "Armadura"),
        ItemType::Weapon => crate::pt!("Weapon", "Arma"),
        ItemType::Card => crate::pt!("Card", "Carta"),
        ItemType::PetEgg => crate::pt!("Pet egg", "Ovo de mascote"),
        ItemType::PetArmor => crate::pt!("Pet armour", "Acessório de mascote"),
        ItemType::Ammo => crate::pt!("Ammunition", "Munição"),
        ItemType::ShadowGear => crate::pt!("Shadow gear", "Equipamento sombrio"),
        ItemType::Etc | ItemType::Unknown | ItemType::Unknown2 | ItemType::Max => crate::pt!("Etc", "Diversos"),
    }
}

/// An item's full name: refine, slots, cards, maker
pub(super) fn display_name(data: &DataTable, producers: &CharNameCache, item: &Item) -> String {
    format_equipment_display_name(
        item,
        data.item_slot_count.as_ref(),
        data.card_name.as_ref(),
        producers,
    )
}

/// An item's icon, greyed while it is not appraised
pub(super) fn icon(ui: &mut UiFrame, item: &Item, r: Rect) {
    if let Some(path) = item.icon_path() {
        frame::image(ui, r, path, tint(item));
    }
}

fn tint(item: &Item) -> [f32; 4] {
    if item.is_identified {
        WHITE
    } else {
        NOT_APPRAISED
    }
}

/// What the panel shows of one item, worked out when the picked item (or its refine, cards, appraisal) changes
struct Shown {
    key: u64,
    name_rows: Vec<String>,
    art: Option<String>,
}

#[derive(Default)]
pub(super) struct ItemDetail {
    shown: Option<Shown>,
    about: ScrolledText,
}

/// What a shown item's text depends on
fn key(item: &Item) -> u64 {
    let mut h = DefaultHasher::new();
    (
        item.index,
        item.item_id,
        item.is_identified,
        item.refining_level,
        item.slot,
    )
        .hash(&mut h);
    h.finish()
}

impl ItemDetail {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        data: &DataTable,
        producers: &CharNameCache,
        item: &Item,
        area: Rect,
        textures: &mut Textures,
        events: &mut Vec<GameEvent>,
    ) {
        let line = style::line(ui);
        let box_w = ART_W + 2.0 * ART_PAD;
        let box_h = ART_H + 2.0 * ART_PAD;
        let tx = area.x + box_w + 16.0;
        let tw = (area.x + area.w - tx).max(0.0);
        let key = key(item);
        if self.shown.as_ref().is_none_or(|s| s.key != key) {
            let name = display_name(data, producers, item);
            let art = item
                .resource_name
                .as_ref()
                .map(|n| nostalre_resources::ui::collection::named(n));
            if let Some(path) = &art {
                textures.want(path);
            }
            self.shown = Some(Shown {
                key,
                name_rows: style::wrap(ui, &name, tw).into_iter().take(2).collect(),
                art,
            });
        }
        let Some(shown) = &self.shown else {
            return;
        };

        style::slot(ui, Rect::new(area.x, area.y, box_w, box_h), false);
        if let Some(path) = &shown.art {
            let art = Rect::new(area.x + ART_PAD, area.y + ART_PAD, ART_W, ART_H);
            frame::image(ui, art, path.clone(), tint(item));
        }
        let mut y = area.y;
        for row in &shown.name_rows {
            style::text_bold(ui, tx, style::baseline(ui, y), row, style::TEXT);
            y += line;
        }
        let kind = if !item.is_identified {
            crate::pt!("Not appraised", "Não identificado")
        } else if item.is_damaged {
            crate::pt!("Damaged", "Danificado")
        } else {
            type_label(item.item_type)
        };
        style::text(ui, tx, style::baseline(ui, y + 2.0), kind, style::DIM);
        y += line + 10.0;
        cards(ui, data, item, tx, y, textures, events);

        let y = area.y + box_h + 14.0;
        style::rule(ui, area.x, y, area.w);
        let text = Rect::new(area.x, y + 12.0, area.w, area.y + area.h - y - 12.0);
        self.about.build(ui, key, text, || {
            data.item_description
                .as_ref()
                .and_then(|t| t.get(item.item_id, item.is_identified))
                .map(|lines| strip_color_codes(&lines.join("\n")))
                .unwrap_or_default()
        });
    }
}

/// The card slots of an equipment: what is in each, empty ones framed; a click shows the card
fn cards(
    ui: &mut UiFrame,
    data: &DataTable,
    item: &Item,
    x: f32,
    y: f32,
    textures: &mut Textures,
    events: &mut Vec<GameEvent>,
) {
    if !item.is_equipment() || item.producer_char_id().is_some() {
        return;
    }
    let count = data
        .item_slot_count
        .as_ref()
        .map_or(0, |t| t.get_slot_count(item.item_id));
    for i in 0..usize::from(count).min(item.slot.len()) {
        let rect = Rect::new(x + i as f32 * (CARD + CARD_GAP), y, CARD, CARD);
        let resp = ui.interact(WidgetId(CARD_BASE_ID + i as u32), rect);
        style::slot(ui, rect, resp.hovered());
        let card = item.slot[i];
        if card == 0 || card == SLOT_EMPTY {
            continue;
        }
        if let Some(path) = data.item_icon_path(card) {
            textures.want(&path);
            let pad = ((CARD - CARD_ICON) / 2.0).floor();
            let icon = Rect::new(rect.x + pad, rect.y + pad, CARD_ICON, CARD_ICON);
            frame::image(ui, icon, path, WHITE);
        }
        if resp.hovered()
            && let Some(name) = data.item_name.as_ref().map(|t| t.get_name_or_id(card))
        {
            ui.tooltip(rect.x, rect.y + CARD + 4.0, &name);
        }
        if resp.clicked() {
            events.push(GameEvent::ShowCardInfo { item_id: card });
        }
    }
}

/// A list row with an item: its icon, its name cut to fit, `note` on the right
pub(super) fn item_row(
    ui: &mut UiFrame,
    id: WidgetId,
    rect: Rect,
    picked: bool,
    item: &Item,
    name: &str,
    note: Option<&str>,
) -> Response {
    let resp = frame::row(ui, id, rect, picked);
    let icon_x = rect.x + frame::ROW_TEXT;
    let icon_y = rect.y + ((rect.h - frame::ICON) / 2.0).floor();
    icon(
        ui,
        item,
        Rect::new(icon_x, icon_y, frame::ICON, frame::ICON),
    );
    let baseline = frame::row_baseline(ui, rect);
    let note_w = note.map_or(0.0, |n| style::measure(ui, n) + 16.0);
    let name_x = icon_x + frame::ICON + 10.0;
    let name = style::fit(ui, name, rect.x + rect.w - name_x - note_w - 8.0);
    style::text(ui, name_x, baseline, &name, style::TEXT);
    if let Some(note) = note {
        style::text_right(ui, rect.x + rect.w - 8.0, baseline, note, style::DIM);
    }
    resp
}
