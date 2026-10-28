//! Swap Equipment: the server's answers kept in the set and said in chat, and the requests checked first as kRO's
//! client checks them (CGameMode 0xc86740: 0x12e add, 0x12f remove, 0x130 CHANGE, 0x131 single). Lines and colours
//! from the 2025 exe (`docs/nostalre-kro-exe-equip-window.md`, "Swap Equipment").
use crate::App;
use nostalre_game::data_table::msg_string_table::MsgLine;
use nostalre_game::entity::EntityState;
use nostalre_ui_component::game::chat_window::ChatChannel;

const MSG_NO_ITEM_TO_REPLACE: u16 = 3009;
const MSG_CANNOT_ADD: u16 = 3033;
const MSG_CANNOT_SWAP: u16 = 3040;
const MSG_PLACED: u16 = 3143;
const MSG_REMOVED: u16 = 3144;
const MSG_SECONDS_LEFT: u16 = 3230;
const MSG_WHILE_KO: u16 = 2999;
const MSG_WHILE_CASTING: u16 = 3000;
const MSG_WHILE_TRADING: u16 = 3001;
const MSG_WHILE_VENDING: u16 = 3002;
const MSG_WHILE_NPC: u16 = 3003;
/// kRO's 0x00c8ff00 for the set's changes and the wait, as RGB
const DONE_COLOR: [f32; 4] = [0.0, 1.0, 200.0 / 255.0, 1.0];

impl App {
    /// What stops the player touching the set, in kRO's order (trading, KO'ed, vending, casting, talking to an NPC);
    /// kRO's client refuses nothing for a chat room
    fn equip_switch_refusal(&self) -> Option<u16> {
        let player = self.game.world.entities.player();
        if self.game.character.trade.is_active() {
            Some(MSG_WHILE_TRADING)
        } else if player.is_some_and(|p| p.state() == EntityState::Dead) {
            Some(MSG_WHILE_KO)
        } else if self.windows.my_shop_window.is_open() {
            Some(MSG_WHILE_VENDING)
        } else if player.is_some_and(|p| p.cast_remaining > 0.0) {
            Some(MSG_WHILE_CASTING)
        } else if self.windows.npc_dialog.dialog.is_open() {
            Some(MSG_WHILE_NPC)
        } else {
            None
        }
    }

    fn equip_switch_refused(&mut self) -> bool {
        match self.equip_switch_refusal() {
            Some(msg) => {
                self.add_msg_line(MsgLine::red(msg), &[]);
                true
            }
            None => false,
        }
    }

    /// "<item> have been placed on / removed from the Swap Equipment window." (kRO's "%s%s")
    fn equip_switch_item_line(&mut self, index: u16, msg_id: u16) {
        let name = self
            .game
            .character
            .inventory
            .get_item(index)
            .map(|item| {
                self.game
                    .data_table
                    .item_name_for(item.item_id, item.is_identified)
            })
            .unwrap_or_default();
        if let Some(text) = self.server_msg_line(msg_id, &[]) {
            self.windows.chat_window.add_message(
                format!("{name}{text}"),
                DONE_COLOR,
                ChatChannel::System,
            );
        }
    }

    pub(super) fn handle_equip_switch_added(&mut self, index: u16, location: u32, flag: u16) {
        match flag {
            0 => {
                self.game.character.equip_switch.add(index, location);
                self.equip_switch_item_line(index, MSG_PLACED);
            }
            3 => self.add_msg_line(MsgLine::red(MSG_NO_ITEM_TO_REPLACE), &[]),
            _ => self.add_msg_line(MsgLine::red(MSG_CANNOT_ADD), &[]),
        }
    }

    pub(super) fn handle_equip_switch_removed(&mut self, index: u16, failed: u16) {
        match failed {
            0 => {
                self.equip_switch_item_line(index, MSG_REMOVED);
                self.game.character.equip_switch.remove(index);
            }
            1 => {
                self.add_msg_line(MsgLine::red(MSG_NO_ITEM_TO_REPLACE), &[]);
                self.game.character.equip_switch.remove(index);
            }
            _ => {}
        }
    }

    /// rAthena answers 0 for a swap and for an empty set alike (the server stands a sitting player up itself)
    pub(super) fn handle_equip_switch_answered(&mut self, failed: u16) {
        match failed {
            0 => {}
            1 => self.add_msg_line(MsgLine::red(MSG_NO_ITEM_TO_REPLACE), &[]),
            _ => self.add_msg_line(MsgLine::red(MSG_CANNOT_SWAP), &[]),
        }
    }

    pub(super) fn request_equip_switch_add(&mut self, index: u16, location: u32) {
        if self.equip_switch_refused() || self.game.character.equip_switch.contains(index) {
            return;
        }
        self.channel
            .send_packet(nostalre_network::build_equip_switch_add_packet(
                index,
                location,
                self.active_packetver,
            ));
    }

    pub(super) fn request_equip_switch_remove(&mut self, index: u16) {
        if self.equip_switch_refused() {
            return;
        }
        self.channel
            .send_packet(nostalre_network::build_equip_switch_remove_packet(
                index,
                self.active_packetver,
            ));
    }

    /// The wait after CHANGE, said as kRO's "%d%s" in its colour; true while it lasts
    fn equip_switch_waiting(&mut self, now_ms: u64) -> bool {
        let Some(secs) = self.game.character.equip_switch.change_wait_secs(now_ms) else {
            return false;
        };
        if let Some(text) = self.server_msg_line(MSG_SECONDS_LEFT, &[]) {
            self.windows.chat_window.add_message(
                format!("{secs}{text}"),
                DONE_COLOR,
                ChatChannel::System,
            );
        }
        true
    }

    pub(super) fn request_equip_switch(&mut self) {
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        if self.equip_switch_refused() || self.equip_switch_waiting(now_ms) {
            return;
        }
        self.game.character.equip_switch.changed(now_ms);
        self.channel
            .send_packet(nostalre_network::build_equip_switch_packet(
                self.active_packetver,
            ));
    }

    pub(super) fn request_equip_switch_single(&mut self, index: u16) {
        let now_ms = self.start_time.elapsed().as_millis() as u64;
        if self.equip_switch_waiting(now_ms) {
            return;
        }
        self.channel
            .send_packet(nostalre_network::build_equip_switch_single_packet(
                index,
                self.active_packetver,
            ));
    }
}
