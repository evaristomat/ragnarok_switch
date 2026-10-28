use crate::game_state::{GameState, TOKEN_OF_SIEGFRIED};
use crate::ui::escape::{EscapeGame, modal_besides_menu, route_escape};
use crate::ui::windows::{Dispatch, REGISTRY, Windows};
use nostalre_game::boss_info::BossMark;
use nostalre_game::character::Character;
use nostalre_game::cursor::RenderEntry;
use nostalre_game::effect::casting_skill;
use nostalre_game::entity::{Entity, EntityCategory, EntityState};
use nostalre_game::event::GameEvent;
use nostalre_game::guild::Guild;
use nostalre_game::keybinding::HotkeyAction;
use nostalre_game::minimap_mark::{CrimsonMarks, MinimapMarks};
use nostalre_game::navigation::Navigation;
use nostalre_game::party::Party;
use nostalre_game::quest::QuestMarker;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui_component::game::basic_info_window::BASIC_INFO_WINDOW_ID;
use nostalre_ui_component::game::drop_quantity_dialog::DropQuantityDialog;
use nostalre_ui_component::game::hotkey_bar::{DOCK_W, HOTKEY_BAR_WINDOW_ID};
use nostalre_ui_component::game::inventory_window::INV_WINDOW_ID;
use nostalre_ui_component::game::levelup_notification_window::LevelUpClick;
use nostalre_ui_component::game::minimap_window::{MarkerType, MinimapMarker, quest_marker_color};
use nostalre_ui_component::game::refine_window::REFINE_WINDOW_ID;
use nostalre_ui_component::game::quest_window::{QUEST_WINDOW_ID,QUEST_DETAIL_WINDOW_ID};
use nostalre_ui_component::hud::{CastInfo, Place, TargetInfo, Unit};
use nostalre_ui_component::{BuildCtx, InGameWindow, Window};
use std::collections::HashMap;

/// Room between the frameless chat and the HUD's bottom centre
const CHAT_GAP: f32 = 24.0;
/// Below this a narrow screen's chat would be no use: it may reach under the bottom centre instead
const MIN_FRAMELESS_CHAT_W: f32 = 300.0;

