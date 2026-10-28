use crate::App;
use nostalre_game::effect::EffectQueue;
use nostalre_game::npc_shop::ShopKind;
use nostalre_game::targeting::MapProperties;

/// The session transitions whose reset policy lives in `on_session_change`.
/// Handlers that detect a transition keep their non-reset work (map loading,
/// entity spawning, packet sends) and call in here for the clears.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SessionChange {
    MapChange,
    Logout,
    Death,
    Resurrect,
}

#[cfg(test)]
mod npc_transition_tests {
    use super::*;
    use crate::Config;

    #[test]
    fn map_and_logout_end_the_previous_npc_conversation() {
        for change in [SessionChange::MapChange, SessionChange::Logout] {
            let mut app = App::new(Config::default());
            app.windows.npc_dialog.dialog.open_text(100, "Previous map NPC");
            app.windows.npc_dialog.dialog.wait_for_next(100);
            app.windows.npc_dialog.string_input.text = "old input".into();
            app.windows.npc_dialog.string_input.cursor_pos = 9;
            app.on_session_change(change);
            let dialog = &app.windows.npc_dialog.dialog;
            assert!(!dialog.is_open(), "{change:?} kept the old NPC conversation");
            assert_eq!(dialog.npc_id, 0);
            assert!(!dialog.say_visible && dialog.text.is_empty());
            assert!(app.windows.npc_dialog.string_input.text.is_empty());
            assert_eq!(app.windows.npc_dialog.string_input.cursor_pos, 0);
        }
    }
}

