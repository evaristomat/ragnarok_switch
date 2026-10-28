use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::body_effect::Trigger;
use crate::config::WindowStateEntry;
use crate::ui::windows::Windows;
use models::enums::EnumWithNumberValue;
use models::enums::client_effect_icon::ClientEffectIcon;
use models::enums::effect_id::EffectId;
use models::enums::skill_enums::SkillEnum;
use nostalre_formats::act::ActFile;
use nostalre_formats::gat::GatFile;
use nostalre_formats::map_coordinates::MapCoordinates;
use nostalre_game::ailment::AilmentOverlay;
use nostalre_game::app_state::AppState;
use nostalre_game::arrow::ArrowProjectile;
use nostalre_game::banner::BannerState;
use nostalre_game::cast_scope::CastScope;
use nostalre_game::character::Character;
use nostalre_game::chat_room::ChatRoomRegistry;
use nostalre_game::companion::{ElementalState, HomunculusState, MercenaryState, SummonHp};
use nostalre_game::cursor::{
    CursorAnimationState, CursorType, PendingCompanionSkill, PendingSkillTarget,
};
use nostalre_game::damage_number::DamageNumberManager;
use nostalre_game::data_table::DataTable;
use nostalre_game::day_night::DayNightState;
use nostalre_game::doridori::DoridoriTracker;
use nostalre_game::effect::effects::body_aura::BodyAuraSpec;
use nostalre_game::effect::{EffectMessage, EffectQueue};
use nostalre_game::effects::AmbientEffectScheduler;
use nostalre_game::entity::{ChatBubbleState, EntityState};
use nostalre_game::entity_collection::EntityCollection;
use nostalre_game::event::{CharacterInfo, GameEvent};
use nostalre_game::floor_item::FloorItem;
use nostalre_game::gr2_model::{Gr2Asset, Gr2ModelInstance};
use nostalre_game::graffiti::Graffiti;
use nostalre_game::party::Party;
use nostalre_game::pet::PetState;
use nostalre_game::poptip::PoptipStack;
use nostalre_game::progress_bar::ProgressBar;
use nostalre_game::quest::{QuestLog, QuestMarker};
use nostalre_game::scheduled_hit::ScheduledHitQueue;
use nostalre_game::server_time::ServerTimeClock;
use nostalre_game::skill::SkillTargetType;
use nostalre_game::targeting::MapProperties;
use nostalre_network::session::Session;
use nostalre_renderer::camera::SavedCameraView;
use nostalre_renderer::{EntitySprite, SpriteTextures};
use nostalre_ui::state::StateCache;
use nostalre_ui_component::game::chat_window::{self};
use nostalre_ui_component::game::confirm_dialog::ConfirmResult;
use nostalre_ui_component::game::equipment_window::EQ_WINDOW_ID;
use nostalre_ui_component::game::inventory_window::INV_WINDOW_ID;
use nostalre_ui_component::game::skill_tree_window::SKILL_WINDOW_ID;

/// One-shot ice-shatter animation played when a freeze ends, following the
/// entity by gid. `started_at` is lazily set to the render clock on first draw.
pub struct FreezeShatter {
    pub gid: u32,
    pub started_at: Option<f32>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SelfConfig {
    pub refuse_party_invite: bool,
    pub open_equipment_window: bool,
    pub call_enabled: bool,
    pub pet_autofeed: bool,
    pub homun_autofeed: bool,
    pub hide_costumes: bool,
}

#[derive(Default)]
pub struct PendingConfirms {
    pub pending_trade_partner: Option<(u32, String)>,
    pub pending_trade_request: Option<u32>,
    pub pending_adopt_request: Option<(u32, u32)>,
    /// Proposer's aid and gid from ZC_REQ_COUPLE, echoed back in the reply.
    pub pending_marriage_proposal: Option<(u32, u32)>,
    pub pending_invite_aid: Option<u32>,
    /// Character name and requested state of an in-flight whisper allow/deny; the
    /// local block list only changes once the server confirms it.
    pub pending_whisper_block: Option<(String, bool)>,
    /// Target and text of the last whisper sent; the outgoing line is only shown
    /// once the server acks it, since a command typed there never comes back.
    pub pending_whisper_out: Option<(String, String)>,
    /// Target of the last `/check`; the stat block reply carries no name.
    pub pending_gm_check: Option<String>,
    active: Option<Box<dyn FnOnce(bool) -> Option<GameEvent>>>,
}

impl PendingConfirms {
    pub(crate) fn dispatch(&mut self, result: ConfirmResult) -> Option<GameEvent> {
        let ctor = self.active.take()?;
        ctor(result == ConfirmResult::Ok)
    }
}

pub struct CombatState {
    /// Controller focus is independent from attacking: choosing a mob sends no attack.
    pub selected_target_id: Option<u32>,
    pub attack_target_id: Option<u32>,
    pub last_attacked_enemy: Option<u32>,
    pub attack_request_cooldown: f32,
    pub attack_range: i16,
    pub attack_is_locked: bool,
    /// Whether an attack request has gone out since the current target was picked.
    /// The server keeps swinging until told to stop, so only then is a lock-on
    /// cancel worth sending.
    pub attack_request_sent: bool,
    pub waiting_item_throw_ack: bool,
    pub damage_numbers: DamageNumberManager,
    /// Destination clicked while the swing (or a pickup) held the character.
    /// Released on the next state change, not once the motion ends.
    pub queued_move: Option<(i32, i32)>,
    /// State the character was in when `queued_move` was taken.
    pub queued_move_state: Option<EntityState>,
}

impl Default for CombatState {
    fn default() -> Self {
        Self::new()
    }
}

impl CombatState {
    pub fn new() -> Self {
        Self {
            attack_target_id: None,
            selected_target_id: None,
            last_attacked_enemy: None,
            attack_request_cooldown: 0.0,
            attack_range: 1,
            attack_is_locked: false,
            attack_request_sent: false,
            waiting_item_throw_ack: false,
            damage_numbers: DamageNumberManager::new(),
            queued_move: None,
            queued_move_state: None,
        }
    }
}

#[derive(Default)]
pub struct EffectKeys {
    pub status_buff_keys: HashMap<(u32, i16), u32>,
    pub opt3_keys: HashMap<(u32, i32), u32>,
    pub next_status_buff_key: u32,
    pub level_aura_keys: HashMap<u32, (u32, &'static [EffectId])>,
    pub boss_aura_keys: HashMap<u32, u32>,
    /// gid -> (effect key, rank the tint was baked from)
    pub toprank_keys: HashMap<u32, (u32, i32)>,
    pub warp_portal_keys: HashMap<u32, u32>,
    /// The body effect each (gid, trigger) shows: spirit spheres, shields, souls, the statuses' (`body_effect.rs`)
    pub body_effect_keys: HashMap<(u32, Trigger), u32>,
    /// A unit's spheres drawn by a kRO class of their own (`kro_spheres`): gid -> (the class, its key)
    pub sphere_class_keys: HashMap<u32, (EffectId, u32)>,
    pub sight_aura_keys: HashMap<u32, u32>,
    pub ruwach_aura_keys: HashMap<u32, u32>,
    pub blind_overlay_key: Option<u32>,
    pub weather_keys: HashMap<EffectId, u32>,
    /// The aura each (gid, efst) puts on a unit's weapon (`custom.skill.weapon_aura`)
    pub weapon_aura: HashMap<(u32, i16), nostalre_game::effect::WeaponAura>,
    /// The Devotion links each caster draws (0x01cf), one effect key a link
    pub devotion_link_keys: HashMap<u32, Vec<u32>>,
    /// A caster's link that its status takes off (Marionette's, Shadow Form's: `nostalre_effects::status_link`)
    pub status_link_keys: HashMap<(u32, EffectId), u32>,
}

impl EffectKeys {
    pub fn clear(&mut self) {
        self.status_buff_keys.clear();
        self.opt3_keys.clear();
        self.next_status_buff_key = 0;
        self.level_aura_keys.clear();
        self.boss_aura_keys.clear();
        self.toprank_keys.clear();
        self.warp_portal_keys.clear();
        self.body_effect_keys.clear();
        self.sphere_class_keys.clear();
        self.sight_aura_keys.clear();
        self.ruwach_aura_keys.clear();
        self.blind_overlay_key = None;
        self.weather_keys.clear();
        self.weapon_aura.clear();
        self.devotion_link_keys.clear();
        self.status_link_keys.clear();
    }