pub fn build_in_game_ui(
    game: &mut GameState,
    windows: &mut Windows,
    ui: &mut UiFrame,
    texture_size_fn: &dyn Fn(&str) -> Option<(u32, u32)>,
    _render_list: &[RenderEntry],
) -> Vec<GameEvent> {
    #[cfg(test)]
    if std::env::var_os("NOSTALRE_UI_LAYOUT_PREVIEW").is_some() {
        ui.ctx.pad_labels = true;
    }
    let mut events = Vec::new();

    windows.npc_shop.setup_modal(ui);

    // The server never sends our own HP via party, and same-map members' HP only
    // arrives on change — so refresh rows from live state before `ctx` borrows
    // the party immutably.
    sync_party_live_state(game);
    if windows.hud.is_modern() {
        let input = &mut windows.hud.input;
        input.player_cast = player_cast(game);
        input.target = hud_target(game);
        input.attackers = attackers(game);
        let clock = &game.session.server_time;
        input.ping_ms = clock.is_synced().then(|| clock.rtt());
        input.place = place(game);
        input.chat = windows.chat_window.bounds();
    }
    windows.chat_window.frameless = windows.hud.is_modern();
    windows.chat_window.compact_history = windows.hud.uses_controller(ui.ctx.pad_labels);
    windows.chat_window.bottom_reserve = if windows.hud.shows_touch(ui.ctx.pad_labels) {
        nostalre_ui_component::hud::TOUCH_LEFT_COLUMN_H
    } else {
        0.0
    };
    windows.chat_window.max_width = windows.hud.is_modern().then(|| {
        ((ui.ctx.screen_width - DOCK_W) / 2.0 - CHAT_GAP)
            .floor()
            .max(MIN_FRAMELESS_CHAT_W)
    });

    let local_aid = game
        .session
        .login_session
        .as_ref()
        .map(|s| s.account_id)
        .unwrap_or(0);
    let local_gid = local_aid;
    let job_class = game.world.entities.player_job();

    let mut ctx = BuildCtx {
        character: &mut game.character,
        data: &game.data_table,
        party: game.party.as_ref(),
        friends: &game.friends,
        guild: game.guild.as_ref(),
        quest_log: &game.quest_log,
        homunculus: game.companions.homunculus.as_ref(),
        mercenary: game.companions.mercenary.as_ref(),
        pet: &game.companions.pet,
        companion_ai: &mut game.companions.companion_ai,
        job_class,
        local_aid,
        local_gid,
    };

    if windows.world_map_window.is_open() {
        windows.world_map_window.current_map = game.session.current_map.clone();
        if let Some(coords) = &game.session.map_coords {
            windows.world_map_window.map_width = coords.gat_width();
            windows.world_map_window.map_height = coords.gat_height();
        }
        if let Some(player) = game.world.entities.player() {
            windows.world_map_window.player_position = Some(player.movement.position());
            windows.world_map_window.player_direction = player.direction;
        }
    }

    // Snapshot before the Escape router, which can dismiss the dialog itself.
    let had_disconnect_dialog =
        game.session.disconnect_dialog_shown && windows.confirm_dialog.state.is_some();

    events.extend(route_escape(
        ui,
        windows,
        EscapeGame {
            pending_casts: &mut game.pending_casts,
            capture_targeting: &mut game.companions.capture_targeting,
            pet_roulette: &mut game.companions.pet_roulette,
            combat: &mut game.combat,
        },
        &mut ctx,
    ));

    let other_modal = modal_besides_menu(windows, &ctx);
    if other_modal || windows.menu.owns_keyboard(&ctx) {
        ui.block_keyboard();
    }

    // a dialog the server opens (an NPC, an invitation, death) comes before the menu, which reads raw keys
    if !windows.hud.is_modern() || other_modal {
        windows.menu.close();
    }
    if windows.menu.is_open() && windows.chat_window.is_active() {
        windows.chat_window.cancel_input();
    }
    // the menu covers the HUD and the windows it places
    let menu_open = windows.menu.is_open();
    // Under every window: an open one covers the HUD
    if !windows.hud.is_modern() || menu_open {
        windows.hud.hidden();
    } else if windows.hud.shows_touch(ui.ctx.pad_labels) {
        let base = windows.hud.input.touch_slots.slot(0);
        for (i, face) in windows.hud.input.touch_slots.faces.iter_mut().enumerate() {
            windows
                .hotkey_bar
                .refresh_face(base + i, ctx.character, face);
        }
    }
    let controller_ui = windows.hud.uses_controller(ui.ctx.pad_labels);
    let placement = (windows.hud.is_modern() && (!menu_open || controller_ui)).then(|| {
        let rows = ctx.character.hotkeys.visible_rows() as usize;
        windows.hud.build(ui, &ctx, rows)
    });
    windows.hotkey_bar.controller_hud = windows.hud.uses_controller(ui.ctx.pad_labels);
    windows.hotkey_bar.dock = placement.map(|p| p.hotkeys);
    windows.minimap_window.round = placement.map(|p| p.minimap);
    if placement.is_some_and(|p| p.menu_tapped) {
        windows.menu.open();
        // the tap is the menu line's: the menu drawn this frame does not take it too
        ui.ctx.mouse_clicked = false;
    }

    let z_order = ui.get_z_order();
    ui.compute_hovered_window(&z_order);
    for &win_id in &z_order {
        if !matches!(win_id,QUEST_WINDOW_ID|QUEST_DETAIL_WINDOW_ID) {
            dispatch_window(windows, win_id, ui, &mut ctx, &mut events);
        }
    }
    for &(win_id, _) in REGISTRY {
        if !z_order.contains(&win_id) && !matches!(win_id,QUEST_WINDOW_ID|QUEST_DETAIL_WINDOW_ID) {
            dispatch_window(windows, win_id, ui, &mut ctx, &mut events);
        }
    }

    storage_moves(windows, ctx.character, &mut events);

    windows.hotkey_bar.chat_is_active = windows.chat_window.is_active();
    windows.hotkey_bar.companion_skills.clear();
    if let Some(m) = ctx.mercenary {
        windows
            .hotkey_bar
            .companion_skills
            .extend(m.skills.iter().cloned());
    }
    if let Some(h) = ctx.homunculus {
        windows
            .hotkey_bar
            .companion_skills
            .extend(h.skills.iter().cloned());
    }
    if !menu_open {
        events.extend(windows.hotkey_bar.build(ui, &mut ctx));
    }

    if let Some(player) = game.world.entities.player() {
        windows.minimap_window.player_position = Some(player.movement.position());
        windows.minimap_window.player_direction = player.direction;
    }
    if let Some(coords) = &game.session.map_coords {
        windows.minimap_window.map_width = coords.gat_width();
        windows.minimap_window.map_height = coords.gat_height();
    }
    windows.minimap_window.map_name = game.session.current_map.clone();
    game.minimap_marks.prune(ui.elapsed_secs);
    if let (Some(nav), Some(map), Some(player)) = (
        &game.navigation,
        game.session.current_map.as_deref(),
        game.world.entities.player(),
    ) && nav.reached(map, player.movement.cell_position())
    {
        game.navigation = None;
    }
    windows.minimap_window.entity_markers = collect_minimap_markers(
        ctx.party,
        ctx.guild,
        ServerMarks {
            quest_markers: &game.quest_markers,
            marks: &game.minimap_marks,
            crimson_marks: &game.crimson_marks,
            boss_mark: game.boss_mark.as_ref(),
            navigation: game.navigation.as_ref(),
        },
        game.session.current_map.as_deref(),
        ctx.local_aid,
    );
    if let Some(marker) = followed_quest_marker(ctx.quest_log, game.session.current_map.as_deref(), ctx.job_class, ctx.character.job_level) {
        windows.minimap_window.entity_markers.push(marker);
    }
    if !menu_open || controller_ui {
        let minimap_events = windows.minimap_window.build(ui, &mut ctx);
        if !menu_open {
            events.extend(minimap_events);
        }
        game.crimson_marks.count_draw();
    }

    if !windows.hud.is_modern() {
        events.extend(windows.status_icon_bar.build(ui, &mut ctx));
    }

    // Journal pages must cover the HUD's late minimap and hotkey draw calls.
    // Their keyboard ownership and Escape routing still run before the HUD.
    dispatch_window(windows,QUEST_WINDOW_ID,ui,&mut ctx,&mut events);
    dispatch_window(windows,QUEST_DETAIL_WINDOW_ID,ui,&mut ctx,&mut events);

    if windows.menu.is_open() {
        windows.menu.place = windows.hud.input.place.clone();
        let mut menu_events = windows.menu.build(ui, &mut ctx);
        // one drop at a time, as the inventory's: the next waits for the server's answer
        let drops = |e: &GameEvent| matches!(e, GameEvent::RequestDropItem { .. });
        if game.combat.waiting_item_throw_ack {
            menu_events.retain(|e| !drops(e));
        } else if menu_events.iter().any(drops) {
            game.combat.waiting_item_throw_ack = true;
        }
        storage_moves(windows, ctx.character, &mut menu_events);
        events.extend(menu_events);
    }
    events.extend(windows.npc_dialog.build(ui, &mut ctx));
    events.extend(windows.npc_shop.build(ui, &mut ctx));
    events.extend(windows.warp_list_window.build(ui));
    events.extend(windows.item_list_selection_window.build(ui));
    windows.system_menu.can_resurrect = windows.system_menu.dead
        && !game.session.map_properties.enable_pk()
        && !game.session.map_properties.is_siege()
        && ctx
            .character
            .inventory
            .all_items()
            .iter()
            .any(|item| item.item_id == TOKEN_OF_SIEGFRIED && item.count > 0);
    events.extend(windows.system_menu.build(ui, &mut ctx));
    events.extend(windows.item_info_window.build(ui, &mut ctx));
    events.extend(InGameWindow::build(
        &mut windows.item_pickup_notification,
        ui,
        &mut ctx,
    ));

    windows.confirm_dialog.build(ui);
    if had_disconnect_dialog && windows.confirm_dialog.state.is_none() {
        game.session.pending_disconnect_exit = true;
        game.session.disconnect_dialog_shown = false;
    }

    if let Some(result) = windows.confirm_dialog.take_result()
        && let Some(event) = game.pending_confirms.dispatch(result)
    {
        events.push(event);
    }

    events.extend(windows.context_menu.build(ui));

    events.extend(windows.map_missing_window.build(ui));

    match windows.levelup_notification.build(ui) {
        LevelUpClick::Base if windows.hud.is_modern() => {
            windows.menu.open_on(HotkeyAction::ToggleStatus)
        }
        LevelUpClick::Job if windows.hud.is_modern() => {
            windows.menu.open_on(HotkeyAction::ToggleSkillTree)
        }
        LevelUpClick::Base => windows.status_window.open(),
        LevelUpClick::Job => ctx.character.skills.open(),
        LevelUpClick::None => {}
    }

    if let Some(cancelled) = ui.draw_drag_icon() {
        if cancelled.source_id == HOTKEY_BAR_WINDOW_ID {
            // the bar is not drawn under the menu: a drag from it that ends there is not a drop off the bar
            if !windows.menu.is_open()
                && ctx.character.hotkeys.get_slot(cancelled.item_index)
                    != nostalre_game::hotkey::HotkeySlotContent::Empty
            {
                ctx.character.hotkeys.clear_slot(cancelled.item_index);
                events.push(GameEvent::RequestHotkeyChange {
                    index: cancelled.item_index as u16,
                    is_skill: false,
                    id: 0,
                    count: 0,
                });
            }
        } else if cancelled.source_id == REFINE_WINDOW_ID {
            // the item dragged out of the refine window goes back to the inventory
            ctx.character.refine.clear_item();
        } else if cancelled.source_id == INV_WINDOW_ID && ui.hovered_window().is_none() {
            if game.combat.waiting_item_throw_ack {
            } else if windows.equipment_window.is_visible() {
                windows
                    .chat_window
                    .add_system("Please close the Equipment window.".to_string());
            } else if let Some(item) = ctx
                .character
                .inventory
                .get_item(cancelled.item_index as u16)
            {
                if item.count > 1 {
                    let mut dialog = DropQuantityDialog::new(item.index, item.count, &item.name);
                    dialog.has_grf_textures = windows.drop_dialog_has_grf_textures;
                    if dialog.has_grf_textures {
                        dialog.set_texture_sizes(texture_size_fn);
                    }
                    windows.drop_quantity_dialog = Some(dialog);
                } else {
                    events.push(GameEvent::RequestDropItem {
                        index: item.index,
                        count: 1,
                    });
                    game.combat.waiting_item_throw_ack = true;
                }
            }
        }
    }

    if run_transient_dialog(
        &mut windows.drop_quantity_dialog,
        ui,
        &mut ctx,
        &mut events,
        |e| matches!(e, GameEvent::RequestDropItem { .. }),
    ) {
        game.combat.waiting_item_throw_ack = true;
    }

    if let Some(dialog) = &mut windows.guild_expel_dialog {
        dialog.has_grf_textures = windows.drop_dialog_has_grf_textures;
        if dialog.has_grf_textures {
            dialog.set_texture_sizes(texture_size_fn);
        }
    }
    run_transient_dialog(
        &mut windows.guild_expel_dialog,
        ui,
        &mut ctx,
        &mut events,
        |e| matches!(e, GameEvent::ConfirmedGuildExpel { .. }),
    );

    if let Some(dialog) = &mut windows.skill_talkbox_dialog {
        dialog.has_grf_textures = windows.drop_dialog_has_grf_textures;
        if dialog.has_grf_textures {
            dialog.set_texture_sizes(texture_size_fn);
        }
    }
    run_transient_dialog(
        &mut windows.skill_talkbox_dialog,
        ui,
        &mut ctx,
        &mut events,
        |e| matches!(e, GameEvent::ConfirmedSkillTalkbox { .. }),
    );

    run_transient_dialog(
        &mut windows.card_insert_dialog,
        ui,
        &mut ctx,
        &mut events,
        |e| matches!(e, GameEvent::RequestCardInsert { .. }),
    );

    drop(ctx);
    update_broadcast_overlays(game, ui, windows.menu.is_open() || windows.quest_window.is_open() || windows.quest_detail_window.is_open());

    if ui.hovered_window().is_some() {
        ui.ctx.scroll_delta = 0.0;
    }

    events
}

