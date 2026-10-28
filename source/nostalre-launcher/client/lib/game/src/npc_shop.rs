use crate::char_name::CharNameCache;
use crate::data_table::DataTable;
use crate::data_table::item_resource_table::ItemResourceTable;
use crate::data_table::msg_string_table::MsgLine;
use crate::event::{BarterItem, BarterPurchase};
use crate::item::Item;
use models::enums::EnumWithNumberValue;
use models::enums::item::ItemType;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcShopMode {
    Buy,
    Sell,
}

/// What drives a buy shop: the NPC shop packets, a market's (a stock, 0x0b7a) or a barter's (items for items,
/// 0x0b78; `extended` 0x0b79 with zeny and several items). Market and barter stay open after a purchase until
/// their close request; the server holds the player until then.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShopKind {
    #[default]
    Plain,
    Market,
    Barter {
        extended: bool,
    },
}

#[derive(Debug, Clone)]
pub struct ShopBuyItem {
    pub item: Item,
    pub price: i32,
    pub discount_price: i32,
}

#[derive(Debug, Clone)]
pub struct ShopSellItem {
    pub item: Item,
    pub price: i32,
    pub overcharge_price: i32,
}

#[derive(Debug, Clone)]
pub struct ShopBasketItem {
    pub source_index: usize,
    pub quantity: i16,
}

#[derive(Debug)]
pub struct NpcShopData {
    pub mode: Option<NpcShopMode>,
    pub npc_id: u32,
    pub buy_items: Vec<ShopBuyItem>,
    pub sell_items: Vec<ShopSellItem>,
    pub basket: Vec<ShopBasketItem>,
    pub selected_index: Option<usize>,
    pub kind: ShopKind,
    /// Per buy item, how many are left; `None` (or no entry) is no limit
    pub stock: Vec<Option<u32>>,
    /// Per buy item of a barter, its terms
    pub barter: Vec<BarterItem>,
    /// A barter purchase on its way, `(buy item, amount)`: its answer (0x00ca) names nothing, so the stock comes
    /// down only when it succeeds
    pending: Vec<(usize, i16)>,
}

impl Default for NpcShopData {
    fn default() -> Self {
        Self::new()
    }
}

impl NpcShopData {
    pub fn new() -> Self {
        Self {
            mode: None,
            npc_id: 0,
            buy_items: Vec::new(),
            sell_items: Vec::new(),
            basket: Vec::new(),
            selected_index: None,
            kind: ShopKind::Plain,
            stock: Vec::new(),
            barter: Vec::new(),
            pending: Vec::new(),
        }
    }

    pub fn is_open(&self) -> bool {
        self.mode.is_some()
    }

    pub fn open_buy(&mut self, npc_id: u32, items: Vec<ShopBuyItem>) {
        *self = Self {
            mode: Some(NpcShopMode::Buy),
            npc_id,
            buy_items: items,
            ..Self::new()
        };
    }

    pub fn is_market(&self) -> bool {
        self.kind == ShopKind::Market
    }

    pub fn is_barter(&self) -> bool {
        matches!(self.kind, ShopKind::Barter { .. })
    }

    /// What an item has left beyond the basket; `None` when its stock has no limit
    pub fn stock_left(&self, index: usize) -> Option<u32> {
        let stock = (*self.stock.get(index)?)?;
        Some(stock.saturating_sub(self.in_basket(index).max(0) as u32))
    }

    pub fn open_sell(&mut self, npc_id: u32, items: Vec<ShopSellItem>) {
        *self = Self {
            mode: Some(NpcShopMode::Sell),
            npc_id,
            sell_items: items,
            ..Self::new()
        };
    }