    /// A key for an effect kept on a unit, apart from the server's ids (high bit set)
    pub fn next_key(&mut self) -> u32 {
        let key = 0x8000_0000 | self.next_status_buff_key;
        self.next_status_buff_key = (self.next_status_buff_key + 1) & 0x7fff_ffff;
        key
    }

    /// Shows `spec` on `gid` for `trigger` (`count` its counter) in place of what it showed; `None` takes it off. A
    /// status the server sends again while its effect shows keeps it, so its fade-in does not start over.
    pub fn set_body_effect(
        &mut self,
        queue: &mut EffectQueue,
        gid: u32,
        trigger: Trigger,
        spec: Option<BodyAuraSpec>,
        count: u8,
    ) {
        let slot = (gid, trigger);
        let refreshed = matches!(slot.1, Trigger::Status(_)) && spec.is_some();
        if refreshed && self.body_effect_keys.contains_key(&slot) {
            return;
        }
        if let Some(old) = self.body_effect_keys.remove(&slot) {
            queue.despawn(old);
        }
        if let Some(spec) = spec {
            let key = self.next_key();
            queue.spawn_body_aura(gid, key, spec, count);
            self.body_effect_keys.insert(slot, key);
        }
    }

    /// A unit's spheres by `class` (none: not a job kRO draws so), as kRO's handler keeps one class a unit
    /// (0x805a80): made at its first count, then told each count, or made anew, or taken off, as its `renewal` says;
    /// another class, or none, takes the old one off
    pub fn set_sphere_class(
        &mut self,
        queue: &mut EffectQueue,
        gid: u32,
        class: Option<EffectId>,
        hidden: bool,
        count: u8,
    ) {
        use nostalre_game::effect::effects::kro_spheres::{Renewal, renewal};
        let held = self.sphere_class_keys.get(&gid).copied();
        let renew = class.map(|c| renewal(c, hidden));
        if let (Some((held, key)), Some(class), Some(Renewal::Keep)) = (held, class, renew)
            && held == class
        {
            queue.send(key, EffectMessage::Count(count));
            return;
        }
        if let Some((_, key)) = held {
            queue.despawn(key);
            self.sphere_class_keys.remove(&gid);
        }
        if let Some(class) = class.filter(|_| count > 0 && renew != Some(Renewal::Remove)) {
            let key = self.next_key();
            queue.spawn_on_keyed_counted(class, gid, key, count);
            self.sphere_class_keys.insert(gid, (class, key));
        }
    }

    /// Tells a unit's sphere class something its packets brought (a skill spending its servants)
    pub fn tell_sphere_class(&self, queue: &mut EffectQueue, gid: u32, message: EffectMessage) {
        if let Some(&(_, key)) = self.sphere_class_keys.get(&gid) {
            queue.send(key, message);
        }
    }

    /// Takes every body effect off `gid`
    pub fn clear_body_effects(&mut self, queue: &mut EffectQueue, gid: u32) {
        if let Some((_, key)) = self.sphere_class_keys.remove(&gid) {
            queue.despawn(key);
        }
        self.body_effect_keys.retain(|&(g, _), &mut key| {
            if g == gid {
                queue.despawn(key);
            }
            g != gid
        });
    }