/// Builds a transient dialog held in `slot`, clears the slot when the dialog
/// emits its confirm event or a `DialogClosed`, and appends its events (minus
/// `DialogClosed`) to `events`. Returns whether the confirm event fired, so the
/// caller can run any follow-up side effect. A no-op when the slot is empty.
fn run_transient_dialog<D: InGameWindow>(
    slot: &mut Option<D>,
    ui: &mut UiFrame,
    ctx: &mut BuildCtx,
    events: &mut Vec<GameEvent>,
    is_confirm: impl Fn(&GameEvent) -> bool,
) -> bool {
    let Some(dialog) = slot.as_mut() else {
        return false;
    };
    let dialog_events = InGameWindow::build(dialog, ui, ctx);
    let confirmed = dialog_events.iter().any(&is_confirm);
    if confirmed
        || dialog_events
            .iter()
            .any(|e| matches!(e, GameEvent::DialogClosed))
    {
        *slot = None;
    }
    events.extend(
        dialog_events
            .into_iter()
            .filter(|e| !matches!(e, GameEvent::DialogClosed)),
    );
    confirmed
}

const BOSS_MARK_COLOR: [f32; 3] = [1.0, 0.2, 0.2];
/// A Crimson Marker's square: kRO's minimap draws it in 0xffff0000 (0x76d3a0)
const CRIMSON_MARK_COLOR: [f32; 3] = [1.0, 0.0, 0.0];