    pub fn add_to_basket(&mut self, source_index: usize, quantity: i16) {
        let quantity = match self.stock_left(source_index) {
            Some(left) => quantity.min(left.min(i16::MAX as u32) as i16),
            None => quantity,
        };
        if quantity <= 0 {
            return;
        }
        let stackable = self
            .item_at(source_index)
            .map(|i| i.item_type.is_stackable())
            .unwrap_or(true);
        if let Some(existing) = self
            .basket
            .iter_mut()
            .find(|c| c.source_index == source_index)
        {
            if !stackable {
                return;
            }
            existing.quantity += quantity;
        } else {
            self.basket.push(ShopBasketItem {
                source_index,
                quantity,
            });
        }
    }

    pub fn remove_from_basket(&mut self, basket_index: usize, quantity: i16) {
        let Some(line) = self.basket.get_mut(basket_index) else {
            return;
        };
        if quantity >= line.quantity {
            self.basket.remove(basket_index);
        } else if quantity > 0 {
            line.quantity -= quantity;
        }
    }

    pub fn basket_quantity(&self, basket_index: usize) -> i16 {
        self.basket
            .get(basket_index)
            .map(|line| line.quantity)
            .unwrap_or(0)
    }

    pub fn basket_total(&self) -> i64 {
        self.basket
            .iter()
            .map(|basket_item| {
                let unit_price = match self.mode {
                    Some(NpcShopMode::Buy) => self
                        .buy_items
                        .get(basket_item.source_index)
                        .map(|i| i.discount_price)
                        .unwrap_or(0),
                    Some(NpcShopMode::Sell) => self
                        .sell_items
                        .get(basket_item.source_index)
                        .map(|i| i.overcharge_price)
                        .unwrap_or(0),
                    None => 0,
                };
                unit_price as i64 * basket_item.quantity as i64
            })
            .sum()
    }

    pub fn item_count(&self) -> usize {
        match self.mode {
            Some(NpcShopMode::Buy) => self.buy_items.len(),
            Some(NpcShopMode::Sell) => self.sell_items.len(),
            None => 0,
        }
    }

    pub fn item_display_name(
        &self,
        index: usize,
        data: &DataTable,
        producers: &CharNameCache,
    ) -> String {
        self.item_at(index)
            .map(|item| {
                crate::display_name::format_equipment_display_name(
                    item,
                    data.item_slot_count.as_ref(),
                    data.card_name.as_ref(),
                    producers,
                )
            })
            .unwrap_or_default()
    }

    pub fn item_at(&self, index: usize) -> Option<&Item> {
        match self.mode {
            Some(NpcShopMode::Buy) => self.buy_items.get(index).map(|i| &i.item),
            Some(NpcShopMode::Sell) => self.sell_items.get(index).map(|i| &i.item),
            None => None,
        }
    }

    pub fn item_icon_path(&self, index: usize) -> Option<String> {
        match self.mode {
            Some(NpcShopMode::Buy) => self.buy_items.get(index).and_then(|i| i.item.icon_path()),
            Some(NpcShopMode::Sell) => self.sell_items.get(index).and_then(|i| i.item.icon_path()),
            None => None,
        }
    }

    pub fn item_is_identified(&self, index: usize) -> bool {
        match self.mode {
            Some(NpcShopMode::Buy) => true,
            Some(NpcShopMode::Sell) => self
                .sell_items
                .get(index)
                .map(|i| i.item.is_identified)
                .unwrap_or(true),
            None => true,
        }
    }

    /// Undiscounted/base price. `item_price` returns the discount/overcharge-adjusted
    /// value; the two differ only when the merchant has the relevant skill.
    pub fn item_base_price(&self, index: usize) -> i32 {
        match self.mode {
            Some(NpcShopMode::Buy) => self.buy_items.get(index).map(|i| i.price).unwrap_or(0),
            Some(NpcShopMode::Sell) => self.sell_items.get(index).map(|i| i.price).unwrap_or(0),
            None => 0,
        }
    }

    pub fn item_price(&self, index: usize) -> i32 {
        match self.mode {
            Some(NpcShopMode::Buy) => self
                .buy_items
                .get(index)
                .map(|i| i.discount_price)
                .unwrap_or(0),
            Some(NpcShopMode::Sell) => self
                .sell_items
                .get(index)
                .map(|i| i.overcharge_price)
                .unwrap_or(0),
            None => 0,
        }
    }

