mod adoption;
mod character;
mod chat;
mod companion;
mod config;
mod connection;
mod entity;
mod equip_switch;
mod equipment_attributes;
mod friends;
mod gm;
mod guild;
mod inventory;
mod lifecycle;
mod login;
mod mail;
pub(crate) mod marriage;
mod npc;
mod party;
mod pet;
mod production;
mod quest;
mod skill;
mod storage;
mod trade;

use crate::App;
use crate::body_effect::{Counter, Trigger};
use models::enums::skill_enums::SkillEnum;
use nostalre_formats::act::SpriteActionType;
use nostalre_formats::grf::GrfArchive;
use nostalre_game::app_state::AppState;
use nostalre_game::autocounter;
use nostalre_game::chat_room::ChatRoom;
use nostalre_game::entity::{EntityType, ForcedAnimation};
use nostalre_game::event::GameEvent;
use nostalre_game::gm::MANNER_POINT_STEP;
use nostalre_game::show_digit::ShowDigitClock;
use nostalre_game::skill::teleport_lvl1_destination;
use nostalre_network::*;
use nostalre_profiling::profile_function;
use nostalre_renderer::Renderer;
use nostalre_ui_component::Window as UiWindow;
use nostalre_ui_component::account::char_create_window::CharCreateWindow;
use nostalre_ui_component::game::chat_room_member_window;
use nostalre_ui_component::game::guild_expel_dialog::GuildExpelDialog;
use nostalre_ui_component::game::party_helper_window::MODE_CREATE;
use winit::event_loop::ActiveEventLoop;

const BLADE_STOP_GRIP_FRAME: usize = 4;

pub(crate) fn char_create_error_message(error_code: u8) -> &'static str {
    match error_code {
        0x00 => nostalre_ui_component::pt!("That name already exists.", "Esse nome ja existe."),
        0x01 => nostalre_ui_component::pt!(
            "You are not eligible to create a character.",
            "Esta conta nao pode criar personagens."
        ),
        0x02 => nostalre_ui_component::pt!(
            "Symbols in character names are forbidden.",
            "Esse nome tem simbolos proibidos."
        ),
        0x03 => nostalre_ui_component::pt!(
            "This character slot is unavailable.",
            "Esse espaco de personagem esta indisponivel."
        ),
        _ => nostalre_ui_component::pt!(
            "Character creation failed.",
            "Falha ao criar personagem. Consulte o log do servidor."
        ),
    }
}

pub(crate) fn char_delete_reserve_error(result: u32) -> &'static str {
    match result {
        3 => "A database error occurred.",
        4 => "Leave your guild to delete this character.",
        5 => "Leave your party to delete this character.",
        _ => "Unable to schedule deletion.",
    }
}

pub(crate) fn char_delete_confirm_error(result: u32) -> &'static str {
    match result {
        2 => "This character cannot be deleted.",
        3 => "A database error occurred.",
        4 => "The deletion delay has not elapsed yet.",
        5 => "The birthdate does not match.",
        _ => "Character deletion failed.",
    }
}

pub(crate) fn preload_window<W: UiWindow>(
    window: &mut W,
    renderer: &mut Renderer,
    grf: &GrfArchive,
) {
    if !window.has_grf_textures() {
        let paths = W::grf_texture_paths();
        let loaded = renderer.preload_textures(&paths, grf);
        window.set_has_grf_textures(loaded);
        if loaded {
            window.set_texture_sizes(&|name| renderer.texture_cache.texture_size(name));
        }
    }
}

impl App {
    /// A unit's name: the party list's, else its name reply's; None before either.
    pub(crate) fn name_of(&self, aid: u32) -> Option<String> {
        self.game
            .party
            .as_ref()
            .and_then(|p| p.member(aid))
            .map(|m| m.name.clone())
            .or_else(|| {
                self.game
                    .world
                    .entities
                    .get(aid)
                    .and_then(|e| e.name.clone())
            })
    }

    /// An item's display name from the item tables, "Item #id" when no table names it.
    pub(crate) fn item_name(&self, item_id: u32) -> String {
        self.item_name_for(item_id, true)
    }

    /// The name as the player sees it: an unidentified item's is its unidentified one.
    pub(crate) fn item_name_for(&self, item_id: u32, identified: bool) -> String {
        self.game.data_table.item_name_for(item_id, identified)
    }

