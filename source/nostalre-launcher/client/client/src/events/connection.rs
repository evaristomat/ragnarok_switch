use super::lifecycle::SessionChange;
use super::preload_window;
use crate::App;
use nostalre_game::app_state::AppState;
use nostalre_game::entity::Entity;
use nostalre_game::sprite_path::weapon_view_id_to_type;
use nostalre_network::{
    KeepaliveMode, NetworkCommand, build_map_loaded_packet, build_select_accessible_map_packet,
    build_zone_enter_packet, ip_u32_to_string,
};
use nostalre_ui_component::Window as UiWindow;
use nostalre_ui_component::account::char_select_window::CharSelectWindow;
use nostalre_ui_component::game::card_insert_dialog::CardInsertDialog;
use nostalre_ui_component::game::chat_room_board;
use nostalre_ui_component::game::drop_quantity_dialog::DropQuantityDialog;
use nostalre_ui_component::game::vending_board;
use winit::event_loop::ActiveEventLoop;

/// How often and how long a character select is asked again while no map server is up (`retry_select_or_give_up`)
const SELECT_TRIES: u8 = 20;
const SELECT_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

impl App {
    pub(super) fn handle_character_list_received(
        &mut self,
        mut characters: Vec<nostalre_game::event::CharacterInfo>,
    ) {
        tracing::info!("Received {} character(s)", characters.len());
        for ch in &mut characters {
            ch.sex = self.character_sex(ch.sex);
        }
        self.game.sprite_caches.sprites.clear();
        self.account_anims.clear();
        for ch in &characters {
            self.load_char_select_sprite(ch);
        }
        let mut char_win = CharSelectWindow::new(characters);
        char_win.preselect_slot(self.config.last_char_slot);
        if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer) {
            preload_window(&mut char_win, renderer, grf);
        }
        let solo_slot = self
            .config
            .solo
            .as_mut()
            .and_then(|solo| solo.char_id.take())
            .and_then(|char_id| {
                solo_slot(char_win.characters.iter().map(|c| (c.gid, c.slot)), char_id)
            });
        self.char_select_window = Some(char_win);
        self.account_dialog.dismiss();
        self.game.session.app_state = AppState::CharacterSelect;
        // the hub's character is not on this account after all: the picker shows
        self.solo_entering &= solo_slot.is_some();
        if let Some(slot) = solo_slot {
            self.pending_events
                .push(nostalre_game::event::GameEvent::RequestSelectCharacter { slot });
        }
    }

    pub(crate) fn handle_character_created(
        &mut self,
        mut character: nostalre_game::event::CharacterInfo,
    ) {
        tracing::info!(
            "Character '{}' created in slot {}",
            character.name,
            character.slot
        );
        character.sex = self.character_sex(character.sex);
        self.load_char_select_sprite(&character);
        if let Some(win) = &mut self.char_select_window {
            win.characters.push(character);
        }
        self.char_create_window = None;
        self.game.session.app_state = AppState::CharacterSelect;
    }

    fn character_sex(&self, listed: u8) -> u8 {
        let account_sex = self
            .game
            .session
            .login_session
            .as_ref()
            .map_or(0, |s| s.sex);
        character_sex(listed, account_sex)
    }

    fn load_char_select_sprite(&mut self, ch: &nostalre_game::event::CharacterInfo) {
        // no entity to read the garment from: the character list carries it
        let sprite = self.build_player_entity_sprite(
            ch.class,
            ch.class,
            ch.sex,
            ch.head,
            ch.hair_color,
            ch.cloth_color,
            weapon_view_id_to_type(ch.weapon),
            ch.weapon,
            ch.head_top,
            ch.head_mid,
            ch.head_bottom,
            ch.shield,
            ch.robe,
            false,
            false,
        );
        self.keep_player_sprite(ch.gid, ch.class, sprite);
        self.account_anims
            .insert(ch.gid, nostalre_formats::act::SpriteAnimationState::new(0));
    }

    pub(super) fn handle_zone_server_connect_info(
        &mut self,
        char_id: u32,
        map_name: String,
        ip: u32,
        port: i16,
    ) {
        if let Some(session) = &mut self.game.session.login_session {
            session.store_zone_info(char_id, map_name);
        }
        let addr = format!("{}:{}", ip_u32_to_string(ip), port);
        self.channel.send_cmd(NetworkCommand::Disconnect);
        self.channel.send_cmd(NetworkCommand::Connect {
            addr,
            expect_aid: true,
        });
        if let Some(session) = &self.game.session.login_session {
            self.channel.send_packet(build_zone_enter_packet(session));
        }
        self.channel
            .send_cmd(NetworkCommand::SetKeepalive(KeepaliveMode::MapServer));
    }

    /// The re-entry handshake resends the landing cell in `ZC_ACCEPT_ENTER`, so the
    /// coordinates carried here are not needed.
    pub(super) fn handle_zone_server_changed(&mut self, map_name: String, ip: u32, port: i16) {
        tracing::info!(
            "Zone server change to '{map_name}' at {}:{port}",
            ip_u32_to_string(ip)
        );
        self.on_session_change(SessionChange::MapChange);
        self.clear_map_actors();
        self.game.session.current_map = None;
        self.game.character.inventory.clear();
        if let Some(session) = &mut self.game.session.login_session {
            session.map_name = map_name;
        }

        let addr = format!("{}:{}", ip_u32_to_string(ip), port);
        self.channel.send_cmd(NetworkCommand::Disconnect);
        self.channel.send_cmd(NetworkCommand::Connect {
            addr,
            expect_aid: true,
        });
        if let Some(session) = &self.game.session.login_session {
            self.channel.send_packet(build_zone_enter_packet(session));
        }
        self.channel
            .send_cmd(NetworkCommand::SetKeepalive(KeepaliveMode::MapServer));
    }

    pub(super) fn handle_accessible_maps_received(
        &mut self,
        maps: Vec<nostalre_game::event::AccessibleMap>,
    ) {
        let Some(index) = maps.iter().position(|m| m.status == 0) else {
            // the char server lists fallback maps when no map server holds ours: with none up at all (the app
            // starts them with the game) the map server is still registering, so the select is asked again
            self.retry_select_or_give_up();
            return;
        };
        let slot = self.config.last_char_slot.or_else(|| {
            self.game
                .session
                .selected_character
                .as_ref()
                .map(|c| c.slot as u8)
        });
        let Some(slot) = slot else {
            tracing::warn!("Received accessible maps with no selected character slot");
            return;
        };
        tracing::info!(
            "Redirecting character to accessible map '{}'",
            maps[index].name
        );
        self.channel.send_packet(build_select_accessible_map_packet(
            slot,
            index as u8,
            self.active_packetver,
        ));
    }

    /// Asks for the selected character again in a moment, while tries are left; then the picker and why.
    fn retry_select_or_give_up(&mut self) {
        let Some(slot) = self
            .game
            .session
            .selected_character
            .as_ref()
            .and_then(|c| u8::try_from(c.slot).ok())
        else {
            tracing::warn!("No accessible map-server available for character");
            return;
        };
        let left = match self.select_retry {
            Some((_, _, left)) => left,
            None => SELECT_TRIES,
        };
        if left == 0 {
            tracing::warn!("No accessible map-server available for character");
            self.select_retry = None;
            self.solo_entering = false;
            self.account_dialog.show(
                "No map server is running yet. Try again in a moment.",
                false,
                |_| {},
            );
            return;
        }
        tracing::info!("No map server up yet, selecting slot {slot} again ({left} tries left)");
        self.select_retry = Some((
            Some(std::time::Instant::now() + SELECT_RETRY_DELAY),
            slot,
            left - 1,
        ));
    }

    /// The select `retry_select_or_give_up` put off, once it is due.
    pub(crate) fn retry_select_when_due(&mut self, now: std::time::Instant) {
        if let Some((Some(due), slot, left)) = self.select_retry
            && now >= due
        {
            self.select_retry = Some((None, slot, left));
            self.pending_events
                .push(nostalre_game::event::GameEvent::RequestSelectCharacter { slot });
        }
    }

    pub(crate) fn handle_restart_ack(&mut self) {
        // the hub's picture, before the world is cleared
        self.save_player_snapshot();
        self.capture_window_state();
        self.config.save("config.json");
        self.on_session_change(SessionChange::Logout);
        self.reconnect_to_char_server();
    }

    pub(super) fn handle_map_entered(&mut self, x: u16, y: u16, dir: u8) {
        let map_name = self.game.session.login_session.as_ref().map(|s| {
            s.map_name
                .strip_suffix(".gat")
                .unwrap_or(&s.map_name)
                .to_string()
        });
        if let Some(map_name) = &map_name {
            tracing::info!("Entering map: {map_name}");
            self.load_map(map_name);
            self.game.session.current_map = Some(map_name.clone());
        }

        let session_sex = self
            .game
            .session
            .login_session
            .as_ref()
            .map(|s| s.sex)
            .unwrap_or(1);
        let account_id = self
            .game
            .session
            .login_session
            .as_ref()
            .map(|s| s.account_id)
            .unwrap_or(0);
        // the character's looks; its sex already its own or the account's (`character_sex`)
        let selected = self.game.session.selected_character.clone();
        let look = |field: fn(&nostalre_game::event::CharacterInfo) -> u16| {
            selected.as_ref().map_or(0, field)
        };
        let (job, head, hair_color, cloth_color) = (
            look(|c| c.class),
            look(|c| c.head),
            look(|c| c.hair_color),
            look(|c| c.cloth_color),
        );
        let (weapon, shield_id) = (look(|c| c.weapon), look(|c| c.shield));
        let (head_top, head_mid, head_bottom) = (
            look(|c| c.head_top),
            look(|c| c.head_mid),
            look(|c| c.head_bottom),
        );
        let sex = selected.as_ref().map_or(session_sex, |c| c.sex);
        let effect_state = selected.as_ref().map_or(0, |c| c.effect_state);

        let mut entity = Entity::new_player(
            account_id,
            job,
            sex,
            head,
            hair_color,
            weapon,
            head_top,
            head_mid,
            head_bottom,
            shield_id,
            x,
            y,
            dir,
        );
        entity.effect_state = effect_state;
        entity.cloth_color = cloth_color;
        entity.robe = self
            .game
            .session
            .selected_character
            .as_ref()
            .map_or(0, |c| c.robe);
        entity.is_gm = self.config.is_gm_account(account_id) && self.config.see_self_as_gm_when_gm;
        if let Some(guild) = self.game.guild.as_ref() {
            entity.guild_id = guild.gdid;
            entity.guild_emblem_version = guild.emblem_version;
        }
        self.game.world.entities.set_player_id(account_id);
        self.game.world.entities.insert(entity);
        self.game.character.hand_look = (weapon, shield_id);

        for &(bit, efst) in nostalre_game::sprite_path::OPTION_STATUS_ICONS {
            if effect_state & bit != 0 {
                self.track_player_status(efst, true, 0, 0);
            }
        }

        let sprite_job = nostalre_game::sprite_path::visual_job(
            job,
            effect_state,
            nostalre_game::sprite_path::StatusMount::default(),
        );
        let weapon_type = weapon_view_id_to_type(weapon);
        self.load_player_sprite(
            account_id,
            sprite_job,
            sex,
            head,
            hair_color,
            cloth_color,
            weapon_type,
            weapon,
            head_top,
            head_mid,
            head_bottom,
            shield_id,
        );

        if nostalre_game::sprite_path::has_falcon(effect_state) {
            self.spawn_falcon_visual(account_id);
        }
        if let Some((pid, design)) = self.game.player_cart_from_option() {
            self.spawn_cart_visual(pid, design);
        }
        self.refresh_player_status_buffs();

        self.warp_camera_to(x as f32, y as f32);
        self.char_select_window = None;

        if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer) {
            preload_window(&mut self.windows.chat_window, renderer, grf);
            preload_window(&mut self.windows.system_menu, renderer, grf);
            preload_window(&mut self.windows.inventory_window, renderer, grf);
            preload_window(&mut self.windows.cart_window, renderer, grf);
            preload_window(&mut self.windows.storage_window, renderer, grf);
            preload_window(&mut self.windows.storage_password_window, renderer, grf);
            preload_window(&mut self.windows.trade_window, renderer, grf);
            preload_window(&mut self.windows.mailbox_window, renderer, grf);
            preload_window(&mut self.windows.read_mail_window, renderer, grf);
            preload_window(&mut self.windows.cart_select_window, renderer, grf);
            preload_window(&mut self.windows.equipment_window, renderer, grf);
            preload_window(&mut self.windows.npc_dialog, renderer, grf);
            preload_window(&mut self.windows.warp_list_window, renderer, grf);
            preload_window(&mut self.windows.item_list_selection_window, renderer, grf);
            preload_window(&mut self.windows.make_item_window, renderer, grf);
            preload_window(&mut self.windows.vending_shop_window, renderer, grf);
            preload_window(&mut self.windows.vending_setup_window, renderer, grf);
            preload_window(&mut self.windows.my_shop_window, renderer, grf);
            preload_window(&mut self.windows.npc_shop, renderer, grf);
            renderer.preload_textures(&chat_room_board::grf_texture_paths(), grf);
            preload_window(&mut self.windows.chat_room_create_window, renderer, grf);
            preload_window(&mut self.windows.chat_room_member_window, renderer, grf);
            preload_window(&mut self.windows.emotion_window, renderer, grf);
            preload_window(&mut self.windows.shortcut_list_window, renderer, grf);
            preload_window(&mut self.windows.quest_window, renderer, grf);
            preload_window(&mut self.windows.quest_detail_window, renderer, grf);
            preload_window(&mut self.windows.item_info_window, renderer, grf);
            preload_window(&mut self.windows.book_window, renderer, grf);
            preload_window(&mut self.windows.monster_info_window, renderer, grf);
            preload_window(&mut self.windows.equipment_properties_window, renderer, grf);
            preload_window(&mut self.windows.sound_options, renderer, grf);
            preload_window(&mut self.windows.graphic_options, renderer, grf);
            preload_window(&mut self.windows.hotkey_config_window, renderer, grf);
            preload_window(&mut self.windows.item_pickup_notification, renderer, grf);
            preload_window(&mut self.windows.skill_tree_window, renderer, grf);
            preload_window(&mut self.windows.refine_window, renderer, grf);
            preload_window(&mut self.windows.stylist_window, renderer, grf);
            preload_window(&mut self.windows.hotkey_bar, renderer, grf);
            preload_window(&mut self.windows.basic_info_window, renderer, grf);
            preload_window(&mut self.windows.minimap_window, renderer, grf);
            preload_window(&mut self.windows.world_map_window, renderer, grf);
            preload_window(&mut self.windows.status_window, renderer, grf);
            preload_window(&mut self.windows.levelup_notification, renderer, grf);
            preload_window(&mut self.windows.party_friends_window, renderer, grf);
            preload_window(&mut self.windows.party_helper_window, renderer, grf);
            preload_window(&mut self.windows.guild_window, renderer, grf);
            preload_window(&mut self.windows.emblem_picker_window, renderer, grf);
            preload_window(&mut self.windows.homunculus_window, renderer, grf);
            preload_window(&mut self.windows.mercenary_window, renderer, grf);
            preload_window(&mut self.windows.pet_window, renderer, grf);
            preload_window(&mut self.windows.mercenary_skill_window, renderer, grf);
            preload_window(&mut self.windows.homun_skill_window, renderer, grf);
            preload_window(&mut self.windows.companion_ai_config_window, renderer, grf);
            preload_window(&mut self.windows.confirm_dialog, renderer, grf);
            self.windows.drop_dialog_has_grf_textures =
                renderer.preload_textures(&DropQuantityDialog::grf_texture_paths(), grf);
            self.windows.card_insert_dialog_has_grf_textures =
                renderer.preload_textures(&CardInsertDialog::grf_texture_paths(), grf);

            renderer.preload_textures(&vending_board::grf_texture_paths(), grf);

            if let Some(current_map) = &self.game.session.current_map {
                let texture = renderer.load_minimap_texture(current_map, grf, self.game.session.gat.as_ref());
                self.windows.minimap_window.set_map_texture(texture);
            }
        }

        if let Some(info) = &self.game.session.selected_character {
            self.game.character.init_from_info(info);
        }
        self.refresh_level_aura(account_id);

        self.select_retry = None;
        self.solo_entering = false;
        self.game.session.app_state = AppState::InGame;
        if !self.window_state_restored {
            self.game
                .apply_window_state(&mut self.windows, &self.config.window_state);
            self.window_state_restored = true;
        }
        self.game
            .character
            .hotkeys
            .set_visible_rows(self.config.hotkey_visible_rows);
        self.game
            .character
            .hotkeys
            .set_battle_mode(self.config.battle_mode);
        self.windows
            .npc_shop
            .set_drag_all(self.config.no_item_amount_question);

        self.game.character.inventory.clear();
        self.channel
            .send_packet(build_map_loaded_packet(self.active_packetver));
        self.request_guild_data();
    }

    /// Drop every actor but the player. A warp is authoritative about what is
    /// around us afterwards, so it applies to a warp landing on the map we are
    /// already standing on as much as to a real map change.
    fn clear_map_actors(&mut self) {
        let player_sprite = self
            .game
            .world
            .entities
            .player_id()
            .and_then(|pid| self.game.sprite_caches.sprites.remove(&pid));
        self.game.sprite_caches.sprites.clear();
        self.game.sprite_caches.gr2_models.clear();
        if let Some(renderer) = &mut self.renderer {
            renderer.gr2_models.clear();
        }
        if let Some(loader) = &mut self.gr2_loader {
            loader.invalidate();
        }
        self.game.world.entities.clear_non_player();
        self.game.world.clear_map_scoped();
        self.game.companions.pet.clear_entity();
        self.game.quest_markers.clear();
        self.game.boss_mark = None;
        self.game.assets.floor_item_sprites.clear();
        self.game.schedulers.repeat_sounds.clear();
        if let Some(guild) = &mut self.game.guild {
            guild.clear_live_positions();
        }
        if let (Some(pid), Some(sprite)) = (self.game.world.entities.player_id(), player_sprite) {
            self.game.sprite_caches.sprites.insert(pid, sprite);
        }
        self.game.sprite_caches.carts.clear();
        self.game.sprite_caches.falcons.clear();
        if let Some(pid) = self.game.world.entities.player_id() {
            let (cart, falcon) = self
                .game
                .world
                .entities
                .get(pid)
                .map(|e| {
                    (
                        e.cart_type,
                        nostalre_game::sprite_path::has_falcon(e.effect_state),
                    )
                })
                .unwrap_or((None, false));
            if let Some(design) = cart {
                self.spawn_cart_visual(pid, design);
            }
            if falcon {
                self.spawn_falcon_visual(pid);
            }
        }
    }

    pub(super) fn handle_map_changed(&mut self, map_name: String, x: i16, y: i16) {
        self.on_session_change(SessionChange::MapChange);
        let map_name = map_name
            .strip_suffix(".gat")
            .unwrap_or(&map_name)
            .to_string();
        if self.game.session.current_map.as_deref() != Some(&map_name) {
            self.load_map(&map_name);
            self.game.session.current_map = Some(map_name.clone());
            // Renderer-side gr2 models were already dropped by load_map.
            self.clear_map_actors();
            self.game.sprite_caches.sprite_cache.clear();
            self.game.sprite_caches.failed_sprite_loads.clear();

            if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer) {
                let texture = renderer.load_minimap_texture(&map_name, grf, self.game.session.gat.as_ref());
                self.windows.minimap_window.set_map_texture(texture);
            }
            self.windows.minimap_window.on_map_changed();
            self.game.minimap_marks.clear();
            self.windows.world_map_window.on_map_changed();
        } else {
            self.clear_map_actors();
        }
        if self.game.session.player_dead {
            if let Some(entity) = self.game.world.entities.player_mut() {
                entity.revive();
            }
            self.on_session_change(SessionChange::Resurrect);
        }
        if let Some(entity) = self.game.world.entities.player_mut() {
            entity.movement.set_position(x as f32, y as f32);
        }
        self.warp_camera_to(x as f32, y as f32);

        let surviving: Vec<u32> = self.game.world.entities.iter().map(|e| e.id).collect();
        for gid in surviving {
            self.refresh_level_aura(gid);
            self.refresh_boss_aura(gid);
            self.refresh_detect_aura(gid);
            self.refresh_pk_rank_aura(gid);
            self.refresh_warp_portal(gid);
        }
        self.refresh_player_status_buffs();

        self.game.character.inventory.clear();
        self.channel
            .send_packet(build_map_loaded_packet(self.active_packetver));
        self.request_guild_data();
    }

    pub(super) fn handle_player_moved(
        &mut self,
        start_x: u16,
        start_y: u16,
        dest_x: u16,
        dest_y: u16,
        start_time: u32,
    ) {
        self.input.walk_server_acked = true;
        let already_moving_to_dest = self
            .game
            .world
            .entities
            .player()
            .filter(|e| e.movement.is_moving())
            .and_then(|e| e.movement.destination())
            .is_some_and(|(dx, dy)| dx == dest_x && dy == dest_y);
        let local_ms = self.start_time.elapsed().as_millis() as u32;
        self.game
            .session
            .server_time
            .observe_server_tick(start_time, local_ms);
        let latency_coupled = self.config.custom.latency_coupled_walk;
        if (latency_coupled || !already_moving_to_dest)
            && let Some(gat) = &self.game.session.gat
        {
            let path = nostalre_game::path::path_search(gat, start_x, start_y, dest_x, dest_y);
            // Start at local now, not the server tick: fast-forwarding jumps the
            // player forward by one round-trip at each segment seam.
            let now = if latency_coupled {
                self.game
                    .session
                    .server_time
                    .server_to_local_secs(start_time, local_ms)
            } else {
                local_ms as f32 / 1000.0
            };
            if let Some(entity) = self.game.world.entities.player_mut() {
                entity
                    .movement
                    .correct_to_cell(start_x as f32, start_y as f32);
                if !path.is_empty() {
                    entity.begin_move(path, now);
                    if latency_coupled {
                        entity.movement.track_server_tick(start_time);
                    }
                }
            }
        }
    }

    pub(super) fn handle_server_tick(&mut self, server_tick: u32, local_send_time_ms: u32) {
        let local_now_ms = self.start_time.elapsed().as_millis() as u32;
        self.game.session.server_time.on_server_tick(
            server_tick,
            local_now_ms,
            local_send_time_ms,
            self.config.custom.latency_coupled_walk,
        );
    }

    pub(super) fn handle_disconnect_ack(&mut self, allowed: bool, event_loop: &ActiveEventLoop) {
        #[cfg(test)]
        if std::env::var_os("NOSTALRE_NPC_QUIT_VALIDATION").is_some() {
            println!("NPC-EXIT-CHECK disconnect_allowed={allowed}");
        }
        if allowed {
            self.capture_window_state();
            self.config.save("config.json");
            self.channel.send_cmd(NetworkCommand::Disconnect);
            event_loop.exit();
        } else {
            self.windows
                .chat_window
                .add_system("You cannot exit now.".to_string());
        }
    }

    pub(super) fn handle_disconnected(&mut self, reason: String, event_loop: &ActiveEventLoop) {
        if reason == "User exit" {
            event_loop.exit();
        } else if self.game.session.app_state == AppState::InGame {
            let reason_clone = reason.clone();
            self.game.session.disconnect_dialog_shown = true;
            self.windows.confirm_dialog.show(
                &format!("Disconnected from server: {reason_clone}"),
                false,
                |_| {},
            );
        } else {
            self.solo_entering = false;
            self.account_dialog
                .show(&format!("Disconnected: {reason}"), false, |_| {});
        }
    }
}