/// An NPC's navigation guide's cell (0x08e2): ours, kRO draws its guide in a window of its own
const NAVIGATION_MARK_COLOR: [f32; 3] = [0.2, 0.9, 1.0];

fn followed_quest_marker(log: &nostalre_game::quest::QuestLog, map: Option<&str>, job: u16, job_level: u32) -> Option<MinimapMarker> {
    use nostalre_game::quest_guidance;
    let map=map?;
    let quest=log.followed();
    let guide=quest.and_then(|q|quest_guidance::for_quest(log,q.id)).or_else(||{
        if quest.is_none() {quest_guidance::novice_hint(map,job,job_level)} else {None}
    })?;
    let target=quest_guidance::next_on_map(&guide.destination,map)?;
    Some(MinimapMarker { x:f32::from(target.x), y:f32::from(target.y), marker_type:MarkerType::Square(if target.name.starts_with("Portal") {[0.2,0.9,1.0]} else {[1.0,0.85,0.1]}), name:Some(format!("Objetivo: {}",target.name)) })
}

/// The marks the server puts on the minimap: quest markers, its mark channel (`ZC_COMPASS` viewpoints), the units
/// a Crimson Marker marks, the Convex Mirror's MVP and an NPC's navigation guide
struct ServerMarks<'a> {
    quest_markers: &'a HashMap<u32, QuestMarker>,
    marks: &'a MinimapMarks,
    crimson_marks: &'a CrimsonMarks,
    boss_mark: Option<&'a BossMark>,
    navigation: Option<&'a Navigation>,
}

/// What the minimap marks: party members and same-map guild members, and the server's marks. NPCs and portals are
/// deliberately absent — the original never marked them.
fn collect_minimap_markers(
    party: Option<&Party>,
    guild: Option<&Guild>,
    server: ServerMarks,
    current_map: Option<&str>,
    local_aid: u32,
) -> Vec<MinimapMarker> {
    let ServerMarks {
        quest_markers,
        marks,
        crimson_marks,
        boss_mark,
        navigation,
    } = server;
    let mut markers = Vec::new();
    let current_map = current_map.map(nostalre_game::map_key);
    if let Some(party) = party {
        for member in &party.members {
            if member.aid == local_aid
                || !member.online
                || !member.has_live_position
                || current_map.as_deref() != Some(nostalre_game::map_key(&member.map).as_str())
            {
                continue;
            }
            markers.push(MinimapMarker {
                x: member.x as f32,
                y: member.y as f32,
                marker_type: MarkerType::PartyMember {
                    leader: member.leader,
                },
                name: Some(member.name.clone()),
            });
        }
    }
    if let Some(guild) = guild {
        for member in &guild.members {
            if member.aid == local_aid || !member.has_live_position {
                continue;
            }
            markers.push(MinimapMarker {
                x: member.x as f32,
                y: member.y as f32,
                marker_type: MarkerType::GuildMember,
                name: None,
            });
        }
    }
    // kRO's minimap takes the markers with a colour (0x7d03e0)
    for marker in quest_markers.values().filter(|marker| marker.color != 0) {
        markers.push(MinimapMarker {
            x: marker.x as f32,
            y: marker.y as f32,
            marker_type: MarkerType::Mark(quest_marker_color(marker.color)),
            name: None,
        });
    }
    for mark in marks.iter() {
        markers.push(MinimapMarker {
            x: mark.x as f32,
            y: mark.y as f32,
            marker_type: MarkerType::Mark(mark.rgb()),
            name: None,
        });
    }
    if let Some((x, y)) = navigation
        .zip(current_map.as_deref())
        .and_then(|(nav, map)| nav.cell_on(map))
    {
        markers.push(MinimapMarker {
            x: f32::from(x),
            y: f32::from(y),
            marker_type: MarkerType::Mark(NAVIGATION_MARK_COLOR),
            name: None,
        });
    }
    for mark in crimson_marks.iter().filter(|m| m.shown()) {
        markers.push(MinimapMarker {
            x: f32::from(mark.x),
            y: f32::from(mark.y),
            marker_type: MarkerType::Square(CRIMSON_MARK_COLOR),
            name: None,
        });
    }
    if let Some(boss) = boss_mark {
        markers.push(MinimapMarker {
            x: boss.x as f32,
            y: boss.y as f32,
            marker_type: MarkerType::Mark(BOSS_MARK_COLOR),
            name: Some(boss.name.clone()),
        });
    }
    markers
}