    pub(crate) fn handle_game_events(&mut self, event_loop: &ActiveEventLoop) {
        profile_function!("handle_game_events");
        let events = self.channel.drain_events();
        for event in events {
            match event {
                GameEvent::LoginAccepted {
                    account_id,
                    login_id1,
                    login_id2,
                    sex,
                    servers,
                    web_auth_token,
                } => {
                    self.handle_login_accepted(account_id, login_id1, login_id2, sex, servers);
                    if let Some(session) = &mut self.game.session.login_session {
                        session.web_auth_token = web_auth_token;
                    }
                }
                GameEvent::LoginRefused { error_code } => {
                    self.handle_login_refused(error_code);
                }
                GameEvent::CharServerConnectRefused { error_code } => {
                    self.handle_char_server_connect_refused(error_code);
                }
                GameEvent::CharacterListReceived { characters } => {
                    self.handle_character_list_received(characters);
                }
                GameEvent::ZoneServerConnectInfo {
                    char_id,
                    map_name,
                    ip,
                    port,
                } => {
                    self.handle_zone_server_connect_info(char_id, map_name, ip, port);
                }
                GameEvent::ZoneServerChanged { map_name, ip, port } => {
                    self.handle_zone_server_changed(map_name, ip, port);
                }
                GameEvent::AccessibleMapsReceived { maps } => {
                    self.handle_accessible_maps_received(maps);
                }
                GameEvent::RestartAck => {
                    self.handle_restart_ack();
                }
                GameEvent::DisconnectAck { allowed } => {
                    self.handle_disconnect_ack(allowed, event_loop);
                }

                GameEvent::MapEntered { x, y, dir, .. } => {
                    self.handle_map_entered(x, y, dir);
                }
                GameEvent::MapChanged { map_name, x, y } => {
                    self.handle_map_changed(map_name, x, y);
                }
                GameEvent::MapPropertyChanged(properties) => {
                    // kRO answers every map property so; a map's special popup comes back
                    self.channel
                        .send_packet(build_blocking_play_cancel_packet(self.active_packetver));
                    self.game.combat.damage_numbers.combat_hidden = properties.is_siege();
                    let left_pk_zone =
                        self.game.session.map_properties.is_pk_zone() && !properties.is_pk_zone();
                    self.game.session.map_properties = properties;
                    if left_pk_zone {
                        self.clear_pvp_ranks();
                    }
                }
                GameEvent::PlayerMoved {
                    start_x,
                    start_y,
                    dest_x,
                    dest_y,
                    start_time,
                } => {
                    self.handle_player_moved(start_x, start_y, dest_x, dest_y, start_time);
                }

                GameEvent::EntitySpawned {
                    gid,
                    aid,
                    job,
                    speed,
                    sex,
                    head,
                    weapon,
                    shield,
                    robe,
                    head_top,
                    head_mid,
                    head_bottom,
                    hair_color,
                    cloth_color,
                    x,
                    y,
                    direction,
                    body_state,
                    health_state,
                    effect_state,
                    base_level,
                    is_boss,
                    posture,
                    guild_id,
                    guild_emblem_version,
                    is_new_entry,
                } => {
                    self.handle_entity_spawned(
                        gid,
                        aid,
                        job,
                        speed,
                        sex,
                        head,
                        nostalre_game::sprite_path::weapon_view(weapon),
                        nostalre_game::sprite_path::left_hand_view(shield),
                        robe,
                        head_top,
                        head_mid,
                        head_bottom,
                        hair_color,
                        cloth_color,
                        x,
                        y,
                        direction,
                        body_state,
                        health_state,
                        effect_state,
                        base_level,
                        is_boss,
                        posture,
                        guild_id,
                        guild_emblem_version,
                        is_new_entry,
                    );
                }
                GameEvent::EntityMoved {
                    gid,
                    start_x,
                    start_y,
                    dest_x,
                    dest_y,
                    start_time,
                } => {
                    self.handle_entity_moved(gid, start_x, start_y, dest_x, dest_y, start_time);
                }
                GameEvent::EntityVanished { gid, vanish_type } => {
                    // A summon's HP comes only at its spawn (`clif_spawn`), so it is kept out of sight, dropped dead
                    if matches!(vanish_type, models::enums::vanish::VanishType::Die) {
                        self.game.companions.summons.remove(&gid);
                    }
                    self.handle_entity_vanished(gid, vanish_type);
                }
                GameEvent::EntityStopMove { gid, x, y } => {
                    self.game.world.entities.apply_entity_stop_move(gid, x, y);
                }
                GameEvent::EntityHighJumped { gid, x, y } => {
                    // By the time this relocate arrives the leap has carried the
                    // caster off-screen (faded), so teleport to the landing cell
                    // straight away — the landing effect drops it back in.
                    self.game.world.entities.apply_entity_stop_move(gid, x, y);
                }
                GameEvent::EntityAction {
                    gid,
                    target_gid,
                    action,
                    damage,
                    left_damage,
                    attack_mt,
                    attacked_mt,
                    count,
                    start_time,
                    ..
                } => {
                    self.handle_entity_action(
                        gid,
                        target_gid,
                        action,
                        damage,
                        left_damage,
                        attack_mt,
                        attacked_mt,
                        count,
                        start_time,
                    );
                }
                GameEvent::EntityDirectionChanged { gid, head_dir, dir } => {
                    self.game
                        .world
                        .entities
                        .apply_entity_direction_changed(gid, head_dir, dir);
                }
                GameEvent::CharNameReceived { char_id, name } => {
                    self.game.character.char_names.insert(char_id, name);
                }
                GameEvent::EntityNameReceived { gid, name } => {
                    self.game
                        .world
                        .entities
                        .apply_entity_name_received(gid, name);
                }
                GameEvent::EntityNamesReceived {
                    gid,
                    name,
                    party_name,
                    guild_name,
                    position_name,
                    title_id,
                } => {
                    // The character's own names carry the title it wears (0xcf2a50)
                    if gid == self.local_aid_gid().0 {
                        self.game.character.titles.current = title_id;
                    }
                    let title = self.game.data_table.titles.name(title_id).to_string();
                    self.game.world.entities.apply_entity_names_received(
                        gid,
                        name,
                        party_name,
                        guild_name,
                        position_name,
                        title,
                    );
                }
                GameEvent::AchievementsRewarded { achievement_ids } => {
                    self.game
                        .character
                        .titles
                        .set_rewarded(&self.game.data_table.titles, &achievement_ids);
                }
                GameEvent::TitleChangeAnswered { ok, title_id } => {
                    self.handle_title_change_answered(ok, title_id);
                }
                GameEvent::EquipSwitchList { entries } => {
                    self.game.character.equip_switch.set_list(entries);
                }
                GameEvent::EquipSwitchAdded {
                    index,
                    location,
                    flag,
                } => self.handle_equip_switch_added(index, location, flag),
                GameEvent::EquipSwitchRemoved { index, failed } => {
                    self.handle_equip_switch_removed(index, failed);
                }
                GameEvent::EquipSwitchAnswered { failed } => {
                    self.handle_equip_switch_answered(failed);
                }
                GameEvent::EntityHpChanged { gid, hp, max_hp } => {
                    self.handle_entity_hp_changed(gid, hp, max_hp);
                }
                GameEvent::EntityOptionChanged {
                    gid,
                    body_state,
                    health_state,
                    effect_state,
                } => {
                    self.handle_entity_option_changed(gid, body_state, health_state, effect_state);
                }
                GameEvent::PlayEffectOnEntity {
                    gid,
                    effect_id,
                    value,
                } => {
                    self.handle_play_effect_on_entity(gid, effect_id, value);
                }
                GameEvent::PlayMiscEffectOnEntity { gid, code } => {
                    self.handle_play_misc_effect_on_entity(gid, code);
                }
                GameEvent::SpiritsChanged { gid, count } => self.set_spirits(gid, count),
                GameEvent::MillenniumShieldChanged { gid, count } => {
                    self.set_body_effect(gid, Trigger::Counter(Counter::MillenniumShield), count);
                }
                GameEvent::CharmsChanged {
                    gid,
                    element,
                    count,
                } => {
                    for (charm, count) in Counter::charms(element, count) {
                        self.set_body_effect(gid, Trigger::Counter(charm), count);
                    }
                }
                GameEvent::SoulEnergyChanged { gid, count } => {
                    self.set_body_effect(gid, Trigger::Counter(Counter::SoulEnergy), count);
                }
                GameEvent::PvpRankingChanged {
                    account_id,
                    ranking,
                    total,
                } => {
                    self.handle_pvp_ranking_changed(account_id, ranking, total);
                }
                GameEvent::DevotionLinks { gid, targets } => {
                    self.handle_devotion_links(gid, &targets);
                }
                GameEvent::ElementalInit {
                    gid,
                    hp,
                    max_hp,
                    sp,
                    max_sp,
                } => {
                    self.game.companions.elemental =
                        Some(nostalre_game::companion::ElementalState {
                            gid,
                            hp,
                            max_hp,
                            sp,
                            max_sp,
                        });
                }
                // kRO's handler ignores a change before the elemental is known (0x7bae53)
                GameEvent::ElementalParChanged { par, value } => {
                    if let Some(elemental) = self.game.companions.elemental.as_mut() {
                        elemental.set(par, value);
                    }
                }
                GameEvent::SummonHpInit { gid, hp, max_hp } => {
                    self.game
                        .companions
                        .summons
                        .insert(gid, nostalre_game::companion::SummonHp { hp, max_hp });
                }
                // kRO's handler (0x7f4330) ignores a change for a summon it was never told of
                GameEvent::SummonHpChanged { gid, par, value } => {
                    if let Some(summon) = self.game.companions.summons.get_mut(&gid) {
                        summon.set(par, value);
                    }
                }
                GameEvent::ShadowScars { gid, amount } => self.handle_shadow_scars(gid, amount),
                GameEvent::ComboDelay { gid, delay_ms } => {
                    self.game.world.combo_delays.insert(gid, delay_ms);
                    if let Some(entity) = self.game.world.entities.get_mut(gid) {
                        entity.rearm_pose_hold(delay_ms as f32 / 1000.0);
                    }
                }
                GameEvent::BladeStop {
                    src_gid,
                    dest_gid,
                    active,
                } => {
                    for gid in [src_gid, dest_gid] {
                        if let Some(entity) = self.game.world.entities.get_mut(gid) {
                            entity.rooted = active;
                            if active {
                                entity.movement.stop();
                            }
                        }
                    }
                    if let Some(caster) = self.game.world.entities.get_mut(src_gid) {
                        caster.forced_animation = active.then(|| {
                            ForcedAnimation::held(
                                SpriteActionType::Skill as usize,
                                BLADE_STOP_GRIP_FRAME,
                            )
                        });
                    }
                }
                GameEvent::EntityOpt3Changed {
                    gid,
                    effect_state,
                    base_level,
                    opt3,
                } => {
                    self.handle_entity_opt3_changed(gid, effect_state, base_level, opt3);
                }
                GameEvent::StatusEffectChanged {
                    gid,
                    efst,
                    active,
                    remain_ms,
                    val1,
                } => {
                    self.handle_status_effect_changed(gid, efst, active, remain_ms, val1);
                }
                GameEvent::SoundEffect {
                    name,
                    act,
                    term_ms,
                    gid,
                } => {
                    self.handle_sound_effect(name, act, term_ms, gid);
                }
                GameEvent::EntityResurrected { gid } => {
                    self.handle_entity_resurrected(gid);
                }
                GameEvent::MvpReward { gid } => {
                    self.handle_mvp_reward(gid);
                }
                GameEvent::MvpFeedback { kind } => {
                    self.handle_mvp_feedback(kind);
                }
                GameEvent::FamePointsGained { kind, point, total } => {
                    self.handle_fame_points_gained(kind, point, total);
                }
                GameEvent::PvpPointsReceived { win, lose, point } => {
                    self.handle_pvp_points_received(win, lose, point);
                }
                GameEvent::EntitySpriteChanged {
                    gid,
                    sprite_type,
                    value,
                    value2,
                } => {
                    self.handle_entity_sprite_changed(gid, sprite_type, value, value2);
                }
                GameEvent::EntityEmotion { gid, emotion_type } => {
                    let duration = nostalre_game::emotion::emote_duration(
                        self.game.assets.emotion_act.as_ref(),
                        emotion_type,
                    );
                    self.game
                        .world
                        .entities
                        .apply_entity_emotion(gid, emotion_type, duration);
                }

                GameEvent::NpcDialogText { npc_id, text } => {
                    let icons = nostalre_game::npc_text::item_icons(&text)
                        .filter_map(|id| self.game.data_table.item_icon_path(id))
                        .collect();
                    self.preload_item_icons(icons);
                    self.windows.npc_dialog.dialog.open_text(npc_id, &text);
                }
                GameEvent::NpcDialogClear { npc_id } => {
                    self.windows.npc_dialog.dialog.clear_text(npc_id);
                }
                GameEvent::NpcDialogNext { npc_id } => {
                    self.windows.npc_dialog.dialog.wait_for_next(npc_id);
                }
                GameEvent::NpcDialogClose { .. } => {
                    if self.windows.npc_dialog.dialog.is_open() {
                        self.windows.npc_dialog.dialog.wait_for_close();
                    }
                }
                GameEvent::NpcDialogMenu { npc_id, items } => {
                    self.windows.npc_dialog.dialog.show_menu(npc_id, items);
                }
                GameEvent::WarpList {
                    skill,
                    destinations,
                } => {
                    let auto_destination = self
                        .config
                        .custom
                        .skill
                        .al_teleport
                        .skip_lvl1_menu
                        .then(|| {
                            teleport_lvl1_destination(skill, &destinations).map(str::to_string)
                        })
                        .flatten();
                    match auto_destination {
                        Some(map_name) => {
                            self.channel.send_packet(build_select_warppoint_packet(
                                skill,
                                &map_name,
                                self.active_packetver,
                            ));
                        }
                        None => self.windows.warp_list_window.open(skill, destinations),
                    }
                }
                GameEvent::NpcInputNumber { npc_id } => {
                    self.windows.npc_dialog.dialog.wait_for_number_input(npc_id);
                }
                GameEvent::NpcInputString { npc_id } => {
                    self.windows.npc_dialog.dialog.wait_for_string_input(npc_id);
                }
                GameEvent::NpcDealTypeSelect { npc_id } => {
                    self.windows.npc_dialog.dialog.show_deal_type(npc_id);
                }

                GameEvent::NpcShopBuyList { npc_id, items } => {
                    self.handle_npc_shop_buy_list(npc_id, items);
                }
                GameEvent::NpcShopSellList { npc_id, items } => {
                    self.handle_npc_shop_sell_list(npc_id, items);
                }
                GameEvent::NpcShopBuyResult { result } => {
                    self.handle_npc_shop_buy_result(result);
                }
                GameEvent::NpcMarketList { items } => {
                    self.handle_npc_market_list(&items);
                }
                GameEvent::NpcBarterList { extended, items } => {
                    self.handle_npc_barter_list(extended, items);
                }
                GameEvent::NpcMarketBought { success, items } => {
                    let line = self
                        .windows
                        .npc_shop
                        .shop
                        .apply_market_bought(success, &items);
                    self.add_msg_line(line, &[]);
                }
                GameEvent::NpcShopSellResult { result } => {
                    self.handle_npc_shop_sell_result(result);
                }

                GameEvent::ChatRoomUpsert {
                    owner_aid,
                    room_id,
                    max_count,
                    cur_count,
                    atype,
                    title,
                } => {
                    self.game.chat_rooms.upsert(ChatRoom {
                        room_id,
                        owner_aid,
                        title,
                        cur_count,
                        max_count,
                        atype,
                    });
                }
                GameEvent::ChatRoomDestroy { room_id } => {
                    self.game.chat_rooms.remove(room_id);
                    if self.windows.chat_room_member_window.room_id() == room_id {
                        self.windows.chat_room_member_window.close();
                    }
                }
                GameEvent::ChatRoomEntered { room_id, members } => {
                    let (title, max_count, public) = self
                        .game
                        .chat_rooms
                        .get(room_id)
                        .map(|r| (r.title.clone(), r.max_count, r.atype != 0))
                        .unwrap_or_default();
                    let local_name = self.game.character.name.clone();
                    self.windows.chat_room_member_window.open_joined(
                        room_id,
                        &title,
                        max_count,
                        public,
                        members,
                        &local_name,
                    );
                    self.windows.chat_room_member_window.push_message(
                        "You entered the room.".to_string(),
                        chat_room_member_window::JOIN_MSG_COLOR,
                    );
                    self.windows
                        .chat_window
                        .add_system("You entered the room.".to_string());
                }
                GameEvent::ChatRoomCreateResult { flag } => {
                    if flag == 0 {
                        if let Some((title, limit, public)) = self.game.pending_chat_room.take() {
                            let local_name = self.game.character.name.clone();
                            self.windows.chat_room_member_window.open_created(
                                0,
                                &title,
                                limit,
                                public,
                                &local_name,
                            );
                        }
                        self.windows.chat_room_create_window.close();
                    } else {
                        let reason = match flag {
                            1 => "Room limit exceeded.",
                            2 => "A room with that name already exists.",
                            _ => "Could not create the room.",
                        };
                        self.windows.chat_window.add_system(reason.to_string());
                    }
                }
                GameEvent::ChatRoomMemberJoined { name, .. } => {
                    self.windows.chat_room_member_window.add_member(&name);
                    let msg = format!("{name} has joined the room.");
                    self.windows
                        .chat_room_member_window
                        .push_message(msg.clone(), chat_room_member_window::JOIN_MSG_COLOR);
                    self.windows.chat_window.add_system(msg);
                }
                GameEvent::ChatRoomMemberLeft { name, kicked, .. } => {
                    let verb = if kicked {
                        "was kicked from"
                    } else {
                        "has left"
                    };
                    let msg = format!("{name} {verb} the room.");
                    if self.windows.chat_room_member_window.is_local(&name) {
                        self.windows.chat_room_member_window.close();
                    } else {
                        self.windows.chat_room_member_window.remove_member(&name);
                        self.windows
                            .chat_room_member_window
                            .push_message(msg.clone(), chat_room_member_window::LEAVE_MSG_COLOR);
                    }
                    self.windows.chat_window.add_system(msg);
                }
                GameEvent::ChatRoomOwnerChanged { name } => {
                    self.windows.chat_room_member_window.set_owner(&name);
                    let msg = format!("{name} is now the room owner.");
                    self.windows
                        .chat_room_member_window
                        .push_message(msg.clone(), chat_room_member_window::SYSTEM_MSG_COLOR);
                    self.windows.chat_window.add_system(msg);
                }
                GameEvent::ChatRoomJoinRefused { result } => {
                    let reason = match result {
                        1 => "Room is full.",
                        2 => "Wrong password.",
                        3 => "You have been kicked from this room.",
                        4 => "Not enough Zeny to enter.",
                        5 => "Your level is too low to enter.",
                        6 => "Your level is too high to enter.",
                        _ => "Cannot enter the room.",
                    };
                    self.windows.chat_window.add_system(reason.to_string());
                }

                GameEvent::InventoryNormalItems { items } => {
                    self.handle_inventory_normal_items(items);
                }
                GameEvent::InventoryEquipmentItems { items } => {
                    self.handle_inventory_equipment_items(items);
                }
                GameEvent::InventoryItemPickup {
                    index,
                    item_id,
                    count,
                    item_type,
                    is_identified,
                    is_damaged,
                    refining_level,
                    grade,
                    options,
                    slot,
                    location,
                    result,
                } => {
                    self.handle_inventory_item_pickup(
                        index,
                        item_id,
                        count,
                        item_type,
                        is_identified,
                        is_damaged,
                        refining_level,
                        grade,
                        options,
                        slot,
                        location,
                        result,
                    );
                }
                GameEvent::InventoryUseItemResult {
                    index,
                    count,
                    success,
                } => {
                    if success {
                        use nostalre_game::effect::consumable_effects::{
                            consumable_use_effect, is_mercenary_potion,
                        };
                        let item_id = self
                            .game
                            .character
                            .inventory
                            .get_item(index)
                            .map(|item| item.item_id as u32);
                        let used_effect = item_id.and_then(consumable_use_effect);
                        let target_gid = item_id
                            .filter(|id| is_mercenary_potion(*id))
                            .and(self.game.companions.mercenary.as_ref().map(|m| m.gid))
                            .filter(|gid| *gid != 0)
                            .or_else(|| self.game.world.entities.player_id());
                        if let (Some(effect), Some(gid)) = (used_effect, target_gid) {
                            self.effect_queue.spawn_on(effect, gid);
                        }
                        self.game
                            .character
                            .inventory
                            .update_item_count(index, count);
                    }
                }
                GameEvent::InventoryEquipResult {
                    index,
                    wear_location,
                    view_id,
                    success,
                } => {
                    self.handle_inventory_equip_result(index, wear_location, view_id, success);
                }
                GameEvent::InventoryArrowEquipped { index } => {
                    let ammo_mask = nostalre_game::inventory::InventoryData::slot_mask(
                        nostalre_game::inventory::EquipmentLocation::Ammo,
                    );
                    self.game
                        .character
                        .inventory
                        .update_wear_state(index, ammo_mask);
                }
                GameEvent::InventoryUnequipResult {
                    index,
                    wear_location,
                    success,
                } => {
                    self.handle_inventory_unequip_result(index, wear_location, success);
                }
                GameEvent::InventorySlots { slots } => {
                    self.game.character.inventory.slots = slots;
                }
                GameEvent::OverweightPercent { percent } => {
                    self.game.character.inventory.overweight_percent = percent;
                }
                GameEvent::InventoryItemRemoved { index, count } => {
                    self.game
                        .character
                        .inventory
                        .subtract_item_count(index, count);
                    self.game.combat.waiting_item_throw_ack = false;
                }
                GameEvent::CartNormalItems { items } => {
                    self.handle_cart_normal_items(items);
                }
                GameEvent::CartEquipmentItems { items } => {
                    self.handle_cart_equipment_items(items);
                }
                GameEvent::CartItemAdded {
                    index,
                    item_id,
                    count,
                    item_type,
                    is_identified,
                    is_damaged,
                    refining_level,
                    slot,
                } => {
                    self.handle_cart_item_added(
                        index,
                        item_id,
                        count,
                        item_type,
                        is_identified,
                        is_damaged,
                        refining_level,
                        slot,
                    );
                }
                GameEvent::CartItemRemoved { index, count } => {
                    self.handle_cart_item_removed(index, count);
                }
                GameEvent::CartCountInfo {
                    cur_weight,
                    max_weight,
                    cur_count,
                    max_count,
                } => {
                    self.handle_cart_count_info(cur_weight, max_weight, cur_count, max_count);
                }
                GameEvent::CartOff => {
                    self.handle_cart_off();
                }

                GameEvent::StorageNormalItems { items } => {
                    self.handle_storage_normal_items(items);
                }
                GameEvent::StorageEquipItems { items } => {
                    self.handle_storage_equip_items(items);
                }
                GameEvent::StorageOpened { cur, max } => {
                    self.handle_storage_opened(cur, max);
                }
                GameEvent::StorageItemAdded {
                    index,
                    item_id,
                    count,
                    item_type,
                    is_identified,
                    is_damaged,
                    refining_level,
                    slot,
                } => {
                    self.handle_storage_item_added(
                        index,
                        item_id,
                        count,
                        item_type,
                        is_identified,
                        is_damaged,
                        refining_level,
                        slot,
                    );
                }
                GameEvent::StorageItemRemoved { index, amount } => {
                    self.handle_storage_item_removed(index, amount);
                }
                GameEvent::StorageClosed => {
                    self.handle_storage_closed();
                }
                GameEvent::StoragePasswordRequest { prompt } => {
                    self.handle_storage_password_request(prompt);
                }
                GameEvent::OpenStoragePasswordPrompt { prompt } => {
                    self.open_storage_password_dialog(prompt);
                }
                GameEvent::StoragePasswordResult {
                    outcome,
                    error_count,
                } => {
                    self.handle_storage_password_result(outcome, error_count);
                }
                GameEvent::RequestStoragePassword {
                    change,
                    password,
                    new_password,
                } => {
                    self.channel.send_packet(build_ack_store_password_packet(
                        change,
                        &password,
                        &new_password,
                        self.active_packetver,
                    ));
                }

                GameEvent::ExchangeRequested { name, gid, level } => {
                    self.handle_exchange_requested(name, gid, level);
                }
                GameEvent::ExchangeAckResult { result, level } => {
                    self.handle_exchange_ack_result(result, level);
                }
                GameEvent::ExchangeItemAdded {
                    item_id,
                    item_type,
                    count,
                    is_identified,
                    is_damaged,
                    refining_level,
                    slot,
                } => {
                    self.handle_exchange_item_added(
                        item_id,
                        item_type,
                        count,
                        is_identified,
                        is_damaged,
                        refining_level,
                        slot,
                    );
                }
                GameEvent::ExchangeAddResult { index, result } => {
                    self.handle_exchange_add_result(index, result);
                }
                GameEvent::ExchangeConcluded { who } => {
                    self.handle_exchange_concluded(who);
                }
                GameEvent::ExchangeCanceled => {
                    self.handle_exchange_canceled();
                }
                GameEvent::ExchangeCompleted { result } => {
                    self.handle_exchange_completed(result);
                }
                GameEvent::ExchangeUndo => {
                    self.handle_exchange_undo();
                }

                GameEvent::MailWindow { open } => {
                    self.handle_mail_window(open);
                }
                GameEvent::MailInboxReceived { entries } => {
                    self.handle_mail_inbox_received(entries);
                }
                GameEvent::MailOpened { mail } => {
                    self.handle_mail_opened(mail);
                }
                GameEvent::MailDeleteAck { mail_id, ok } => {
                    self.handle_mail_delete_ack(mail_id, ok);
                }
                GameEvent::MailGetItemAck { result } => {
                    self.handle_mail_get_item_ack(result);
                }
                GameEvent::MailAddItemAck { index, ok } => {
                    self.handle_mail_add_item_ack(index, ok);
                }
                GameEvent::MailSendAck { ok } => {
                    self.handle_mail_send_ack(ok);
                }
                GameEvent::MailNewReceived {
                    mail_id,
                    title,
                    sender,
                } => {
                    self.handle_mail_new_received(mail_id, title, sender);
                }
                GameEvent::MailReturnAck { mail_id, ok } => {
                    self.handle_mail_return_ack(mail_id, ok);
                }
                GameEvent::ShowSystemMessage { message } => {
                    self.windows.chat_window.add_system(message);
                }
                GameEvent::ServerMsg { msg_id } => {
                    self.handle_server_msg(msg_id, &[]);
                }
                GameEvent::ServerMsgColor { msg_id, color } => {
                    self.handle_server_msg_color(msg_id, color);
                }
                GameEvent::ServerMsgValue { msg_id, value } => {
                    self.handle_server_msg(msg_id, &[&value.to_string()]);
                }
                GameEvent::ServerSkillMsg { skill_id, msg_id } => {
                    self.handle_server_skill_msg(skill_id, msg_id);
                }
                GameEvent::MapInfoNotice { atype } => {
                    self.handle_map_info_notice(atype);
                }
                GameEvent::ServerColoredMessage {
                    account_id,
                    color,
                    message,
                } => {
                    self.handle_server_colored_message(account_id, color, message);
                }
                GameEvent::UserCount { count } => {
                    self.handle_user_count(count);
                }
                GameEvent::MannerPointResult { result } => {
                    self.handle_manner_point_result(result);
                }
                GameEvent::MannerPointGiven {
                    positive,
                    other_name,
                } => {
                    self.handle_manner_point_given(positive, other_name);
                }
                GameEvent::GmStatusReceived { status } => {
                    self.handle_gm_status(&status);
                }
                GameEvent::AccountNameReceived { aid, name } => {
                    self.handle_account_name(aid, name);
                }
                GameEvent::SkillMsg { msg_no } => {
                    self.handle_skill_msg(msg_no);
                }
                GameEvent::BindOnEquipNotice { index } => {
                    self.handle_bind_on_equip_notice(index);
                }
                GameEvent::TalkboxContents { aid, message } => {
                    self.handle_talkbox_contents(aid, message);
                }
                GameEvent::ShowDigit { mode, value } => {
                    self.game.show_digit = Some(ShowDigitClock::new(mode, value));
                }
                GameEvent::BossInfoReceived {
                    kind,
                    x,
                    y,
                    respawn_hour,
                    respawn_minute,
                    name,
                } => {
                    self.handle_boss_info(kind, x, y, respawn_hour, respawn_minute, name);
                }
                GameEvent::WhisperSettingResult { allow, result, all } => {
                    self.handle_whisper_setting_result(allow, result, all);
                }
                GameEvent::MemoResult { result } => {
                    self.handle_memo_result(result);
                }
                GameEvent::ProgressBarStarted { duration_secs } => {
                    self.game.session.progress_bar =
                        Some(nostalre_game::progress_bar::ProgressBar::new(duration_secs));
                }
                GameEvent::ProgressBarCancelled => {
                    self.finish_progress_bar();
                }

                GameEvent::CardInsertItemList { equip_indices, .. } => {
                    self.handle_card_insert_item_list(equip_indices);
                }
                GameEvent::CardInsertResult {
                    equip_index,
                    card_index,
                    result,
                } => {
                    self.handle_card_insert_result(equip_index, card_index, result);
                }

                GameEvent::FloorItemAppeared {
                    id,
                    item_id,
                    is_identified,
                    x,
                    y,
                    sub_x,
                    sub_y,
                    count,
                    is_falling,
                } => {
                    self.handle_floor_item_appeared(
                        id,
                        item_id,
                        is_identified,
                        x,
                        y,
                        sub_x,
                        sub_y,
                        count,
                        is_falling,
                    );
                }
                GameEvent::FloorItemDisappeared { id } => {
                    self.game.world.floor_items.remove(&id);
                    self.game.assets.floor_item_sprites.remove(&id);
                }

                GameEvent::ChatMessage { gid, message } => {
                    self.handle_chat_message(gid, message);
                }
                GameEvent::OwnChatMessage { message } => {
                    self.handle_own_chat_message(message);
                }
                GameEvent::RankingReceived { ranking } => {
                    self.game.character.pending_ranking = Some(ranking);
                }
                GameEvent::BroadcastMessage {
                    message,
                    color,
                    banner,
                } => {
                    self.handle_broadcast_message(message, color, banner);
                }
                GameEvent::StarSkillNotice {
                    map_name,
                    monster_id,
                    star,
                    result,
                } => {
                    self.handle_star_skill_notice(map_name, monster_id, star, result);
                }
                GameEvent::StarPlaceRequest { which } => {
                    self.handle_star_place_request(which);
                }
                GameEvent::RequestAgreeStarPlace { which } => {
                    self.channel
                        .send_packet(build_agree_star_place_packet(which, self.active_packetver));
                }

                GameEvent::ParameterChanged { var_id, value } => {
                    self.handle_parameter_changed(var_id, value);
                }
                GameEvent::Recovery { var_id, amount } => {
                    self.handle_recovery(var_id, amount);
                }
                GameEvent::ExpGained {
                    aid,
                    amount,
                    is_base,
                    is_quest,
                } => {
                    self.handle_exp_gained(aid, amount, is_base, is_quest);
                }
                GameEvent::StatusChanged {
                    status_type,
                    base,
                    bonus,
                } => {
                    self.game
                        .character
                        .apply_status_changed(status_type, base, bonus);
                }
                GameEvent::AttackRangeChanged { range } => {
                    self.game.combat.attack_range = range;
                }
                GameEvent::AttackFailedForDistance {
                    target_gid,
                    target_x,
                    target_y,
                    x,
                    y,
                    range,
                } => {
                    self.handle_attack_failed_for_distance(
                        target_gid, target_x, target_y, x, y, range,
                    );
                }

                GameEvent::SkillCasting {
                    gid,
                    target_gid,
                    skill,
                    property,
                    delay_ms,
                    x,
                    y,
                } => {
                    if autocounter::is_kn_autocounter(skill)
                        && self.game.world.entities.player_id() == Some(gid)
                    {
                        self.start_autocounter_channel(gid);
                    } else {
                        // kRO picks a cast's directional class by the facing before the caster turns (Arms
                        // Cannon, 0x7ecff7)
                        let facing = self.skill_facing(skill, gid, None);
                        self.game.world.entities.apply_skill_casting(
                            gid,
                            target_gid,
                            skill,
                            delay_ms,
                            x,
                            y,
                            self.game.data_table.skill_name.as_ref(),
                        );
                        self.spawn_skill_begin_cast(skill, gid, property, delay_ms, facing);
                        self.spawn_cast_mark(skill, gid, target_gid, x, y, delay_ms);
                    }
                }
                GameEvent::SkillListReceived { skills } => {
                    self.handle_skill_list_received(skills);
                }
                GameEvent::SkillUpdated {
                    skill,
                    level,
                    sp_cost,
                    attack_range,
                    upgradable,
                    target_type,
                } => {
                    self.handle_skill_updated(
                        skill,
                        level,
                        sp_cost,
                        attack_range,
                        upgradable,
                        target_type,
                    );
                }
                GameEvent::SkillAdded { skill } => {
                    self.handle_skill_added(skill);
                }
                GameEvent::SkillRemoved { skill } => {
                    self.game.character.skills.remove_skill(skill);
                }
                GameEvent::SkillDamage {
                    skill,
                    src_gid,
                    target_gid,
                    damage,
                    attack_mt,
                    attacked_mt,
                    count,
                    level,
                    action,
                    start_time,
                } => {
                    self.note_script_skill_result(src_gid);
                    self.handle_skill_damage(
                        skill,
                        src_gid,
                        target_gid,
                        damage,
                        attack_mt,
                        attacked_mt,
                        count,
                        level,
                        action,
                        start_time,
                    );
                }
                GameEvent::SkillNoDamage {
                    skill,
                    src_gid,
                    target_gid,
                    level,
                } => {
                    self.note_script_skill_result(src_gid);
                    if autocounter::is_kn_autocounter(skill)
                        && self.game.world.entities.player_id() == Some(src_gid)
                    {
                        self.start_autocounter_channel(src_gid);
                    } else {
                        if skill != SkillEnum::MgSrecovery {
                            self.game.world.entities.show_skill_chat_bubble(
                                src_gid,
                                skill,
                                self.game.data_table.skill_name.as_ref(),
                            );
                        }
                        self.game
                            .world
                            .entities
                            .apply_skill_no_damage(skill, src_gid, target_gid);
                        self.spawn_skill_no_damage_effects(skill, src_gid, target_gid, level);
                    }
                }
                GameEvent::GroundSkill {
                    skill,
                    src_gid,
                    level,
                    x,
                    y,
                } => {
                    self.note_script_skill_result(src_gid);
                    self.game.world.entities.show_skill_chat_bubble(
                        src_gid,
                        skill,
                        self.game.data_table.skill_name.as_ref(),
                    );
                    self.game
                        .world
                        .entities
                        .apply_ground_skill(skill, src_gid, x, y);
                    self.spawn_ground_skill_effects(skill, src_gid, level, x, y);
                    if skill == SkillEnum::AcShower {
                        self.spawn_arrow_shower(src_gid, x, y);
                    }
                    let falcon_target = if self.game.sprite_caches.falcons.contains_key(&src_gid)
                        && matches!(skill, SkillEnum::HtDetecting | SkillEnum::SnSight)
                    {
                        match (
                            self.game.session.map_coords.as_ref(),
                            self.game.session.gat.as_ref(),
                        ) {
                            (Some(coords), Some(gat)) => {
                                let (wx, _, wz) =
                                    coords.cell_to_world(x as f32 + 0.5, y as f32 + 0.5);
                                Some([wx, gat.get_height(x as f32 + 0.5, y as f32 + 0.5), wz])
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if let Some(target) = falcon_target {
                        self.start_falcon_flight(src_gid, target);
                    }
                }
                GameEvent::MonsterInfoReceived { mut info } => {
                    info.name = self
                        .game
                        .world
                        .entities
                        .iter()
                        .find(|e| {
                            e.entity_type == EntityType::Monster
                                && e.job == info.job
                                && e.name.is_some()
                        })
                        .and_then(|e| e.name.clone())
                        .unwrap_or_else(|| format!("#{}", info.job));
                    self.windows.monster_info_window.show(info);
                }
                GameEvent::SkillUnitEntered {
                    aid,
                    creator_aid,
                    x,
                    y,
                    unit_id,
                    is_visible,
                } => {
                    self.handle_skill_unit_entered(aid, creator_aid, x, y, unit_id, is_visible);
                }
                GameEvent::GraffitiEntered {
                    aid,
                    creator_aid,
                    x,
                    y,
                    message,
                } => {
                    self.handle_graffiti_entered(aid, creator_aid, x, y, message);
                }
                GameEvent::MapCellChanged { x, y, cell_type } => {
                    self.handle_map_cell_changed(x, y, cell_type);
                }
                GameEvent::SkillUnitDisappeared { aid } => {
                    self.handle_skill_unit_disappeared(aid);
                }
                GameEvent::SkillUnitUpdated { aid } => {
                    self.handle_skill_unit_updated(aid);
                }
                GameEvent::SkillCastCancel { gid } => {
                    self.fire_autocounter_on_cancel(gid);
                    self.game.world.entities.apply_skill_cast_cancel(gid);
                    self.clear_cast_mark(gid);
                }
                GameEvent::SkillFailed {
                    skill,
                    cause,
                    btype,
                    item_id,
                } => {
                    if let Some(gid) = self.game.world.entities.player_id() {
                        self.cancel_begin_cast_effects(gid);
                        // a cast the server failed (e.g. a missing gemstone) ends like a cancelled one: otherwise
                        // the player stays in Casting and every click to walk is dropped
                        let casting = self.game.world.entities.player().is_some_and(|p| {
                            p.state() == nostalre_game::entity::EntityState::Casting
                        });
                        if casting {
                            self.game.world.entities.apply_skill_cast_cancel(gid);
                            self.clear_cast_mark(gid);
                        }
                    }
                    self.handle_skill_failed(skill, cause, btype, item_id);
                }
                GameEvent::SkillPostDelay { skill, delay_ms } => {
                    let now = self.start_time.elapsed().as_secs_f32();
                    self.game.character.cooldowns.set_skill_cooldown(
                        skill,
                        delay_ms as f32 / 1000.0,
                        now,
                    );
                }
                GameEvent::AfterCastDelay { delay_ms } => {
                    let now = self.start_time.elapsed().as_secs_f32();
                    self.game
                        .character
                        .cooldowns
                        .set_global_cooldown(delay_ms as f32 / 1000.0, now);
                }

                GameEvent::ServerTick {
                    server_tick,
                    local_send_time_ms,
                } => {
                    self.handle_server_tick(server_tick, local_send_time_ms);
                }
                GameEvent::HotkeyListReceived { slots } => {
                    self.game.character.hotkeys.set_from_server(&slots);
                }
                GameEvent::Disconnected(reason) => {
                    self.handle_disconnected(reason, event_loop);
                }
                GameEvent::ActionFailure { error_code } => {
                    if error_code != chat::ARROWFAIL_SUCCESS {
                        self.game.combat.attack_target_id = None;
                        self.game.world.entities.apply_action_failure();
                        if let Some(gid) = self.game.world.entities.player_id() {
                            self.cancel_begin_cast_effects(gid);
                        }
                    }
                    self.handle_action_failure(error_code);
                }

                GameEvent::PartyMemberList { name, members } => {
                    self.handle_party_member_list(name, members);
                }
                GameEvent::PartyMemberAdded {
                    aid,
                    name,
                    map,
                    leader,
                    online,
                    x,
                    y,
                    class,
                    base_level,
                } => {
                    self.handle_party_member_added(nostalre_game::party::PartyMember {
                        aid,
                        name,
                        map,
                        leader,
                        online,
                        x,
                        y,
                        class,
                        base_level,
                        ..Default::default()
                    });
                }
                GameEvent::PartyMemberDead { aid, dead } => {
                    self.handle_party_member_dead(aid, dead);
                }
                GameEvent::PartyMemberJob {
                    aid,
                    class,
                    base_level,
                } => {
                    self.handle_party_member_job(aid, class, base_level);
                }
                GameEvent::PartyMemberRemoved { aid, name, result } => {
                    self.handle_party_member_removed(aid, name, result);
                }
                GameEvent::PartyMemberHp { aid, hp, max_hp } => {
                    self.handle_party_member_hp(aid, hp, max_hp);
                }
                GameEvent::PartyMemberPosition { aid, x, y } => {
                    self.handle_party_member_position(aid, x, y);
                }
                GameEvent::PartyConfigChanged {
                    exp_option,
                    item_pickup_rule,
                    item_division_rule,
                } => {
                    self.handle_party_config_changed(
                        exp_option,
                        item_pickup_rule,
                        item_division_rule,
                    );
                }
                GameEvent::SelfConfigChanged { kind, enabled } => {
                    self.handle_self_config_changed(kind, enabled);
                }
                // drawn once the 2025 rig shows where kRO puts the dialog and what the popup looks like (3.8)
                GameEvent::NpcDialogPlacement(placement) => {
                    tracing::debug!("NPC dialog placement not applied yet: {placement:?}");
                }
                GameEvent::SpecialPopup { id } => self.handle_special_popup(id),
                GameEvent::SpecialItemObtained {
                    player,
                    item_id,
                    source,
                    refine: _,
                } => {
                    self.handle_special_item_obtained(&player, item_id, source);
                }
                GameEvent::NpcMusic { name } => {
                    self.play_npc_music(&name);
                }
                GameEvent::UnequipAllAnswered { refused } => {
                    self.handle_unequip_all_answered(refused);
                }
                GameEvent::PartyInviteReceived {
                    party_grid,
                    party_name,
                } => {
                    self.handle_party_invite_received(party_grid, party_name);
                }
                GameEvent::PartyInviteResult { name, answer } => {
                    self.handle_party_invite_result(name, answer);
                }
                GameEvent::PartyCreateResult { result } => {
                    self.handle_party_create_result(result);
                }
                GameEvent::PartyItemPickup {
                    aid,
                    item_id,
                    identified,
                    refine,
                } => {
                    self.handle_party_item_pickup(aid, item_id, identified, refine);
                }
                GameEvent::PartyChatMessage { message, .. } => {
                    self.handle_party_chat_message(message);
                }
                GameEvent::GuildChatMessage { message } => {
                    self.handle_guild_chat_message(message);
                }
                GameEvent::WhisperReceived { sender, message } => {
                    self.handle_whisper_received(sender, message);
                }
                GameEvent::WhisperAck { result } => {
                    self.handle_whisper_ack(result);
                }
                GameEvent::FriendListReceived { friends } => {
                    self.handle_friend_list_received(friends);
                }
                GameEvent::FriendStateChanged { aid, gid, online } => {
                    self.handle_friend_state_changed(aid, gid, online);
                }
                GameEvent::FriendAddResult {
                    result,
                    aid,
                    gid,
                    name,
                } => {
                    self.handle_friend_add_result(result, aid, gid, name);
                }
                GameEvent::FriendRemoved { aid, gid } => {
                    self.handle_friend_removed(aid, gid);
                }
                GameEvent::FriendRequestReceived {
                    req_aid,
                    req_gid,
                    name,
                } => {
                    self.handle_friend_request_received(req_aid, req_gid, name);
                }

                GameEvent::GuildMenuFlag { flag } => {
                    self.handle_guild_menu_flag(flag);
                }
                GameEvent::GuildInfo {
                    gdid,
                    name,
                    level,
                    exp,
                    max_exp,
                    member_num,
                    max_member_num,
                    avg_level,
                    point,
                    honor,
                    virtue,
                    master_name,
                    manage_land,
                    emblem_version,
                } => {
                    self.handle_guild_info(
                        gdid,
                        name,
                        level,
                        exp,
                        max_exp,
                        member_num,
                        max_member_num,
                        avg_level,
                        point,
                        honor,
                        virtue,
                        master_name,
                        manage_land,
                        emblem_version,
                    );
                }
                GameEvent::GuildMembers { members } => {
                    self.handle_guild_members(members);
                }
                GameEvent::GuildPositions { positions } => {
                    self.handle_guild_positions(positions);
                }
                GameEvent::GuildPositionNames { names } => {
                    self.handle_guild_position_names(names);
                }
                GameEvent::GuildMemberPositionsChanged { entries } => {
                    self.handle_guild_member_positions_changed(entries);
                }
                GameEvent::GuildMemberPosition { aid, x, y } => {
                    if let Some(guild) = &mut self.game.guild {
                        if x < 0 || y < 0 {
                            guild.clear_position_of(aid);
                        } else {
                            guild.set_position(aid, x as u16, y as u16);
                        }
                    }
                }
                GameEvent::GuildSkills { point, skills } => {
                    self.handle_guild_skills(point, skills);
                }
                GameEvent::GuildBanList { entries } => {
                    self.handle_guild_ban_list(entries);
                }
                GameEvent::GuildNotice { subject, body } => {
                    self.handle_guild_notice(subject, body);
                }
                GameEvent::GuildRelations { relations } => {
                    self.handle_guild_relations(relations);
                }
                GameEvent::GuildEmblem {
                    gdid,
                    version,
                    bmp,
                    from_map_server,
                } => {
                    self.handle_guild_emblem(gdid, version, bmp, from_map_server);
                }
                GameEvent::GuildEmblemUploaded { gdid, version, bmp } => {
                    self.handle_guild_emblem_uploaded(gdid, version, bmp);
                }
                GameEvent::GuildIdentityUpdated {
                    gdid,
                    emblem_version,
                    right,
                    is_master,
                    name,
                } => {
                    self.handle_guild_identity_updated(
                        gdid,
                        emblem_version,
                        right,
                        is_master,
                        name,
                    );
                }
                GameEvent::GuildCreateResult { result } => {
                    self.handle_guild_create_result(result);
                }
                GameEvent::GuildMemberOnline {
                    aid,
                    gid,
                    online,
                    appearance,
                } => {
                    self.handle_guild_member_online(aid, gid, online, appearance);
                }
                GameEvent::GuildMemberLeft { gid, reason } => {
                    self.handle_guild_member_left(gid, reason);
                }
                GameEvent::GuildMemberExpelled { gid, reason } => {
                    self.handle_guild_member_expelled(gid, reason);
                }
                GameEvent::EntityGuildPositionChanged { aid, position } => {
                    self.game
                        .world
                        .entities
                        .apply_entity_position_name(aid, position);
                }
                GameEvent::EntityGuildChanged {
                    aid,
                    gdid,
                    emblem_version,
                } => {
                    self.game
                        .world
                        .entities
                        .apply_entity_guild_changed(aid, gdid, emblem_version);
                    self.request_entity_guild_emblem(gdid, emblem_version);
                    self.apply_guild_emblem_to_models(gdid, emblem_version);
                }
                GameEvent::GuildDisbandResult { reason } => {
                    self.handle_guild_disband_result(reason);
                }
                GameEvent::GuildInviteReceived { gdid, name } => {
                    self.handle_guild_invite_received(gdid, name);
                }
                GameEvent::GuildAllyRequestReceived { aid, name } => {
                    self.handle_guild_ally_request_received(aid, name);
                }
                GameEvent::GuildAllyResult { answer } => {
                    self.handle_guild_ally_result(answer);
                }
                GameEvent::GuildHostileResult { result } => {
                    self.handle_guild_hostile_result(result);
                }
                GameEvent::GuildJoinResult { answer } => {
                    self.handle_guild_join_result(answer);
                }
                GameEvent::GuildRelationDeleted { gdid, relation } => {
                    self.handle_guild_relation_deleted(gdid, relation);
                }

                GameEvent::AutoCastSkill {
                    skill,
                    level,
                    sp_cost,
                    attack_range,
                    skill_target_type,
                } => {
                    self.handle_auto_cast_skill(
                        skill,
                        level,
                        sp_cost,
                        attack_range,
                        skill_target_type,
                    );
                }
                GameEvent::ItemIdentifyList { indices } => {
                    self.handle_item_identify_list(indices);
                }
                GameEvent::ItemIdentifyResult { index, ok } => {
                    self.handle_item_identify_result(index, ok);
                }
                GameEvent::MakingArrowList { item_ids } => {
                    self.handle_making_arrow_list(item_ids);
                }
                GameEvent::AutoSpellList { skills } => {
                    self.handle_auto_spell_list(skills);
                }
                GameEvent::SkillSelectList { why, skills } => {
                    self.handle_skill_select_list(why, skills);
                }
                GameEvent::ItemListWindowOpened { kind } => self.handle_item_list_window(kind),
                GameEvent::NavigationGuide { navigation } => {
                    self.handle_navigation_guide(navigation)
                }
                // Cart Decoration's list (0x097f), shown as kRO's handler (0x7f5190) hands it to its cart window
                // the rig titles it "Cart List" (2026-10-17)
                GameEvent::CartDecorationList { identity, carts } => {
                    self.preload_cart_previews(&carts);
                    self.windows
                        .cart_select_window
                        .open_decoration(identity, carts, "Cart List");
                }
                GameEvent::WeaponRefineList { items } => {
                    self.handle_weapon_refine_list(items);
                }
                GameEvent::WeaponRefineResult { result, item_id } => {
                    self.handle_weapon_refine_result(result, item_id);
                }
                GameEvent::ItemRefiningResult {
                    index,
                    refine,
                    result,
                } => {
                    self.handle_item_refining_result(index, refine, result);
                    self.game.character.refine.finish(result);
                }
                GameEvent::StylistOpened => {
                    self.game.character.stylist.open();
                    // every headgear the grid shows and every item a pick can cost
                    let data = &self.game.data_table;
                    let icons = data
                        .styling_shop
                        .lists
                        .values()
                        .flatten()
                        .flat_map(|goods| [Some(goods.goods), goods.item])
                        .flatten()
                        .filter_map(|id| data.item_icon_path(id))
                        .collect();
                    self.preload_item_icons(icons);
                }
                GameEvent::StylistResult { success } => {
                    self.game.character.stylist.finish(success);
                }
                GameEvent::RefineWindowOpened => {
                    self.game.character.refine.open();
                    self.game.character.inventory.open();
                }
                GameEvent::RefineMaterials {
                    index,
                    blessings,
                    materials,
                } => {
                    let icons = materials
                        .iter()
                        .map(|m| m.item_id)
                        .chain([nostalre_game::refine::BLACKSMITH_BLESSING])
                        .filter_map(|id| self.game.data_table.item_icon_path(id))
                        .collect();
                    self.preload_item_icons(icons);
                    self.game
                        .character
                        .refine
                        .set_materials(index, blessings, materials);
                }
                GameEvent::RefineAnnounced {
                    name,
                    item_id,
                    refine,
                    success,
                } => {
                    self.handle_refine_announced(&name, item_id, refine, success);
                }
                GameEvent::RepairItemList { items } => {
                    let target_aid = self
                        .game
                        .pending_casts
                        .pending_repair_target
                        .take()
                        .unwrap_or(0);
                    self.handle_repair_item_list(target_aid, items);
                }
                GameEvent::RepairItemResult { index, ok } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::REPAIR);
                    self.handle_repair_item_result(index, ok);
                }
                GameEvent::MakableItemList { item_ids } => {
                    self.handle_makable_item_list(item_ids);
                }
                GameEvent::CookingList { mk_type, item_ids } => {
                    self.handle_cooking_list(mk_type, item_ids);
                }
                GameEvent::MakingItemResult { result } => {
                    self.handle_making_item_result(result);
                }
                GameEvent::OpenVendingSetup { max_items } => {
                    self.handle_open_vending_setup(max_items);
                }
                GameEvent::VendingOwnStock { items } => {
                    self.handle_vending_own_stock(items);
                }
                GameEvent::VendingBoardShown { aid, name } => {
                    self.handle_vending_board_shown(aid, name);
                }
                GameEvent::VendingBoardHidden { aid } => {
                    self.handle_vending_board_hidden(aid);
                }
                GameEvent::VendingShopList {
                    aid,
                    unique_id,
                    items,
                } => {
                    self.handle_vending_shop_list(aid, unique_id, items);
                }
                GameEvent::VendingPurchaseResult {
                    index,
                    curcount,
                    result,
                } => {
                    self.handle_vending_purchase_result(index, curcount, result);
                }
                GameEvent::VendingStockDecrement { index, count } => {
                    self.handle_vending_stock_decrement(index, count);
                }
                GameEvent::VendingOpenResult { result } => {
                    self.handle_vending_open_result(result);
                }

                GameEvent::CharacterCreated { character } => {
                    self.handle_character_created(character);
                }
                GameEvent::CharacterCreateFailed { error_code } => {
                    tracing::warn!("character creation refused by server: code=0x{error_code:02x}");
                    if let Some(win) = &mut self.char_create_window {
                        win.error_message = Some(char_create_error_message(error_code).to_string());
                    }
                }
                GameEvent::CharacterDeleteReserved {
                    gid,
                    result,
                    delete_reserved_date,
                } => {
                    if let Some(win) = &mut self.char_select_window {
                        if result == 0 || result == 1 {
                            win.open_delete_dialog(gid, delete_reserved_date);
                        } else {
                            win.set_delete_status(char_delete_reserve_error(result).to_string());
                        }
                    }
                }
                GameEvent::CharacterDeleted { gid, result } => {
                    if let Some(win) = &mut self.char_select_window {
                        if result == 1 {
                            win.remove_character(gid);
                            win.close_delete_dialog();
                            self.account_anims.remove(&gid);
                        } else {
                            win.set_delete_dialog_error(
                                char_delete_confirm_error(result).to_string(),
                            );
                        }
                    }
                }
                GameEvent::CharacterDeleteCancelled { gid: _, result } => {
                    if let Some(win) = &mut self.char_select_window {
                        if result == 1 {
                            win.close_delete_dialog();
                        } else {
                            win.set_delete_dialog_error("Failed to cancel deletion.".to_string());
                        }
                    }
                }

                GameEvent::HomunPropertyReceived { property } => {
                    self.handle_homun_property(property);
                }
                GameEvent::CompanionStateChanged { state, gid, data } => {
                    self.handle_companion_state_changed(state, gid, data);
                }
                GameEvent::HomunFeedResult { success, item_id } => {
                    self.handle_homun_feed_result(success, item_id);
                }
                GameEvent::MercenaryInfoReceived { info, is_init } => {
                    self.handle_mercenary_info(info, is_init);
                }
                GameEvent::MercenaryParamChanged { var, value } => {
                    self.handle_mercenary_param_changed(var, value);
                }
                GameEvent::HomunParamChanged { var, value } => {
                    self.handle_homun_param_changed(var, value);
                }
                GameEvent::HomunSkillList { skills } => {
                    self.handle_homun_skill_list(skills);
                }
                GameEvent::HomunSkillUpdate {
                    skill,
                    level,
                    sp_cost,
                    attack_range,
                    upgradable,
                } => {
                    self.handle_homun_skill_update(skill, level, sp_cost, attack_range, upgradable);
                }
                GameEvent::MercenarySkillList { skills } => {
                    self.handle_mercenary_skill_list(skills);
                }

                GameEvent::PetCaptureStart => {
                    self.handle_pet_capture_start();
                }
                GameEvent::PetCaptureResult { ok } => {
                    self.handle_pet_capture_result(ok);
                }
                GameEvent::PetProperty { property } => {
                    self.handle_pet_property(property);
                }
                GameEvent::PetFeedResult { ok, food_item_id } => {
                    self.handle_pet_feed_result(ok, food_item_id);
                }
                GameEvent::PetStateChanged { ty, gid, data } => {
                    self.handle_pet_state_changed(ty, gid, data);
                }
                GameEvent::PetEggList { indices } => {
                    self.handle_pet_egg_list(indices);
                }
                GameEvent::PetAct { gid, data } => {
                    self.handle_pet_act(gid, data);
                }

                GameEvent::QuestListReceived { quests } => {
                    self.handle_quest_list_received(quests);
                }
                GameEvent::QuestAdded { quest } => {
                    self.handle_quest_added(quest);
                }
                GameEvent::QuestRemoved { quest_id } => {
                    self.handle_quest_removed(quest_id);
                }
                GameEvent::QuestHuntUpdated { entries } => {
                    self.handle_quest_hunt_updated(entries);
                }
                GameEvent::QuestActiveChanged { quest_id, active } => {
                    self.handle_quest_active_changed(quest_id, active);
                }
                GameEvent::QuestNpcMarker {
                    npc_id,
                    x,
                    y,
                    effect,
                    color,
                } => {
                    self.handle_quest_npc_marker(npc_id, x, y, effect, color);
                }
                GameEvent::MinimapMark {
                    id,
                    action,
                    x,
                    y,
                    color,
                } => {
                    let now = self.start_time.elapsed().as_secs_f32();
                    self.game.minimap_marks.apply(id, action, x, y, color, now);
                }
                GameEvent::CrimsonMarker { aid, x, y } => {
                    self.game.crimson_marks.apply(aid, x, y);
                }

                GameEvent::CoupleNameReceived { name } => {
                    self.game.character.partner_name = name;
                }
                GameEvent::WeddingCelebration { account_id } => {
                    self.handle_wedding_celebration(account_id);
                }
                GameEvent::MarriageProposed {
                    proposer_aid,
                    proposer_gid,
                    name,
                } => {
                    self.handle_marriage_proposed(proposer_aid, proposer_gid, name);
                }
                GameEvent::MarriageProposalArmed => {
                    self.game.pending_casts.marriage_targeting = true;
                }
                GameEvent::RequestMarriage { target_aid } => {
                    self.channel.send_packet(build_marry_request_packet(
                        target_aid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RespondMarriageProposal { accept } => {
                    if let Some((proposer_aid, proposer_gid)) =
                        self.game.pending_confirms.pending_marriage_proposal.take()
                    {
                        self.channel.send_packet(build_marry_reply_packet(
                            proposer_aid,
                            proposer_gid,
                            accept,
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::Divorced { name } => {
                    self.handle_divorced(name);
                }
                GameEvent::NpcCutin { image, position } => {
                    self.handle_npc_cutin(image, position);
                }
                GameEvent::AdoptionRequested {
                    father_aid,
                    mother_aid,
                    name,
                } => {
                    self.handle_adoption_requested(father_aid, mother_aid, name);
                }
                GameEvent::AdoptionMessage { msg_no } => {
                    self.handle_adoption_message(msg_no);
                }

                _ => {}
            }
        }
        self.game.world.entities.clear_just_spawned_flags();
    }
}

impl App {
    pub(crate) fn handle_ui_events(
        &mut self,
        events: Vec<GameEvent>,
        event_loop: &ActiveEventLoop,
    ) {
        nostalre_profiling::profile_function!();
        for event in events {
            match event {
                GameEvent::RequestLogin { username, password } => {
                    #[cfg(target_os = "horizon")]
                    {
                        let _ = (username, password);
                        self.open_local_profile();
                    }
                    #[cfg(not(target_os = "horizon"))]
                    {
                        self.sound_queue.ui(nostalre_game::sound::tables::ui::LOGIN);
                        self.config.keep_login_id = self.login_window.keep_id;
                        self.config.saved_username = if self.login_window.keep_id {
                            username.clone()
                        } else {
                            String::new()
                        };
                        self.config.save("config.json");
                        self.account_dialog.show_message("Please wait...");
                        let Some(server) =
                            self.config.login_servers.get(self.selected_login_server)
                        else {
                            continue;
                        };
                        let addr = format!("{}:{}", server.host, server.port);
                        self.channel.send_cmd(NetworkCommand::Connect {
                            addr,
                            expect_aid: false,
                        });
                        self.sound_queue
                            .ui(nostalre_game::sound::tables::ui::BUTTON);
                        self.channel.send_packet(build_login_packet(
                            &username,
                            &password,
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::SelectLoginServer { index } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    self.select_login_server(index);
                    self.login_server_list_window = None;
                    self.game.session.app_state = AppState::Login;
                }
                GameEvent::RequestSelectServer { index } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    if let Some(server_win) = &self.server_list_window
                        && let Some(server) = server_win.servers.get(index)
                    {
                        let addr = format!("{}:{}", ip_u32_to_string(server.ip), server.port);
                        self.channel.send_cmd(NetworkCommand::Disconnect);
                        self.channel.send_cmd(NetworkCommand::Connect {
                            addr: addr.clone(),
                            expect_aid: true,
                        });
                        if let Some(session) = &mut self.game.session.login_session {
                            session.char_server_addr = Some(addr);
                            session.world_name = server.name.clone();
                            self.channel.send_packet(build_char_enter_packet(session));
                            self.channel.send_cmd(NetworkCommand::SetKeepalive(
                                KeepaliveMode::CharServer {
                                    account_id: session.account_id,
                                },
                            ));
                        }
                    }
                }
                GameEvent::RequestSelectCharacter { slot } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    if let Some(char_win) = &self.char_select_window {
                        self.game.session.selected_character = char_win
                            .characters
                            .iter()
                            .find(|c| c.slot == slot as i8)
                            .cloned();
                    }
                    if self.config.last_char_slot != Some(slot) {
                        self.config.last_char_slot = Some(slot);
                        self.config.save("config.json");
                    }
                    self.channel
                        .send_packet(build_select_char_packet(slot, self.active_packetver));
                }
                GameEvent::RequestCreateCharacter { slot } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    let account_sex = self
                        .game
                        .session
                        .login_session
                        .as_ref()
                        .map_or(0, |s| s.sex);
                    let mut win = CharCreateWindow::new(slot, account_sex);
                    if let (Some(grf), Some(renderer)) = (&self.grf, &mut self.renderer) {
                        preload_window(&mut win, renderer, grf);
                    }
                    self.char_create_window = Some(win);
                    self.char_create_built_appearance = None;
                    self.game.session.app_state = AppState::CharacterCreate;
                }
                GameEvent::RequestMakeCharacter {
                    name,
                    slot,
                    hair_style,
                    hair_color,
                    job,
                    sex,
                } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    self.channel.send_packet(build_make_char_packet(
                        &name,
                        slot,
                        hair_style,
                        hair_color,
                        job,
                        sex,
                        self.active_packetver,
                    ));
                }
                GameEvent::CancelCreateCharacter => {
                    self.char_create_window = None;
                    self.game.session.app_state = AppState::CharacterSelect;
                }
                GameEvent::RequestDeleteCharacterReserve { gid } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    self.channel
                        .send_packet(build_delete_char_reserve_packet(gid, self.active_packetver));
                }
                GameEvent::RequestDeleteCharacterConfirm { gid, birthdate } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    self.channel.send_packet(build_delete_char_confirm_packet(
                        gid,
                        &birthdate,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestDeleteCharacterCancel { gid } => {
                    self.sound_queue
                        .ui(nostalre_game::sound::tables::ui::BUTTON);
                    self.channel
                        .send_packet(build_delete_char_cancel_packet(gid, self.active_packetver));
                }
                GameEvent::BackToServerSelect => {
                    // A local profile has no server chooser to return to.
                    if self.server_list_window.is_none() {
                        self.channel.send_cmd(NetworkCommand::Disconnect);
                        event_loop.exit();
                        return;
                    }
                    self.game.session.app_state = AppState::ServerSelect;
                    self.char_select_window = None;
                    self.char_create_window = None;
                    self.account_anims.clear();
                    self.windows.system_menu.open = false;
                    self.channel.send_cmd(NetworkCommand::Disconnect);
                }
                GameEvent::BackToLogin => {
                    self.game.session.app_state = AppState::Login;
                    self.server_list_window = None;
                    self.char_select_window = None;
                    self.char_create_window = None;
                    self.account_anims.clear();
                    self.game.session.login_session = None;
                    self.windows.system_menu.open = false;
                    self.channel.send_cmd(NetworkCommand::Disconnect);
                }
                // the app's hub is the character select there: leaving the game goes back to it, as Exit does
                GameEvent::BackToCharacterSelect
                    if cfg!(target_os = "android") && self.config.solo.is_some() =>
                {
                    self.windows.system_menu.open = false;
                    self.channel
                        .send_packet(build_req_disconnect_packet(self.active_packetver));
                }
                GameEvent::BackToCharacterSelect => {
                    self.windows.system_menu.open = false;
                    self.windows.map_missing_window.hide();
                    self.clear_companions();
                    self.channel
                        .send_packet(build_restart_packet(self.active_packetver));
                }
                GameEvent::ReturnToSavePoint => {
                    self.channel
                        .send_packet(build_return_savepoint_packet(self.active_packetver));
                }
                GameEvent::RequestStandingResurrection => {
                    self.channel
                        .send_packet(build_standing_resurrection_packet(self.active_packetver));
                }
                GameEvent::RequestMapRecoveryWarp => {
                    let char_name = self
                        .game
                        .session
                        .selected_character
                        .as_ref()
                        .map(|c| c.name.as_str())
                        .unwrap_or("Unknown");
                    let full_msg = format!("{char_name} : {}", self.config.map_recovery_command);
                    self.channel
                        .send_packet(build_chat_packet(&full_msg, self.active_packetver));
                }
                GameEvent::QuitGame => {
                    self.windows.system_menu.open = false;
                    self.channel
                        .send_packet(build_req_disconnect_packet(self.active_packetver));
                }
                GameEvent::RequestNpcContact { npc_id } => {
                    self.channel
                        .send_packet(build_contact_npc_packet(npc_id, self.active_packetver));
                }
                GameEvent::RequestNpcNext { npc_id } => {
                    self.channel
                        .send_packet(build_npc_next_packet(npc_id, self.active_packetver));
                }
                GameEvent::RequestNpcClose { npc_id } => {
                    self.game.npc_cutins = [None, None, None];
                    self.channel
                        .send_packet(build_npc_close_packet(npc_id, self.active_packetver));
                }
                GameEvent::RequestNpcMenuSelect { npc_id, choice } => {
                    if choice == 255 {
                        self.game.npc_cutins = [None, None, None];
                    }
                    self.channel.send_packet(build_npc_menu_select_packet(
                        npc_id,
                        choice,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestJoinChatRoom { room_id } => {
                    self.channel
                        .send_packet(build_req_enter_room_packet(room_id, self.active_packetver));
                }
                GameEvent::ToggleChatRoomCreate => {
                    self.windows.chat_room_create_window.toggle();
                }
                GameEvent::RequestCreateChatRoom {
                    title,
                    limit,
                    public,
                    password,
                } => {
                    self.game.pending_chat_room = Some((title.clone(), limit, public));
                    self.channel.send_packet(build_create_chatroom_packet(
                        &title,
                        limit,
                        public,
                        &password,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestChangeChatRoom {
                    title,
                    limit,
                    public,
                    password,
                } => {
                    self.channel.send_packet(build_change_chatroom_packet(
                        &title,
                        limit,
                        public,
                        &password,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestLeaveChatRoom => {
                    self.channel
                        .send_packet(build_exit_room_packet(self.active_packetver));
                    self.windows.chat_room_member_window.close();
                }
                GameEvent::RequestEditChatRoomSettings => {
                    let w = &self.windows.chat_room_member_window;
                    let (room_id, title, limit, public) = (
                        w.room_id(),
                        w.title().to_string(),
                        w.max_count(),
                        w.public(),
                    );
                    self.windows
                        .chat_room_create_window
                        .open_change(room_id, &title, limit, public);
                }
                GameEvent::RequestKickChatMember { name } => {
                    self.channel
                        .send_packet(build_expel_chat_member_packet(&name, self.active_packetver));
                }
                GameEvent::RequestChangeChatOwner { name } => {
                    self.channel
                        .send_packet(build_change_chat_owner_packet(&name, self.active_packetver));
                }
                GameEvent::RequestOpenChatMemberMenu { name, x, y } => {
                    use nostalre_ui_component::game::context_menu::{
                        ContextMenuAction, ContextMenuItem,
                    };
                    let items = vec![
                        ContextMenuItem {
                            label: "Hand Over Chat".to_string(),
                            action: ContextMenuAction::ChangeChatOwner { name: name.clone() },
                        },
                        ContextMenuItem {
                            label: "Kick".to_string(),
                            action: ContextMenuAction::KickFromChatRoom { name },
                        },
                    ];
                    self.windows.context_menu.open_at(x, y, items);
                }
                GameEvent::RequestSelectWarppoint { skill, map_name } => {
                    self.channel.send_packet(build_select_warppoint_packet(
                        skill,
                        &map_name,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestNpcInputNumber { npc_id, value } => {
                    self.channel.send_packet(build_npc_input_number_packet(
                        npc_id,
                        value,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestNpcInputString { npc_id, text } => {
                    self.channel.send_packet(build_npc_input_string_packet(
                        npc_id,
                        &text,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestNpcDealType { npc_id, deal_type } => {
                    self.channel.send_packet(build_npc_deal_type_packet(
                        npc_id,
                        deal_type,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestNpcShopBuy { items } => {
                    self.channel.send_packet(build_purchase_item_list_packet(
                        &items,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestNpcShopSell { items } => {
                    self.channel
                        .send_packet(build_sell_item_list_packet(&items, self.active_packetver));
                }
                GameEvent::RequestNpcBarterBuy { extended, items } => {
                    self.channel.send_packet(build_barter_purchase_packet(
                        extended,
                        &items,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestNpcMarketBuy { items } => {
                    self.channel
                        .send_packet(build_market_purchase_packet(&items, self.active_packetver));
                }
                GameEvent::RequestNpcShopClose => self.close_npc_shop(),
                GameEvent::ShowItemInfo { index } => {
                    let producers = &self.game.character.char_names;
                    if let Some(item) = self.game.character.inventory.get_item(index) {
                        let is_book = self.item_is_book(item.item_id);
                        self.windows.item_info_window.show(
                            item,
                            &self.game.data_table,
                            producers,
                            is_book,
                        );
                        let tex_paths = self.windows.item_info_window.pending_texture_paths();
                        self.preload_item_icons(tex_paths);
                    }
                }
                // a navigation link in NPC text, as the server's guide
                GameEvent::NavigationGuide { navigation } => {
                    self.handle_navigation_guide(navigation)
                }
                GameEvent::ShowItemInfoDirect { item } => {
                    let is_book = self.item_is_book(item.item_id);
                    self.windows.item_info_window.show(
                        &item,
                        &self.game.data_table,
                        &self.game.character.char_names,
                        is_book,
                    );
                    let tex_paths = self.windows.item_info_window.pending_texture_paths();
                    self.preload_item_icons(tex_paths);
                }
                GameEvent::ShowCardInfo { item_id } => {
                    self.windows
                        .item_info_window
                        .show_card(item_id, &self.game.data_table);
                    let tex_paths = self.windows.item_info_window.pending_card_texture_paths();
                    self.preload_item_icons(tex_paths);
                }
                GameEvent::ShowCardIllustration { item_id } => {
                    let name = self.item_name(item_id);
                    let illust_path = self
                        .game
                        .data_table
                        .card_illustration
                        .as_ref()
                        .and_then(|t| t.illustration_path(item_id));
                    if let Some(path) = illust_path {
                        self.windows
                            .item_info_window
                            .show_illustration(item_id, name, path);
                        let tex_paths = self
                            .windows
                            .item_info_window
                            .pending_illustration_texture_paths();
                        self.preload_item_icons(tex_paths);
                    }
                }
                GameEvent::ReadBook { item_id } => {
                    if let Some(grf) = self.grf.as_ref()
                        && let Ok(data) = grf.read_file(&nostalre_resources::table::book(item_id))
                    {
                        let content = nostalre_game::book::BookContent::parse(&data);
                        self.windows.book_window.show(content);
                        self.windows.item_info_window.close();
                    }
                }
                GameEvent::RequestUseItem { index } => {
                    if self.player_hidden() {
                        continue;
                    }
                    let account_id = self
                        .game
                        .session
                        .login_session
                        .as_ref()
                        .map(|s| s.account_id)
                        .unwrap_or(0);
                    self.channel.send_packet(build_use_item_packet(
                        index,
                        account_id,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestEquipItem { index, location } => {
                    self.channel.send_packet(build_equip_item_packet(
                        index,
                        location,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestUnequipItem { index } => {
                    self.channel
                        .send_packet(build_unequip_item_packet(index, self.active_packetver));
                }
                GameEvent::RequestDropItem { index, count } => {
                    self.channel.send_packet(build_drop_item_packet(
                        index,
                        count,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestRemoveOption => {
                    self.channel
                        .send_packet(build_remove_option_packet(self.active_packetver));
                }
                GameEvent::RequestEquipSwitchAdd { index, location } => {
                    self.request_equip_switch_add(index, location);
                }
                GameEvent::RequestEquipSwitchRemove { index } => {
                    self.request_equip_switch_remove(index);
                }
                GameEvent::RequestEquipSwitch => self.request_equip_switch(),
                GameEvent::RequestEquipSwitchSingle { index } => {
                    self.request_equip_switch_single(index);
                }
                GameEvent::ApplyDamageIndicator { skin, direction } => {
                    self.apply_damage_indicator(skin, direction);
                }
                GameEvent::RequestChangeTitle { title_id } => {
                    self.channel
                        .send_packet(nostalre_network::build_change_title_packet(
                            title_id,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestUnequipAll { location } => {
                    self.channel
                        .send_packet(nostalre_network::build_unequip_all_packet(
                            location,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestMoveItemBodyToCart { index, count } => {
                    self.channel
                        .send_packet(build_move_item_body_to_cart_packet(
                            index,
                            count,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestMoveItemCartToBody { index, count } => {
                    self.channel
                        .send_packet(build_move_item_cart_to_body_packet(
                            index,
                            count,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestMoveItemStoreToCart { index, count } => {
                    self.channel
                        .send_packet(build_move_item_store_to_cart_packet(
                            index,
                            count,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestMoveItemCartToStore { index, count } => {
                    self.channel
                        .send_packet(build_move_item_cart_to_store_packet(
                            index,
                            count,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestMoveItemBodyToStore { index, count } => {
                    self.channel
                        .send_packet(build_move_item_body_to_store_packet(
                            index,
                            count,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestMoveItemStoreToBody { index, count } => {
                    self.channel
                        .send_packet(build_move_item_store_to_body_packet(
                            index,
                            count,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestCloseStorage => {
                    self.channel
                        .send_packet(build_close_store_packet(self.active_packetver));
                }
                GameEvent::RequestExchangeItem { target_aid } => {
                    let name = self.name_of(target_aid).unwrap_or_default();
                    self.game.pending_confirms.pending_trade_partner = Some((target_aid, name));
                    self.channel.send_packet(build_req_exchange_item_packet(
                        target_aid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RespondExchangeRequest { accept } => {
                    self.respond_exchange_request(accept);
                }
                GameEvent::RequestAddExchangeItem { index, count } => {
                    self.channel.send_packet(build_add_exchange_item_packet(
                        index,
                        count,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestConcludeExchange => {
                    self.channel
                        .send_packet(build_conclude_exchange_item_packet(self.active_packetver));
                }
                GameEvent::RequestCancelExchange => {
                    self.channel
                        .send_packet(build_cancel_exchange_item_packet(self.active_packetver));
                }
                GameEvent::RequestExecExchange => {
                    self.channel
                        .send_packet(build_exec_exchange_item_packet(self.active_packetver));
                }
                GameEvent::RequestMailList => {
                    self.channel
                        .send_packet(build_mail_get_list_packet(self.active_packetver));
                }
                GameEvent::RequestMailOpen { mail_id } => {
                    self.channel
                        .send_packet(build_mail_open_packet(mail_id, self.active_packetver));
                }
                GameEvent::RequestMailDelete { mail_id } => {
                    self.channel
                        .send_packet(build_mail_delete_packet(mail_id, self.active_packetver));
                }
                GameEvent::RequestMailGetItem { mail_id } => {
                    self.channel
                        .send_packet(build_mail_get_item_packet(mail_id, self.active_packetver));
                }
                GameEvent::RequestMailResetItem { ty } => {
                    self.channel
                        .send_packet(build_mail_reset_item_packet(ty, self.active_packetver));
                }
                GameEvent::RequestMailAddItem { index, amount } => {
                    self.channel.send_packet(build_mail_add_item_packet(
                        index,
                        amount,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestMailSend { to, title, body } => {
                    self.channel.send_packet(build_mail_send_packet(
                        &to,
                        &title,
                        &body,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestMailReturn { mail_id, sender } => {
                    self.channel.send_packet(build_req_mail_return_packet(
                        mail_id,
                        &sender,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestCartOff => {
                    self.channel
                        .send_packet(build_cartoff_packet(self.active_packetver));
                }
                GameEvent::RequestChangeCart { num } => {
                    self.channel
                        .send_packet(build_change_cart_packet(num, self.active_packetver));
                }
                GameEvent::RequestSelectCart { identity, cart } => {
                    self.channel
                        .send_packet(nostalre_network::build_select_cart_packet(
                            identity,
                            cart,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestSetCartPick { .. } => {}
                GameEvent::ToggleCart => {
                    self.game.character.cart.toggle();
                }
                GameEvent::RequestSkillLevelUp { skill } => {
                    self.channel
                        .send_packet(build_upgrade_skill_packet(skill, self.active_packetver));
                }
                GameEvent::RequestStatChange { status_id, amount } => {
                    self.channel.send_packet(build_stat_change_packet(
                        status_id,
                        amount,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestTraitStatChange { status_id, amount } => {
                    self.channel.send_packet(build_trait_stat_change_packet(
                        status_id,
                        amount,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestCompanionMove { gid, x, y } => {
                    self.channel.send_packet(build_companion_move_packet(
                        gid,
                        x as u16,
                        y as u16,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestCompanionAttack { gid, target_gid } => {
                    self.channel.send_packet(build_companion_attack_packet(
                        gid,
                        target_gid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestCompanionMoveToOwner { gid } => {
                    self.channel
                        .send_packet(build_companion_move_to_owner_packet(
                            gid,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestSetConfig { kind, enabled } => {
                    self.channel.send_packet(build_config_packet(
                        kind.config_id(),
                        enabled as i32,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestHomunMenu { command } => {
                    self.channel.send_packet(build_homun_menu_packet(
                        command as i8,
                        self.active_packetver,
                    ));
                    if command == 2 {
                        self.clear_homunculus();
                    }
                }
                GameEvent::RequestHomunRest => {
                    if !self.skill_on_cooldown(SkillEnum::AmRest) {
                        let target_id = self.game.world.entities.player_id().unwrap_or(0);
                        self.channel.send_packet(build_use_skill_packet(
                            SkillEnum::AmRest,
                            1,
                            target_id,
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::RequestHomunDelete => {
                    let name = self
                        .game
                        .companions
                        .homunculus
                        .as_ref()
                        .map(|h| h.name.clone())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "your homunculus".to_string());
                    self.game.arm_confirm(
                        &mut self.windows,
                        &format!("Delete {name} permanently?"),
                        |accept| accept.then_some(GameEvent::RequestHomunMenu { command: 2 }),
                    );
                }
                GameEvent::RequestMercenaryCommand { command } => {
                    self.channel.send_packet(build_mercenary_command_packet(
                        command,
                        self.active_packetver,
                    ));
                    if command == 2 {
                        self.clear_mercenary();
                    }
                }
                GameEvent::RequestRenameHomun { name } => {
                    self.channel
                        .send_packet(build_rename_homun_packet(&name, self.active_packetver));
                }
                GameEvent::ToggleHomunculusWindow => {
                    self.windows.homunculus_window.toggle();
                }
                GameEvent::ToggleMercenaryWindow => {
                    self.windows.mercenary_window.toggle();
                }
                GameEvent::ToggleMercenarySkillWindow => {
                    self.windows.mercenary_skill_window.toggle();
                }
                GameEvent::ToggleHomunSkillWindow => {
                    self.windows.homun_skill_window.toggle();
                }
                GameEvent::ToggleCompanionAiConfig => {
                    self.windows.companion_ai_config_window.toggle();
                }
                GameEvent::SaveCompanionAiConfig => {
                    if let Err(e) = self
                        .game
                        .companions
                        .companion_ai
                        .save(crate::game_state::COMPANION_AI_CONFIG_PATH)
                    {
                        tracing::warn!("failed to save companion AI config: {e}");
                    }
                }
                GameEvent::RevertCompanionAiConfig => {
                    self.game.companions.companion_ai =
                        nostalre_ai::config::CompanionAiConfig::load_or_default(
                            crate::game_state::COMPANION_AI_CONFIG_PATH,
                        );
                }
                GameEvent::ResetCompanionAiConfig => {
                    self.game.companions.companion_ai =
                        nostalre_ai::config::CompanionAiConfig::default();
                }
                GameEvent::ToggleCompanionStandby { is_mercenary } => {
                    self.push_owner_command_to(
                        is_mercenary,
                        nostalre_game::companion::OwnerCommand::follow(),
                        false,
                    );
                }
                GameEvent::ToggleCompanionPatrol { is_mercenary } => {
                    let patrolling = self.companion_is_patrolling(is_mercenary);
                    let armed = self.game.pending_casts.pending_companion_patrol.is_some();
                    self.game.pending_casts.pending_companion_patrol = None;
                    if patrolling {
                        self.push_owner_command_to(
                            is_mercenary,
                            nostalre_game::companion::OwnerCommand::stop(),
                            false,
                        );
                    } else if !armed {
                        self.game.pending_casts.pending_companion_patrol = Some(is_mercenary);
                    }
                }
                GameEvent::HotkeyListReceived { slots } => {
                    self.game.character.hotkeys.set_from_server(&slots);
                }
                GameEvent::RequestHotkeyChange {
                    index,
                    is_skill,
                    id,
                    count,
                } => {
                    let is_skill_i8 = if is_skill { 1i8 } else { 0i8 };
                    self.channel.send_packet(build_shortcut_key_change_packet(
                        index,
                        is_skill_i8,
                        id,
                        count,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestUseSkill { skill, level } => {
                    self.handle_request_use_skill(skill, level);
                }
                GameEvent::RequestCompanionUseSkill {
                    is_mercenary,
                    skill,
                    level,
                } => {
                    self.handle_request_companion_use_skill(is_mercenary, skill, level);
                }
                GameEvent::RequestPickupItem { id } => {
                    self.channel
                        .send_packet(build_pickup_item_packet(id, self.active_packetver));
                }
                GameEvent::RequestRefineSelect { index } => {
                    self.channel.send_packet(build_refining_select_item_packet(
                        index,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestRefine {
                    index,
                    material,
                    blessing,
                } => {
                    self.channel.send_packet(build_req_refining_packet(
                        index,
                        material,
                        blessing,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestStylistBuy { choice } => {
                    self.channel
                        .send_packet(build_stylist_buy_packet(&choice, self.active_packetver));
                }
                GameEvent::RequestStylistClose => {
                    self.game.character.stylist.close();
                    self.channel
                        .send_packet(build_stylist_close_packet(self.active_packetver));
                }
                GameEvent::RequestRefineClose => {
                    self.channel
                        .send_packet(build_close_refining_ui_packet(self.active_packetver));
                }
                GameEvent::RequestCardInsertList { card_index } => {
                    self.game.pending_casts.pending_card_composition_index = Some(card_index);
                    self.channel.send_packet(build_card_composition_list_packet(
                        card_index,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestCardInsert {
                    card_index,
                    equip_index,
                } => {
                    self.channel.send_packet(build_card_composition_packet(
                        card_index,
                        equip_index,
                        self.active_packetver,
                    ));
                    self.game.pending_casts.pending_card_composition_index = None;
                }
                GameEvent::RequestIdentifyItem { index } => {
                    self.channel
                        .send_packet(build_req_itemidentify_packet(index, self.active_packetver));
                }
                GameEvent::RequestMakingArrow { item_id } => {
                    self.channel
                        .send_packet(build_req_makingarrow_packet(item_id, self.active_packetver));
                }
                GameEvent::RequestCooking { mk_type, item_id } => {
                    self.channel.send_packet(build_req_cooking_packet(
                        mk_type,
                        item_id,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestMakingItem { item_id, materials } => {
                    self.channel.send_packet(build_req_makingitem_packet(
                        item_id,
                        materials,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestWeaponRefine { index } => {
                    self.channel
                        .send_packet(build_req_weaponrefine_packet(index, self.active_packetver));
                }
                GameEvent::RequestRepairItem {
                    index,
                    item_id,
                    refine,
                    cards,
                } => {
                    self.channel.send_packet(build_req_itemrepair_packet(
                        index,
                        item_id,
                        refine,
                        cards,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestSelectAutoSpell { skill } => {
                    self.channel
                        .send_packet(build_select_autospell_packet(skill, self.active_packetver));
                }
                GameEvent::RequestItemListWindow {
                    kind,
                    confirmed,
                    items,
                } => {
                    self.channel
                        .send_packet(nostalre_network::build_itemlistwin_res_packet(
                            kind,
                            confirmed,
                            &items,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestSelectSkill { why, skill } => {
                    self.channel.send_packet(build_skill_select_response_packet(
                        why,
                        skill,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestOpenStore { shop_name, items } => {
                    self.channel.send_packet(build_req_openstore2_packet(
                        &shop_name,
                        &items,
                        self.active_packetver,
                    ));
                    self.game.pending_casts.pending_shop_name = Some(shop_name);
                }
                GameEvent::RequestCancelVendingSetup => {
                    self.channel
                        .send_packet(build_req_cancel_openstore_packet(self.active_packetver));
                }
                GameEvent::RequestCloseStore => {
                    self.close_own_shop();
                }
                GameEvent::RequestBuyFromVendor { aid } => {
                    self.channel
                        .send_packet(build_req_buy_frommc_packet(aid, self.active_packetver));
                }
                GameEvent::RequestPurchaseFromVendor {
                    aid,
                    unique_id,
                    items,
                } => {
                    self.channel.send_packet(build_purchase_frommc2_packet(
                        aid,
                        unique_id,
                        &items,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestSendChat { message } => {
                    self.run_chat_command(&message);
                }
                GameEvent::RequestSendWhisper { name, message } => {
                    if !self.admit_chat_line(&message) {
                        continue;
                    }
                    self.channel.send_packet(build_whisper_packet(
                        &name,
                        &message,
                        self.active_packetver,
                    ));
                    self.game.pending_confirms.pending_whisper_out = Some((name, message));
                }
                GameEvent::ToggleShortcutList => {
                    if !self.windows.shortcut_list_window.is_open() {
                        self.windows
                            .shortcut_list_window
                            .set_bindings(&self.config.shortcut_commands);
                    }
                    self.windows.shortcut_list_window.toggle();
                }
                GameEvent::ShortcutBindingsChanged(commands) => {
                    self.config.shortcut_commands = commands;
                    self.save_config_soon();
                }
                GameEvent::ShowQuestWindow => {self.windows.quest_window.open();}
                GameEvent::ToggleQuestWindow => {
                    self.windows.quest_window.toggle();
                }
                GameEvent::OpenQuestDetail { quest_id } => {
                    self.game.quest_log.follow(quest_id);
                    if let Some(character)=&self.game.session.selected_character {
                        let path=format!("quest-follow-pre-{}.txt",character.gid);
                        let _=std::fs::write(path,quest_id.to_string());
                    }
                    self.windows.quest_detail_window.open(quest_id);
                }
                GameEvent::RequestToggleQuestActive { quest_id, active } => {
                    self.channel
                        .send_packet(nostalre_network::build_active_quest_packet(
                            quest_id,
                            active,
                            self.active_packetver,
                        ));
                }
                GameEvent::Disconnected(ref reason) if reason == "User exit" => {
                    self.channel.send_cmd(NetworkCommand::Disconnect);
                    event_loop.exit();
                }
                GameEvent::ToggleInventory => {
                    self.game.character.inventory.toggle();
                }
                GameEvent::ToggleEquipment => {
                    self.windows.equipment_window.toggle();
                }
                GameEvent::ToggleEquipmentProperties => {
                    self.windows.equipment_properties_window.toggle();
                }
                GameEvent::ToggleSkills => {
                    self.game.character.skills.toggle();
                }
                GameEvent::ToggleEmotionWindow => {
                    self.windows.emotion_window.toggle();
                }
                GameEvent::RequestEmotion { emote_type } => {
                    self.channel
                        .send_packet(build_emotion_packet(emote_type, self.active_packetver));
                }
                GameEvent::ToggleStatusWindow => {
                    self.windows.status_window.toggle();
                }
                GameEvent::RunHotkeyAction { action } => {
                    self.dispatch_action(action);
                }
                GameEvent::OpenWindow(window) => {
                    if !self.window_shown(&window) {
                        self.handle_ui_events(vec![*window], event_loop);
                    }
                }
                GameEvent::ToggleMinimap => {
                    self.windows.minimap_window.cycle_visibility();
                }
                GameEvent::ToggleSoundOptions => {
                    self.open_sound_options();
                }
                GameEvent::SoundSettingsChanged { settings, persist } => {
                    self.apply_sound_settings(settings, persist);
                }
                GameEvent::ToggleGraphicOptions => {
                    self.open_graphic_options();
                }
                GameEvent::ToggleSystemMenu => {
                    self.windows.system_menu.open = !self.windows.system_menu.open;
                }
                GameEvent::HudLayoutChanged { .. } => {
                    self.config.hud_layout = crate::config::HudLayout::Modern;
                    self.windows.hud.layout = crate::config::HudLayout::Modern;
                    self.save_config_soon();
                }
                GameEvent::GraphicsSettingsChanged { settings, persist } => {
                    self.apply_graphics_settings(settings, persist);
                }
                GameEvent::KeyBindingsChanged(keys) => self.apply_key_bindings(*keys),
                GameEvent::ToggleHotkeyConfig => {
                    if !self.windows.hotkey_config_window.is_open() {
                        self.windows
                            .hotkey_config_window
                            .set_bindings(&self.config.keybindings, &self.config.emotion_keys);
                    }
                    self.windows.hotkey_config_window.toggle();
                }
                GameEvent::TogglePartyWindow => {
                    self.windows.party_friends_window.open_party_tab();
                }
                GameEvent::ToggleFriendWindow => {
                    self.windows.party_friends_window.open_friend_tab();
                }
                GameEvent::RequestPartyInvite { target_aid } => {
                    let pv = self.active_packetver;
                    if self.game.party.is_none() {
                        // The party is created asynchronously server-side, so the invite must
                        // wait for the create ack — sending it now would be dropped.
                        let party_name = self
                            .game
                            .session
                            .selected_character
                            .as_ref()
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| "Party".to_string());
                        self.channel
                            .send_packet(build_make_party_packet(&party_name, pv));
                        self.game.pending_confirms.pending_invite_aid = Some(target_aid);
                    } else {
                        self.channel
                            .send_packet(build_req_join_party_packet(target_aid, pv));
                    }
                }
                GameEvent::RespondPartyInvite { party_grid, accept } => {
                    self.channel.send_packet(build_join_party_reply_packet(
                        party_grid,
                        accept,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestAdoption { target_aid } => {
                    self.channel.send_packet(build_adopt_request_packet(
                        target_aid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RespondAdoptionRequest { accept } => {
                    if let Some((father_aid, mother_aid)) =
                        self.game.pending_confirms.pending_adopt_request.take()
                    {
                        self.channel.send_packet(build_adopt_reply_packet(
                            father_aid,
                            mother_aid,
                            accept,
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::RespondGuildInvite { gdid, accept } => {
                    self.channel.send_packet(build_ans_join_guild(
                        gdid,
                        accept,
                        self.active_packetver,
                    ));
                }
                GameEvent::RespondGuildAlly { aid, accept } => {
                    self.channel
                        .send_packet(build_ally_guild(aid, accept, self.active_packetver));
                }
                GameEvent::RequestLeaveParty => {
                    self.channel
                        .send_packet(build_leave_party_packet(self.active_packetver));
                }
                GameEvent::RequestExpelMember { aid, name } => {
                    self.channel.send_packet(build_expel_party_member_packet(
                        aid,
                        &name,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestPartyExpOption { exp_share } => {
                    let (pickup, division) = self
                        .game
                        .party
                        .as_ref()
                        .map(|p| (p.item_pickup_rule, p.item_division_rule))
                        .unwrap_or_default();
                    self.channel.send_packet(build_change_party_option_packet(
                        exp_share as u32,
                        pickup,
                        division,
                        self.active_packetver,
                    ));
                }
                GameEvent::SendPartyChat { message } => {
                    self.channel
                        .send_packet(build_party_chat_packet(&message, self.active_packetver));
                }
                GameEvent::ShowPartyHelper { mode } => {
                    let local_aid = self
                        .game
                        .session
                        .login_session
                        .as_ref()
                        .map(|s| s.account_id)
                        .unwrap_or(0);
                    let is_leader = self
                        .game
                        .party
                        .as_ref()
                        .and_then(|p| p.leader_aid())
                        .map(|aid| aid == local_aid)
                        .unwrap_or(false);
                    let (exp, pickup, division) = self
                        .game
                        .party
                        .as_ref()
                        .map(|p| (p.exp_share, p.item_pickup_rule, p.item_division_rule))
                        .unwrap_or((false, 0, 0));
                    let editable = mode == MODE_CREATE || is_leader;
                    self.windows
                        .party_helper_window
                        .open(mode, exp, pickup, division, editable);
                }
                GameEvent::RequestPartyCreate {
                    name,
                    item_pickup_rule,
                    item_division_rule,
                } => {
                    self.channel.send_packet(build_make_party2_packet(
                        &name,
                        item_pickup_rule,
                        item_division_rule,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestPartyInviteByName { name } => {
                    self.channel.send_packet(build_party_invite_by_name_packet(
                        &name,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestChangePartyLeader { aid } => {
                    self.channel
                        .send_packet(build_change_party_leader_packet(aid, self.active_packetver));
                }
                GameEvent::RequestGuildInfoBurst => {
                    let pv = self.active_packetver;
                    self.channel.send_packet(build_req_guild_menuinterface(pv));
                    for atype in 0..=4 {
                        self.channel.send_packet(build_req_guild_menu(atype, pv));
                    }
                }
                GameEvent::RequestGuildMenu { atype } => {
                    self.channel
                        .send_packet(build_req_guild_menu(atype, self.active_packetver));
                }
                GameEvent::ShowGuildMemberMenu {
                    aid,
                    gid,
                    name,
                    x,
                    y,
                } => {
                    self.handle_show_guild_member_menu(aid, gid, name, x, y);
                }
                GameEvent::RequestSetGuildNotice { subject, body } => {
                    if let Some(gdid) = self.game.guild.as_ref().map(|g| g.gdid) {
                        self.channel.send_packet(build_guild_notice(
                            gdid,
                            &subject,
                            &body,
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::RequestGuildLeave => {
                    let name = self
                        .game
                        .guild
                        .as_ref()
                        .map(|g| g.name.clone())
                        .filter(|n| !n.is_empty())
                        .unwrap_or_else(|| "the guild".to_string());
                    self.game
                        .arm_confirm(&mut self.windows, &format!("Leave {name}?"), |accept| {
                            accept.then_some(GameEvent::ConfirmedGuildLeave)
                        });
                }
                GameEvent::ConfirmedSkillTalkbox {
                    skill,
                    level,
                    x,
                    y,
                    message,
                } => {
                    self.channel.send_packet(
                        nostalre_network::build_use_skill_to_ground_with_talkbox_packet(
                            skill,
                            level,
                            x,
                            y,
                            &message,
                            self.active_packetver,
                        ),
                    );
                }
                GameEvent::RequestGuildExpel { aid, gid, name } => {
                    self.windows.guild_expel_dialog = Some(GuildExpelDialog::new(aid, gid, name));
                }
                GameEvent::ConfirmedGuildLeave => {
                    if let Some(g) = &self.game.guild {
                        let aid = self
                            .game
                            .session
                            .login_session
                            .as_ref()
                            .map(|s| s.account_id)
                            .unwrap_or(0) as i32;
                        self.channel.send_packet(build_req_leave_guild(
                            g.gdid,
                            aid,
                            aid,
                            "",
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::ConfirmedGuildExpel {
                    aid,
                    gid,
                    name: _,
                    reason,
                } => {
                    if let Some(gdid) = self.game.guild.as_ref().map(|g| g.gdid) {
                        self.channel.send_packet(build_req_ban_guild(
                            gdid,
                            aid as i32,
                            gid as i32,
                            &reason,
                            self.active_packetver,
                        ));
                    }
                }
                GameEvent::RequestChangeMemberPosition {
                    aid,
                    gid,
                    position_id,
                } => {
                    self.channel.send_packet(build_req_change_memberpos(
                        aid as i32,
                        gid as i32,
                        position_id,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestChangePositionInfo { positions } => {
                    self.channel
                        .send_packet(build_reg_change_guild_positioninfo(
                            &positions,
                            self.active_packetver,
                        ));
                }
                GameEvent::RequestUpgradeGuildSkill { skill } => {
                    self.channel
                        .send_packet(build_upgrade_skill_packet(skill, self.active_packetver));
                }
                GameEvent::RequestGuildInvite { target_aid } => {
                    let (my_aid, my_gid) = self.local_aid_gid();
                    self.channel.send_packet(build_req_join_guild(
                        target_aid,
                        my_aid,
                        my_gid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestGuildAlly { target_aid } => {
                    let (my_aid, my_gid) = self.local_aid_gid();
                    self.channel.send_packet(build_req_ally_guild(
                        target_aid,
                        my_aid,
                        my_gid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestGuildHostile { target_aid } => {
                    self.channel
                        .send_packet(build_req_hostile_guild(target_aid, self.active_packetver));
                }
                GameEvent::RequestDeleteGuildRelation { gdid, relation } => {
                    let msg = if relation == 0 {
                        "Cancel this alliance?"
                    } else {
                        "Cancel this antagonist declaration?"
                    };
                    self.game
                        .arm_confirm(&mut self.windows, msg, move |accept| {
                            accept.then_some(GameEvent::ConfirmedDeleteGuildRelation {
                                gdid,
                                relation,
                            })
                        });
                }
                GameEvent::ConfirmedDeleteGuildRelation { gdid, relation } => {
                    self.channel.send_packet(build_req_delete_related_guild(
                        gdid,
                        relation,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestSelectEmblem => {
                    self.open_emblem_picker();
                }
                GameEvent::RequestUploadEmblem { path } => {
                    self.upload_emblem_file(&path);
                }
                GameEvent::RequestWorldMapTexture { path } => {
                    let loaded = match (&self.grf, &mut self.renderer) {
                        (Some(grf), Some(renderer)) => {
                            renderer.preload_textures(&[path.as_str()], grf)
                        }
                        _ => false,
                    };
                    self.windows.world_map_window.texture_loaded(&path, loaded);
                }
                GameEvent::RequestAddFriend { name } => {
                    self.channel
                        .send_packet(build_add_friend_packet(&name, self.active_packetver));
                }
                GameEvent::RequestDeleteFriend { aid, gid } => {
                    self.channel.send_packet(build_delete_friend_packet(
                        aid,
                        gid,
                        self.active_packetver,
                    ));
                }
                GameEvent::RespondFriendRequest {
                    req_aid,
                    req_gid,
                    accept,
                } => {
                    self.channel.send_packet(build_ack_add_friend_packet(
                        req_aid,
                        req_gid,
                        accept,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestWhisper { name } => {
                    self.windows.chat_window.start_whisper(name);
                }

                GameEvent::RequestTryCapture { gid } => {
                    self.channel
                        .send_packet(build_trycapture_packet(gid, self.active_packetver));
                }
                GameEvent::RequestPetCommand { csub } => {
                    self.handle_request_pet_command(csub);
                }
                GameEvent::RequestRenamePet { name } => {
                    self.channel
                        .send_packet(build_rename_pet_packet(&name, self.active_packetver));
                }
                GameEvent::RequestSelectPetEgg { index } => {
                    self.channel
                        .send_packet(build_select_petegg_packet(index, self.active_packetver));
                    self.game.companions.pet.egg_index = Some(index);
                    self.game.character.inventory.set_item_damaged(index, true);
                }
                GameEvent::RequestPetAct { data } => {
                    self.channel
                        .send_packet(build_pet_act_packet(data, self.active_packetver));
                }
                GameEvent::RequestMannerPoint {
                    target_aid,
                    positive,
                } => {
                    if positive {
                        self.channel.send_packet(build_give_manner_point_packet(
                            target_aid,
                            true,
                            MANNER_POINT_STEP,
                            self.active_packetver,
                        ));
                    } else {
                        let name = self
                            .name_of(target_aid)
                            .unwrap_or_else(|| "this player".to_string());
                        self.game.arm_confirm(
                            &mut self.windows,
                            &format!("Take manner points from {name}? This cannot be undone."),
                            move |accept| {
                                accept.then_some(GameEvent::GiveMannerPoint {
                                    target_aid,
                                    positive: false,
                                })
                            },
                        );
                    }
                }
                GameEvent::GiveMannerPoint {
                    target_aid,
                    positive,
                } => {
                    self.channel.send_packet(build_give_manner_point_packet(
                        target_aid,
                        positive,
                        MANNER_POINT_STEP,
                        self.active_packetver,
                    ));
                }
                GameEvent::RequestAccountName { aid } => {
                    self.channel
                        .send_packet(build_account_name_packet(aid, self.active_packetver));
                }
                GameEvent::RequestPetFeed => {
                    self.game.arm_confirm(
                        &mut self.windows,
                        "Are you sure you want to feed your pet?",
                        |accept| accept.then_some(GameEvent::RequestPetCommand { csub: 1 }),
                    );
                }
                GameEvent::TogglePetWindow => {
                    self.windows.pet_window.toggle();
                }
                _ => {}
            }
        }
    }
}