    pub fn sell_item_count(&self, index: usize) -> i16 {
        self.sell_items
            .get(index)
            .map(|i| i.item.count)
            .unwrap_or(1)
    }

    /// How many of the item at `index` the basket holds
    fn in_basket(&self, index: usize) -> i16 {
        self.basket
            .iter()
            .filter(|c| c.source_index == index)
            .map(|c| c.quantity)
            .sum()
    }

    pub fn sell_item_remaining(&self, index: usize) -> i16 {
        self.sell_item_count(index) - self.in_basket(index)
    }

    pub fn needs_quantity_prompt(&self, index: usize) -> bool {
        let Some(item) = self.item_at(index) else {
            return false;
        };
        if !item.item_type.is_stackable() {
            return false;
        }
        match self.mode {
            Some(NpcShopMode::Sell) => self.sell_item_remaining(index) > 1,
            Some(NpcShopMode::Buy) => self.stock_left(index).is_none_or(|left| left > 1),
            None => false,
        }
    }

    pub fn visible_sell_indices(&self) -> Vec<usize> {
        (0..self.sell_items.len())
            .filter(|&i| self.sell_item_remaining(i) > 0)
            .collect()
    }

    pub fn remove_sold_items(&mut self) {
        for basket_item in &self.basket {
            if let Some(sell_item) = self.sell_items.get_mut(basket_item.source_index) {
                sell_item.item.count -= basket_item.quantity;
            }
        }
        self.sell_items.retain(|i| i.item.count > 0);
        self.basket.clear();
        self.selected_index = None;
    }

    pub fn apply_buy_list(
        &mut self,
        npc_id: u32,
        fallback_npc_id: u32,
        items: Vec<(u32, i32, i32, u8)>,
        data_table: &crate::data_table::DataTable,
    ) -> Vec<String> {
        let buy_items: Vec<ShopBuyItem> = items
            .into_iter()
            .map(|(item_id, price, discount_price, item_type)| {
                shop_buy_item(item_id, item_type, price, discount_price, data_table)
            })
            .collect();
        let shop_npc_id = if npc_id != 0 { npc_id } else { fallback_npc_id };
        self.open_buy(shop_npc_id, buy_items);
        self.buy_item_icons()
    }

    /// A market shop's list (0x0b7a names no NPC: the one the dialog was with)
    pub fn apply_market_list(
        &mut self,
        npc_id: u32,
        items: &[crate::event::MarketItem],
        data_table: &DataTable,
    ) -> Vec<String> {
        let buy_items = items
            .iter()
            .map(|m| {
                let price = i32::try_from(m.price).unwrap_or(i32::MAX);
                shop_buy_item(m.item_id, m.item_type, price, price, data_table)
            })
            .collect();
        self.open_buy(npc_id, buy_items);
        self.kind = ShopKind::Market;
        self.stock = items.iter().map(|m| m.stock).collect();
        self.buy_item_icons()
    }

    /// A barter shop's list, with the NPC the dialog was with; the zeny an extended barter also takes is the price
    pub fn apply_barter_list(
        &mut self,
        npc_id: u32,
        extended: bool,
        items: Vec<BarterItem>,
        data_table: &DataTable,
    ) -> Vec<String> {
        let buy_items = items
            .iter()
            .map(|b| {
                let zeny = i32::try_from(b.zeny).unwrap_or(i32::MAX);
                let mut buy = shop_buy_item(b.item_id, b.item_type, zeny, zeny, data_table);
                // rAthena's refines stop at 20 (MAX_REFINE)
                buy.item.refining_level = u8::try_from(b.refine).unwrap_or(u8::MAX);
                buy
            })
            .collect();
        self.open_buy(npc_id, buy_items);
        self.kind = ShopKind::Barter { extended };
        self.stock = items.iter().map(|b| b.stock).collect();
        self.barter = items;
        let mut icons = self.buy_item_icons();
        icons.extend(
            self.barter
                .iter()
                .flat_map(|b| &b.costs)
                .filter_map(|c| data_table.item_icon_path(c.item_id)),
        );
        icons
    }