fn sync_party_live_state(game: &mut GameState) {
    let local_aid = game
        .session
        .login_session
        .as_ref()
        .map(|s| s.account_id)
        .unwrap_or(0);
    if let Some(party) = &mut game.party {
        for m in &mut party.members {
            if m.aid == local_aid {
                m.hp = Some(game.character.hp);
                m.max_hp = Some(game.character.max_hp);
                if let Some(p) = game.world.entities.player() {
                    (m.x, m.y) = p.movement.cell_position();
                }
            } else if let Some(e) = game
                .world
                .entities
                .get(game.world.entities.resolve_key(m.aid))
            {
                if let (Some(hp), Some(max_hp)) = (e.hp, e.max_hp) {
                    m.hp = Some(hp);
                    m.max_hp = Some(max_hp);
                }
                (m.x, m.y) = e.movement.cell_position();
                // On-screen beats waiting for the next position packet; a member
                // who is on our map but out of view keeps the packet's answer.
                m.has_live_position = true;
            }
        }
    }
}

/// Storage moves the windows asked for, as the storage window starts them (asking how many of a stack)
fn storage_moves(windows: &mut Windows, character: &mut Character, events: &mut Vec<GameEvent>) {
    let moves: Vec<(bool, u16)> = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::RequestDepositItem { index } => Some((true, *index)),
            GameEvent::RequestWithdrawItem { index } => Some((false, *index)),
            _ => None,
        })
        .collect();
    if moves.is_empty() {
        return;
    }
    events.retain(|e| {
        !matches!(
            e,
            GameEvent::RequestDepositItem { .. } | GameEvent::RequestWithdrawItem { .. }
        )
    });
    for (deposit, index) in moves {
        let started = if deposit {
            windows.storage_window.begin_deposit_body(character, index)
        } else {
            windows.storage_window.begin_withdraw(character, index)
        };
        events.extend(started);
    }
}

fn update_broadcast_overlays(game: &mut GameState, ui: &mut UiFrame, menu_open: bool) {
    let now = ui.elapsed_secs;
    let dt = game
        .broadcast
        .broadcast_last_elapsed
        .map_or(0.0, |last| (now - last).clamp(0.0, 0.1));
    game.broadcast.broadcast_last_elapsed = Some(now);

    game.broadcast.poptip.tick(dt);
    game.broadcast.notice.tick(dt);
    draw_broadcast_poptip(game, ui);
    draw_notice_window(game, ui);
    update_map_title(game, ui, dt, !menu_open);
    update_broadcast_banner(game, ui, dt);
}

/// The map's title on entering it, laid out as kRO's 0x9285e0/0x929ab0 lay it: the banner (every
/// `display_mapname` image is 500 by 190) scaled to a fifth of the screen's height, centred, its top at 0.12 of
/// the height; the subtitle 16 and the main title 44 of the banner's pixels high, centred, their tops at 72 and 95;
/// white, the whole faded as the banner fades
fn update_map_title(game: &mut GameState, ui: &mut UiFrame, dt: f32, visible: bool) {
    const BANNER: (f32, f32) = (500.0, 190.0);
    const SUB: (f32, f32) = (16.0, 72.0);
    const MAIN: (f32, f32) = (44.0, 95.0);
    let Some(banner) = &mut game.session.map_title else {
        return;
    };
    if !banner.tick(dt) {
        game.session.map_title = None;
        return;
    }
    // Keep aging while hidden so closing a menu never restarts the map banner.
    if !visible { return; }
    let (screen_w, screen_h) = (ui.ctx.screen_width, ui.ctx.screen_height);
    let height = screen_h * 0.2;
    let scale = height / BANNER.1;
    let width = BANNER.0 * scale;
    let top = screen_h * 0.12;
    let alpha = banner.alpha();
    let (v, i) = nostalre_ui::draw::quad_vertices(
        (screen_w - width) / 2.0,
        top,
        width,
        height,
        [1.0, 1.0, 1.0, alpha],
    );
    ui.draw_calls.push(nostalre_ui::draw::DrawCall {
        vertices: v.into(),
        indices: i.into(),
        texture: nostalre_ui::draw::TextureRef::Named(banner.title.background.clone().into()),
    });
    for (text, (size, y)) in [
        (&banner.title.sub_title, SUB),
        (&banner.title.main_title, MAIN),
    ] {
        let text_scale = size * scale / ui.atlas.line_height;
        let text_w = ui.atlas.measure_text(text) * text_scale;
        let baseline = top + y * scale + ui.atlas.ascent * text_scale;
        let (v, i) = nostalre_ui::draw::text_vertices_scaled(
            text,
            (screen_w - text_w) / 2.0,
            baseline,
            [1.0, 1.0, 1.0, alpha],
            ui.atlas,
            text_scale,
        );
        if !v.is_empty() {
            ui.draw_calls.push(nostalre_ui::draw::DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: nostalre_ui::draw::TextureRef::FontAtlas,
            });
        }
    }
}