    /// The aura on a unit's weapon: one of them, when two statuses each bring their own
    pub fn weapon_aura(&self, gid: u32) -> Option<nostalre_game::effect::WeaponAura> {
        self.weapon_aura
            .iter()
            .find_map(|(&(g, _), &aura)| (g == gid).then_some(aura))
    }
}

#[derive(Default)]
pub struct HoverState {
    pub hovered_entity_id: Option<u32>,
    pub hovered_player_id: Option<u32>,
    /// Set purely so the player's own name plate shows on hover; never a click
    /// or skill target.
    pub hovered_self_id: Option<u32>,
    /// AID of the ground skill unit under the cursor: any trap while a
    /// trap-targeting skill is armed, otherwise only one a plain attack can
    /// hit.
    pub hovered_skill_unit_id: Option<u32>,
    pub hovered_floor_item_id: Option<u32>,
    pub hovered_chat_room: Option<u32>,
    pub hovered_vending: Option<u32>,
    pub hovered_entity_cursor: Option<CursorType>,
    pub cell_cursor: CursorType,
}

impl HoverState {
    /// The id to target for a click: the hovered entity, or the owner of a
    /// hovered vending board (both share the same id).
    pub fn target_id(&self) -> Option<u32> {
        self.hovered_entity_id.or(self.hovered_vending)
    }
}

pub struct CursorInput {
    pub in_game: bool,
    pub right_mouse_down: bool,
    pub ui_any_hovered: bool,
    pub ui_any_interactive_hovered: bool,
    pub item_drag_active: bool,
}

pub struct CursorPending {
    pub companion_target_armed: bool,
    pub pending_companion_skill: bool,
    pub pending_companion_patrol: bool,
    pub capture_targeting: bool,
    pub pending_skill: bool,
    pub marriage_targeting: bool,
}

pub fn cursor_type_from_hover(
    hover: &HoverState,
    input: CursorInput,
    pending: CursorPending,
) -> CursorType {
    if !input.in_game {
        return if input.ui_any_interactive_hovered {
            CursorType::Click
        } else {
            CursorType::Default
        };
    }
    let base = if input.right_mouse_down {
        CursorType::Rotate
    } else if input.item_drag_active {
        CursorType::Default
    } else if input.ui_any_interactive_hovered {
        CursorType::Click
    } else if input.ui_any_hovered {
        CursorType::Default
    } else if pending.companion_target_armed {
        CursorType::Lock2
    } else if pending.pending_companion_skill
        || pending.pending_companion_patrol
        || pending.capture_targeting
        || pending.pending_skill
        || pending.marriage_targeting
    {
        CursorType::Lock
    } else if hover.hovered_chat_room.is_some() {
        CursorType::Click
    } else if hover.hovered_vending.is_some() {
        CursorType::Click
    } else if let Some(cursor) = hover.hovered_entity_cursor {
        cursor
    } else {
        hover.cell_cursor
    };
    if hover.hovered_floor_item_id.is_some() && !input.item_drag_active {
        CursorType::Pick
    } else {
        base
    }
}

#[derive(Default)]
pub struct PendingCasts {
    pub pending_skill_target: Option<PendingSkillTarget>,
    pub pending_companion_skill: Option<PendingCompanionSkill>,
    /// A patrol order awaiting its destination click; the flag is `is_mercenary`.
    pub pending_companion_patrol: Option<bool>,
    pub pending_skill: Option<SkillEnum>,
    pub pending_skill_level: Option<i16>,
    pub pending_ground_cast: Option<(SkillEnum, i16, i16, i16)>,
    /// A trap-targeting skill waiting out the walk to its trap: skill, level and
    /// the trap's unit AID.
    pub pending_skill_unit_cast: Option<(SkillEnum, i16, u32)>,
    pub pending_card_composition_index: Option<u16>,
    /// Skill that opened the shared 0x01ad arrow/converter list, so the reply
    /// can be routed to the right context (the server disambiguates the same way
    /// via menuskill_id).
    pub pending_list_skill: Option<SkillEnum>,
    /// AID of the Repair Weapon target, remembered from the cast so the
    /// server's broken-item list can be attributed back to that player.
    pub pending_repair_target: Option<u32>,
    pub pending_pickup_item_id: Option<u32>,
    /// Shop title submitted with CZ_REQ_OPENSTORE2; applied to our own entity on
    /// open since the server sends ZC_STORE_ENTRY to everyone but us.
    pub pending_shop_name: Option<String>,
    /// Armed by ZC_START_COUPLE: the next click on another player proposes
    /// marriage to them. The original game models it as a pseudo-skill cursor.
    pub marriage_targeting: bool,
}

pub struct Companions {
    pub homunculus: Option<HomunculusState>,
    pub mercenary: Option<MercenaryState>,
    pub elemental: Option<ElementalState>,
    /// The player's summons by unit (an ABR's or a Bionic's HP, 0x0b6b/0x0b6c)
    pub summons: HashMap<u32, SummonHp>,
    pub pet: PetState,
    pub companion_ai: nostalre_ai::config::CompanionAiConfig,
    /// Target armed by the first click of the two-click owner attack, confirmed by
    /// the second. Index 0 = homunculus (Alt+right-click), 1 = mercenary (Alt+left-click).
    pub companion_attack_target: [Option<u32>; 2],
    /// Destination cell of a companion move order, drawn as a red cell until the
    /// companion reaches it. Same indices as `companion_attack_target`.
    pub companion_move_marker: [Option<(i32, i32)>; 2],
    /// Armed by ZC_START_CAPTURE: the next click on a valid mob opens the roulette.
    pub capture_targeting: bool,
    pub pet_roulette: Option<nostalre_game::pet::PetRoulette>,
}

pub struct SessionState {
    pub app_state: AppState,
    pub login_session: Option<Session>,
    pub selected_character: Option<CharacterInfo>,
    pub current_map: Option<String>,
    pub map_properties: MapProperties,
    pub map_coords: Option<MapCoordinates>,
    pub gat: Option<GatFile>,
    /// The map's sign boards and their world points (the cell's ground, raised by the sign's lift)
    pub map_signs: Vec<(nostalre_game::sign_board::SignBoard, [f32; 3])>,
    /// The map's title while it shows (on entering it)
    pub map_title: Option<nostalre_game::map_title::MapTitleBanner>,
    /// Per-cell ground-lightmap tint applied to sprites standing on the map.
    pub actor_lightmap: Option<nostalre_game::lightmap::ActorLightmap>,
    pub player_dead: bool,
    /// Set on indoor maps; locks the camera rotation to the fixed indoor angle.
    pub camera_locked: bool,
    /// Hallucination: ripples the local player's whole screen.
    pub screen_ripple: bool,
    /// Pitch and zoom kept per environment, restored when a map of that kind is
    /// entered again.
    pub saved_camera_outdoor: SavedCameraView,
    pub saved_camera_indoor: SavedCameraView,
    pub server_time: ServerTimeClock,
    pub disconnect_dialog_shown: bool,
    pub pending_disconnect_exit: bool,
    pub progress_bar: Option<ProgressBar>,
    /// Sliding window of `/doridori` head flips, for the sitting SP bonus.
    pub doridori: DoridoriTracker,
}

impl Default for SessionState {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionState {
    /// Our account id, once logged in
    pub fn own_account_id(&self) -> Option<u32> {
        self.login_session.as_ref().map(|s| s.account_id)
    }