    /// The basket as a barter purchase, paid from `inventory`; the basket empties (a second click buys nothing)
    /// and the lines wait for the answer
    pub fn take_barter_purchase(
        &mut self,
        inventory: &crate::inventory::InventoryData,
    ) -> Vec<BarterPurchase> {
        let lines = std::mem::take(&mut self.basket);
        let purchase = lines
            .iter()
            .filter_map(|line| {
                let terms = self.barter.get(line.source_index)?;
                let paying = terms
                    .costs
                    .first()
                    .and_then(|c| inventory.find_by_item_id(c.item_id));
                Some(BarterPurchase {
                    item_id: terms.item_id,
                    shop_index: terms.shop_index,
                    amount: line.quantity.max(0) as u32,
                    inv_index: paying.map_or(0, |item| item.index),
                })
            })
            .collect();
        self.pending = lines.iter().map(|l| (l.source_index, l.quantity)).collect();
        purchase
    }

    /// A market purchase's answer: the stock goes down by what was bought (the basket emptied when the buy went
    /// out); the shop stays open
    pub fn apply_market_bought(&mut self, success: bool, bought: &[(u32, u16)]) -> MsgLine {
        for (item_id, qty) in bought {
            if let Some(i) = self
                .buy_items
                .iter()
                .position(|b| b.item.item_id == *item_id)
            {
                self.take_stock(i, u32::from(*qty));
            }
        }
        if success {
            DEAL_DONE
        } else {
            MsgLine::red(3043)
        }
    }

    /// A barter purchase's answer (the shop's 0x00ca): on success the waiting lines come off the stock; the shop
    /// stays open
    pub fn apply_barter_result(&mut self, result: u8) -> Option<MsgLine> {
        for (index, qty) in std::mem::take(&mut self.pending) {
            if result == 0 {
                self.take_stock(index, qty.max(0) as u32);
            }
        }
        buy_result_line(result)
    }

    fn take_stock(&mut self, index: usize, amount: u32) {
        if let Some(Some(left)) = self.stock.get_mut(index) {
            *left = left.saturating_sub(amount);
        }
    }

    fn buy_item_icons(&self) -> Vec<String> {
        self.buy_items
            .iter()
            .filter_map(|i| i.item.icon_path())
            .collect()
    }

    pub fn apply_sell_list(
        &mut self,
        npc_id: u32,
        fallback_npc_id: u32,
        items: Vec<(i16, i32, i32)>,
        inventory: &crate::inventory::InventoryData,
    ) -> Vec<String> {
        let sell_items: Vec<ShopSellItem> = items
            .into_iter()
            .filter_map(|(index, price, overcharge_price)| {
                let inv_item = inventory.get_item(index as u16)?;
                Some(ShopSellItem {
                    item: inv_item.clone(),
                    price,
                    overcharge_price,
                })
            })
            .collect();
        let shop_npc_id = if npc_id != 0 { npc_id } else { fallback_npc_id };
        self.open_sell(shop_npc_id, sell_items);
        self.sell_items
            .iter()
            .filter_map(|i| i.item.icon_path())
            .collect()
    }

    pub fn apply_buy_result(&mut self, result: u8) -> Option<MsgLine> {
        self.close();
        buy_result_line(result)
    }

    /// 0x00cb: kRO's exe says 54 on a sale, 57 on a refusal, nothing else
    pub fn apply_sell_result(&mut self, result: u8) -> Option<MsgLine> {
        self.close();
        match result {
            0 => Some(DEAL_DONE),
            1 => Some(MsgLine::red(57)),
            _ => None,
        }
    }

    pub fn resolve_resource_names(&mut self, table: &ItemResourceTable) {
        for buy_item in &mut self.buy_items {
            buy_item.item.resolve_resource_name(table);
        }
        for item in &mut self.sell_items {
            item.item.resolve_resource_name(table);
        }
    }