impl App {
    fn reconnect_to_char_server(&mut self) -> bool {
        if self.channel.cmd_tx.is_none() {
            return false;
        }
        let Some(session) = &self.game.session.login_session else {
            return false;
        };
        let Some(addr) = &session.char_server_addr else {
            return false;
        };
        self.channel.send_cmd(NetworkCommand::Disconnect);
        self.channel.send_cmd(NetworkCommand::Connect {
            addr: addr.clone(),
            expect_aid: true,
        });
        self.channel
            .send_packet(nostalre_network::build_char_enter_packet(session));
        self.channel
            .send_cmd(NetworkCommand::SetKeepalive(KeepaliveMode::CharServer {
                account_id: session.account_id,
            }));
        self.game.session.app_state = AppState::CharacterSelect;
        true
    }
}

/// A character's own sex as listed: 0 female, 1 male, 99 the account's.
fn character_sex(listed: u8, account_sex: u8) -> u8 {
    if listed <= 1 { listed } else { account_sex }
}

/// The slot of Ragnarok Solo's chosen character among the listed (char_id, slot) pairs.
fn solo_slot(characters: impl IntoIterator<Item = (u32, i8)>, char_id: u32) -> Option<u8> {
    characters
        .into_iter()
        .find(|&(gid, _)| gid == char_id)
        .and_then(|(_, slot)| u8::try_from(slot).ok())
}