    pub fn new() -> Self {
        Self {
            app_state: AppState::Login,
            login_session: None,
            selected_character: None,
            current_map: None,
            map_properties: MapProperties::default(),
            map_coords: None,
            gat: None,
            map_signs: Vec::new(),
            map_title: None,
            actor_lightmap: None,
            player_dead: false,
            camera_locked: false,
            screen_ripple: false,
            saved_camera_outdoor: SavedCameraView::default(),
            saved_camera_indoor: SavedCameraView::default(),
            server_time: ServerTimeClock::new(),
            disconnect_dialog_shown: false,
            progress_bar: None,
            pending_disconnect_exit: false,
            doridori: DoridoriTracker::default(),
        }
    }
}

#[derive(Default)]
pub struct World {
    pub entities: EntityCollection,
    pub floor_items: HashMap<u32, FloorItem>,
    pub arrows: Vec<ArrowProjectile>,
    /// Positioned ground skill units keyed by unit AID: the model-backed traps,
    /// plus the model-less units a plain attack can target (Ice Wall). Only the
    /// visible ones — hidden traps wait in `hidden_traps`.
    pub trap_units: HashMap<u32, TrapUnit>,
    /// Blows landing on a ground skill unit, keyed by unit AID. Units have no
    /// entity to carry the queue an actor would.
    pub skill_unit_hits: HashMap<u32, ScheduledHitQueue>,
    /// Traps placed hidden to us (cast by others); revealed to `trap_units` when
    /// the server sends a skill-unit update (e.g. an ankle snare springs).
    pub hidden_traps: HashMap<u32, TrapUnit>,
    pub freeze_shatters: Vec<FreezeShatter>,
    /// Graffiti ground decals keyed by unit AID.
    pub graffiti: HashMap<u32, Graffiti>,
    /// What each in-progress cast marks out, keyed by caster gid.
    pub cast_marks: HashMap<u32, CastMark>,
    /// Talkie Box text with the world position of the box that holds it, keyed
    /// by skill unit AID. The position is captured when the text arrives: the
    /// server turns the box into a sprung trap immediately afterwards, which
    /// takes it out of `trap_units`.
    pub talkbox_bubbles: HashMap<u32, ([f32; 3], ChatBubbleState)>,
    /// Each unit's last combo window in ms (0x01d2), which its combo blows hold their pose for. Kept for the
    /// session, map changes included, as kRO's table (its 0x01d2 handler's, 0x88b9b0) is never cleared in play.
    pub combo_delays: HashMap<u32, u32>,
}

impl World {
    /// Drop everything the world holds that belongs to the map we are leaving.
    /// Entities are left alone: a map change keeps the player, a logout drops
    /// every one of them.
    pub fn clear_map_scoped(&mut self) {
        self.floor_items.clear();
        self.arrows.clear();
        self.trap_units.clear();
        self.skill_unit_hits.clear();
        self.hidden_traps.clear();
        self.freeze_shatters.clear();
        self.graffiti.clear();
        self.cast_marks.clear();
        self.talkbox_bubbles.clear();
    }
}

#[derive(Clone, Copy)]
pub struct TrapUnit {
    pub unit_id: u16,
    pub world: [f32; 3],
    pub cell: (i16, i16),
}

/// The unit on `cell` that a plain attack can be swung at. A unit occupies
/// exactly one cell, so the hovered cell picks it.
pub fn attackable_unit_at(units: &HashMap<u32, TrapUnit>, cell: (i32, i32)) -> Option<u32> {
    units
        .iter()
        .find(|(_, unit)| {
            unit.cell == (cell.0 as i16, cell.1 as i16)
                && nostalre_game::effect::is_attackable_skill_unit(unit.unit_id)
        })
        .map(|(&aid, _)| aid)
}

/// The marker a cast puts on the world while it channels: a square on the ground
/// for a placed cast, or a reticle riding the victim of a targeted one.
pub enum CastMark {
    Scope(CastScope),
    Lockon { target_gid: u32, remaining: f32 },
}

#[derive(Default)]
pub struct SpriteCaches {
    pub sprites: HashMap<u32, Rc<EntitySprite>>,
    /// Animation state of GR2 model entities (emperium, guardians…) keyed by
    /// gid; the matching draw resources live in `Renderer::gr2_models`.
    pub gr2_models: HashMap<u32, Gr2ModelInstance>,
    /// Skeleton and clips of each loaded `.gr2`, keyed by GRF path.
    pub gr2_assets: HashMap<String, Rc<Gr2Asset>>,
    pub guild_head_sprites: HashMap<u32, Rc<EntitySprite>>,
    pub sprite_cache: HashMap<String, Rc<EntitySprite>>,
    pub carts: HashMap<u32, crate::sprite::CartVisual>,
    pub falcons: HashMap<u32, crate::sprite::FalconVisual>,
    pub cart_preview_sprites: HashMap<u8, Rc<EntitySprite>>,
    pub failed_sprite_loads: HashSet<u32>,
}

#[derive(Default)]
pub struct AssetHandles {
    pub cursor_textures: Option<SpriteTextures>,
    pub cursor_act: Option<ActFile>,
    pub cursor_animation: CursorAnimationState,
    pub lock_cursor_animation: CursorAnimationState,
    /// Marker on a homunculus or mercenary target armed by the first Alt-click.
    pub companion_lock_animation: CursorAnimationState,
    pub emotion_textures: Option<SpriteTextures>,
    pub emotion_act: Option<ActFile>,
    pub status_overlay_sprites: HashMap<AilmentOverlay, (SpriteTextures, ActFile)>,
    pub floor_item_sprites: HashMap<u32, (Rc<SpriteTextures>, ActFile)>,
    /// `shadow.spr` uploaded once, for actors that carry no sprite of their own.
    pub shadow_sprite: Option<(SpriteTextures, ActFile)>,
    pub damage_number_textures: Option<SpriteTextures>,
    pub damage_number_act: Option<nostalre_formats::act::ActFile>,
    pub damage_msg_textures: Option<SpriteTextures>,
    pub damage_msg_act: Option<nostalre_formats::act::ActFile>,
    pub rank_font_textures: Option<SpriteTextures>,
    pub rank_font_act: Option<nostalre_formats::act::ActFile>,
    pub time_font_textures: Option<SpriteTextures>,
    pub time_font_act: Option<nostalre_formats::act::ActFile>,
}

pub struct Schedulers {
    pub ambient_effects: AmbientEffectScheduler,
    pub map_cloud: nostalre_game::map_cloud::MapCloudScheduler,
    pub ambient_sounds: nostalre_game::sound::ambient::AmbientSoundScheduler,
    pub repeat_sounds: nostalre_game::sound::repeat::RepeatSoundScheduler,
    pub day_night: DayNightState,
}

impl Default for Schedulers {
    fn default() -> Self {
        Self::new()
    }
}

impl Schedulers {
    pub fn new() -> Self {
        Self {
            ambient_effects: AmbientEffectScheduler::empty(),
            map_cloud: nostalre_game::map_cloud::MapCloudScheduler::default(),
            ambient_sounds: nostalre_game::sound::ambient::AmbientSoundScheduler::empty(),
            repeat_sounds: nostalre_game::sound::repeat::RepeatSoundScheduler::new(),
            day_night: DayNightState::default(),
        }
    }
}

pub struct Broadcast {
    pub banner: BannerState,
    /// kRO's notice window: broadcasts and item announces
    pub notice: nostalre_game::notice::NoticeWindow,
    pub poptip: PoptipStack,
    pub broadcast_last_elapsed: Option<f32>,
}

impl Default for Broadcast {
    fn default() -> Self {
        Self::new()
    }
}

impl Broadcast {
    pub fn new() -> Self {
        Self {
            banner: BannerState::new(),
            notice: Default::default(),
            poptip: PoptipStack::new(),
            broadcast_last_elapsed: None,
        }
    }
}

pub struct Prefs {
    pub noshift_mode: bool,
    pub noctrl_mode: bool,
    pub show_exp: bool,
    pub hide_public_chat: bool,
    pub show_miss: bool,
    pub blocked_whispers: Vec<String>,
    pub self_config: SelfConfig,
}

impl Default for Prefs {
    fn default() -> Self {
        Self::new()
    }
}

impl Prefs {
    pub fn new() -> Self {
        Self {
            noshift_mode: false,
            noctrl_mode: true,
            show_exp: true,
            hide_public_chat: false,
            show_miss: true,
            blocked_whispers: Vec::new(),
            self_config: SelfConfig::default(),
        }
    }
}

pub struct GameState {
    pub session: SessionState,
    pub requested_guild_emblems: HashSet<(u32, i32)>,
    pub world: World,
    pub sprite_caches: SpriteCaches,
    pub character: Character,
    pub data_table: DataTable,
    pub assets: AssetHandles,
    pub broadcast: Broadcast,
    pub pending_confirms: PendingConfirms,
    /// NPC cutin illustrations by position slot (0 left / 1 middle / 2 right);
    /// GRF base name under `data/texture/유저인터페이스/illust/`.
    pub npc_cutins: [Option<String>; 3],
    pub chat_rooms: ChatRoomRegistry,
    pub pending_chat_room: Option<(String, i16, bool)>,
    pub hover: HoverState,
    pub pending_casts: PendingCasts,
    pub combat: CombatState,
    pub prefs: Prefs,
    pub party: Option<Party>,
    pub guild: Option<nostalre_game::guild::Guild>,
    pub guild_menu_flag: i32,
    pub friends: nostalre_game::friends::FriendList,
    pub companions: Companions,
    pub quest_log: QuestLog,
    /// Over-NPC quest markers keyed by NPC block id (account-id space). Cleared
    /// on map change; the server re-sends on load.
    pub quest_markers: std::collections::HashMap<u32, QuestMarker>,
    /// Marks the server put on the minimap, e.g. the town guide's directions.
    pub minimap_marks: nostalre_game::minimap_mark::MinimapMarks,
    /// The player's last chat lines, for a Super Novice's prayer (0x01ed)
    pub prayer_lines: nostalre_game::prayer::PrayerLines,
    /// The last line sent and its repeats, for kRO's refusal of the same line a third time
    pub repeated_line: nostalre_game::chat_repeat::RepeatGuard,
    /// An NPC's navigation guide (0x08e2), until the player reaches its cell or another comes
    pub navigation: Option<nostalre_game::navigation::Navigation>,
    /// The units the player's Crimson Marker marks (0x09c1), drawn on the minimap
    pub crimson_marks: nostalre_game::minimap_mark::CrimsonMarks,
    /// The map's MVP as last reported to a Convex Mirror.
    pub boss_mark: Option<nostalre_game::boss_info::BossMark>,
    pub show_digit: Option<nostalre_game::show_digit::ShowDigitClock>,
    pub debug_show_pick_bounds: bool,
    pub show_ping: bool,
    pub show_fps: bool,
    pub schedulers: Schedulers,
    pub effect_keys: EffectKeys,
}

pub const COMPANION_AI_CONFIG_PATH: &str = "companion_ai.json";

/// Item id of the Token of Siegfried, which enables standing resurrection.
pub(crate) const TOKEN_OF_SIEGFRIED: u32 = 7621;

impl GameState {
    /// Resolves a skill's cast metadata `(target type, attack range)` from the
    /// player's skills first, then the mercenary's, then the homunculus'.
    /// Companion skill IDs live in their own ranges, so the lookup order is
    /// unambiguous. The cast packet is identical for all three; the server
    /// attributes companion-range IDs to the companion.
    pub fn resolve_cast_skill(&self, skill: SkillEnum) -> Option<(SkillTargetType, i16)> {
        if let Some(s) = self.character.skills.get_skill(skill) {
            return Some((s.skill_target_type, s.attack_range));
        }
        if let Some(m) = &self.companions.mercenary
            && let Some(s) = m.skills.iter().find(|s| s.skill == skill)
        {
            return Some((s.skill_target_type, s.attack_range));
        }
        if let Some(h) = &self.companions.homunculus
            && let Some(s) = h.skills.iter().find(|s| s.skill == skill)
        {
            return Some((s.skill_target_type, s.attack_range));
        }
        self.character
            .item_skills
            .get(skill)
            .map(|s| (s.skill_target_type, s.attack_range))
    }