/// kRO's notice window as the 2025 rig shows it (2026-10-19): centred, its top 51 pixels down (0x7b7ef0 moves it
/// to y 0x33), a dark see-through box with a grey border, the line in its colour
fn draw_notice_window(game: &GameState, ui: &mut UiFrame) {
    const TOP: f32 = 51.0;
    const PAD_X: f32 = 8.0;
    const PAD_Y: f32 = 3.0;
    const FILL: [f32; 4] = [0.0, 0.0, 0.0, 0.5];
    const BORDER: [f32; 4] = [90.0 / 255.0, 90.0 / 255.0, 90.0 / 255.0, 1.0];
    let Some((text, color)) = game.broadcast.notice.current() else {
        return;
    };
    let width = ui.atlas.measure_text(text) + PAD_X * 2.0;
    let height = ui.atlas.line_height + PAD_Y * 2.0;
    let x = ui.ctx.screen_width * 0.5 - width * 0.5;
    let quads = [
        (x, TOP, width, height, FILL),
        (x, TOP, width, 1.0, BORDER),
        (x, TOP + height - 1.0, width, 1.0, BORDER),
        (x, TOP, 1.0, height, BORDER),
        (x + width - 1.0, TOP, 1.0, height, BORDER),
    ];
    for (qx, qy, qw, qh, qc) in quads {
        let (v, i) = nostalre_ui::draw::quad_vertices(qx, qy, qw, qh, qc);
        ui.draw_calls.push(nostalre_ui::draw::DrawCall {
            vertices: v.into(),
            indices: i.into(),
            texture: nostalre_ui::draw::TextureRef::White,
        });
    }
    ui.text(x + PAD_X, TOP + PAD_Y + ui.atlas.ascent, text, color);
}

fn draw_broadcast_poptip(game: &mut GameState, ui: &mut UiFrame) {
    const BASE_Y: f32 = 90.0;
    if game.broadcast.poptip.is_empty() {
        return;
    }
    let center_x = ui.ctx.screen_width * 0.5;
    let line_h = ui.atlas.line_height + 4.0;
    const PAD: f32 = 4.0;
    for (index, (text, alpha)) in game.broadcast.poptip.iter().enumerate() {
        let width = ui.atlas.measure_text(text);
        let x = center_x - width * 0.5;
        let y = BASE_Y - index as f32 * line_h;

        let box_w = width + PAD * 2.0;
        let box_h = ui.atlas.line_height + PAD * 2.0;
        let box_x = x - PAD;
        let box_y = y - ui.atlas.line_height * 0.5 - PAD;
        let (bg_v, bg_i) = nostalre_ui::draw::quad_vertices(
            box_x,
            box_y,
            box_w,
            box_h,
            [0.0, 0.0, 0.0, 0.8 * alpha],
        );
        ui.draw_calls.push(nostalre_ui::draw::DrawCall {
            vertices: bg_v.into(),
            indices: bg_i.into(),
            texture: nostalre_ui::draw::TextureRef::White,
        });

        ui.text(x, y, text, [1.0, 1.0, 1.0, alpha]);
    }
}

fn update_broadcast_banner(game: &mut GameState, ui: &mut UiFrame, dt: f32) {
    const BAR_Y: f32 = 40.0;
    const BAR_H: f32 = 24.0;
    const BG_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.7];
    const TEXT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

    if !game.broadcast.banner.visible() {
        return;
    }
    game.broadcast.banner.tick(dt);

    let Some(render) = game.broadcast.banner.render() else {
        return;
    };
    let text_width = ui.atlas.measure_text(render.text);
    if game.broadcast.banner.current_scrolled_off(text_width) {
        game.broadcast.banner.advance();
        return;
    }

    let center_x = ui.ctx.screen_width * 0.5;
    let bar_left = center_x - render.half_width;
    let bar_right = center_x + render.half_width;

    let (bg_v, bg_i) =
        nostalre_ui::draw::quad_vertices(bar_left, BAR_Y, render.half_width * 2.0, BAR_H, BG_COLOR);
    ui.draw_calls.push(nostalre_ui::draw::DrawCall {
        vertices: bg_v.into(),
        indices: bg_i.into(),
        texture: nostalre_ui::draw::TextureRef::White,
    });

    let text_x = bar_left + render.text_offset_x;
    let baseline_y = BAR_Y + (BAR_H + ui.atlas.ascent) * 0.5;
    let (tv, ti) = nostalre_ui::draw::text_vertices_clipped(
        render.text,
        text_x,
        baseline_y,
        TEXT_COLOR,
        ui.atlas,
        bar_left,
        bar_right,
    );
    if !tv.is_empty() {
        ui.draw_calls.push(nostalre_ui::draw::DrawCall {
            vertices: tv.into(),
            indices: ti.into(),
            texture: nostalre_ui::draw::TextureRef::FontAtlas,
        });
    }
}