    pub fn close(&mut self) {
        *self = Self::new();
    }
}

/// "The deal has successfully completed."
const DEAL_DONE: MsgLine = MsgLine::cyan(54);

/// A shop answer's line as kRO's exe shows it: 0x00ca at 0x7cf030, 0x00cb inline in the packet switch, 0x0b4e at
/// 0x7cce00.
///
/// 0x00ca's result (rAthena's `e_purchase_result`, a barter's too: the exe tells them apart only by the code);
/// 4 to 8 and past 14 show nothing
fn buy_result_line(result: u8) -> Option<MsgLine> {
    Some(match result {
        0 => DEAL_DONE,
        1 => MsgLine::red(55),
        2 => MsgLine::red(56),
        3 => MsgLine::red(221),
        9 => MsgLine::red(1940),
        10 => MsgLine::cyan(2917),
        11 => MsgLine::red(3554),
        12 => MsgLine::cyan(3555),
        13 => MsgLine::red(3557),
        14 => MsgLine::red(3556),
        _ => return None,
    })
}

/// An item an NPC sells, named and drawn from the item tables
fn shop_buy_item(
    item_id: u32,
    item_type: u8,
    price: i32,
    discount_price: i32,
    data_table: &DataTable,
) -> ShopBuyItem {
    ShopBuyItem {
        item: Item::from_tables(
            item_id,
            ItemType::from_value(item_type as usize),
            data_table,
        ),
        price,
        discount_price,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_table::card_name_table::CardNameTable;
    use crate::data_table::item_slot_count_table::ItemSlotCountTable;
    use models::enums::item::ItemType;
    use std::collections::{HashMap, HashSet};

    fn make_item(item_id: u32, name: &str) -> Item {
        Item {
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
        }
    }

    fn sample_buy_items() -> Vec<ShopBuyItem> {
        vec![
            ShopBuyItem {
                item: make_item(501, "Red Potion"),
                price: 50,
                discount_price: 50,
            },
            ShopBuyItem {
                item: make_item(502, "Orange Potion"),
                price: 200,
                discount_price: 200,
            },
            ShopBuyItem {
                item: {
                    let mut i = make_item(1201, "Knife");
                    i.item_type = ItemType::Weapon;
                    i
                },
                price: 50000,
                discount_price: 50000,
            },
        ]
    }

    fn sample_sell_items() -> Vec<ShopSellItem> {
        vec![
            ShopSellItem {
                item: {
                    let mut i = make_item(501, "Red Potion");
                    i.index = 1;
                    i.count = 10;
                    i.resource_name = Some("빨간포션".into());
                    i
                },
                price: 25,
                overcharge_price: 25,
            },
            ShopSellItem {
                item: {
                    let mut i = make_item(1201, "Stiletto");
                    i.item_type = ItemType::Weapon;
                    i.index = 5;
                    i.count = 1;
                    i.resource_name = Some("스틸레토".into());
                    i
                },
                price: 5000,
                overcharge_price: 5500,
            },
        ]
    }

    // kRO's exe: a line for each result it knows, nothing for the rest
    #[test]
    fn shop_answers_show_kros_lines_and_stay_silent_on_the_rest() {
        assert_eq!(buy_result_line(0), Some(DEAL_DONE));
        assert_eq!(buy_result_line(1), Some(MsgLine::red(55)));
        assert_eq!(buy_result_line(12), Some(MsgLine::cyan(3555)));
        for silent in [4, 5, 8, 15, 255] {
            assert_eq!(buy_result_line(silent), None, "result {silent}");
        }
        let mut shop = NpcShopData::new();
        assert_eq!(shop.apply_sell_result(0), Some(DEAL_DONE));
        assert_eq!(shop.apply_sell_result(1), Some(MsgLine::red(57)));
        assert_eq!(shop.apply_sell_result(2), None);
    }

    #[test]
    fn sell_list_names_match_the_inventory_composition() {
        let data = DataTable {
            item_slot_count: Some(ItemSlotCountTable::from_entries(HashMap::from([(1201, 3)]))),
            card_name: Some(CardNameTable::from_data(
                HashMap::from([(4001, "Poring".to_string())]),
                HashSet::new(),
            )),
            ..DataTable::default()
        };
        let producers = CharNameCache::default();

        let mut inventory = crate::inventory::InventoryData::new();
        inventory.add_item({
            let mut i = make_item(1201, "Knife");
            i.item_type = ItemType::Weapon;
            i.index = 5;
            i.refining_level = 7;
            i.slot = [4001, 0, 0, 0];
            i
        });

        let mut shop = NpcShopData::new();
        shop.apply_sell_list(100, 100, vec![(5, 5000, 5500)], &inventory);

        assert_eq!(
            shop.item_display_name(0, &data, &producers),
            "+7 Poring Knife [3]"
        );
    }

    #[test]
    fn shop_lifecycle_buy() {
        let mut shop = NpcShopData::new();
        assert!(!shop.is_open());

        shop.open_buy(100, sample_buy_items());
        assert!(shop.is_open());
        assert_eq!(shop.mode, Some(NpcShopMode::Buy));
        assert_eq!(shop.item_count(), 3);
        assert_eq!(
            shop.item_display_name(0, &DataTable::default(), &CharNameCache::default()),
            "Red Potion"
        );
        assert_eq!(shop.item_price(1), 200);
        assert!(shop.needs_quantity_prompt(0));
        assert!(!shop.needs_quantity_prompt(2));

        shop.add_to_basket(0, 10);
        shop.add_to_basket(1, 5);
        assert_eq!(shop.basket.len(), 2);
        assert_eq!(shop.basket_total(), 10 * 50 + 5 * 200);

        shop.add_to_basket(0, 5);
        assert_eq!(shop.basket.len(), 2);
        assert_eq!(shop.basket[0].quantity, 15);
        assert_eq!(shop.basket_total(), 15 * 50 + 5 * 200);

        shop.add_to_basket(2, 1);
        shop.add_to_basket(2, 1);
        assert_eq!(
            shop.basket[2].quantity, 1,
            "equipment cannot be bought twice"
        );
        shop.remove_from_basket(2, 1);

        shop.remove_from_basket(0, 6);
        assert_eq!(shop.basket_quantity(0), 9);
        assert_eq!(shop.basket_total(), 9 * 50 + 5 * 200);

        shop.remove_from_basket(0, 99);
        assert_eq!(shop.basket.len(), 1);
        assert_eq!(shop.basket_total(), 5 * 200);

        shop.close();
        assert!(!shop.is_open());
        assert!(shop.basket.is_empty());
    }

    #[test]
    fn a_market_caps_the_basket_at_its_stock_and_takes_a_purchase_off_it() {
        use crate::event::MarketItem;
        let mut shop = NpcShopData::new();
        let phracon = MarketItem {
            item_id: 1010,
            item_type: 3,
            price: 200,
            stock: Some(5),
        };
        let emveretarcon = MarketItem {
            item_id: 1011,
            stock: Some(1),
            ..phracon
        };
        shop.apply_market_list(7, &[phracon, emveretarcon], &DataTable::default());
        assert!(shop.is_market());
        assert_eq!((shop.npc_id, shop.item_price(0)), (7, 200));

        shop.add_to_basket(0, 3);
        shop.add_to_basket(0, 9);
        assert_eq!(shop.basket_quantity(0), 5, "no more than the stock");
        assert_eq!(shop.stock_left(0), Some(0));
        assert!(!shop.needs_quantity_prompt(1), "one left needs no quantity");

        shop.basket.clear(); // as the window does when the buy goes out
        shop.add_to_basket(1, 1);
        assert_eq!(shop.apply_market_bought(true, &[(1010, 5)]), DEAL_DONE);
        assert_eq!(shop.stock, vec![Some(0), Some(1)]);
        assert_eq!(
            shop.basket_quantity(0),
            1,
            "what was added while waiting stays"
        );
        assert!(shop.is_open());

        shop.open_buy(8, sample_buy_items());
        assert!(
            !shop.is_market(),
            "a plain shop after a market has no stock"
        );
    }

    /// An expanded barter's refined item (from 20250402) is the shop's row at that refine, so its name says "+7"
    #[test]
    fn a_barter_item_keeps_the_refine_it_comes_with() {
        let blade = BarterItem {
            item_id: 1108,
            item_type: 5,
            shop_index: 1,
            stock: None,
            zeny: 0,
            refine: 7,
            costs: vec![],
        };
        let mut shop = NpcShopData::new();
        shop.apply_barter_list(9, true, vec![blade], &DataTable::default());
        assert_eq!(shop.item_at(0).map(|item| item.refining_level), Some(7));
    }

    #[test]
    fn a_barter_pays_with_items_and_lowers_its_stock_only_when_the_trade_goes_through() {
        use crate::event::BarterCost;
        let mut shop = NpcShopData::new();
        let bradium = BarterItem {
            item_id: 6224,
            item_type: 3,
            shop_index: 4,
            stock: Some(3),
            zeny: 90_000,
            refine: 0,
            costs: vec![BarterCost {
                item_id: 984,
                refine: 0,
                amount: 2,
            }],
        };
        let carnium = BarterItem {
            item_id: 6223,
            shop_index: 5,
            stock: None,
            ..bradium.clone()
        };
        shop.apply_barter_list(9, true, vec![bradium, carnium], &DataTable::default());
        assert!(shop.is_barter());
        assert_eq!(shop.kind, ShopKind::Barter { extended: true });
        assert_eq!((shop.npc_id, shop.item_price(0)), (9, 90_000));
        assert_eq!(shop.stock_left(1), None, "no stock kept");

        let mut inventory = crate::inventory::InventoryData::new();
        inventory.add_item(Item {
            index: 12,
            ..make_item(984, "Oridecon")
        });
        shop.add_to_basket(0, 2);
        let purchase = shop.take_barter_purchase(&inventory);
        assert_eq!(
            purchase,
            [BarterPurchase {
                item_id: 6224,
                shop_index: 4,
                amount: 2,
                inv_index: 12
            }]
        );
        assert!(shop.basket.is_empty(), "a second click buys nothing");

        assert_eq!(shop.apply_barter_result(14), Some(MsgLine::red(3556)));
        assert_eq!(
            shop.stock_left(0),
            Some(3),
            "a refused trade keeps the stock"
        );

        shop.add_to_basket(0, 2);
        shop.take_barter_purchase(&inventory);
        assert_eq!(shop.apply_barter_result(0), Some(DEAL_DONE));
        assert_eq!(shop.stock_left(0), Some(1));
        assert!(shop.is_open());
    }

    #[test]
    fn base_price_differs_from_adjusted_when_skill_applies() {
        let mut shop = NpcShopData::new();
        shop.open_buy(
            100,
            vec![ShopBuyItem {
                item: make_item(501, "Red Potion"),
                price: 50,
                discount_price: 43,
            }],
        );
        assert_eq!(shop.item_base_price(0), 50);
        assert_eq!(shop.item_price(0), 43);

        shop.open_sell(200, sample_sell_items());
        assert_eq!(shop.item_base_price(1), 5000);
        assert_eq!(shop.item_price(1), 5500);
    }

    #[test]
    fn shop_lifecycle_sell() {
        let mut shop = NpcShopData::new();
        shop.open_sell(200, sample_sell_items());
        assert_eq!(shop.mode, Some(NpcShopMode::Sell));
        assert_eq!(shop.item_count(), 2);
        assert!(shop.needs_quantity_prompt(0));
        assert!(!shop.needs_quantity_prompt(1));

        shop.add_to_basket(1, 1);
        assert_eq!(shop.basket_total(), 5500);

        shop.close();
        assert!(!shop.is_open());
    }
}