    /// Running or Warg Dashing: the server moves us on its own and refuses a move request (`clif.cpp`,
    /// `clif_parse_WalkToXY`)
    pub fn player_moves_on_its_own(&self) -> bool {
        let warg_dash = ClientEffectIcon::Wugdash.value() as i16;
        self.world.entities.player().is_some_and(|e| e.is_running)
            || self.character.has_status(warg_dash)
    }

    /// Whether the running cast pins the player in place: the server's own rule
    /// for a move request (`unit_can_move`), so the two never disagree. Exceed
    /// Break never pins; Free Cast releases everything but guild skills.
    pub fn casting_blocks_action(&self) -> bool {
        let Some(player) = self.world.entities.player() else {
            return false;
        };
        if player.state() != EntityState::Casting
            || player.active_skill == Some(SkillEnum::LgExeedbreak)
        {
            return false;
        }
        !self.character.skills.knows(SkillEnum::SaFreecast)
            || player.active_skill.is_some_and(|skill| skill.is_guild())
    }

    /// Reads the player's cart design out of the `OPTION_CART` bits carried in
    /// `effect_state` and records it on the entity and character. Returns the
    /// `(player gid, design)` to spawn, or `None` when the player has no cart.
    /// Servers that deliver the cart as a status change instead leave these
    /// bits clear, so this is a no-op there and the status path takes over.
    pub fn player_cart_from_option(&mut self) -> Option<(u32, u8)> {
        let pid = self.world.entities.player_id()?;
        let design = nostalre_game::sprite_path::cart_design_from_option(
            self.world.entities.get(pid)?.effect_state,
        )?;
        self.character.cart_design = Some(design);
        if let Some(entity) = self.world.entities.get_mut(pid) {
            entity.cart_type = Some(design);
        }
        Some((pid, design))
    }

    pub fn reset_effects(&mut self, queue: &mut EffectQueue) {
        queue.clear();
        self.effect_keys.clear();
        self.schedulers.map_cloud.forget_spawned();
        self.schedulers.ambient_effects.forget_spawned();
    }

    pub fn new() -> Self {
        Self {
            session: SessionState::new(),
            requested_guild_emblems: HashSet::new(),
            world: World::default(),
            sprite_caches: SpriteCaches::default(),
            character: Character::new(),
            data_table: DataTable::new(),
            assets: AssetHandles::default(),
            broadcast: Broadcast::new(),
            pending_confirms: PendingConfirms::default(),
            npc_cutins: [None, None, None],
            chat_rooms: ChatRoomRegistry::new(),
            pending_chat_room: None,
            hover: HoverState::default(),
            pending_casts: PendingCasts::default(),
            combat: CombatState::new(),
            prefs: Prefs::new(),
            party: None,
            guild: None,
            guild_menu_flag: 0,
            friends: nostalre_game::friends::FriendList::default(),
            companions: Companions {
                homunculus: None,
                mercenary: None,
                elemental: None,
                summons: HashMap::new(),
                pet: PetState::default(),
                companion_ai: nostalre_ai::config::CompanionAiConfig::load_or_default(
                    COMPANION_AI_CONFIG_PATH,
                ),
                companion_attack_target: [None; 2],
                companion_move_marker: [None; 2],
                capture_targeting: false,
                pet_roulette: None,
            },
            quest_log: QuestLog::default(),
            quest_markers: std::collections::HashMap::new(),
            minimap_marks: nostalre_game::minimap_mark::MinimapMarks::default(),
            prayer_lines: Default::default(),
            repeated_line: Default::default(),
            navigation: None,
            crimson_marks: nostalre_game::minimap_mark::CrimsonMarks::default(),
            boss_mark: None,
            show_digit: None,
            debug_show_pick_bounds: false,
            show_ping: false,
            show_fps: false,
            schedulers: Schedulers::new(),
            effect_keys: EffectKeys::default(),
        }
    }

    pub fn arm_confirm(
        &mut self,
        windows: &mut Windows,
        message: &str,
        ctor: impl FnOnce(bool) -> Option<GameEvent> + 'static,
    ) {
        self.pending_confirms.active = Some(Box::new(ctor));
        windows.confirm_dialog.show_confirm(message);
    }

    /// Returns true when the request must be auto-refused; otherwise records the
    /// pending request and opens the accept dialog.
    pub fn begin_trade_request(
        &mut self,
        windows: &mut Windows,
        name: String,
        gid: u32,
        auto_refuse: bool,
    ) -> bool {
        if auto_refuse {
            return true;
        }
        self.pending_confirms.pending_trade_partner = Some((gid, name.clone()));
        self.pending_confirms.pending_trade_request = Some(gid);
        self.arm_confirm(
            windows,
            &format!("Do you want to trade with {name}?"),
            |accept| Some(GameEvent::RespondExchangeRequest { accept }),
        );
        false
    }