/// An entity's cast for a HUD bar, none for the skills whose overhead bar is hidden too
fn cast_of(game: &GameState, entity: &Entity) -> Option<CastInfo> {
    entity.cast_progress()?;
    let skill = entity.active_skill?;
    if casting_skill(skill).hide_cast_bar {
        return None;
    }
    Some(CastInfo {
        name: game.data_table.skill_name_for(skill).to_string(),
        remaining: entity.cast_remaining,
        total: entity.cast_total_duration,
    })
}

fn player_cast(game: &GameState) -> Option<CastInfo> {
    cast_of(game, game.world.entities.player()?)
}

/// HP as the server last told it: 0x0977's numbers, else what the name packet carried
fn unit_hp(entity: &Entity) -> Option<f32> {
    entity.hp_percentage().or_else(|| {
        let info = entity.mob_info.as_ref()?;
        match (info.hp, info.max_hp, info.hp_percent) {
            (Some(hp), Some(max), _) if max > 0 => Some(hp as f32 / max as f32),
            (_, _, Some(percent)) => Some(f32::from(percent) / 100.0),
            _ => None,
        }
    })
}

fn unit_of(entity: &Entity) -> Unit {
    Unit {
        name: entity.name.clone().unwrap_or_default(),
        hp: unit_hp(entity),
    }
}

/// Alive: a dead unit lingers while it fades out, keeping its last target, and its HP reaches 0
/// before the vanish that kills it does
fn is_alive(entity: &Entity) -> bool {
    !entity.pending_death
        && entity.state() != EntityState::Dead
        && !entity.is_fading()
        && entity.hp != Some(0)
}

/// The player as a target's target: HP from the character, which the server keeps current
fn player_unit(game: &GameState) -> Unit {
    let c = &game.character;
    Unit {
        name: c.name.clone(),
        hp: (c.max_hp > 0).then(|| c.hp as f32 / c.max_hp as f32),
    }
}

fn is_combatant(entity: &Entity) -> bool {
    is_alive(entity)
        && matches!(
            entity.category(),
            EntityCategory::Monster | EntityCategory::Player
        )
}

/// What is being attacked, else the last enemy attacked while it is still in sight
/// (`docs/nostalre-hud.md`, "The target")
fn hud_target(game: &GameState) -> Option<TargetInfo> {
    let entities = &game.world.entities;
    let entity = [
        game.combat.selected_target_id,
        game.combat.attack_target_id,
        game.combat.last_attacked_enemy,
    ]
    .into_iter()
    .flatten()
    .filter_map(|id| entities.get(id))
    .find(|e| is_combatant(e))?;
    let level = entity
        .mob_info
        .as_ref()
        .and_then(|m| m.level)
        .or_else(|| u16::try_from(entity.base_level).ok().filter(|&l| l > 0));
    let player_id = entities.player().map(|p| p.id);
    let aim = entity
        .target_gid
        .or(entity.cast_target_gid)
        .and_then(|gid| {
            if Some(gid) == player_id {
                Some(player_unit(game))
            } else {
                entities.get(gid).map(unit_of)
            }
        });
    Some(TargetInfo {
        id: entity.id,
        unit: unit_of(entity),
        level,
        boss: entity.is_boss,
        cast: cast_of(game, entity),
        target: aim,
    })
}

fn place(game: &GameState) -> Option<Place> {
    let map = game.session.current_map.clone()?;
    let (x, y) = game.world.entities.player()?.movement.cell_position();
    Some(Place { map, x, y })
}

/// Monsters whose last attack went at the player
fn attackers(game: &GameState) -> Vec<Unit> {
    let entities = &game.world.entities;
    let Some(player) = entities.player() else {
        return Vec::new();
    };
    entities
        .iter()
        .filter(|e| {
            e.category() == EntityCategory::Monster
                && is_alive(e)
                && e.target_gid == Some(player.id)
        })
        .map(unit_of)
        .collect()
}

