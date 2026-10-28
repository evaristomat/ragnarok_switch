use crate::App;
use nostalre_game::event::{BarterItem, MarketItem};
use nostalre_game::npc_shop::ShopKind;
use nostalre_network::{
    build_barter_close_packet, build_market_close_packet, build_npc_trade_quit_packet,
};

impl App {
    pub(super) fn handle_npc_shop_buy_list(
        &mut self,
        npc_id: u32,
        items: Vec<(u32, i32, i32, u8)>,
    ) {
        let fallback_npc_id = self.windows.npc_dialog.dialog.npc_id;
        let icon_paths = self.windows.npc_shop.shop.apply_buy_list(
            npc_id,
            fallback_npc_id,
            items,
            &self.game.data_table,
        );
        self.npc_shop_shown(icon_paths);
    }

    /// A market's or a barter's list names no NPC: the one the dialog was with
    pub(super) fn handle_npc_market_list(&mut self, items: &[MarketItem]) {
        let npc_id = self.windows.npc_dialog.dialog.npc_id;
        let icon_paths =
            self.windows
                .npc_shop
                .shop
                .apply_market_list(npc_id, items, &self.game.data_table);
        self.npc_shop_shown(icon_paths);
    }

    pub(super) fn handle_npc_barter_list(&mut self, extended: bool, items: Vec<BarterItem>) {
        let npc_id = self.windows.npc_dialog.dialog.npc_id;
        let icon_paths = self.windows.npc_shop.shop.apply_barter_list(
            npc_id,
            extended,
            items,
            &self.game.data_table,
        );
        self.npc_shop_shown(icon_paths);
    }

    /// A shop's list took over from the NPC's dialog
    fn npc_shop_shown(&mut self, icon_paths: Vec<String>) {
        self.windows.npc_dialog.dialog.close();
        self.preload_item_icons(icon_paths);
    }

    /// Tells the server the shop window is gone, the way its kind needs, and closes it: a market or a barter by its
    /// close request (until then the server holds the player), a buy or sell shop by 0x09d4
    pub(crate) fn close_npc_shop(&mut self) {
        let shop = &self.windows.npc_shop.shop;
        let pv = self.active_packetver;
        let packet = match (shop.kind, shop.mode) {
            (ShopKind::Market, _) => Some(build_market_close_packet(pv)),
            (ShopKind::Barter { extended }, _) => Some(build_barter_close_packet(extended, pv)),
            (ShopKind::Plain, Some(_)) => Some(build_npc_trade_quit_packet(pv)),
            (ShopKind::Plain, None) => None,
        };
        if let Some(packet) = packet {
            self.channel.send_packet(packet);
        }
        self.windows.npc_shop.close();
    }

    pub(super) fn handle_npc_shop_sell_list(&mut self, npc_id: u32, items: Vec<(i16, i32, i32)>) {
        let fallback_npc_id = self.windows.npc_dialog.dialog.npc_id;
        let icon_paths = self.windows.npc_shop.shop.apply_sell_list(
            npc_id,
            fallback_npc_id,
            items,
            &self.game.character.inventory,
        );
        self.npc_shop_shown(icon_paths);
    }

    /// 0x00ca answers a plain shop (which then closes) and a barter (which stays open)
    pub(super) fn handle_npc_shop_buy_result(&mut self, result: u8) {
        let shop = &mut self.windows.npc_shop.shop;
        let line = if shop.is_barter() {
            shop.apply_barter_result(result)
        } else {
            shop.apply_buy_result(result)
        };
        if let Some(line) = line {
            self.add_msg_line(line, &[]);
        }
    }

    pub(super) fn handle_npc_shop_sell_result(&mut self, result: u8) {
        if let Some(line) = self.windows.npc_shop.shop.apply_sell_result(result) {
            self.add_msg_line(line, &[]);
        }
    }
}