    pub fn apply_window_state(
        &mut self,
        windows: &mut Windows,
        window_state: &HashMap<u32, WindowStateEntry>,
    ) {
        if let Some(entry) = window_state.get(&INV_WINDOW_ID.0) {
            if entry.open {
                self.character.inventory.open();
            }
            windows.inventory_window.set_minimized(entry.collapsed);
        }
        if let Some(entry) = window_state.get(&EQ_WINDOW_ID.0) {
            windows.equipment_window.open = entry.open;
            windows.equipment_window.set_minimized(entry.collapsed);
        }
        if let Some(entry) = window_state.get(&SKILL_WINDOW_ID.0)
            && entry.open
        {
            self.character.skills.open();
        }
        if let Some(entry) = window_state.get(&chat_window::CHAT_WINDOW_ID.0) {
            let size_index = if !entry.open {
                0
            } else if entry.collapsed {
                1
            } else {
                5
            };
            windows.chat_window.set_initial_size_index(size_index);
        }
    }

    pub fn extract_window_state(
        &self,
        windows: &Windows,
        state_cache: &StateCache,
    ) -> HashMap<u32, (bool, bool)> {
        let mut result = HashMap::new();
        result.insert(
            INV_WINDOW_ID.0,
            (
                self.character.inventory.is_open(),
                windows.inventory_window.is_minimized(),
            ),
        );
        result.insert(
            EQ_WINDOW_ID.0,
            (
                windows.equipment_window.is_open(),
                windows.equipment_window.is_minimized(),
            ),
        );
        result.insert(SKILL_WINDOW_ID.0, (self.character.skills.is_open(), false));
        let size_index = windows.chat_window.get_size_index(state_cache);
        result.insert(
            chat_window::CHAT_WINDOW_ID.0,
            (size_index > 0, size_index == 1),
        );
        result
    }
}

#[cfg(test)]
mod cart_option_tests {
    use super::*;
    use nostalre_game::entity::Entity;

    fn with_player(effect_state: i32) -> GameState {
        let mut game = GameState::new();
        let mut player = Entity::new_player(1000, 0, 1, 0, 0, 0, 0, 0, 0, 0, 5, 5, 0);
        player.effect_state = effect_state;
        game.world.entities.set_player_id(1000);
        game.world.entities.insert(player);
        game
    }

    #[test]
    fn spawns_player_cart_from_option_bit() {
        let mut game = with_player(0x100);
        assert_eq!(game.player_cart_from_option(), Some((1000, 3)));
        assert_eq!(game.character.cart_design, Some(3));
        assert_eq!(game.world.entities.get(1000).unwrap().cart_type, Some(3));
    }

    #[test]
    fn no_cart_when_option_bit_clear() {
        let mut game = with_player(0);
        assert_eq!(game.player_cart_from_option(), None);
        assert_eq!(game.character.cart_design, None);
    }
}

#[cfg(test)]
mod free_cast_tests {
    use super::*;
    use nostalre_game::entity::Entity;
    use nostalre_game::event::SkillInfo;

    fn casting_player(skill: SkillEnum) -> GameState {
        let mut game = GameState::new();
        let mut player = Entity::new_player(1000, 0, 1, 0, 0, 0, 0, 0, 0, 0, 5, 5, 0);
        player.enter_casting(2.0, skill);
        game.world.entities.set_player_id(1000);
        game.world.entities.insert(player);
        game
    }

    fn learn_free_cast(game: &mut GameState) {
        list_free_cast(game, 5);
    }

    fn list_free_cast(game: &mut GameState, level: i16) {
        game.character.skills.apply_skill_list(vec![SkillInfo {
            skill: SkillEnum::SaFreecast,
            level,
            sp_cost: 0,
            attack_range: 0,
            upgradable: false,
            skill_target_type: SkillTargetType::Passive,
        }]);
    }

    #[test]
    fn free_cast_releases_the_cast_pin_except_for_guild_skills() {
        let mut game = casting_player(SkillEnum::MgFirebolt);
        assert!(game.casting_blocks_action());

        learn_free_cast(&mut game);
        assert!(!game.casting_blocks_action());

        let mut guild_cast = casting_player(SkillEnum::GdBattleorder);
        learn_free_cast(&mut guild_cast);
        assert!(guild_cast.casting_blocks_action());
    }

    #[test]
    fn running_or_warg_dashing_moves_on_its_own() {
        let mut game = casting_player(SkillEnum::MgFirebolt);
        assert!(!game.player_moves_on_its_own());
        let warg_dash = ClientEffectIcon::Wugdash.value() as i16;
        game.character.apply_status(warg_dash, 0, 0, 0, true);
        assert!(game.player_moves_on_its_own());
        game.character.clear_status(warg_dash);
        game.world.entities.player_mut().unwrap().is_running = true;
        assert!(game.player_moves_on_its_own());
    }

    #[test]
    fn exceed_break_moves_while_casting_and_an_unlearned_free_cast_does_not_free() {
        assert!(!casting_player(SkillEnum::LgExeedbreak).casting_blocks_action());

        // the server lists Free Cast at level 0 before it is learned
        let mut game = casting_player(SkillEnum::MgFirebolt);
        list_free_cast(&mut game, 0);
        assert!(game.casting_blocks_action());
    }

    #[test]
    fn a_player_who_is_not_casting_is_never_pinned() {
        let mut game = casting_player(SkillEnum::MgFirebolt);
        game.world.entities.apply_skill_cast_cancel(1000);
        assert!(!game.casting_blocks_action());
    }
}

#[cfg(test)]
mod skill_resolve_tests {
    use super::*;
    use nostalre_game::companion::MercenaryState;
    use nostalre_game::event::SkillInfo;

    #[test]
    fn resolves_mercenary_skill_metadata_for_cast() {
        let mut game = GameState::new();
        let mut merc = MercenaryState::new(2000);
        merc.skills = vec![SkillInfo {
            skill: SkillEnum::MsBash,
            level: 3,
            sp_cost: 15,
            attack_range: 9,
            upgradable: false,
            skill_target_type: SkillTargetType::Target,
        }];
        game.companions.mercenary = Some(merc);

        assert_eq!(
            game.resolve_cast_skill(SkillEnum::MsBash),
            Some((SkillTargetType::Target, 9))
        );
        assert_eq!(game.resolve_cast_skill(SkillEnum::SmBash), None);
    }

    #[test]
    fn item_granted_skill_arms_its_own_target_class_and_range() {
        use models::enums::skill_enums::SkillEnum;
        use nostalre_game::skill::ItemSkill;
        use nostalre_game::targeting::{TargetClass, skill_target_class};

        let mut game = GameState::new();
        let resurrection = SkillEnum::AllResurrection;
        let firewall = SkillEnum::MgFirewall;
        assert_eq!(game.resolve_cast_skill(resurrection), None);

        game.character.item_skills.insert(
            resurrection,
            ItemSkill {
                level: 1,
                sp_cost: 60,
                attack_range: 9,
                skill_target_type: SkillTargetType::Friend,
            },
        );
        game.character.item_skills.insert(
            firewall,
            ItemSkill {
                level: 1,
                sp_cost: 40,
                attack_range: 9,
                skill_target_type: SkillTargetType::Ground,
            },
        );

        let (target_type, range) = game.resolve_cast_skill(resurrection).unwrap();
        assert_eq!(skill_target_class(target_type), TargetClass::Supportive);
        assert_eq!(range, 9);
        let (target_type, _) = game.resolve_cast_skill(firewall).unwrap();
        assert_eq!(skill_target_class(target_type), TargetClass::Ground);

        game.character.clear();
        assert_eq!(game.resolve_cast_skill(resurrection), None);
    }
}

#[cfg(test)]
mod trade_request_tests {
    use super::*;