#[cfg(test)]
mod solo_tests {
    use super::{character_sex, solo_slot};

    #[test]
    fn a_character_keeps_its_own_sex() {
        // a female character on a male account
        assert_eq!(character_sex(0, 1), 0);
        assert_eq!(character_sex(99, 1), 1);
    }

    #[test]
    fn finds_the_chosen_characters_slot() {
        let listed = [(150000, 0), (150001, 2), (150002, 4)];
        assert_eq!(solo_slot(listed, 150001), Some(2));
        assert_eq!(solo_slot(listed, 999), None);
    }
}

#[cfg(test)]
mod offline_account_regression {
    use super::*;
    use crate::Config;
    use std::io::Read;
    use std::time::{Duration, Instant};

    #[test]
    fn refused_local_profile_stays_on_a_visible_account_screen() {
        let mut app = App::new(Config::default());
        assert!(app.server_list_window.is_none());
        app.handle_char_server_connect_refused(1);
        assert_eq!(app.game.session.app_state, AppState::Login);
        assert!(
            app.account_dialog
                .state
                .as_ref()
                .is_some_and(|dialog| !dialog.no_buttons)
        );
    }

    #[test]
    fn refused_online_character_server_preserves_the_existing_chooser() {
        let mut app = App::new(Config::default());
        app.server_list_window = Some(
            nostalre_ui_component::account::server_list_window::ServerListWindow::new(Vec::new()),
        );
        app.handle_char_server_connect_refused(1);
        assert_eq!(app.game.session.app_state, AppState::ServerSelect);
        assert!(app.account_dialog.state.is_some());
    }