impl App {
    pub(crate) fn on_session_change(&mut self, change: SessionChange) {
        match change {
            SessionChange::MapChange => {
                // A portal ends the old NPC interaction locally. Do not send a
                // Close to its entity after entering another map; later script
                // packets can still start an intentional post-warp dialog.
                self.windows.npc_dialog.reset_conversation();
                self.game.reset_effects(&mut self.effect_queue);
                self.effect_holder.clear();
                self.sound_queue.clear();
                self.sound.stop_all_sfx();
                self.game.world.clear_map_scoped();
                self.game.combat.damage_numbers.clear();
                self.game.character.storage.clear();
                // the server forgets an open refine window on any warp (`pc_setpos`)
                self.game.character.refine.close();
                // and its stylist state (`pc_setpos`)
                self.game.character.stylist.close();
                // a market or a barter holds the player over a warp until its close request (a buy or sell shop
                // is left alone: its empty list after a warp would be a failed purchase)
                if self.windows.npc_shop.shop.kind != ShopKind::Plain {
                    self.close_npc_shop();
                }
                self.windows.storage_password_window.close();
                self.game.character.trade.reset();
                self.windows.trade_window.reset_input();
                self.game.pending_confirms.pending_trade_partner = None;
                self.game.session.map_properties = MapProperties::default();
                self.game.combat.damage_numbers.combat_hidden = false;
                self.game.pending_casts.pending_skill_target = None;
                self.game.pending_casts.pending_skill = None;
                self.game.pending_casts.pending_skill_level = None;
                self.game.pending_casts.pending_skill_unit_cast = None;
                self.game.pending_casts.pending_companion_patrol = None;
                if let Some(h) = self.game.companions.homunculus.as_mut() {
                    h.ai.on_map_change();
                }
                if let Some(m) = self.game.companions.mercenary.as_mut() {
                    m.ai.on_map_change();
                }
                self.game.pending_casts.marriage_targeting = false;
                self.game.combat.selected_target_id = None;
                self.game.combat.attack_target_id = None;
                self.game.combat.attack_request_sent = false;
                self.game.combat.queued_move = None;
                self.game.combat.queued_move_state = None;
                self.game.npc_cutins = [None, None, None];
                self.game.session.progress_bar = None;
            }
            SessionChange::Logout => {
                self.windows.npc_dialog.reset_conversation();
                self.sound_queue.clear();
                self.sound.stop_all_sfx();
                self.window_state_restored = false;
                self.game.session.screen_ripple = false;
                self.char_select_window = None;
                self.game.character.clear();
                self.game.world.entities.clear();
                self.game.sprite_caches.sprites.clear();
                self.game.sprite_caches.carts.clear();
                self.game.sprite_caches.falcons.clear();
                self.game.sprite_caches.gr2_models.clear();
                if let Some(renderer) = &mut self.renderer {
                    renderer.gr2_models.clear();
                }
                if let Some(loader) = &mut self.gr2_loader {
                    loader.invalidate();
                }
                self.game.sprite_caches.sprite_cache.clear();
                self.game.world.clear_map_scoped();
                self.game.boss_mark = None;
                self.game.show_digit = None;
                self.game.assets.floor_item_sprites.clear();
                self.game.chat_rooms.clear();
                self.game.combat.waiting_item_throw_ack = false;
                self.windows.drop_quantity_dialog = None;
                self.windows.guild_expel_dialog = None;
                self.windows.skill_talkbox_dialog = None;
                self.windows.card_insert_dialog = None;
                self.game.pending_casts.pending_card_composition_index = None;
                self.game.pending_casts.pending_pickup_item_id = None;
                self.game.combat.attack_target_id = None;
                self.game.combat.attack_request_sent = false;
                self.game.combat.queued_move = None;
                self.game.combat.queued_move_state = None;
                self.game.companions.homunculus = None;
                self.game.companions.mercenary = None;
                self.game.companions.elemental = None;
                self.game.companions.summons.clear();
                self.game.companions.pet = nostalre_game::pet::PetState::default();
                self.game.companions.capture_targeting = false;
                self.game.companions.pet_roulette = None;
                self.game.pending_casts.marriage_targeting = false;
                self.game.quest_log.clear();
                self.game.quest_markers.clear();
                self.game.minimap_marks.clear();
                // an "Unequip All" left unanswered greys no other character's button
                self.windows.equipment_window.unequip_all_answered();
                self.game.navigation = None;
                self.windows.pet_window.set_visible(false);
                self.game.companions.companion_attack_target = [None; 2];
                self.game.companions.companion_move_marker = [None; 2];
                self.windows.homunculus_window.set_visible(false);
                self.windows.mercenary_window.set_visible(false);
                self.game.guild = None;
                self.game.sprite_caches.guild_head_sprites.clear();
                self.windows.guild_window.open = false;
                // The server only re-sends party info when the new character is
                // actually in a party, so a stale one would survive the switch.
                self.game.party = None;
                self.game.friends = nostalre_game::friends::FriendList::default();
                self.windows.party_friends_window.open = false;
                self.windows.world_map_window.close();
                self.game.session.current_map = None;
                self.game.session.progress_bar = None;
                self.game.session.map_coords = None;
                self.game.session.gat = None;
                self.game.session.map_signs.clear();
                self.game.session.map_title = None;
                self.effect_holder.clear();
                self.effect_queue = EffectQueue::new();
                self.game.schedulers.map_cloud.clear(&mut self.effect_queue);
                self.game.schedulers.ambient_effects =
                    nostalre_game::effects::AmbientEffectScheduler::empty();
                self.game.schedulers.ambient_sounds =
                    nostalre_game::sound::ambient::AmbientSoundScheduler::empty();
                self.game.schedulers.repeat_sounds.clear();
                self.game.effect_keys.status_buff_keys.clear();
                self.game.effect_keys.opt3_keys.clear();
                self.game.effect_keys.next_status_buff_key = 0;
                self.game.effect_keys.level_aura_keys.clear();
                self.game.effect_keys.boss_aura_keys.clear();
                self.game.effect_keys.warp_portal_keys.clear();
                self.game.effect_keys.sight_aura_keys.clear();
                self.game.effect_keys.ruwach_aura_keys.clear();
                self.game.schedulers.day_night.reset();
                if let Some(renderer) = &mut self.renderer {
                    renderer.ground_renderer = None;
                    renderer.model_renderer = None;
                    renderer.water_renderer = None;
                    renderer.grid_selector = None;
                }
                self.play_bgm_track("01.mp3");
            }
            SessionChange::Death => {
                self.game.session.player_dead = true;
                self.game.session.screen_ripple = false;
                self.windows.system_menu.open_dead();
            }
            SessionChange::Resurrect => {
                self.game.session.player_dead = false;
                self.windows.system_menu.close_dead();
            }
        }
    }
}