    #[test]
    fn auto_refuse_skips_dialog_and_normal_path_opens_it() {
        let mut game = GameState::new();
        let mut windows = Windows::new();
        assert!(game.begin_trade_request(&mut windows, "Alice".to_string(), 42, true));
        assert!(game.pending_confirms.pending_trade_request.is_none());
        assert!(game.pending_confirms.pending_trade_partner.is_none());
        assert!(windows.confirm_dialog.state.is_none());

        assert!(!game.begin_trade_request(&mut windows, "Alice".to_string(), 42, false));
        assert_eq!(game.pending_confirms.pending_trade_request, Some(42));
        assert_eq!(
            game.pending_confirms.pending_trade_partner,
            Some((42, "Alice".to_string()))
        );
        assert!(windows.confirm_dialog.state.is_some());
    }
}

#[cfg(test)]
mod pending_confirms_tests {
    use super::*;

    #[test]
    fn arm_confirm_dispatches_on_accept_and_clears_on_refuse() {
        let mut game = GameState::new();
        let mut windows = Windows::new();

        game.arm_confirm(&mut windows, "Join party?", |accept| {
            Some(GameEvent::RespondPartyInvite {
                party_grid: 7,
                accept,
            })
        });
        assert!(windows.confirm_dialog.state.is_some());
        assert!(matches!(
            game.pending_confirms.dispatch(ConfirmResult::Ok),
            Some(GameEvent::RespondPartyInvite {
                party_grid: 7,
                accept: true
            })
        ));
        assert!(game.pending_confirms.dispatch(ConfirmResult::Ok).is_none());

        game.arm_confirm(&mut windows, "Feed pet?", |accept| {
            accept.then_some(GameEvent::RequestPetCommand { csub: 1 })
        });
        assert!(
            game.pending_confirms
                .dispatch(ConfirmResult::Cancel)
                .is_none()
        );
    }
}

#[cfg(test)]
mod window_state_persistence_tests {
    use super::*;

    #[test]
    fn closed_skill_window_persists_and_restores_closed() {
        let cache = StateCache::new();
        let mut game = GameState::new();
        let windows = Windows::new();
        game.character.skills.open();
        assert_eq!(
            game.extract_window_state(&windows, &cache)
                .get(&SKILL_WINDOW_ID.0),
            Some(&(true, false))
        );

        game.character.skills.close();
        let captured = game.extract_window_state(&windows, &cache);
        assert_eq!(captured.get(&SKILL_WINDOW_ID.0), Some(&(false, false)));

        let (open, collapsed) = captured[&SKILL_WINDOW_ID.0];
        let mut window_state = HashMap::new();
        window_state.insert(
            SKILL_WINDOW_ID.0,
            WindowStateEntry {
                position: [0.0, 0.0],
                open,
                collapsed,
            },
        );
        let mut next_login = GameState::new();
        let mut next_windows = Windows::new();
        next_login.apply_window_state(&mut next_windows, &window_state);
        assert!(!next_login.character.skills.is_open());
    }
}

#[cfg(test)]
mod cursor_from_hover_tests {
    use super::*;

    fn in_game() -> CursorInput {
        CursorInput {
            in_game: true,
            right_mouse_down: false,
            ui_any_hovered: false,
            ui_any_interactive_hovered: false,
            item_drag_active: false,
        }
    }

    fn no_pending() -> CursorPending {
        CursorPending {
            companion_target_armed: false,
            pending_companion_skill: false,
            pending_companion_patrol: false,
            capture_targeting: false,
            pending_skill: false,
            marriage_targeting: false,
        }
    }

    #[test]
    fn cursor_follows_hover_priority_chain() {
        let mut hover = HoverState::default();

        hover.cell_cursor = CursorType::NoWalk;
        assert_eq!(
            cursor_type_from_hover(&hover, in_game(), no_pending()),
            CursorType::NoWalk
        );

        hover.hovered_entity_cursor = Some(CursorType::Attack);
        assert_eq!(
            cursor_type_from_hover(&hover, in_game(), no_pending()),
            CursorType::Attack
        );

        hover.hovered_vending = Some(7);
        assert_eq!(
            cursor_type_from_hover(&hover, in_game(), no_pending()),
            CursorType::Click
        );

        hover.hovered_chat_room = Some(3);
        assert_eq!(
            cursor_type_from_hover(&hover, in_game(), no_pending()),
            CursorType::Click
        );

        let pending = CursorPending {
            pending_skill: true,
            ..no_pending()
        };
        assert_eq!(
            cursor_type_from_hover(&hover, in_game(), pending),
            CursorType::Lock
        );

        hover.hovered_floor_item_id = Some(1);
        assert_eq!(
            cursor_type_from_hover(&hover, in_game(), no_pending()),
            CursorType::Pick
        );

        let dragging = CursorInput {
            right_mouse_down: true,
            ..in_game()
        };
        assert_eq!(
            cursor_type_from_hover(&HoverState::default(), dragging, no_pending()),
            CursorType::Rotate
        );

        let out_of_game = CursorInput {
            in_game: false,
            ui_any_interactive_hovered: true,
            ..in_game()
        };
        assert_eq!(
            cursor_type_from_hover(&hover, out_of_game, no_pending()),
            CursorType::Click
        );
    }

    #[test]
    fn item_drag_keeps_default_cursor_over_world_targets() {
        let mut hover = HoverState::default();
        hover.cell_cursor = CursorType::NoWalk;
        hover.hovered_entity_cursor = Some(CursorType::Attack);
        hover.hovered_floor_item_id = Some(1);

        let dragging = CursorInput {
            item_drag_active: true,
            ..in_game()
        };
        assert_eq!(
            cursor_type_from_hover(&hover, dragging, no_pending()),
            CursorType::Default
        );

        let dragging_over_ui = CursorInput {
            item_drag_active: true,
            ui_any_interactive_hovered: true,
            ..in_game()
        };
        assert_eq!(
            cursor_type_from_hover(&hover, dragging_over_ui, no_pending()),
            CursorType::Default
        );

        let rotating = CursorInput {
            item_drag_active: true,
            right_mouse_down: true,
            ..in_game()
        };
        assert_eq!(
            cursor_type_from_hover(&hover, rotating, no_pending()),
            CursorType::Rotate,
            "camera rotation feedback still wins over a drag"
        );
    }
}

#[cfg(test)]
mod effect_reset_tests {
    use super::*;

