use crate::App;
use nostalre_game::data_table::msg_string_table::MsgLine;
use nostalre_game::event::{GameEvent, PartyMemberData};
use nostalre_game::party::{Party, PartyMember};
impl App {
    pub(super) fn handle_party_member_list(&mut self, name: String, members: Vec<PartyMemberData>) {
        let old = self.game.party.take();
        let mut party = Party::new(name);
        if let Some(old) = &old {
            party.exp_share = old.exp_share;
            party.item_pickup_rule = old.item_pickup_rule;
            party.item_division_rule = old.item_division_rule;
        }
        for m in members {
            // what the list does not carry (HP, position, death) stays as we knew it
            let kept = old
                .as_ref()
                .and_then(|p| p.member(m.aid))
                .cloned()
                .unwrap_or_default();
            party.members.push(PartyMember {
                aid: m.aid,
                name: m.name,
                map: m.map,
                leader: m.leader,
                online: m.online,
                class: m.class,
                base_level: m.base_level,
                ..kept
            });
        }
        self.game.party = Some(party);
    }

    pub(super) fn handle_party_member_added(&mut self, member: PartyMember) {
        let on_our_map = self
            .game
            .session
            .current_map
            .as_deref()
            .map(nostalre_game::map_key)
            == Some(nostalre_game::map_key(&member.map));
        self.game
            .party
            .get_or_insert_with(|| Party::new(String::new()))
            .upsert_member(PartyMember {
                has_live_position: on_our_map,
                ..member
            });
    }

    pub(super) fn handle_party_member_dead(&mut self, aid: u32, dead: bool) {
        if let Some(party) = &mut self.game.party {
            party.set_dead(aid, dead);
        }
    }

    pub(super) fn handle_party_member_job(&mut self, aid: u32, class: u16, base_level: u16) {
        if let Some(party) = &mut self.game.party {
            party.set_job(aid, class, base_level);
        }
        // the only word of another player's new level: kRO drops or lights its level aura on it
        if let Some(entity) = self.game.world.entities.get_mut(aid) {
            entity.base_level = base_level.min(i16::MAX as u16) as i16;
            self.refresh_level_aura(aid);
        }
    }

    pub(super) fn handle_party_member_removed(&mut self, aid: u32, _name: String, _result: u8) {
        if Some(aid) == self.game.session.own_account_id() {
            self.game.party = None;
            self.windows.party_friends_window.open = false;
            return;
        }
        if let Some(party) = &mut self.game.party {
            party.remove_member(aid);
        }
    }

    pub(super) fn handle_party_member_hp(&mut self, aid: u32, hp: u32, max_hp: u32) {
        if let Some(party) = &mut self.game.party {
            party.set_hp(aid, hp, max_hp);
        }
    }

    pub(super) fn handle_party_member_position(&mut self, aid: u32, x: i16, y: i16) {
        if let Some(party) = &mut self.game.party {
            if x < 0 || y < 0 {
                party.clear_position_of(aid);
            } else {
                party.set_position(aid, x as u16, y as u16);
            }
        }
    }

    pub(super) fn handle_party_config_changed(
        &mut self,
        exp_option: u32,
        item_pickup_rule: u8,
        item_division_rule: u8,
    ) {
        if let Some(party) = &mut self.game.party {
            party.exp_share = exp_option != 0;
            party.item_pickup_rule = item_pickup_rule;
            party.item_division_rule = item_division_rule;
        }
    }

    pub(super) fn handle_party_invite_received(&mut self, party_grid: u32, party_name: String) {
        if self.game.prefs.self_config.refuse_party_invite {
            self.channel
                .send_packet(nostalre_network::build_join_party_reply_packet(
                    party_grid,
                    false,
                    self.active_packetver,
                ));
            return;
        }
        let msg = format!("Join party \"{party_name}\"?");
        self.game
            .arm_confirm(&mut self.windows, &msg, move |accept| {
                Some(GameEvent::RespondPartyInvite { party_grid, accept })
            });
    }

    pub(super) fn handle_party_invite_result(&mut self, name: String, answer: u8) {
        let text = match answer {
            0 => format!("{name} is already in a party."),
            1 => format!("{name} rejected the party invitation."),
            2 => format!("{name} joined the party."),
            3 => "The party is full.".to_string(),
            4 => format!("{name} is already a party member."),
            _ => format!("Party invitation to {name} failed."),
        };
        self.windows.chat_window.add_system(text);
    }

    pub(super) fn handle_party_create_result(&mut self, result: u8) {
        if result == 0 {
            self.windows.party_friends_window.open = true;
            // The party now exists server-side; send any invite that was deferred
            // while waiting for this ack.
            if let Some(aid) = self.game.pending_confirms.pending_invite_aid.take() {
                self.channel
                    .send_packet(nostalre_network::build_req_join_party_packet(
                        aid,
                        self.active_packetver,
                    ));
            }
        } else {
            self.game.pending_confirms.pending_invite_aid = None;
            self.windows
                .chat_window
                .add_system("Failed to create party.".to_string());
        }
    }

    /// 0x0b67, as kRO's exe says it (0x7fb980): another member's pickup only, named from the party list, 1276
    /// in cyan
    pub(super) fn handle_party_item_pickup(
        &mut self,
        aid: u32,
        item_id: u32,
        identified: bool,
        refine: u8,
    ) {
        if Some(aid) == self.game.session.own_account_id() {
            return;
        }
        let Some(who) = self
            .game
            .party
            .as_ref()
            .and_then(|p| p.member(aid))
            .map(|m| m.name.clone())
        else {
            return;
        };
        let item = self.item_name_for(item_id, identified);
        let item = if refine > 0 {
            format!("+{refine} {item}")
        } else {
            item
        };
        self.add_msg_line(MsgLine::cyan(1276), &[&who, &item]);
    }

    /// The text comes as the sender typed it, "Name : text" (rAthena checks the name and forwards it as is).
    pub(super) fn handle_party_chat_message(&mut self, message: String) {
        self.windows.chat_window.add_party(message);
    }
}