    #[test]
    fn first_account_open_sends_handshake_over_the_real_client_worker() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut session = nostalre_network::session::Session::new(20250716);
        session.store_login(2000000, 123, 456, 1);
        session.char_server_addr = Some(listener.local_addr().unwrap().to_string());
        let expected = nostalre_network::build_char_enter_packet(&session);
        let mut app = App::new(Config::default());
        app.start_account_interface(|app| app.connect_local_characters(session));
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut socket = loop {
            match listener.accept() {
                Ok((socket, _)) => break socket,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "first account request never reached the client worker"
                    );
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => panic!("accept: {error}"),
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut received = vec![0; expected.len()];
        socket.read_exact(&mut received).unwrap();
        assert_eq!(received, expected);
    }

    #[test]
    fn character_list_releases_the_real_buttonless_account_dialog() {
        let mut app = App::new(Config::default());
        app.account_dialog
            .show_message("Abrindo personagens locais...");
        assert!(app.account_dialog.state.as_ref().unwrap().no_buttons);
        app.handle_character_list_received(Vec::new());
        assert_eq!(app.game.session.app_state, AppState::CharacterSelect);
        assert!(app.char_select_window.is_some());
        assert!(app.account_dialog.state.is_none());
    }

    #[cfg(target_os = "linux")]
    // Test-only wrapper: the real App owns its window, renderer, UI and network.
    // The external C character server supplies the same cached profile as Horizon.
    #[test]
    #[ignore = "requires Xvfb/Vulkan, game data and the actual offline C servers"]
    fn full_window_offline_character_map_walk_and_actions() {
        use nostalre_game::event::GameEvent;
        use winit::application::ApplicationHandler;
        use winit::event::WindowEvent;
        use winit::event_loop::{ActiveEventLoop, EventLoop};
        use winit::platform::x11::EventLoopBuilderExtX11;
        use winit::window::WindowId;

        struct WindowRun {
            validation: Option<crate::gameplay_validation::Probe>,
            app: App,
            began: Instant,
            initialized: bool,
            map_frames: u32,
            last: Option<(String, f32, f32)>,
            walked: f32,
            incoming: Option<tokio::sync::mpsc::UnboundedReceiver<GameEvent>>,
            forward: Option<tokio::sync::mpsc::UnboundedSender<GameEvent>>,
            equip_ok: u32,
            equip_failed: u32,
            dealt_damage: u32,
            npc_text: u32,
            npc_visible: bool,
            character_lists: u32,
            map_entries: u32,
            was_in_game: bool,
            controller_started: Option<Instant>,
            controller_finished: bool,
            controller_distance: [f32; 2],
        }
        impl ApplicationHandler for WindowRun {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                if self.initialized {
                    return;
                }
                self.initialized = true;
                let solo = self.app.config.solo.take();
                self.app.resumed(event_loop);
                self.app.config.solo = solo;
                assert!(self.app.renderer.is_some() && self.app.grf.is_some());
                self.incoming = self.app.channel.event_rx.take();
                let (forward, events) = tokio::sync::mpsc::unbounded_channel();
                self.forward = Some(forward);
                self.app.channel.event_rx = Some(events);
                let raw =
                    std::fs::read(std::env::var("NOSTALRE_OFFLINE_SESSION").unwrap()).unwrap();
                assert_eq!(raw.len(), 96);
                let number = |at| u32::from_ne_bytes(raw[at..at + 4].try_into().unwrap());
                let mut session = nostalre_network::session::Session::new(20250716);
                session.store_login(number(0), number(4) as i32, number(8), raw[16]);
                session.char_server_addr = Some("127.0.0.1:6121".into());
                session.world_name = "Offline graphical test".into();
                self.app.connect_local_characters(session);
            }
            fn window_event(
                &mut self,
                event_loop: &ActiveEventLoop,
                id: WindowId,
                event: WindowEvent,
            ) {
                if matches!(&event, WindowEvent::RedrawRequested) {
                    if let Some(validation) = &mut self.validation {
                        validation.before_frame(&mut self.app, event_loop);
                    }
                }
                // Exercise the same normalized left-stick axes emitted by Horizon.
                // The script is waiting here; no scripted Walk commands run during this phase.
                if self.controller_started.is_none() && self.npc_visible && self.map_entries == 1 {
                    if let Some(player) = self.app.game.world.entities.player() {
                        let (x, y) = player.movement.cell_position();
                        if x.abs_diff(150) <= 1 && y.abs_diff(180) <= 1 {
                            self.controller_started = Some(Instant::now());
                        }
                    }
                }
                let controller_phase = self.controller_started.and_then(|start| {
                    let seconds = start.elapsed().as_secs_f32();
                    if seconds < 0.6 {
                        Some(0)
                    } else if (1.4..2.0).contains(&seconds) {
                        Some(1)
                    } else {
                        None
                    }
                });
                if self.controller_started.is_some() {
                    let yaw = self.app.renderer.as_ref().unwrap().camera.yaw;
                    let direction = match controller_phase {
                        Some(0) => -1.0,
                        Some(1) => 1.0,
                        _ => 0.0,
                    };
                    self.app.gamepad.axis(0, f64::from(direction * yaw.cos()));
                    self.app.gamepad.axis(1, f64::from(direction * yaw.sin()));
                    if self.controller_started.unwrap().elapsed() >= Duration::from_millis(2400) {
                        self.controller_finished = true;
                    }
                }
                if let Some(incoming) = &mut self.incoming {
                    while let Ok(event) = incoming.try_recv() {
                        if let Some(validation) = &mut self.validation {
                            validation.observe(&event);
                        }
                        match &event {
                            GameEvent::CharacterListReceived { .. } => self.character_lists += 1,
                            GameEvent::InventoryEquipResult { success: true, .. } => {
                                self.equip_ok += 1
                            }
                            GameEvent::InventoryEquipResult { success: false, .. } => {
                                self.equip_failed += 1
                            }
                            GameEvent::EntityAction {
                                gid,
                                damage,
                                left_damage,
                                ..
                            } if *gid == 2000000 && (*damage > 0 || *left_damage > 0) => {
                                self.dealt_damage += 1
                            }
                            GameEvent::NpcDialogText { text, .. } if !text.is_empty() => {
                                self.npc_text += 1
                            }
                            _ => {}
                        }
                        self.forward.as_ref().unwrap().send(event).unwrap();
                    }
                }
                self.app.window_event(event_loop, id, event);
                if let Some(validation) = &mut self.validation {
                    validation.after_frame(&mut self.app);
                }
                assert!(
                    self.app.game.session.app_state != AppState::ServerSelect
                        || self.app.server_list_window.is_some(),
                    "offline navigation opened an empty server chooser"
                );
                self.npc_visible |= self.app.windows.npc_dialog.dialog.say_visible;
                let in_game = self.app.game.session.app_state == AppState::InGame;
                if in_game && !self.was_in_game {
                    self.map_entries += 1;
                }
                self.was_in_game = in_game;
                if in_game {
                if (std::env::var_os("NOSTALRE_MAP_MEMORY_TEST").is_some() || std::env::var_os("NOSTALRE_COMODO_MEMORY_TEST").is_some())
                    && (self.map_frames % 120 == 0 || self.last.as_ref().is_none_or(|(map, _, _)| Some(map) != self.app.game.session.current_map.as_ref()))
                {
                    let memory = self.app.renderer.as_ref().unwrap().device.device.generate_allocator_report().expect("Vulkan allocation report");
                    println!("MAP-MEMORY map={:?} live={} reserved={} allocations={} entities={} sprites={}", self.app.game.session.current_map, memory.total_allocated_bytes, memory.total_reserved_bytes, memory.allocations.len(), self.app.game.world.entities.iter().count(), self.app.game.sprite_caches.sprites.len());
                    if memory.total_allocated_bytes > 1024 * 1024 * 1024 {
                        let mut allocations = memory.allocations;
                        allocations.sort_by_key(|allocation| std::cmp::Reverse(allocation.size));
                        for allocation in allocations.iter().take(12) { println!("MAP-MEMORY largest={:?} bytes={}", allocation.name, allocation.size); }
                        panic!("Switch map transition exceeds 1 GiB live GPU budget before loading the next map");
                    }
                }
                assert!(
                    self.app.windows.minimap_window.minimap_texture_path.as_ref().is_some_and(|path| {
                        self.app.renderer.as_ref().unwrap().texture_cache.get(path).is_some()
                    }),
                    "loaded map has no usable minimap texture; HUD renders a black circle"
                );
                let texture = self.app.windows.minimap_window.minimap_texture_path.as_deref().unwrap();
                match self.app.game.session.current_map.as_deref() {
                    Some("int_land03") => assert_eq!(texture, "nostalre-generated-minimap/int_land03"),
                    Some("prontera") => assert_eq!(texture, nostalre_resources::ui::minimap::of("prontera")),
                    _ => {}
                }
                    self.map_frames += 1;
                    if let (Some(map), Some(player)) = (
                        self.app.game.session.current_map.as_ref(),
                        self.app.game.world.entities.player(),
                    ) {
                        let (x, z) = player.movement.position();
                        if let Some((previous_map, px, pz)) = &self.last {
                            let distance = ((x - px).powi(2) + (z - pz).powi(2)).sqrt();
                            if previous_map == map && distance < 4.0 {
                                self.walked += distance;
                                if let Some(phase) = controller_phase {
                                    let direction = if phase == 0 { -1.0 } else { 1.0 };
                                    self.controller_distance[phase] +=
                                        ((x - px) * direction).max(0.0);
                                }
                            }
                        }
                        self.last = Some((map.clone(), x, z));
                    }
                }
            }
            fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
                self.app.about_to_wait(event_loop);
                assert!(
                self.began.elapsed() < Duration::from_secs(if std::env::var_os("NOSTALRE_GAMEPLAY_VALIDATION").is_some() { 300 } else if std::env::var_os("NOSTALRE_MAP_MEMORY_TEST").is_some() { 240 } else { 150 }),
                    "full client did not complete its graphical script"
                );
            }
        }

        let config_path = std::env::var("NOSTALRE_UI_CONFIG").expect("external test config");
        let config = Config::load_or_default(&config_path);
        assert!(config.solo.as_ref().and_then(|solo| solo.char_id).is_some());
        assert!(
            config
                .grf_paths
                .iter()
                .all(|path| std::path::Path::new(path).is_file())
        );
        crate::init_logging(std::io::stdout);
        let mut builder = EventLoop::builder();
        builder.with_any_thread(true);
        let event_loop = builder.build().unwrap();
        let mut run = WindowRun {
            validation: crate::gameplay_validation::Probe::from_env(),
            app: App::new(config),
            began: Instant::now(),
            initialized: false,
            map_frames: 0,
            last: None,
            walked: 0.0,
            incoming: None,
            forward: None,
            equip_ok: 0,
            equip_failed: 0,
            dealt_damage: 0,
            npc_text: 0,
            npc_visible: false,
            character_lists: 0,
            map_entries: 0,
            was_in_game: false,
            controller_started: None,
            controller_finished: false,
            controller_distance: [0.0; 2],
        };
        event_loop.run_app(&mut run).unwrap();
        if let Some(validation) = &run.validation {
            assert!(validation.finished, "extended gameplay scenarios did not finish");
        }
        assert!(run.map_frames > 10, "world was never rendered");
        assert!(
            run.walked >= 2.0,
            "real avatar did not walk through the rendered world"
        );
        if std::env::var_os("NOSTALRE_COMODO_MEMORY_TEST").is_some() {
            assert_eq!(run.app.game.session.current_map.as_deref(), Some("comodo"));
            println!("PASS focused Comodo window: frames={} walked={:.2}; PC Vulkan, hardware freeze unverified", run.map_frames, run.walked);
            return;
        }
        assert!(
            run.app.effect_loader.background_worker_healthy(),
            "effect preload did not use its bounded background worker"
        );
        assert!(
            run.equip_ok > 0 && run.equip_failed == 0,
            "equip action did not receive a successful server acknowledgment"
        );
        assert!(
            run.dealt_damage > 0,
            "the player never dealt damage; sending an attack alone is insufficient"
        );
        assert!(
            run.npc_text > 0 && run.npc_visible,
            "NPC response was not displayed by the actual UI"
        );
        assert!(
            run.map_entries >= 2 && run.character_lists >= 3,
            "character selection/re-entry/cancel sequence did not complete"
        );
        assert!(
            run.controller_finished
                && run
                    .controller_distance
                    .iter()
                    .all(|distance| *distance >= 1.0),
            "normalized left-stick input did not move the real avatar in both directions"
        );
        run.app.save_player_snapshot();
        println!(
            "PASS full client window: map_frames={} walked_world_units={:.2} equip_ack={} damage_events={} npc_text={} npc_visible={} map_entries={} character_lists={} controller_distance={:?}; desktop rendering, not Switch FPS approval",
            run.map_frames,
            run.walked,
            run.equip_ok,
            run.dealt_damage,
            run.npc_text,
            run.npc_visible,
            run.map_entries,
            run.character_lists,
            run.controller_distance
        );
    }
}