    #[test]
    fn reset_effects_wipes_queue_and_every_key_map_together() {
        let mut game = GameState::new();
        let mut queue = EffectQueue::new();
        queue.spawn_on_keyed(EffectId::Blessing, 1, 1);
        queue.despawn(2);

        let keys = &mut game.effect_keys;
        keys.status_buff_keys.insert((1, 2), 3);
        keys.opt3_keys.insert((1, 2), 3);
        keys.next_status_buff_key = 9;
        keys.level_aura_keys.insert(1, (1, &[]));
        keys.boss_aura_keys.insert(1, 1);
        keys.toprank_keys.insert(1, (1, 1));
        keys.warp_portal_keys.insert(1, 1);
        keys.body_effect_keys.insert(
            (1, Trigger::Counter(crate::body_effect::Counter::Spirits)),
            1,
        );
        keys.sight_aura_keys.insert(1, 1);
        keys.ruwach_aura_keys.insert(1, 1);
        keys.blind_overlay_key = Some(1);
        keys.weather_keys.insert(EffectId::Snow, 1);

        game.schedulers.map_cloud.set_map("gonryun");
        game.schedulers.map_cloud.update(Some(1), true, &mut queue);
        queue.drain();

        game.reset_effects(&mut queue);

        assert!(queue.drain().is_empty());
        assert!(queue.drain_despawns().is_empty());
        let keys = &game.effect_keys;
        assert!(keys.status_buff_keys.is_empty());
        assert!(keys.opt3_keys.is_empty());
        assert_eq!(keys.next_status_buff_key, 0);
        assert!(keys.level_aura_keys.is_empty());
        assert!(keys.boss_aura_keys.is_empty());
        assert!(keys.toprank_keys.is_empty());
        assert!(keys.warp_portal_keys.is_empty());
        assert!(keys.body_effect_keys.is_empty());
        assert!(keys.sight_aura_keys.is_empty());
        assert!(keys.ruwach_aura_keys.is_empty());
        assert!(keys.blind_overlay_key.is_none());
        assert!(keys.weather_keys.is_empty());

        game.schedulers.map_cloud.update(Some(1), true, &mut queue);
        assert_eq!(
            queue.drain().len(),
            1,
            "a wiped effect world respawns the cloud"
        );
    }
}

#[cfg(test)]
mod attackable_unit_tests {
    use super::*;

    fn unit(unit_id: u16, cell: (i16, i16)) -> TrapUnit {
        TrapUnit {
            unit_id,
            world: [cell.0 as f32, 0.0, cell.1 as f32],
            cell,
        }
    }

    #[test]
    fn only_the_units_a_swing_can_reach_are_picked_from_the_hovered_cell() {
        const UNT_BLASTMINE: u16 = 0x8f;
        const UNT_ICEWALL: u16 = 0x8d;
        const UNT_ANKLESNARE: u16 = 0x91;

        let mut units = HashMap::new();
        units.insert(1, unit(UNT_ANKLESNARE, (10, 10)));
        units.insert(2, unit(UNT_BLASTMINE, (11, 10)));
        units.insert(3, unit(UNT_ICEWALL, (12, 10)));

        assert_eq!(attackable_unit_at(&units, (10, 10)), None);
        assert_eq!(attackable_unit_at(&units, (11, 10)), Some(2));
        assert_eq!(attackable_unit_at(&units, (12, 10)), Some(3));
        assert_eq!(attackable_unit_at(&units, (11, 11)), None);
    }
}

#[cfg(test)]
mod map_scoped_reset_tests {
    use super::*;
    use nostalre_game::entity::Entity;

    #[test]
    fn leaving_a_map_drops_traps_and_every_other_ground_object_but_keeps_entities() {
        let mut game = GameState::new();
        let player = Entity::new_player(1000, 0, 1, 0, 0, 0, 0, 0, 0, 0, 5, 5, 0);
        game.world.entities.set_player_id(1000);
        game.world.entities.insert(player);

        let trap = TrapUnit {
            unit_id: 0x90,
            world: [10.0, 0.0, 10.0],
            cell: (10, 10),
        };
        game.world.trap_units.insert(1, trap);
        game.world.hidden_traps.insert(2, trap);
        game.world.graffiti.insert(
            3,
            Graffiti {
                creator_aid: 1000,
                cell_x: 5,
                cell_y: 5,
                yaw: 0.0,
                message: "hi".to_string(),
            },
        );
        game.world.talkbox_bubbles.insert(
            4,
            ([0.0, 0.0, 0.0], ChatBubbleState::new("box".to_string())),
        );
        game.world.cast_marks.insert(
            1000,
            CastMark::Lockon {
                target_gid: 2000,
                remaining: 1.0,
            },
        );

        game.world.clear_map_scoped();

        assert!(game.world.trap_units.is_empty());
        assert!(game.world.hidden_traps.is_empty());
        assert!(game.world.graffiti.is_empty());
        assert!(game.world.talkbox_bubbles.is_empty());
        assert!(game.world.cast_marks.is_empty());
        assert!(game.world.entities.player().is_some());
    }
}

#[cfg(test)]
mod body_effect_tests {
    use super::*;
    use crate::body_effect::Counter;
    use nostalre_game::effect::effects::body_aura::{CHOOKGI, orbs_only};

    fn spawns_and_despawns(queue: &mut EffectQueue) -> (Vec<u32>, Vec<u32>) {
        let spawned = queue.drain().into_iter().filter_map(|r| r.key).collect();
        (spawned, queue.drain_despawns())
    }

    #[test]
    fn a_counter_swaps_its_body_effect_and_zero_takes_it_off() {
        let (mut keys, mut queue) = (EffectKeys::default(), EffectQueue::default());
        let spirits = Trigger::Counter(Counter::Spirits);
        let spheres = Some(orbs_only(CHOOKGI));
        keys.set_body_effect(&mut queue, 7, spirits.clone(), spheres, 3);
        let (first, gone) = spawns_and_despawns(&mut queue);
        assert_eq!((first.len(), gone.len()), (1, 0));
        keys.set_body_effect(&mut queue, 7, spirits.clone(), spheres, 4);
        let (second, gone) = spawns_and_despawns(&mut queue);
        assert_eq!(gone, first, "the 3 spheres go");
        assert_eq!(second.len(), 1, "4 come");
        keys.set_body_effect(&mut queue, 7, spirits, None, 0);
        assert_eq!(spawns_and_despawns(&mut queue), (vec![], second));
        assert!(keys.body_effect_keys.is_empty());
    }

    #[test]
    fn a_status_sent_again_keeps_its_body_effect_until_it_ends() {
        let (mut keys, mut queue) = (EffectKeys::default(), EffectQueue::default());
        let blade = Trigger::Status("ENCHANTBLADE".into());
        let glow = Some(BodyAuraSpec::default());
        keys.set_body_effect(&mut queue, 7, blade.clone(), glow, 1);
        let (first, _) = spawns_and_despawns(&mut queue);
        keys.set_body_effect(&mut queue, 7, blade.clone(), glow, 1);
        assert_eq!(
            spawns_and_despawns(&mut queue),
            (vec![], vec![]),
            "no fade-in again"
        );
        keys.set_body_effect(&mut queue, 7, blade, None, 0);
        assert_eq!(spawns_and_despawns(&mut queue), (vec![], first));
    }

    #[test]
    fn a_unit_leaving_takes_its_body_effects_and_no_others() {
        let (mut keys, mut queue) = (EffectKeys::default(), EffectQueue::default());
        let spheres = Some(orbs_only(CHOOKGI));
        for (gid, counter) in [
            (7, Counter::Spirits),
            (7, Counter::SoulEnergy),
            (8, Counter::Spirits),
        ] {
            keys.set_body_effect(&mut queue, gid, Trigger::Counter(counter), spheres, 1);
        }
        queue.drain();
        keys.clear_body_effects(&mut queue, 7);
        assert_eq!(queue.drain_despawns().len(), 2);
        assert_eq!(keys.body_effect_keys.len(), 1);
    }
}