fn dispatch_window(
    windows: &mut Windows,
    win_id: WidgetId,
    ui: &mut UiFrame,
    ctx: &mut BuildCtx,
    events: &mut Vec<GameEvent>,
) {
    let Some((_, dispatch)) = REGISTRY.iter().find(|(id, _)| *id == win_id) else {
        return;
    };
    // The frameless HUD shows what basic info holds
    if win_id == BASIC_INFO_WINDOW_ID && windows.hud.is_modern() {
        return;
    }
    match dispatch {
        Dispatch::Trait(acc) => events.extend(acc(windows).build(ui, ctx)),
        Dispatch::VendingAvailable => {
            events.extend(windows.vending_setup_window.build_available(ui, ctx))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classic_training_objective_is_yellow_without_renewal_academy_routes() {
        let log = nostalre_game::quest::QuestLog::default();
        let marker = followed_quest_marker(&log, Some("new_1-1"), 0, 1).unwrap();
        assert_eq!((marker.x, marker.y), (53.0, 114.0));
        assert!(matches!(marker.marker_type, MarkerType::Square([1.0, 0.85, 0.1])));
        assert!(followed_quest_marker(&log, Some("iz_ac01"), 0, 1).is_none());
    }
    use nostalre_game::minimap_mark::MarkAction;
    use nostalre_game::navigation::{NavServices, NavTarget};
    use nostalre_game::party::PartyMember;

    #[test]
    fn quest_reader_is_drawn_above_minimap_and_hotkeys() {
        use nostalre_ui::{context::UiContext,state::StateCache,test_support::test_frame,draw::{self,TextureRef}};
        use nostalre_ui_component::game::quest_window::QUEST_DETAIL_WINDOW_ID;
        for pad in [false,true] {
            let mut app=crate::App::new(crate::Config::default());
            app.game.quest_log.add(nostalre_game::quest::Quest{id:7474,active:true,..Default::default()});
            app.windows.quest_detail_window.open(7474);
            let mut input=UiContext::new(854.0,480.0);input.pad_labels=pad;
            let mut state=StateCache::new();let mut ui=test_frame(&mut input,&mut state);
            build_in_game_ui(&mut app.game,&mut app.windows,&mut ui,&|_|None,&[]);
            let position=ui.state.get::<nostalre_ui::frame::WindowState>(QUEST_DETAIL_WINDOW_ID).unwrap();
            let (x,y)= (position.x,position.y);
            let (expected,_)=draw::text_vertices("↑↓ Rolar    B Voltar",x+14.0,y+375.0-7.0,[0.14,0.20,0.38,1.0],ui.atlas);
            let footer=ui.draw_calls.iter().position(|call|matches!(call.texture,TextureRef::FontAtlas)&&call.vertices.len()==expected.len()
                &&call.vertices.iter().zip(&expected).all(|(a,b)|a.position==b.position)).expect("quest footer rendered by actual UI");
            for call in &ui.draw_calls[footer+1..] {
                if call.vertices.is_empty(){continue;}
                let min_x=call.vertices.iter().map(|v|v.position[0]).fold(f32::INFINITY,f32::min);
                let max_x=call.vertices.iter().map(|v|v.position[0]).fold(f32::NEG_INFINITY,f32::max);
                let min_y=call.vertices.iter().map(|v|v.position[1]).fold(f32::INFINITY,f32::min);
                let max_y=call.vertices.iter().map(|v|v.position[1]).fold(f32::NEG_INFINITY,f32::max);
                assert!(!(max_x>x && min_x<x+350.0 && max_y>y && min_y<y+375.0),
                    "HUD drawn after quest reader hides text: pad={pad} bounds=({min_x},{min_y})..({max_x},{max_y})");
            }
        }
    }

    fn member(aid: u32, name: &str, map: &str, leader: bool, live: bool) -> PartyMember {
        PartyMember {
            aid,
            name: name.to_string(),
            map: map.to_string(),
            leader,
            online: true,
            hp: None,
            max_hp: None,
            x: 100,
            y: 120,
            has_live_position: live,
            ..Default::default()
        }
    }

    #[test]
    fn only_same_map_party_members_guilds_and_server_marks_are_marked() {
        let mut party = Party::new("Adventurers".to_string());
        party.members = vec![
            member(1, "Me", "prontera.gat", false, true),
            member(2, "Leader", "prontera.gat", true, true),
            member(3, "Lidia", "prontera.gat", false, true),
            // Left our map: the server cleared their position.
            member(4, "Garm", "prontera.gat", false, false),
            member(5, "Sohee", "payon.gat", false, true),
        ];
        let mut offline = member(6, "Afk", "prontera.gat", false, true);
        offline.online = false;
        party.members.push(offline);

        let mut marks = MinimapMarks::default();
        marks.apply(0, MarkAction::Show, 134, 221, 0xFF0000, 0.0);

        let mut quest_markers = HashMap::new();
        quest_markers.insert(
            900,
            QuestMarker {
                x: 50,
                y: 60,
                effect: 1,
                color: 2,
            },
        );

        let boss = BossMark {
            x: 100,
            y: 100,
            name: "Baphomet".to_string(),
        };
        let mut crimson = CrimsonMarks::default();
        crimson.apply(77, 30, 40);
        // Ended early: rAthena's -1, -1, kept by its count but not drawn
        crimson.apply(78, -1, -1);
        let navigation = Navigation {
            map: "prontera".into(),
            target: NavTarget::Cell { x: 150, y: 180 },
            services: NavServices::default(),
            hide_window: true,
        };
        let markers = collect_minimap_markers(
            Some(&party),
            None,
            ServerMarks {
                quest_markers: &quest_markers,
                marks: &marks,
                crimson_marks: &crimson,
                boss_mark: Some(&boss),
                navigation: Some(&navigation),
            },
            Some("prontera"),
            1,
        );

        let party_names: Vec<(&str, bool)> = markers
            .iter()
            .filter_map(|m| match (m.marker_type, &m.name) {
                (MarkerType::PartyMember { leader }, Some(name)) => Some((name.as_str(), leader)),
                _ => None,
            })
            .collect();
        assert_eq!(party_names, vec![("Leader", true), ("Lidia", false)]);

        let mark_colors: Vec<[f32; 3]> = markers
            .iter()
            .filter_map(|m| match m.marker_type {
                MarkerType::Mark(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(
            mark_colors,
            vec![
                quest_marker_color(2),
                [1.0, 0.0, 0.0],
                NAVIGATION_MARK_COLOR,
                BOSS_MARK_COLOR
            ],
            "quest marker, the viewpoint mark, the navigation guide's cell and the MVP, all on the mark channel"
        );
        let squares: Vec<(f32, f32)> = markers
            .iter()
            .filter(|m| matches!(m.marker_type, MarkerType::Square(c) if c == CRIMSON_MARK_COLOR))
            .map(|m| (m.x, m.y))
            .collect();
        assert_eq!(squares, vec![(30.0, 40.0)], "the shown Crimson Marker only");
        assert_eq!(markers.len(), 7, "no NPC or portal markers");
    }
}
