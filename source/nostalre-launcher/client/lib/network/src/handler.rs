// Narrowing a server field shows in clippy (CLAUDE.md, "Protocol work": the value check): the 24 left at
// October 14 hold the values rAthena sends (stacks to 30000, counts to 20, unit ids under 0x190).
#![warn(clippy::cast_possible_truncation)]

use models::enums::EnumWithNumberValue;
use models::enums::action::ActionType;
use models::enums::skill::SkillTargetType;
use models::enums::skill_enums::SkillEnum;
use models::enums::status::StatusTypes;
use models::enums::vanish::VanishType;
use packets::packets::*;
use nostalre_game::banner::BannerKind;
use nostalre_game::boss_info::BossInfoKind;
use nostalre_game::chat_room::ChatRoomMember;
use nostalre_game::event::{
    AccessibleMap, BarterCost, BarterItem, CharacterInfo, DialogPlacement, FameKind, FriendData,
    GameEvent, GuildMemberAppearance, HomunculusProperty, MercenaryInfo, MvpFeedbackKind,
    PartyMemberData, PetProperty, Ranking, SelfConfigKind, ServerInfo, SkillInfo,
    SpecialItemSource, StoragePasswordOutcome, StoragePasswordPrompt,
};
use nostalre_game::gm::GmStatus;
use nostalre_game::guild::{GuildBanEntry, GuildMember, GuildPosition, GuildRelation, GuildSkill};
use nostalre_game::inventory::{EquipmentItemData, INVENTORY_BASE_SLOTS, NormalItemData};
use nostalre_game::item::item_options;
use nostalre_game::mail::{MailEntry, MailItem, OpenedMail};
use nostalre_game::minimap_mark::MarkAction;
use nostalre_game::monster_info::MonsterInfo;
use nostalre_game::quest::{Quest, QuestHuntEntry, QuestObjective};
use nostalre_game::show_digit::ShowDigitMode;
use nostalre_game::targeting::{MapKind, MapProperties};
use tracing::debug;

use crate::helpers::{decode_pos, decode_pos2};

/// `SkillEnum` holds every skill of rAthena's skill db (`scripts/skill-enum.py`); an id outside it gives `None`
/// instead of a panic, and its event is dropped or downgraded.
fn known_skill(id: u32) -> Option<SkillEnum> {
    let skill = SkillEnum::try_from_value(id).ok();
    if skill.is_none() {
        debug!("unknown skill id {id}");
    }
    skill
}

fn character_info_from_neo_union(info: &CharacterInfoNeoUnion) -> CharacterInfo {
    let name: String = cstr(&info.name);
    let map: String = cstr(&info.last_map);
    let clamp = |v: i64| v.clamp(0, u32::MAX as i64) as u32;

    CharacterInfo {
        gid: info.gid,
        name,
        class: info.class,
        base_level: info.level,
        base_exp: info.exp_64,
        job_level: info.joblevel,
        map,
        slot: info.char_num,
        head: info.head,
        hair_color: info.hair_color,
        cloth_color: info.body_color,
        weapon: info.weapon,
        head_top: info.head_top,
        head_mid: info.head_mid,
        head_bottom: info.head_bottom,
        shield: info.shield,
        robe: info.robe as u16,
        sex: info.sex,
        hp: clamp(info.hp_64),
        max_hp: clamp(info.maxhp_64),
        sp: clamp(info.sp_64),
        max_sp: clamp(info.maxsp_64),
        str: info.str,
        agi: info.agi,
        vit: info.vit,
        int: info.int,
        dex: info.dex,
        luk: info.luk,
        effect_state: info.effectstate,
        zeny: info.money as i32,
    }
}

fn push_opt3(events: &mut Vec<GameEvent>, gid: u32, effect_state: i32, base_level: i32, opt3: i32) {
    if opt3 != 0 {
        events.push(GameEvent::EntityOpt3Changed {
            gid,
            effect_state,
            base_level,
            opt3,
        });
    }
}

/// rAthena's `OUT_UI_STYLIST`
const UI_STYLIST: u8 = 1;

/// A market's or a barter's stock: -1 is none kept (`npc_market`, `npc_barter`), sent in an unsigned field too
fn shop_stock(amount: i32) -> Option<u32> {
    u32::try_from(amount).ok()
}

/// One of 0x07fd's length-prefixed fields: a length byte, then that many bytes. None when the bytes end first
fn take_field<'a>(rest: &mut &'a [u8]) -> Option<&'a [u8]> {
    let (&len, tail) = rest.split_first()?;
    let field = tail.get(..usize::from(len))?;
    *rest = &tail[usize::from(len)..];
    Some(field)
}

/// 0x07fd after its item id, read by its fields' lengths as kRO's exe does (0x7b7ef0): the player's name, then by
/// type the box item's id, the monster's name or an empty field, then from 20220518 a box item's refine. rAthena's
/// length counts one byte more after a monster or NPC field, left unread.
fn special_item_obtained(kind: u8, item_id: u32, mut rest: &[u8]) -> Option<GameEvent> {
    let player = raw_euc_kr(take_field(&mut rest)?);
    let source = take_field(&mut rest)?;
    let source = match kind {
        0 => SpecialItemSource::Box(u32::from_le_bytes(source.try_into().ok()?)),
        1 => SpecialItemSource::Monster(raw_euc_kr(source)),
        2 => SpecialItemSource::Npc,
        other => {
            debug!("unknown ZC_BROADCASTING_SPECIAL_ITEM_OBTAIN type: {other}");
            return None;
        }
    };
    let refine = match source {
        SpecialItemSource::Box(_) => take_field(&mut rest)
            .and_then(|f| f.try_into().ok())
            .map_or(0, u32::from_le_bytes),
        _ => 0,
    };
    Some(GameEvent::SpecialItemObtained {
        player,
        item_id,
        source,
        refine,
    })
}

/// Where the refine the item comes with (4 bytes, from 20250402) lies in 0x0b79's item head, after the cost count
const BARTER_ITEM_REFINE_AT: usize = 32;
/// The size of 0x0b79's item head, before its costs
const BARTER_ITEM_HEAD: usize = BARTER_ITEM_REFINE_AT + 4;

/// 0x0b79's items, each a fixed head and as many costs as it says (the crate keeps them as raw bytes): item 4,
/// type 2, stock 4, weight 4, shop index 4, zeny 4, view 2, location 4, cost count 4, the item's refine 4; a cost
/// is item 4, refine 2, amount 4, type 2. Stops at the first item the bytes cannot hold.
fn expanded_barter_items(raw: &[u8], count: i32) -> Vec<BarterItem> {
    const COST: usize = 12;
    let u32_at = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
    let u16_at = |at: usize| u16::from_le_bytes(raw[at..at + 2].try_into().unwrap());
    let mut items = Vec::new();
    let mut at = 0;
    for _ in 0..count.max(0) {
        if at + BARTER_ITEM_HEAD > raw.len() {
            break;
        }
        let costs = u32_at(at + 28) as usize;
        let end = at + BARTER_ITEM_HEAD + costs * COST;
        if end > raw.len() {
            break;
        }
        items.push(BarterItem {
            item_id: u32_at(at),
            item_type: raw[at + 4],
            shop_index: u32_at(at + 14),
            stock: shop_stock(u32_at(at + 6) as i32),
            zeny: u32_at(at + 18),
            refine: u32_at(at + BARTER_ITEM_REFINE_AT),
            costs: (0..costs)
                .map(|c| at + BARTER_ITEM_HEAD + c * COST)
                .map(|c| BarterCost {
                    item_id: u32_at(c),
                    refine: u16_at(c + 4),
                    amount: u32_at(c + 6),
                })
                .collect(),
        });
        at = end;
    }
    items
}

pub fn dispatch_packet(packet: &dyn Packet) -> Vec<GameEvent> {
    nostalre_profiling::profile_function!();
    let any = packet.as_any();

    if let Some(p) = any.downcast_ref::<PacketAcAcceptLogin2>() {
        return vec![GameEvent::LoginAccepted {
            account_id: p.aid,
            login_id1: p.auth_code,
            login_id2: p.user_level,
            sex: p.sex,
            servers: p
                .server_list
                .iter()
                .map(|a| ServerInfo {
                    ip: a.ip,
                    port: a.port,
                    name: cstr(&a.name),
                    user_count: a.user_count,
                })
                .collect(),
            // the web server's token (rAthena's `web_auth_token`, 16 characters), in the field named for Twitter
            web_auth_token: cstr(&p.twitter_auth_token),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketAcRefuseLogin>() {
        return vec![GameEvent::LoginRefused {
            error_code: p.error_code,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketAcRefuseLoginR2>() {
        return vec![GameEvent::LoginRefused {
            error_code: p.error_code as u8,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcRefuseEnter>() {
        return vec![GameEvent::CharServerConnectRefused {
            error_code: p.error_code,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcAcceptEnterNeoUnion>() {
        let characters = p
            .char_info
            .iter()
            .map(character_info_from_neo_union)
            .collect();
        return vec![GameEvent::CharacterListReceived { characters }];
    }
    if any.is::<PacketHcAcceptEnterNeoUnionHeader>() || any.is::<PacketHcCharlistNotify>() {
        // slot counts and the number of list pages; the characters follow in HC_ACCEPT_ENTER (0x006b)
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketChSendMapInfo>() {
        return vec![GameEvent::ZoneServerConnectInfo {
            char_id: p.gid,
            map_name: cstr(&p.map_name),
            ip: p.map_server_ip,
            port: p.map_server_port,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcNotifyAccessibleMapname>() {
        let maps = p
            .maps
            .iter()
            .map(|m| AccessibleMap {
                status: m.status,
                name: cstr(&m.map_name),
            })
            .collect();
        return vec![GameEvent::AccessibleMapsReceived { maps }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcAcceptMakecharNeoUnion>() {
        let character = character_info_from_neo_union(&p.charinfo);
        return vec![GameEvent::CharacterCreated { character }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcRefuseMakechar>() {
        return vec![GameEvent::CharacterCreateFailed {
            error_code: p.error_code,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcDeleteChar3Reserved>() {
        return vec![GameEvent::CharacterDeleteReserved {
            gid: p.gid,
            result: p.result as u32,
            delete_reserved_date: p.delete_reserved_date,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcDeleteChar3>() {
        return vec![GameEvent::CharacterDeleted {
            gid: p.gid,
            result: p.result as u32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketHcDeleteChar3Cancel>() {
        return vec![GameEvent::CharacterDeleteCancelled {
            gid: p.gid,
            result: p.result as u32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAcceptEnter2>() {
        let (x, y, dir) = decode_pos(&p.pos_dir);
        return vec![GameEvent::MapEntered {
            x,
            y,
            dir,
            tick: p.start_time,
        }];
    }
    if any.downcast_ref::<PacketZcRestartAck>().is_some() {
        return vec![GameEvent::RestartAck];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqDisconnect>() {
        return vec![GameEvent::DisconnectAck {
            allowed: p.result == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcackMapmove>() {
        let map_name: String = cstr(&p.map_name);
        return vec![GameEvent::MapChanged {
            map_name,
            x: p.x_pos,
            y: p.y_pos,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcackServermove>() {
        let map_name: String = cstr(&p.map_name);
        return vec![GameEvent::ZoneServerChanged {
            map_name,
            ip: p.addr.ip,
            port: p.addr.port,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyPlayermove>() {
        let (x1, y1, x2, y2) = decode_pos2(&p.move_data);
        return vec![GameEvent::PlayerMoved {
            start_x: x1,
            start_y: y1,
            dest_x: x2,
            dest_y: y2,
            start_time: p.move_start_time,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyTime>() {
        return vec![GameEvent::ServerTick {
            server_tick: p.time,
            local_send_time_ms: 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcHighjump>() {
        return vec![GameEvent::EntityHighJumped {
            gid: p.aid,
            x: p.x_pos as u16,
            y: p.y_pos as u16,
        }];
    }

    /// A unit coming into view: standing (0x09ff, with its posture), new on screen (0x09fe) or walking (0x09fd,
    /// spawned at its start cell, then `$moved`). The three share their field names; `AID` is the unit's id
    /// (`GID` is a player's character id, 0 for anything else).
    macro_rules! unit_spawn {
        ($p:expr, $position:expr, $posture:expr, $is_new_entry:expr $(, $moved:expr)?) => {{
            let p = $p;
            let (x, y, direction): (u16, u16, u8) = $position;
            let mut events = vec![GameEvent::EntitySpawned {
                gid: p.aid,
                aid: p.aid,
                job: p.job as u16,
                speed: p.speed as u16,
                sex: p.sex,
                head: p.head as u16,
                weapon: p.weapon,
                shield: p.shield,
                robe: p.robe,
                head_top: p.accessory2,
                head_mid: p.accessory3,
                head_bottom: p.accessory,
                hair_color: p.headpalette,
                cloth_color: p.bodypalette,
                x,
                y,
                direction,
                body_state: p.body_state,
                health_state: p.health_state,
                effect_state: p.effect_state,
                base_level: p.clevel,
                is_boss: p.is_boss,
                posture: $posture,
                guild_id: p.guid,
                guild_emblem_version: p.gemblem_ver as i32,
                is_new_entry: $is_new_entry,
            }];
            $(events.push($moved);)?
            push_opt3(&mut events, p.aid, p.effect_state, p.clevel as i32, p.virtue);
            events
        }};
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyStandentry6>() {
        return unit_spawn!(p, decode_pos(&p.pos_dir), p.state, false);
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyNewentry8>() {
        return unit_spawn!(p, decode_pos(&p.pos_dir), 0, true);
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyMoveentry8>() {
        let (x1, y1, x2, y2) = decode_pos2(&p.move_data);
        let direction =
            nostalre_game::movement::direction_from_positions(x1, y1, x2, y2).unwrap_or(0);
        let moved = GameEvent::EntityMoved {
            gid: p.aid,
            start_x: x1,
            start_y: y1,
            dest_x: x2,
            dest_y: y2,
            start_time: p.move_start_time,
        };
        return unit_spawn!(p, (x1, y1, direction), 0, false, moved);
    }

    if let Some(p) = any.downcast_ref::<PacketZcStopmove>() {
        return vec![GameEvent::EntityStopMove {
            gid: p.aid,
            x: p.x_pos as u16,
            y: p.y_pos as u16,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcFastmove>() {
        return vec![GameEvent::EntityStopMove {
            gid: p.aid,
            x: p.x_pos as u16,
            y: p.y_pos as u16,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcNotifyAct3>() {
        return vec![GameEvent::EntityAction {
            gid: p.gid,
            target_gid: p.target_gid,
            action: action_type(p.action),
            damage: p.damage,
            left_damage: p.left_damage,
            attack_mt: p.attack_mt,
            attacked_mt: p.attacked_mt,
            start_time: p.start_time,
            count: p.count,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcChangeDirection>() {
        return vec![GameEvent::EntityDirectionChanged {
            gid: p.aid,
            head_dir: p.head_dir as u8,
            dir: p.dir,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcNotifyChat>() {
        let message: String = server_message(&p.msg, &p.msg_raw);
        return vec![GameEvent::ChatMessage {
            gid: p.gid,
            message,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyPlayerchat>() {
        let message: String = server_message(&p.msg, &p.msg_raw);
        return vec![GameEvent::OwnChatMessage { message }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckRanking2>() {
        let Some(kind) = FameKind::from_rank_type(p.ranking_type) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::RankingReceived {
            ranking: Ranking {
                kind,
                entries: p
                    .cids
                    .iter()
                    .zip(p.points)
                    .filter(|(cid, _)| **cid != 0)
                    .map(|(cid, point)| (*cid, point))
                    .collect(),
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBroadcast>() {
        let raw: String = server_message(&p.msg, &p.msg_raw);
        let (message, color) = parse_broadcast(&raw);
        let (message, banner) = classify_banner(message);
        return vec![GameEvent::BroadcastMessage {
            message,
            color,
            banner,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBroadcast2>() {
        let message: String = server_message(&p.msg, &p.msg_raw);
        return vec![GameEvent::BroadcastMessage {
            message,
            color: rgb_u32_to_rgba(p.font_color),
            banner: BannerKind::None,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcStarskill>() {
        return vec![GameEvent::StarSkillNotice {
            map_name: cstr(&p.map_name),
            monster_id: p.monster_id,
            star: p.star,
            result: p.result,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcStarplace>() {
        return vec![GameEvent::StarPlaceRequest { which: p.which }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcAckReqnameall2>() {
        return vec![GameEvent::EntityNamesReceived {
            gid: p.gid,
            name: cstr(&p.name),
            party_name: cstr(&p.party_name),
            guild_name: cstr(&p.guild_name),
            position_name: cstr(&p.position_name),
            title_id: p.title_id,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqnameallNpc>() {
        return vec![GameEvent::EntityNameReceived {
            gid: p.gid,
            name: cstr(&p.name),
        }];
    }
    if any.is::<PacketZcPingLive>() {
        return vec![GameEvent::Acknowledged];
    }
    if any.is::<PacketZcLoadConfirm>() {
        // the answer to 0x0447 after each map property (`clif_loadConfirm`): the map is loaded, nothing to do
        return vec![GameEvent::Acknowledged];
    }

    if let Some(p) = any.downcast_ref::<PacketZcAckReqnameBygid>() {
        let name: String = cstr(&p.cname);
        return vec![
            GameEvent::CharNameReceived {
                char_id: p.gid,
                name: name.clone(),
            },
            GameEvent::EntityNameReceived { gid: p.gid, name },
        ];
    }

    if let Some(p) = any.downcast_ref::<PacketZcNotifyVanish>() {
        return vec![GameEvent::EntityVanished {
            gid: p.gid,
            vanish_type: vanish_type(p.atype),
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcNotifyEffect2>() {
        return vec![GameEvent::PlayEffectOnEntity {
            gid: p.aid,
            effect_id: p.effect_id,
            value: None,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyEffect3>() {
        return vec![GameEvent::PlayEffectOnEntity {
            gid: p.aid,
            effect_id: p.effect_id,
            value: Some(p.numdata),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyEffect>() {
        return vec![GameEvent::PlayMiscEffectOnEntity {
            gid: p.aid,
            code: p.effect_id as u8,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyRanking>() {
        return vec![GameEvent::PvpRankingChanged {
            account_id: p.aid,
            ranking: p.ranking,
            total: p.total,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSpirits>() {
        return vec![GameEvent::SpiritsChanged {
            gid: p.aid,
            count: p.num.max(0) as u8,
        }];
    }
    // A Kagerou's or Oboro's charms (`clif_spiritcharm`): their element and count; element 0 when the last one went
    if let Some(p) = any.downcast_ref::<PacketZcSpiritsAttribute>() {
        return vec![GameEvent::CharmsChanged {
            gid: p.aid,
            element: p.spirits_type.clamp(0, 4) as u8,
            count: p.num.max(0) as u8,
        }];
    }
    // A Soul Reaper's souls (`clif_soulball`), the count again whenever it changes
    if let Some(p) = any.downcast_ref::<PacketZcSoulenergy>() {
        return vec![GameEvent::SoulEnergyChanged {
            gid: p.aid,
            count: p.num.max(0) as u8,
        }];
    }
    // rAthena's `state` is always 0, a broken shield sends the count left
    if let Some(p) = any.downcast_ref::<PacketZcMillenniumshield>() {
        return vec![GameEvent::MillenniumShieldChanged {
            gid: p.aid,
            count: p.num.max(0) as u8,
        }];
    }
    // A Devotion caster's links (`clif_devotion`): its five slots, an empty one 0; kRO reads no range
    if let Some(p) = any.downcast_ref::<PacketZcDevotionlist>() {
        return vec![GameEvent::DevotionLinks {
            gid: p.my_aid,
            targets: p.aid.iter().copied().filter(|&aid| aid != 0).collect(),
        }];
    }
    // The player's elemental (`clif_elemental_info`): its unit, HP, max HP, SP and max SP
    if let Some(p) = any.downcast_ref::<PacketZcElInit>() {
        return vec![GameEvent::ElementalInit {
            gid: p.aid as u32,
            hp: p.hp.max(0) as u32,
            max_hp: p.max_hp.max(0) as u32,
            sp: p.sp.max(0) as u32,
            max_sp: p.max_sp.max(0) as u32,
        }];
    }
    // One of its gauges (`clif_elemental_updatestatus`, which sends only these four)
    if let Some(p) = any.downcast_ref::<PacketZcElParChange>() {
        let Some(par) = nostalre_game::companion::ElementalPar::from_var(p.var) else {
            tracing::warn!("elemental parameter {} is none rAthena sends", p.var);
            return vec![];
        };
        return vec![GameEvent::ElementalParChanged {
            par,
            value: p.value.max(0) as u32,
        }];
    }
    // A summon of the player's (`clif_summon_init`, to its master as an ABR or Bionic comes into view)
    if let Some(p) = any.downcast_ref::<PacketZcSummonHpInit>() {
        return vec![GameEvent::SummonHpInit {
            gid: p.summon_aid,
            hp: p.current_hp,
            max_hp: p.max_hp,
        }];
    }
    // One of its gauges (`clif_summon_hp_bar`: always `SP_HP`; kRO's handler 0x7f4330 also stores `SP_MAXHP`)
    if let Some(p) = any.downcast_ref::<PacketZcSummonHpUpdate>() {
        let Some(par) = nostalre_game::companion::SummonPar::from_var(p.var_id) else {
            tracing::warn!("summon parameter {} is none rAthena sends", p.var_id);
            return vec![];
        };
        return vec![GameEvent::SummonHpChanged {
            gid: p.summon_aid,
            par,
            value: p.value,
        }];
    }
    // A unit the player marked (`clif_crimson_marker`, to the caster once a second; -1, -1 ends it)
    if let Some(p) = any.downcast_ref::<PacketZcCMarkerinfo>() {
        return vec![GameEvent::CrimsonMarker {
            aid: p.aid,
            x: p.x_pos,
            y: p.y_pos,
        }];
    }
    // A unit's Shadow Scars (`clif_enchantingshadow_spirit`, to the area as one is added). `unknown_val` is always 0
    // in rAthena, and kRO's handler (0x806660) reads only the unit and the amount
    if let Some(p) = any.downcast_ref::<PacketZcTargetSpirits>() {
        return vec![GameEvent::ShadowScars {
            gid: p.gid,
            amount: p.amount,
        }];
    }
    // The item list window (`clif_skill_itemlistwindow`): Change Material sends 0, Four Spirit Analysis its level
    if let Some(p) = any.downcast_ref::<PacketZcItemlistwinOpen>() {
        return vec![GameEvent::ItemListWindowOpened { kind: p.atype }];
    }
    // Cart Decoration's choice (`clif_SelectCart`): the account id, then one byte a cart
    if let Some(p) = any.downcast_ref::<PacketZcSelectcart>() {
        return vec![GameEvent::CartDecorationList {
            identity: p.identity,
            carts: p.carts.clone(),
        }];
    }
    // A combo's window (`clif_combo_delay`, sent with `SC_COMBO` before the blow's damage): kRO keeps it per unit
    if let Some(p) = any.downcast_ref::<PacketZcCombodelay>() {
        return vec![GameEvent::ComboDelay {
            gid: p.aid,
            delay_ms: p.delay_time,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBladestop>() {
        return vec![GameEvent::BladeStop {
            src_gid: p.src_aid,
            dest_gid: p.dest_aid,
            active: p.flag != 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcResurrection>() {
        return vec![GameEvent::EntityResurrected { gid: p.aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcRecovery>() {
        return vec![GameEvent::Recovery {
            var_id: p.var_id as u16,
            amount: p.amount_32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSound>() {
        let name: String = cstr(&p.file_name);
        return vec![GameEvent::SoundEffect {
            name,
            act: p.act,
            term_ms: p.term,
            gid: p.naid,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMvp>() {
        return vec![GameEvent::MvpReward { gid: p.aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMvpGettingItem>() {
        return vec![GameEvent::MvpFeedback {
            kind: MvpFeedbackKind::Item { item_id: p.itid_32 },
        }];
    }
    if any.downcast_ref::<PacketZcThrowMvpitem>().is_some() {
        return vec![GameEvent::MvpFeedback {
            kind: MvpFeedbackKind::ItemDropped,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUpdateRankingPoint>() {
        let Some(kind) = FameKind::from_rank_type(p.ranking_type) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::FamePointsGained {
            kind,
            point: p.points,
            total: p.points_total,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckPvppoint>() {
        return vec![GameEvent::PvpPointsReceived {
            win: p.pvp.win_point,
            lose: p.pvp.lose_point,
            point: p.pvp.point,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcCouplename>() {
        let name: String = cstr(&p.couple_name);
        return vec![GameEvent::CoupleNameReceived { name }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcCongratulation>() {
        return vec![GameEvent::WeddingCelebration { account_id: p.aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqCouple>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::MarriageProposed {
            proposer_aid: p.aid,
            proposer_gid: p.gid,
            name,
        }];
    }
    if any.downcast_ref::<PacketZcStartCouple>().is_some() {
        return vec![GameEvent::MarriageProposalArmed];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDivorce>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::Divorced { name }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcShowImage2>() {
        let image: String = cstr(&p.image_name);
        return vec![GameEvent::NpcCutin {
            image,
            position: p.atype,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcReqBaby>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::AdoptionRequested {
            father_aid: p.aid,
            mother_aid: p.gid,
            name,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBabymsg>() {
        return vec![GameEvent::AdoptionMessage { msg_no: p.msg_no }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcParChange>() {
        return vec![GameEvent::ParameterChanged {
            var_id: p.var_id,
            value: p.count as i64,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcLongparChange>() {
        return vec![GameEvent::ParameterChanged {
            var_id: p.var_id,
            value: p.amount as i64,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcLonglongparChange>() {
        return vec![GameEvent::ParameterChanged {
            var_id: p.var_id,
            value: p.amount,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyExp>() {
        return vec![GameEvent::ExpGained {
            aid: p.aid,
            amount: p.amount_64,
            is_base: p.var_id as usize == StatusTypes::Baseexp.value(),
            is_quest: p.exp_type == 1,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcStatusValues>() {
        return vec![GameEvent::StatusChanged {
            status_type: p.status_type,
            base: p.default_status,
            bonus: p.plus_status,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcStatus>() {
        let costs = [
            (StatusTypes::StrNextLevelIncreaseCost, p.standard_str),
            (StatusTypes::AgiNextLevelIncreaseCost, p.standard_agi),
            (StatusTypes::VitNextLevelIncreaseCost, p.standard_vit),
            (StatusTypes::IntNextLevelIncreaseCost, p.standard_int),
            (StatusTypes::DexNextLevelIncreaseCost, p.standard_dex),
            (StatusTypes::LukNextLevelIncreaseCost, p.standard_luk),
        ];
        let mut events = vec![GameEvent::ParameterChanged {
            var_id: StatusTypes::Statuspoint.value() as u16,
            value: p.point as i64,
        }];
        events.extend(costs.map(|(status, cost)| GameEvent::ParameterChanged {
            var_id: status.value() as u16,
            value: cost as i64,
        }));
        return events;
    }
    if let Some(p) = any.downcast_ref::<PacketZcStatusChange>() {
        return vec![GameEvent::ParameterChanged {
            var_id: p.status_id,
            value: p.value as i64,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcStatusChangeAck>() {
        if !p.result {
            return vec![];
        }
        return vec![GameEvent::ParameterChanged {
            var_id: p.status_id,
            value: p.value as i64,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAttackRange>() {
        return vec![GameEvent::AttackRangeChanged {
            range: p.current_att_range,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAttackFailureForDistance>() {
        return vec![GameEvent::AttackFailedForDistance {
            target_gid: p.target_aid,
            target_x: p.target_xpos.max(0) as u16,
            target_y: p.target_ypos.max(0) as u16,
            x: p.x_pos.max(0) as u16,
            y: p.y_pos.max(0) as u16,
            range: p.current_att_range,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSpriteChange2>() {
        // 32-bit values: a weapon or shield without a view id is shown by its item id
        return vec![GameEvent::EntitySpriteChanged {
            gid: p.gid,
            sprite_type: p.atype,
            value: p.value_32,
            value2: p.value2_32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcspriteChange>() {
        return vec![GameEvent::EntitySpriteChanged {
            gid: p.gid,
            sprite_type: 0,
            value: p.value,
            value2: 0,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcUseskillAck2>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::SkillCasting {
            gid: p.aid,
            target_gid: p.target_id,
            skill,
            property: p.property,
            delay_ms: p.delay_time,
            x: p.x_pos,
            y: p.y_pos,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckTouseskill>() {
        if !p.result {
            return vec![GameEvent::SkillFailed {
                skill: (p.skid != 0).then(|| known_skill(p.skid as u32)).flatten(),
                cause: p.cause,
                btype: p.num,
                item_id: p.itid,
            }];
        }
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillPostdelay>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::SkillPostDelay {
            skill,
            delay_ms: p.delay_tm,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMsgStateChange2>() {
        if p.index == 46 && p.state {
            return vec![GameEvent::AfterCastDelay {
                delay_ms: p.remain_ms,
            }];
        }
        return vec![GameEvent::StatusEffectChanged {
            gid: p.aid,
            efst: p.index,
            active: p.state,
            remain_ms: p.remain_ms,
            val1: p.val1,
        }];
    }
    // A unit coming into view with a status on (from 20120618)
    if let Some(p) = any.downcast_ref::<PacketZcEfstSetEnter2>() {
        return vec![GameEvent::StatusEffectChanged {
            gid: p.target_id,
            efst: p.atype as i16,
            active: true,
            remain_ms: p.remain_ms,
            val1: p.val1,
        }];
    }
    // 0x196: always the status-OFF packet (flag=0); without this arm buffs never clear on early removal.
    if let Some(p) = any.downcast_ref::<PacketZcMsgStateChange>() {
        return vec![GameEvent::StatusEffectChanged {
            gid: p.aid,
            efst: p.index,
            active: p.state,
            remain_ms: 0,
            val1: 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcShowefstUpdate>() {
        return vec![GameEvent::EntityOpt3Changed {
            gid: p.aid,
            effect_state: p.effect_state,
            base_level: p.clevel,
            opt3: p.show_efst,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcStateChange3>() {
        return vec![GameEvent::EntityOptionChanged {
            gid: p.aid,
            body_state: p.body_state,
            health_state: p.health_state,
            effect_state: p.effect_state,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifySkill2>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::EntityAction {
                gid: p.aid,
                target_gid: p.target_id,
                action: action_type(p.action),
                damage: p.damage,
                left_damage: 0,
                attack_mt: p.attack_mt,
                attacked_mt: p.attacked_mt,
                start_time: p.start_time,
                count: p.count,
            }];
        };
        return vec![GameEvent::SkillDamage {
            skill,
            src_gid: p.aid,
            target_gid: p.target_id,
            damage: p.damage,
            attack_mt: p.attack_mt,
            attacked_mt: p.attacked_mt,
            count: p.count,
            level: p.level,
            action: action_type(p.action),
            start_time: p.start_time,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMsg>() {
        return vec![GameEvent::ServerMsg { msg_id: p.msg }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMsgColor>() {
        return vec![GameEvent::ServerMsgColor {
            msg_id: p.msg_id,
            color: p.msg_color,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMsgValue>() {
        return vec![GameEvent::ServerMsgValue {
            msg_id: p.msg,
            value: p.value,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMsgSkill>() {
        let Ok(msg_id) = u16::try_from(p.msgid) else {
            debug!("skill message id {} out of range", p.msgid);
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::ServerSkillMsg {
            skill_id: p.skid,
            msg_id,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUserCount>() {
        return vec![GameEvent::UserCount { count: p.count }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckGiveMannerPoint>() {
        return vec![GameEvent::MannerPointResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyMannerPointGiven>() {
        return vec![GameEvent::MannerPointGiven {
            positive: p.atype == nostalre_game::gm::MANNER_TYPE_PLUS,
            other_name: raw_euc_kr(&p.other_char_name_raw),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckStatusGm>() {
        return vec![GameEvent::GmStatusReceived {
            status: Box::new(GmStatus {
                str: p.str,
                str_cost: p.standard_str,
                agi: p.agi,
                agi_cost: p.standard_agi,
                vit: p.vit,
                vit_cost: p.standard_vit,
                int: p.int,
                int_cost: p.standard_int,
                dex: p.dex,
                dex_cost: p.standard_dex,
                luk: p.luk,
                luk_cost: p.standard_luk,
                atk: p.att_power,
                atk_plus: p.refining_power,
                matk_max: p.max_matt_power,
                matk_min: p.min_matt_power,
                def: p.itemdef_power,
                def_plus: p.plusdef_power,
                mdef: p.mdef_power,
                mdef_plus: p.plusmdef_power,
                hit: p.hit_success_value,
                flee: p.avoid_success_value,
                flee_plus: p.plus_avoid_success_value,
                critical: p.critical_success_value,
                aspd: p.aspd,
                aspd_plus: p.plus_aspd,
            }),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckAccountname>() {
        return vec![GameEvent::AccountNameReceived {
            aid: p.aid,
            name: raw_euc_kr(&p.name_raw),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillmsg>() {
        return vec![GameEvent::SkillMsg { msg_no: p.msg_no }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyBindOnEquip>() {
        return vec![GameEvent::BindOnEquipNotice { index: p.index }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcTalkboxChatcontents>() {
        return vec![GameEvent::TalkboxContents {
            aid: p.aid,
            message: raw_euc_kr(&p.contents_21_raw),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcShowdigit>() {
        let Some(mode) = ShowDigitMode::from_packet(p.atype) else {
            return vec![];
        };
        return vec![GameEvent::ShowDigit {
            mode,
            value: p.value,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBossInfo>() {
        let Some(kind) = BossInfoKind::from_packet(p.info_type) else {
            return vec![];
        };
        return vec![GameEvent::BossInfoReceived {
            kind,
            x: p.x_pos.max(0) as u16,
            y: p.y_pos.max(0) as u16,
            respawn_hour: p.min_hour,
            respawn_minute: p.min_minute,
            name: raw_euc_kr(&p.name_raw),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcProgress>() {
        return vec![GameEvent::ProgressBarStarted {
            duration_secs: p.time,
        }];
    }
    if any.downcast_ref::<PacketZcProgressCancel>().is_some() {
        return vec![GameEvent::ProgressBarCancelled];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSettingWhisperPc>() {
        return vec![GameEvent::WhisperSettingResult {
            allow: p.atype != 0,
            result: p.result,
            all: false,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSettingWhisperState>() {
        return vec![GameEvent::WhisperSettingResult {
            allow: p.atype != 0,
            result: p.result,
            all: true,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckRememberWarppoint>() {
        return vec![GameEvent::MemoResult {
            result: p.error_code,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyMapinfo>() {
        return vec![GameEvent::MapInfoNotice { atype: p.atype }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcChat>() {
        return vec![GameEvent::ServerColoredMessage {
            account_id: p.account_id,
            color: p.color,
            message: raw_euc_kr(&p.msg_raw),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDispel>() {
        return vec![GameEvent::SkillCastCancel { gid: p.aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUseSkill>() {
        if p.result
            && let Some(skill) = known_skill(p.skid as u32)
        {
            return vec![GameEvent::SkillNoDamage {
                skill,
                src_gid: p.src_aid,
                target_gid: p.target_aid,
                level: p.level_32,
            }];
        }
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyGroundskill>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::GroundSkill {
            skill,
            src_gid: p.aid,
            level: p.level,
            x: p.x_pos,
            y: p.y_pos,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMonsterInfo>() {
        let t = &p.property_table;
        return vec![GameEvent::MonsterInfoReceived {
            info: MonsterInfo {
                name: String::new(),
                job: p.job as u16,
                level: p.level,
                size: p.size,
                hp: p.hp,
                def: p.def,
                race: p.race_type,
                mdef: p.mdef_power,
                property: p.property,
                resistances: [
                    t.water, t.earth, t.fire, t.wind, t.poison, t.saint, t.dark, t.mental, t.undead,
                ],
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillEntry>() {
        return vec![GameEvent::SkillUnitEntered {
            aid: p.aid,
            creator_aid: p.creator_aid,
            x: p.x_pos,
            y: p.y_pos,
            unit_id: p.job as u16,
            is_visible: p.is_visible,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillEntry5>() {
        return vec![GameEvent::SkillUnitEntered {
            aid: p.aid,
            creator_aid: p.creator_aid,
            x: p.x_pos,
            y: p.y_pos,
            // rAthena's unit ids end below 0x190 (`UNT_MAX`)
            unit_id: p.job as u16,
            is_visible: p.is_visible,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillEntry2>() {
        if p.is_contens {
            return vec![GameEvent::GraffitiEntered {
                aid: p.aid,
                creator_aid: p.creator_aid,
                x: p.x_pos,
                y: p.y_pos,
                message: cstr(&p.msg),
            }];
        }
        return vec![GameEvent::SkillUnitEntered {
            aid: p.aid,
            creator_aid: p.creator_aid,
            x: p.x_pos,
            y: p.y_pos,
            unit_id: p.job as u16,
            is_visible: p.is_visible,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUpdateMapinfo>() {
        return vec![GameEvent::MapCellChanged {
            x: p.x_pos,
            y: p.y_pos,
            cell_type: p.atype as i32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillDisappear>() {
        return vec![GameEvent::SkillUnitDisappeared { aid: p.aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillUpdate>() {
        return vec![GameEvent::SkillUnitUpdated { aid: p.aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcEmotion>() {
        return vec![GameEvent::EntityEmotion {
            gid: p.gid,
            emotion_type: p.atype,
        }];
    }

    if let Some(p) = any.downcast_ref::<crate::solo_packets::PacketZcHpInfo>() {
        return vec![GameEvent::EntityHpChanged {
            gid: p.gid,
            hp: p.hp.max(0) as u32,
            max_hp: p.max_hp.max(0) as u32,
        }];
    }
    if let Some(p)=any.downcast_ref::<crate::solo_packets::PacketZcSkillScale>() {
        return SkillEnum::try_from_value(u32::from(p.skill_id)).map(|skill|vec![GameEvent::SkillCasting {gid:p.gid,target_gid:0,skill,property:0,delay_ms:p.cast_ms,x:p.x,y:p.y}]).unwrap_or_default();
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyHpToGroupmR2>() {
        return vec![
            GameEvent::EntityHpChanged {
                gid: p.aid,
                hp: p.hp as u32,
                max_hp: p.maxhp as u32,
            },
            GameEvent::PartyMemberHp {
                aid: p.aid,
                hp: p.hp as u32,
                max_hp: p.maxhp as u32,
            },
        ];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyPositionToGroupm>() {
        return vec![GameEvent::PartyMemberPosition {
            aid: p.aid,
            x: p.x_pos,
            y: p.y_pos,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyPositionToGuildm>() {
        return vec![GameEvent::GuildMemberPosition {
            aid: p.aid,
            x: p.x_pos,
            y: p.y_pos,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcCompass>() {
        return match MarkAction::from_packet(p.atype) {
            Some(action) => vec![GameEvent::MinimapMark {
                id: p.id,
                action,
                x: p.x_pos.max(0) as u16,
                y: p.y_pos.max(0) as u16,
                color: p.color,
            }],
            None => vec![],
        };
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckMakeGroup>() {
        return vec![GameEvent::PartyCreateResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGroupList>() {
        let name: String = cstr(&p.group_name);
        let members = p
            .group_info
            .iter()
            .map(|m| PartyMemberData {
                aid: m.aid,
                name: cstr(&m.character_name),
                map: cstr(&m.map_name),
                leader: m.role == 0,
                online: m.state == 0,
                class: m.class as u16,
                base_level: m.base_level as u16,
            })
            .collect();
        return vec![GameEvent::PartyMemberList { name, members }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddMemberToGroup2>() {
        let name: String = cstr(&p.character_name);
        let map: String = cstr(&p.map_name);
        return vec![GameEvent::PartyMemberAdded {
            aid: p.aid,
            name,
            map,
            leader: p.role == 0,
            online: p.state == 0,
            x: p.x_pos as u16,
            y: p.y_pos as u16,
            class: p.class as u16,
            base_level: p.base_level as u16,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGroupIsalive>() {
        return vec![GameEvent::PartyMemberDead {
            aid: p.aid,
            dead: p.is_dead != 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyMemberinfoToGroupm>() {
        return vec![GameEvent::PartyMemberJob {
            aid: p.aid,
            class: p.job as u16,
            base_level: p.level as u16,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDeleteMemberFromGroup>() {
        let name: String = cstr(&p.character_name);
        return vec![GameEvent::PartyMemberRemoved {
            aid: p.aid,
            name,
            result: p.result,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPartyConfig>() {
        return vec![GameEvent::SelfConfigChanged {
            kind: SelfConfigKind::RefusePartyInvite,
            enabled: p.b_refuse_join_msg,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcConfigNotify>() {
        return vec![GameEvent::SelfConfigChanged {
            kind: SelfConfigKind::OpenEquipmentWindow,
            enabled: p.b_open_equipment_win,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcConfig>() {
        let kind = match p.config {
            0 => SelfConfigKind::OpenEquipmentWindow,
            1 => SelfConfigKind::Call,
            2 => SelfConfigKind::PetAutofeed,
            3 => SelfConfigKind::HomunculusAutofeed,
            // 4 is rAthena's commented-out CONFIG_UNKNOWN
            5 => SelfConfigKind::HideCostumes,
            other => {
                debug!("unknown ZC_CONFIG type: {other}");
                return vec![];
            }
        };
        return vec![GameEvent::SelfConfigChanged {
            kind,
            enabled: p.value != 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDialogWindowSize>() {
        return vec![GameEvent::NpcDialogPlacement(DialogPlacement::Size {
            width: p.width,
            height: p.height,
        })];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDialogWindowPos>() {
        return vec![GameEvent::NpcDialogPlacement(DialogPlacement::Pixels {
            x: p.x,
            y: p.y,
        })];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDialogWindowPos2>() {
        return vec![GameEvent::NpcDialogPlacement(DialogPlacement::Percent {
            x: p.x,
            y: p.y,
        })];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSpecialpopup>() {
        return vec![GameEvent::SpecialPopup { id: p.pp_id }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBroadcastingSpecialItemObtain>() {
        let event = special_item_obtained(p.atype, p.item_id, &p.fields);
        if event.is_none() {
            debug!(
                "ZC_BROADCASTING_SPECIAL_ITEM_OBTAIN not read: {:?}",
                p.fields
            );
        }
        return event.into_iter().collect();
    }
    if let Some(p) = any.downcast_ref::<PacketZcPlayNpcBgm2>() {
        // kRO plays `bgm\<name>.mp3` when music is on; rAthena sends play type 0 alone, "loop for the time being"
        // (`clif_playBGM`)
        return match p.play_type {
            0 => vec![GameEvent::NpcMusic {
                name: raw_euc_kr(&p.bgm),
            }],
            other => {
                debug!("unknown ZC_PLAY_NPC_BGM play type: {other}");
                vec![]
            }
        };
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckTakeoffEquipAll>() {
        // rAthena sends 1 when the player cannot act (`clif_unequipall_reply`); kRO's exe refuses 1 and 2 alike
        return match p.result {
            0 => vec![GameEvent::UnequipAllAnswered { refused: false }],
            1 | 2 => vec![GameEvent::UnequipAllAnswered { refused: true }],
            other => {
                debug!("unknown ZC_ACK_TAKEOFF_EQUIP_ALL result: {other}");
                vec![]
            }
        };
    }
    if let Some(p) = any.downcast_ref::<PacketZcPartyJoinReq>() {
        let party_name: String = cstr(&p.group_name);
        return vec![GameEvent::PartyInviteReceived {
            party_grid: p.grid,
            party_name,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPartyJoinReqAck>() {
        let name: String = cstr(&p.character_name);
        return vec![GameEvent::PartyInviteResult {
            name,
            answer: p.answer as u8,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqGroupinfoChangeV2>() {
        return vec![GameEvent::PartyConfigChanged {
            exp_option: p.exp_option,
            item_pickup_rule: p.item_pickup_rule,
            item_division_rule: p.item_division_rule,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketScNotifyBan>() {
        let reason = match p.error_code {
            1 => "Server closed the connection.",
            2 => "Someone else has logged in with your account.",
            3 => "Connection timed out.",
            4 => "Server is full.",
            8 => "The server still recognizes your last connection. Please try again shortly.",
            9 => "Too many connections from this IP address.",
            10 => "Your paid game time has run out.",
            15 => "You were disconnected by a GM.",
            _ => "You have been disconnected from the server.",
        };
        return vec![GameEvent::Disconnected(reason.to_string())];
    }
    if let Some(p) = any.downcast_ref::<PacketZcItemPickupParty>() {
        return vec![GameEvent::PartyItemPickup {
            aid: p.account_id,
            item_id: p.itid_32,
            identified: p.is_identified,
            refine: p.refining_level2,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyChatParty>() {
        return vec![GameEvent::PartyChatMessage {
            message: server_message(&p.msg, &p.msg_raw),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGuildChat>() {
        let message: String = server_message(&p.msg, &p.msg_raw);
        return vec![GameEvent::GuildChatMessage { message }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcWhisper>() {
        let sender: String = cstr(&p.sender);
        let message: String = server_message(&p.msg, &p.msg_raw);
        return vec![GameEvent::WhisperReceived { sender, message }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckWhisper>() {
        return vec![GameEvent::WhisperAck { result: p.result }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcAckGuildMenuinterface>() {
        return vec![GameEvent::GuildMenuFlag {
            flag: p.guild_memu_flag,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGuildInfo2>() {
        return vec![GameEvent::GuildInfo {
            gdid: p.gdid as u32,
            name: cstr(&p.guildname),
            level: p.level,
            exp: p.exp,
            max_exp: p.max_exp,
            member_num: p.user_num,
            max_member_num: p.max_user_num,
            avg_level: p.user_average_level,
            point: p.point,
            honor: p.honor,
            virtue: p.virtue,
            master_name: cstr(&p.master_name2),
            manage_land: cstr(&p.manage_land),
            emblem_version: p.emblem_version,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMembermgrInfo>() {
        let members = p
            .member_info
            .iter()
            .map(|m| GuildMember {
                aid: m.aid,
                gid: m.gid,
                name: cstr(&m.char_name2),
                job: m.job,
                level: m.level,
                head: m.head_type,
                head_palette: m.head_palette,
                sex: m.sex,
                position_id: m.gposition_id,
                position_name: String::new(),
                contribution_exp: m.member_exp,
                online: m.current_state != 0,
                last_login: m.last_login_time,
                cur_map: String::new(),
                x: 0,
                y: 0,
                has_live_position: false,
            })
            .collect();
        return vec![GameEvent::GuildMembers { members }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUpdateCharstat2>() {
        return vec![GameEvent::GuildMemberOnline {
            aid: p.aid,
            gid: p.gid,
            online: p.status != 0,
            appearance: Some(GuildMemberAppearance {
                sex: p.sex,
                head: p.head,
                head_palette: p.head_palette,
            }),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPositionInfo>() {
        let positions = p
            .member_info
            .iter()
            .map(|pos| GuildPosition {
                id: pos.position_id,
                name: String::new(),
                right: pos.right,
                ranking: pos.ranking,
                pay_rate: pos.pay_rate,
            })
            .collect();
        return vec![GameEvent::GuildPositions { positions }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckChangeGuildPositioninfo>() {
        let positions = p
            .member_list
            .iter()
            .map(|pos| GuildPosition {
                id: pos.position_id,
                name: String::new(),
                right: pos.right,
                ranking: pos.ranking,
                pay_rate: pos.pay_rate,
            })
            .collect();
        let names = p
            .member_list
            .iter()
            .map(|pos| (pos.position_id, cstr(&pos.pos_name)))
            .collect();
        return vec![
            GameEvent::GuildPositions { positions },
            GameEvent::GuildPositionNames { names },
        ];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqChangeMembers>() {
        let entries = p
            .member_info
            .iter()
            .map(|m| (m.aid as u32, m.gid as u32, m.position_id))
            .collect();
        return vec![GameEvent::GuildMemberPositionsChanged { entries }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPositionIdNameInfo>() {
        let names = p
            .member_list
            .iter()
            .map(|pos| (pos.position_id, cstr(&pos.pos_name)))
            .collect();
        return vec![GameEvent::GuildPositionNames { names }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGuildSkillinfo>() {
        let skills = parse_skill_info_list(&p.skill_list)
            .into_iter()
            .map(|s| GuildSkill {
                skill: s.skill,
                level: s.level,
                sp_cost: s.sp_cost,
                attack_range: s.attack_range,
                upgradable: s.upgradable,
                passive: matches!(s.skill_target_type, SkillTargetType::Passive),
            })
            .collect();
        return vec![GameEvent::GuildSkills {
            point: p.skill_point,
            skills,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBanList>() {
        let entries = p
            .ban_list
            .iter()
            .map(|b| GuildBanEntry {
                char_name: raw_euc_kr(&b.charname2_raw),
                reason: raw_euc_kr(&b.reason_raw),
            })
            .collect();
        return vec![GameEvent::GuildBanList { entries }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGuildNotice>() {
        return vec![GameEvent::GuildNotice {
            subject: cstr(&p.subject),
            body: cstr(&p.notice),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMyguildBasicInfo>() {
        let relations = p
            .related_guild_list
            .iter()
            .map(|r| GuildRelation {
                // wire order is <relation>.L <gdid>.L; the generated struct swaps them
                gdid: r.relation,
                name: cstr(&r.guild_name),
                relation: r.gdid,
            })
            .collect();
        return vec![GameEvent::GuildRelations { relations }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGuildEmblemImg>() {
        return vec![GameEvent::GuildEmblem {
            gdid: p.gdid as u32,
            version: p.emblem_version,
            bmp: p.img_raw.clone(),
            from_map_server: true,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUpdateGdid>() {
        return vec![GameEvent::GuildIdentityUpdated {
            gdid: p.gdid,
            emblem_version: p.emblem_version,
            right: p.right,
            is_master: p.is_master,
            name: cstr(&p.gname),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcResultMakeGuild>() {
        return vec![GameEvent::GuildCreateResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckLeaveGuild>() {
        return vec![GameEvent::GuildMemberLeft {
            gid: p.gid,
            reason: cstr(&p.reason_desc),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckBanGuildSso>() {
        return vec![GameEvent::GuildMemberExpelled {
            gid: p.gid,
            reason: cstr(&p.reason_desc),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcChangeGuild>() {
        return vec![GameEvent::EntityGuildChanged {
            aid: p.unit_aid,
            gdid: p.gdid,
            emblem_version: p.emblem_id as i32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcGuildPosition>() {
        return vec![GameEvent::EntityGuildPositionChanged {
            aid: p.aid,
            position: str_cstr(&p.position),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckDisorganizeGuildResult>() {
        return vec![GameEvent::GuildDisbandResult { reason: p.reason }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqJoinGuild>() {
        return vec![GameEvent::GuildInviteReceived {
            gdid: p.gdid,
            name: cstr(&p.guild_name),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqAllyGuild>() {
        return vec![GameEvent::GuildAllyRequestReceived {
            aid: p.other_aid,
            name: cstr(&p.guild_name),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqAllyGuild>() {
        return vec![GameEvent::GuildAllyResult { answer: p.answer }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqHostileGuild>() {
        return vec![GameEvent::GuildHostileResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqJoinGuild>() {
        return vec![GameEvent::GuildJoinResult { answer: p.answer }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDeleteRelatedGuild>() {
        return vec![GameEvent::GuildRelationDeleted {
            gdid: p.opponent_gdid,
            relation: p.relation,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcSayDialog>() {
        let text: String = server_message(&p.msg, &p.msg_raw);
        return vec![GameEvent::NpcDialogText {
            npc_id: p.naid,
            text,
        }];
    }
    // An NPC's navigation guide (`clif_navigateTo`): rAthena's kinds 0, 1 and 3, the travel the guide may use, the
    // window hidden or not, the map, then a cell or a monster
    if let Some(p) = any.downcast_ref::<PacketZcNavigationActive>() {
        use nostalre_game::navigation::{NavServices, NavTarget, Navigation};
        let Some(target) = NavTarget::from_packet(p.atype, p.x_pos, p.y_pos, p.mob_id) else {
            tracing::warn!("navigation of kind {}, none rAthena sends", p.atype);
            return vec![];
        };
        return vec![GameEvent::NavigationGuide {
            navigation: Navigation {
                map: raw_cstr(&p.map_name_raw),
                target,
                services: NavServices::from_flag(p.flag),
                hide_window: p.hide_window != 0,
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAllAchList>() {
        return vec![GameEvent::AchievementsRewarded {
            achievement_ids: p
                .achievements
                .iter()
                .filter(|a| a.rewarded)
                .map(|a| a.achievement_id)
                .collect(),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckChangeTitle>() {
        return vec![GameEvent::TitleChangeAnswered {
            ok: p.result == 0,
            title_id: p.title_id,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcEquipswitchList>() {
        return vec![GameEvent::EquipSwitchList {
            entries: p
                .items
                .iter()
                .map(|item| (item.index as u16, item.position))
                .collect(),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckEquipswitchAdd>() {
        return vec![GameEvent::EquipSwitchAdded {
            index: p.index,
            location: p.position,
            flag: p.flag,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckEquipswitchRemove>() {
        return vec![GameEvent::EquipSwitchRemoved {
            index: p.index,
            failed: p.failed,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckEquipswitch>() {
        return vec![GameEvent::EquipSwitchAnswered { failed: p.failed }];
    }
    // Systems with no window here yet (the plan's Open list): the achievement window (an entry's update 0x0a24, a
    // claim's answer 0x0a26: kRO's client takes no title from either under rAthena, whose claimed titles come back
    // in the full list 0x0a23) and reputation. Parsed, so their layout is checked, then dropped.
    if any.downcast_ref::<PacketZcAchUpdate>().is_some()
        || any.downcast_ref::<PacketZcReqAchRewardAck>().is_some()
        || any.downcast_ref::<PacketZcReputeInfo>().is_some()
    {
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcInventoryExpansionInfo>() {
        return vec![GameEvent::InventorySlots {
            slots: INVENTORY_BASE_SLOTS.saturating_add(p.expansion_size.max(0) as u16),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcOverweightPercent>() {
        return vec![GameEvent::OverweightPercent { percent: p.percent }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcClearDialog>() {
        return vec![GameEvent::NpcDialogClear { npc_id: p.gid }];
    }
    // The script's `setdialogalign`: no bundled script uses it, and the dialog draws its text left-aligned.
    if any.downcast_ref::<PacketZcDialogTextAlign>().is_some() {
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcWaitDialog>() {
        return vec![GameEvent::NpcDialogNext { npc_id: p.naid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcCloseDialog>() {
        return vec![GameEvent::NpcDialogClose { npc_id: p.naid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMenuList>() {
        let raw_msg: String = server_message(&p.msg, &p.msg_raw);
        let items: Vec<String> = raw_msg
            .split(':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
        return vec![GameEvent::NpcDialogMenu {
            npc_id: p.naid,
            items,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcRoomNewentry>() {
        let title = crate::offline_text::decode(&p.title_raw);
        return vec![GameEvent::ChatRoomUpsert {
            owner_aid: p.aid,
            room_id: p.room_id,
            max_count: p.maxcount,
            cur_count: p.curcount,
            atype: p.atype,
            title,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcChangeChatroom>() {
        let title = crate::offline_text::decode(&p.title_raw);
        return vec![GameEvent::ChatRoomUpsert {
            owner_aid: p.aid,
            room_id: p.room_id,
            max_count: p.maxcount,
            cur_count: p.curcount,
            atype: p.atype,
            title,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDestroyRoom>() {
        return vec![GameEvent::ChatRoomDestroy { room_id: p.room_id }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcEnterRoom>() {
        let members = p
            .member_list
            .iter()
            .map(|m| ChatRoomMember {
                name: cstr(&m.name),
                is_owner: m.role == 0,
            })
            .collect();
        return vec![GameEvent::ChatRoomEntered {
            room_id: p.room_id,
            members,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcRefuseEnterRoom>() {
        return vec![GameEvent::ChatRoomJoinRefused { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckCreateChatroom>() {
        return vec![GameEvent::ChatRoomCreateResult { flag: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMemberNewentry>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::ChatRoomMemberJoined {
            name,
            cur_count: p.curcount,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMemberExit>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::ChatRoomMemberLeft {
            name,
            cur_count: p.curcount,
            kicked: p.atype == 1,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcRoleChange>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::ChatRoomOwnerChanged { name }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcWarplist2>() {
        // As many map names as the length holds (the old 0x011c had four).
        let destinations: Vec<String> = p
            .maps
            .iter()
            .map(|m| cstr(&m.map))
            .filter(|name| !name.is_empty())
            .collect();
        if destinations.is_empty() {
            return vec![];
        }
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::WarpList {
            skill,
            destinations,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcOpenEditdlg>() {
        return vec![GameEvent::NpcInputNumber { npc_id: p.naid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcOpenEditdlgstr>() {
        return vec![GameEvent::NpcInputString { npc_id: p.naid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSelectDealtype>() {
        return vec![GameEvent::NpcDealTypeSelect { npc_id: p.naid }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcPcSellItemlist>() {
        let items = p
            .item_list
            .iter()
            .map(|item| (item.index, item.price, item.overchargeprice))
            .collect();
        return vec![GameEvent::NpcShopSellList { npc_id: 0, items }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPcPurchaseResult>() {
        return vec![GameEvent::NpcShopBuyResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPcSellResult>() {
        return vec![GameEvent::NpcShopSellResult { result: p.result }];
    }

    // From 20181002 one list pair carries the inventory, the cart and storage, told apart by `invType`; a start
    // and an end packet frame each list
    if any.is::<PacketZcInventoryStart>() || any.is::<PacketZcInventoryEnd>() {
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcInventoryItemlistNormal>() {
        let items = p.item_info.iter().map(normal_item_from).collect();
        return match p.inv_type {
            INVTYPE_INVENTORY => vec![GameEvent::InventoryNormalItems { items }],
            INVTYPE_CART => vec![GameEvent::CartNormalItems { items }],
            INVTYPE_STORAGE => vec![GameEvent::StorageNormalItems { items }],
            _ => vec![GameEvent::Acknowledged],
        };
    }
    if let Some(p) = any.downcast_ref::<PacketZcInventoryItemlistEquip>() {
        let items = p.item_info.iter().map(equip_item_from).collect();
        return match p.inv_type {
            INVTYPE_INVENTORY => vec![GameEvent::InventoryEquipmentItems { items }],
            INVTYPE_CART => vec![GameEvent::CartEquipmentItems { items }],
            INVTYPE_STORAGE => vec![GameEvent::StorageEquipItems { items }],
            _ => vec![GameEvent::Acknowledged],
        };
    }
    /// A vending list's items: the shop's (0x0b3d) and the vendor's own (0x0b40) come in item structs of their own
    /// with the same fields
    macro_rules! vendor_items {
        ($list:expr) => {
            $list
                .iter()
                .map(|it| nostalre_game::event::VendorItem {
                    index: it.index,
                    item_id: it.itid,
                    amount: it.count,
                    price: it.price,
                    refine: it.refining_level,
                    is_identified: it.is_identified != 0,
                    is_damaged: it.is_damaged != 0,
                    item_type: it.atype,
                })
                .collect()
        };
    }
    // The item packets: 32-bit ids, five options, refine (and grade) after the options
    if let Some(p) = any.downcast_ref::<PacketZcItemPickupAckV7>() {
        return vec![GameEvent::InventoryItemPickup {
            index: p.index,
            item_id: p.itid,
            count: p.count,
            item_type: p.atype,
            is_identified: p.is_identified != 0,
            is_damaged: p.is_damaged != 0,
            refining_level: p.refining_level,
            grade: p.grade,
            options: item_options(&p.option_data),
            slot: cards(&p.slot),
            location: p.location,
            result: p.result,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcItemFallEntryV5>() {
        return vec![GameEvent::FloorItemAppeared {
            id: p.itaid,
            item_id: p.itid,
            is_identified: p.is_identified != 0,
            x: p.x_pos,
            y: p.y_pos,
            sub_x: p.sub_x,
            sub_y: p.sub_y,
            count: p.count,
            is_falling: true,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqWearEquipAckV5>() {
        return vec![GameEvent::InventoryEquipResult {
            index: p.index,
            wear_location: p.wear_location,
            view_id: p.w_item_sprite_number,
            success: p.result == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqTakeoffEquipAckV5>() {
        return vec![GameEvent::InventoryUnequipResult {
            index: p.index,
            wear_location: p.wear_location,
            success: p.flag == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddItemToCartV3>() {
        return vec![GameEvent::CartItemAdded {
            index: p.index as u16,
            item_id: p.itid,
            count: p.amount as i16,
            item_type: p.atype,
            is_identified: p.is_identified != 0,
            is_damaged: p.is_damaged != 0,
            refining_level: p.refining_level,
            slot: cards(&p.slot),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddItemToStoreV3>() {
        return vec![GameEvent::StorageItemAdded {
            index: p.index as u16,
            item_id: p.itid,
            count: p.amount as i16,
            item_type: p.atype,
            is_identified: p.is_identified != 0,
            is_damaged: p.is_damaged != 0,
            refining_level: p.refining_level,
            slot: cards(&p.slot),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddExchangeItemV4>() {
        return vec![GameEvent::ExchangeItemAdded {
            item_id: p.itid,
            item_type: p.atype,
            count: p.amount,
            is_identified: p.is_identified != 0,
            is_damaged: p.is_damaged != 0,
            refining_level: p.refining_level,
            slot: cards(&p.slot),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPcPurchaseItemlistV2>() {
        let items = p
            .item_list
            .iter()
            .map(|item| (item.itid, item.price, item.discountprice, item.atype))
            .collect();
        return vec![GameEvent::NpcShopBuyList { npc_id: 0, items }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcMarketOpen>() {
        let items = p
            .list
            .iter()
            .map(|item| nostalre_game::event::MarketItem {
                item_id: item.item_id,
                item_type: item.item_type,
                price: item.price,
                stock: shop_stock(item.qty as i32),
            })
            .collect();
        return vec![GameEvent::NpcMarketList { items }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUiOpen>() {
        // rAthena's out_ui_type: only the stylist (1) has a window here
        return match p.uitype {
            UI_STYLIST => vec![GameEvent::StylistOpened],
            6 if p.data > 0 => vec![GameEvent::OpenQuestDetail {quest_id:p.data as u32}],
            6 => vec![GameEvent::ShowQuestWindow],
            other => {
                tracing::debug!("UI type {other} not handled");
                Vec::new()
            }
        };
    }
    if let Some(p) = any.downcast_ref::<PacketZcStyleChangeRes>() {
        return vec![GameEvent::StylistResult {
            success: p.flag == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcBarterMarketIteminfo>() {
        let items = p
            .list
            .iter()
            .map(|item| BarterItem {
                item_id: item.item_id,
                item_type: item.item_type,
                shop_index: item.index as u32,
                stock: shop_stock(item.amount),
                zeny: 0,
                refine: 0,
                costs: vec![BarterCost {
                    item_id: item.currency_item_id,
                    refine: 0,
                    amount: item.currency_amount.max(0) as u32,
                }],
            })
            .collect();
        return vec![GameEvent::NpcBarterList {
            extended: false,
            items,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcExpandedBarterMarketIteminfo>() {
        return vec![GameEvent::NpcBarterList {
            extended: true,
            items: expanded_barter_items(&p.items, p.items_count),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNpcMarketPurchaseResult>() {
        // 0 succeeded, 0xffff failed (from 20190807; 1 and 0 before)
        return vec![GameEvent::NpcMarketBought {
            success: p.result == 0,
            items: p.list.iter().map(|item| (item.item_id, item.qty)).collect(),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPcPurchaseItemlistFrommcV3>() {
        let items = vendor_items!(p.item_list);
        return vec![GameEvent::VendingShopList {
            aid: p.aid,
            unique_id: p.unique_id,
            items,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPcPurchaseMyitemlistV2>() {
        let items = vendor_items!(p.item_list);
        return vec![GameEvent::VendingOwnStock { items }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDeleteitemFromMcstore2>() {
        return vec![GameEvent::VendingStockDecrement {
            index: p.index,
            count: p.count,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcRepairitemlist2>() {
        let items = p
            .item_list
            .iter()
            .map(|info| nostalre_game::event::RefineItemRow {
                index: info.index,
                item_id: info.itid,
                refine: info.refining_level,
                cards: cards(&info.slot),
            })
            .collect();
        return vec![GameEvent::RepairItemList { items }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUseItemAck2>() {
        return vec![GameEvent::InventoryUseItemResult {
            index: p.index,
            count: p.count,
            success: p.result,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcEquipArrow>() {
        return vec![GameEvent::InventoryArrowEquipped {
            index: p.index as u16,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcItemThrowAck>() {
        return vec![GameEvent::InventoryItemRemoved {
            index: p.index,
            count: p.count,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDeleteItemFromBody>() {
        return vec![GameEvent::InventoryItemRemoved {
            index: p.index,
            count: p.count,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcDeleteItemFromCart>() {
        return vec![GameEvent::CartItemRemoved {
            index: p.index as u16,
            count: p.count as i16,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyCartitemCountinfo>() {
        return vec![GameEvent::CartCountInfo {
            cur_weight: p.cur_weight,
            max_weight: p.max_weight,
            cur_count: p.cur_count,
            max_count: p.max_count,
        }];
    }
    if any.downcast_ref::<PacketZcCartoff>().is_some() {
        return vec![GameEvent::CartOff];
    }

    if let Some(p) = any.downcast_ref::<PacketZcNotifyStoreitemCountinfo>() {
        return vec![GameEvent::StorageOpened {
            cur: p.cur_count,
            max: p.max_count,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDeleteItemFromStore>() {
        return vec![GameEvent::StorageItemRemoved {
            index: p.index as u16,
            amount: p.count as i16,
        }];
    }
    if any.downcast_ref::<PacketZcCloseStore>().is_some() {
        return vec![GameEvent::StorageClosed];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqStorePassword>() {
        return StoragePasswordPrompt::from_info(p.info)
            .map(|prompt| GameEvent::StoragePasswordRequest { prompt })
            .into_iter()
            .collect();
    }
    if let Some(p) = any.downcast_ref::<PacketZcResultStorePassword>() {
        return StoragePasswordOutcome::from_result(p.result)
            .map(|outcome| GameEvent::StoragePasswordResult {
                outcome,
                error_count: p.error_count,
            })
            .into_iter()
            .collect();
    }

    if let Some(p) = any.downcast_ref::<PacketZcReqExchangeItem2>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::ExchangeRequested {
            name,
            gid: p.gid,
            level: p.level,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckExchangeItem2>() {
        return vec![GameEvent::ExchangeAckResult {
            result: p.result,
            level: p.level,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckAddExchangeItem>() {
        return vec![GameEvent::ExchangeAddResult {
            index: p.index as u16,
            result: p.result,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcConcludeExchangeItem>() {
        return vec![GameEvent::ExchangeConcluded { who: p.who }];
    }
    if any.downcast_ref::<PacketZcCancelExchangeItem>().is_some() {
        return vec![GameEvent::ExchangeCanceled];
    }
    if let Some(p) = any.downcast_ref::<PacketZcExecExchangeItem>() {
        return vec![GameEvent::ExchangeCompleted { result: p.result }];
    }
    if any.downcast_ref::<PacketZcExchangeitemUndo>().is_some() {
        return vec![GameEvent::ExchangeUndo];
    }

    if any.is::<PacketZcNotifyUnreadmail>() {
        // RODEX's unread-mail icon (it replaced ZC_MAIL_RECEIVE 0x024a); RODEX is left out, so there is no mail to
        // show
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMailWindows>() {
        return vec![GameEvent::MailWindow { open: p.atype == 0 }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMailReqGetList>() {
        let entries = p
            .mail_list
            .iter()
            .map(|m| MailEntry {
                mail_id: m.mail_id,
                title: cstr(&m.header),
                read: m.is_open != 0,
                sender: cstr(&m.from_name),
                time: m.delete_time as u32,
            })
            .collect();
        return vec![GameEvent::MailInboxReceived { entries }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMailReqOpen>() {
        let item = if p.count > 0 && p.itid != 0 {
            Some(MailItem {
                nameid: p.itid.into(),
                amount: p.count as u32,
                item_type: p.atype,
                identified: p.is_identified,
                damaged: p.is_damaged,
                refine: p.refining_level,
                cards: cards(&p.slot),
            })
        } else {
            None
        };
        return vec![GameEvent::MailOpened {
            mail: OpenedMail {
                mail_id: p.mail_id as u32,
                title: cstr(&p.header),
                sender: cstr(&p.from_name),
                zeny: p.money,
                item,
                body: p.msg.trim_end_matches('\0').to_string(),
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckMailDelete>() {
        return vec![GameEvent::MailDeleteAck {
            mail_id: p.mail_id as u32,
            ok: p.result == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMailReqGetItem>() {
        return vec![GameEvent::MailGetItemAck {
            result: p.result as u8,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckMailAddItem>() {
        return vec![GameEvent::MailAddItemAck {
            index: p.index as u16,
            ok: p.result == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMailReqSend>() {
        return vec![GameEvent::MailSendAck { ok: p.result == 0 }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMailReceive>() {
        return vec![GameEvent::MailNewReceived {
            mail_id: p.mail_id,
            title: cstr(&p.header),
            sender: cstr(&p.from_name),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckMailReturn>() {
        return vec![GameEvent::MailReturnAck {
            mail_id: p.mail_id as u32,
            ok: p.result == 0,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcItemcompositionList>() {
        return vec![GameEvent::CardInsertItemList {
            card_index: 0,
            equip_indices: p.itidlist.clone(),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckItemcomposition>() {
        return vec![GameEvent::CardInsertResult {
            equip_index: p.equip_index as u16,
            card_index: p.card_index as u16,
            result: p.result,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcItemEntry>() {
        return vec![GameEvent::FloorItemAppeared {
            id: p.itaid,
            item_id: p.itid_32,
            is_identified: p.is_identified,
            x: p.x_pos,
            y: p.y_pos,
            sub_x: p.sub_x,
            sub_y: p.sub_y,
            count: p.count,
            is_falling: false,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcItemDisappear>() {
        return vec![GameEvent::FloorItemDisappeared { id: p.itaid }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcAid>() {
        debug!("zone server confirmed AID={}", p.aid);
        return vec![GameEvent::Acknowledged];
    }
    if any.downcast_ref::<PacketHcBlockCharacter>().is_some() {
        return vec![GameEvent::Acknowledged];
    }
    if any.downcast_ref::<PacketPincodeLoginstate>().is_some() {
        return vec![GameEvent::Acknowledged];
    }
    if let Some(p) = any.downcast_ref::<PacketZcFriendsList>() {
        let friends = p
            .friend_list
            .iter()
            .map(|f| FriendData {
                aid: f.aid,
                gid: f.gid,
                name: cstr(&f.name),
                online: false,
            })
            .collect();
        return vec![GameEvent::FriendListReceived { friends }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcFriendsState>() {
        return vec![GameEvent::FriendStateChanged {
            aid: p.aid,
            gid: p.gid,
            online: !p.state,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddFriendsList>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::FriendAddResult {
            result: p.result as u8,
            aid: p.aid,
            gid: p.gid,
            name,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcReqAddFriends>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::FriendRequestReceived {
            req_aid: p.req_aid,
            req_gid: p.req_gid,
            name,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDeleteFriends>() {
        return vec![GameEvent::FriendRemoved {
            aid: p.aid,
            gid: p.gid,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillinfoList>() {
        return vec![GameEvent::SkillListReceived {
            skills: parse_skill_info_list(&p.skill_list),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillinfoUpdate>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::SkillUpdated {
            skill,
            level: p.level,
            sp_cost: p.spcost,
            attack_range: p.attack_range,
            upgradable: p.upgradable,
            target_type: None,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillinfoUpdate2>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::SkillUpdated {
            skill,
            level: p.level,
            sp_cost: p.spcost,
            attack_range: p.attack_range,
            upgradable: p.upgradable,
            target_type: Some(skill_target_type(p.atype)),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddSkill>() {
        let Some(skill) = known_skill(p.data.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::SkillAdded {
            skill: SkillInfo {
                skill,
                level: p.data.level,
                sp_cost: p.data.spcost,
                attack_range: p.data.attack_range,
                upgradable: p.data.upgradable != 0,
                skill_target_type: skill_target_type(p.data.atype),
            },
        }];
    }
    // From 20190807 the client merges a refreshed skill list rather than replacing it: only this takes a skill away
    if let Some(p) = any.downcast_ref::<PacketZcSkillinfoDelete>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::SkillRemoved { skill }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcShortcutKeyListV2>() {
        // The second tab (rAthena's `hotkey_rowshift2` bar) has no window here.
        if p.tab != 0 {
            return vec![GameEvent::Acknowledged];
        }
        let slots: Vec<(i8, u32, i16)> = p
            .short_cut_key
            .iter()
            .map(|k| (k.is_skill, k.id, k.count))
            .collect();
        return vec![GameEvent::HotkeyListReceived { slots }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcActionFailure>() {
        return vec![GameEvent::ActionFailure {
            error_code: p.error_code,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyMapproperty2>() {
        let kind = MapKind::from_property(p.atype);
        return vec![GameEvent::MapPropertyChanged(MapProperties::with_flags(
            kind,
            p.flags as u64,
        ))];
    }

    if let Some(p) = any.downcast_ref::<PacketZcAutorunSkill>() {
        let Some(skill) = known_skill(p.data.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::AutoCastSkill {
            skill,
            level: p.data.level,
            sp_cost: p.data.spcost,
            attack_range: p.data.attack_range,
            skill_target_type: skill_target_type(p.data.atype),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcItemidentifyList>() {
        return vec![GameEvent::ItemIdentifyList {
            indices: p.itidlist.clone(),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckItemidentify>() {
        return vec![GameEvent::ItemIdentifyResult {
            index: p.index,
            ok: p.result == 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMakingarrowList>() {
        let item_ids = p.arrow_list.iter().map(|a| a.itid_32).collect();
        return vec![GameEvent::MakingArrowList { item_ids }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMakableitemlist>() {
        let item_ids = p.info.iter().map(|e| e.itid_32).collect();
        return vec![GameEvent::MakableItemList { item_ids }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMakingitemList>() {
        let item_ids = p.items.iter().map(|e| e.itid_32).collect();
        return vec![GameEvent::CookingList {
            mk_type: p.make_item,
            item_ids,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckReqmakingitem>() {
        return vec![GameEvent::MakingItemResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcNotifyWeaponitemlist>() {
        let items = p.item_list.iter().map(refine_row_from).collect();
        return vec![GameEvent::WeaponRefineList { items }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckWeaponrefine>() {
        return vec![GameEvent::WeaponRefineResult {
            result: p.msg,
            item_id: p.itid_32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckItemrefining>() {
        return vec![GameEvent::ItemRefiningResult {
            index: p.item_index as u16,
            refine: p.refining_level as u8,
            result: p.result,
        }];
    }
    if any.is::<PacketZcOpenRefiningUi>() {
        return vec![GameEvent::RefineWindowOpened];
    }
    if let Some(p) = any.downcast_ref::<PacketZcRefiningMaterialList>() {
        return vec![GameEvent::RefineMaterials {
            index: p.item_index,
            blessings: p.blacksmith_blessing,
            materials: p
                .req
                .iter()
                .map(|m| nostalre_game::event::RefineMaterial {
                    item_id: m.item_id,
                    chance: m.chance,
                    zeny: m.zeny,
                })
                .collect(),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcBroadcastItemrefiningResult>() {
        return vec![GameEvent::RefineAnnounced {
            name: cstr(&p.name),
            item_id: p.item_id,
            refine: p.refine_level,
            success: p.status != 0,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckItemrepair>() {
        return vec![GameEvent::RepairItemResult {
            index: p.index,
            ok: p.result == 0,
        }];
    }
    // One entry per skill offered
    if let Some(p) = any.downcast_ref::<PacketZcAutospelllist2>() {
        return autospell_list(&p.skid);
    }
    if let Some(p) = any.downcast_ref::<PacketZcSkillSelectRequest>() {
        let skills = p
            .skidlist
            .iter()
            .filter_map(|&s| known_skill(s.into()))
            .collect();
        return vec![GameEvent::SkillSelectList { why: p.why, skills }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcOpenstore>() {
        return vec![GameEvent::OpenVendingSetup {
            max_items: p.itemcount,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAckOpenstore2>() {
        return vec![GameEvent::VendingOpenResult { result: p.result }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcStoreEntry>() {
        let name = crate::offline_text::decode(&p.store_name_raw);
        return vec![GameEvent::VendingBoardShown {
            aid: p.maker_aid,
            name,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDisappearEntry>() {
        return vec![GameEvent::VendingBoardHidden { aid: p.maker_aid }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPcPurchaseResultFrommc>() {
        return vec![GameEvent::VendingPurchaseResult {
            index: p.index,
            curcount: p.curcount,
            result: p.result,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcPropertyHomun>() {
        let name: String = cstr(&p.sz_name);
        return vec![GameEvent::HomunPropertyReceived {
            property: HomunculusProperty {
                name,
                renamed: p.b_modified & 0x1 != 0,
                vaporized: p.b_modified & 0x2 != 0,
                level: p.n_level,
                hunger: p.n_fullness,
                intimacy: p.n_relationship,
                // no accessory from 20190619 (0x0b2f on)
                accessory: 0,
                atk: p.atk,
                matk: p.matk,
                hit: p.hit,
                critical: p.critical,
                def: p.def,
                mdef: p.mdef,
                flee: p.flee,
                aspd: p.aspd,
                hp: p.hp_32,
                max_hp: p.max_hp_32,
                sp: p.sp_32,
                max_sp: p.max_sp_32,
                exp: p.exp_64,
                max_exp: p.max_exp_64,
                skill_points: p.skpoint,
                atk_range: p.atkrange,
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcChangestateMer>() {
        return vec![GameEvent::CompanionStateChanged {
            state: p.state,
            gid: p.gid as u32,
            data: p.data,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcFeedMer>() {
        return vec![GameEvent::HomunFeedResult {
            success: p.c_ret != 0,
            item_id: p.itid_32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMerInit>() {
        let name: String = cstr(&p.name);
        return vec![GameEvent::MercenaryInfoReceived {
            is_init: true,
            info: MercenaryInfo {
                gid: p.aid as u32,
                name,
                level: p.level,
                atk: p.atk,
                matk: p.matk,
                hit: p.hit,
                critical: p.critical,
                def: p.def,
                mdef: p.mdef,
                flee: p.flee,
                aspd: p.aspd,
                atk_range: p.atkrange,
                hp: p.hp as u32,
                max_hp: p.max_hp as u32,
                sp: p.sp as u32,
                max_sp: p.max_sp as u32,
                expire_date: p.expire_date,
                faith: p.faith,
                calls: p.toal_call_num,
                kills: p.approval_monster_kill_counter,
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMerParChange>() {
        return vec![GameEvent::MercenaryParamChanged {
            var: p.var,
            value: p.value,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcHoParChange>() {
        return vec![GameEvent::HomunParamChanged {
            var: p.var,
            value: p.value_64 as i64,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcHoskillinfoList>() {
        return vec![GameEvent::HomunSkillList {
            skills: parse_skill_info_list(&p.skill_list),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcHoskillinfoUpdate>() {
        let Some(skill) = known_skill(p.skid as u32) else {
            return vec![GameEvent::Acknowledged];
        };
        return vec![GameEvent::HomunSkillUpdate {
            skill,
            level: p.level,
            sp_cost: p.spcost,
            attack_range: p.attack_range,
            upgradable: p.upgradable,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcMerSkillinfoList>() {
        return vec![GameEvent::MercenarySkillList {
            skills: parse_skill_info_list(&p.skill_list),
        }];
    }

    if any.downcast_ref::<PacketZcStartCapture>().is_some() {
        return vec![GameEvent::PetCaptureStart];
    }
    if let Some(p) = any.downcast_ref::<PacketZcTrycaptureMonster>() {
        return vec![GameEvent::PetCaptureResult { ok: p.result != 0 }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPropertyPet>() {
        let name: String = cstr(&p.sz_name);
        return vec![GameEvent::PetProperty {
            property: PetProperty {
                name,
                renamed: p.b_modified != 0,
                level: p.n_level,
                hunger: p.n_fullness,
                intimacy: p.n_relationship,
                accessory: p.itid,
                job: p.job,
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcFeedPet>() {
        return vec![GameEvent::PetFeedResult {
            ok: p.c_ret != 0,
            food_item_id: p.itid_32,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcChangestatePet>() {
        return vec![GameEvent::PetStateChanged {
            ty: p.atype,
            gid: p.gid as u32,
            data: p.data,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPeteggList>() {
        let indices = p.egg_list.iter().map(|e| e.index as u16).collect();
        return vec![GameEvent::PetEggList { indices }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcPetAct>() {
        return vec![GameEvent::PetAct {
            gid: p.gid as u32,
            data: p.data,
        }];
    }

    if let Some(p) = any.downcast_ref::<PacketZcAllQuestList3>() {
        return vec![GameEvent::QuestListReceived {
            quests: parse_quest_list3(&p.quest_data_raw, p.quest_count),
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcAddQuest>() {
        let objectives = p
            .hunt
            .iter()
            .take(p.count.max(0) as usize)
            .map(|h| QuestObjective {
                hunt_id: h.hunt_ident,
                mob_id: h.mob_gid,
                name: cstr(&h.mob_name),
                current: h.hunt_count,
                required: 0,
            })
            .collect();
        return vec![GameEvent::QuestAdded {
            quest: Quest {
                id: p.quest_id,
                active: p.active,
                end_time: quest_end_time(p.quest_end_time),
                objectives,
            },
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcHuntingQuestInfo>() {
        let entries = p
            .hunt_list
            .iter()
            .map(|e| QuestHuntEntry {
                hunt_id: e.hunt_ident,
                current: e.count,
                required: e.max_count,
            })
            .collect();
        return vec![GameEvent::QuestHuntUpdated { entries }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcDelQuest>() {
        return vec![GameEvent::QuestRemoved {
            quest_id: p.quest_id,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcUpdateMissionHunt>() {
        let entries = p
            .mob_hunt_list
            .iter()
            .map(|e| QuestHuntEntry {
                hunt_id: e.hunt_ident,
                current: e.count,
                required: e.max_count,
            })
            .collect();
        return vec![GameEvent::QuestHuntUpdated { entries }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcActiveQuest>() {
        return vec![GameEvent::QuestActiveChanged {
            quest_id: p.quest_id,
            active: p.active,
        }];
    }
    if let Some(p) = any.downcast_ref::<PacketZcQuestNotifyEffect>() {
        return vec![GameEvent::QuestNpcMarker {
            npc_id: p.npc_id,
            x: p.x_pos as u16,
            y: p.y_pos as u16,
            effect: p.effect,
            color: p.atype as u8,
        }];
    }

    if matches!(
        nostalre_profiling::debug::packet_trace(),
        nostalre_profiling::debug::PacketTrace::All
            | nostalre_profiling::debug::PacketTrace::Unhandled
    ) {
        tracing::info!("unhandled packet: {}", packet.name());
    }
    vec![]
}

fn cstr(chars: &[char]) -> String {
    chars.iter().take_while(|c| **c != '\0').collect()
}

/// A variable-length text field, cut at its first NUL.
fn str_cstr(text: &str) -> String {
    text.chars().take_while(|c| *c != '\0').collect()
}

fn server_message(_text: &str, _raw: &[u8]) -> String {
    // The PC offline integration test must use the same original-byte decoder
    // as Horizon; otherwise valid Windows-1252 accents look broken in previews.
    if cfg!(target_os="horizon") || std::env::var_os("NOSTALRE_PT_BR").is_some_and(|v|v=="1") {
        crate::offline_text::decode(_raw)
    } else {str_cstr(_text)}
}

fn quest_end_time(t: i32) -> Option<u32> {
    (t > 0).then_some(t as u32)
}

/// ZC_ALL_QUEST_LIST3 0x09f8 (from 20150513): per quest id.L state.B start.L end.L count.W, then `count`
/// objectives of hunt id.L type.L mob id.L min level.W max level.W killed.W total.W name.24B. A list inside a
/// list, which the packet crate cannot lay out: read from its bytes, stopping at the first one cut short.
fn parse_quest_list3(data: &[u8], count: i32) -> Vec<Quest> {
    const QUEST: usize = 15;
    const OBJECTIVE: usize = 44;
    let u32_at = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let i16_at = |b: &[u8], at: usize| i16::from_le_bytes(b[at..at + 2].try_into().unwrap());
    let mut quests = Vec::new();
    let mut rest = data;
    for _ in 0..count.max(0) {
        let Some(head) = rest.get(..QUEST) else { break };
        let objectives = i16_at(head, 13).max(0) as usize;
        let Some(body) = rest.get(QUEST..QUEST + objectives * OBJECTIVE) else {
            break;
        };
        quests.push(Quest {
            id: u32_at(head, 0),
            active: head[4] != 0,
            end_time: quest_end_time(u32_at(head, 9) as i32),
            objectives: body
                .chunks_exact(OBJECTIVE)
                .map(|o| QuestObjective {
                    hunt_id: u32_at(o, 0),
                    mob_id: u32_at(o, 8),
                    name: raw_cstr(&o[20..44]),
                    current: i16_at(o, 16),
                    required: i16_at(o, 18),
                })
                .collect(),
        });
        rest = &rest[QUEST + objectives * OBJECTIVE..];
    }
    quests
}

fn rgb_u32_to_rgba(rgb: u32) -> [f32; 4] {
    [
        ((rgb >> 16) & 0xff) as f32 / 255.0,
        ((rgb >> 8) & 0xff) as f32 / 255.0,
        (rgb & 0xff) as f32 / 255.0,
        1.0,
    ]
}

const BROADCAST_YELLOW: [f32; 4] = [1.0, 1.0, 0.0, 1.0];
const BROADCAST_BLUE: [f32; 4] = [0.0, 1.0, 1.0, 1.0];

fn parse_broadcast(msg: &str) -> (String, [f32; 4]) {
    if let Some(rest) = msg.strip_prefix("tool")
        && rest.len() >= 6
        && let Ok(rgb) = u32::from_str_radix(&rest[..6], 16)
    {
        return (rest[6..].to_string(), rgb_u32_to_rgba(rgb));
    }
    if let Some(rest) = msg.strip_prefix("blue") {
        return (rest.to_string(), BROADCAST_BLUE);
    }
    if let Some(rest) = msg.strip_prefix("ssss") {
        return (rest.to_string(), BROADCAST_YELLOW);
    }
    (msg.to_string(), BROADCAST_YELLOW)
}

fn classify_banner(msg: String) -> (String, BannerKind) {
    if let Some(rest) = msg.strip_prefix('@') {
        return (rest.to_string(), BannerKind::Once);
    }
    if let Some(rest) = msg.strip_prefix("$$")
        && let Some((count, tail)) = rest.split_once('$')
        && let Ok(count) = count.parse::<u16>()
    {
        let text = tail.strip_prefix('$').unwrap_or(tail);
        let text = text.split_once('$').map_or(text, |(_, m)| m);
        return (text.to_string(), BannerKind::Repeat(count));
    }
    (msg, BannerKind::None)
}

/// A text field read from raw bytes as the generated `[char]` fields are (one char per byte), cut at its NUL.
fn raw_cstr(bytes: &[u8]) -> String {
    bytes
        .iter()
        .take_while(|b| **b != 0)
        .map(|&b| b as char)
        .collect()
}

/// Fixed-width text fields are not NUL-guaranteed: cut at the first NUL, then
/// decode, so Korean text survives.
fn raw_euc_kr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    encoding_rs::EUC_KR.decode(&bytes[..end]).0.into_owned()
}

fn parse_skill_info_list(list: &[packets::packets::SKILLINFO]) -> Vec<SkillInfo> {
    let mut skill_info = Vec::with_capacity(list.len());
    for s in list {
        if let Ok(skill) = SkillEnum::try_from_value(s.skid as u32) {
            skill_info.push(SkillInfo {
                skill,
                level: s.level,
                sp_cost: s.spcost,
                attack_range: s.attack_range,
                upgradable: s.upgradable != 0,
                skill_target_type: skill_target_type(s.atype),
            });
        }
    }
    skill_info
}

/// rAthena's `e_inventory_type`, the `invType` of the 20181002 item lists (3, guild storage, is not used)
const INVTYPE_INVENTORY: u8 = 0;
const INVTYPE_CART: u8 = 1;
const INVTYPE_STORAGE: u8 = 2;

/// `Flag` bits of the 20120925 item lists
const ITEM_FLAG_IDENTIFIED: u8 = 1;
const ITEM_FLAG_DAMAGED: u8 = 2;

/// The four cards (32-bit from 20181121)
fn cards(slot: &EQUIPSLOTINFO) -> [u32; 4] {
    [slot.card1_32, slot.card2_32, slot.card3_32, slot.card4_32]
}

fn normal_item_from(i: &NormalitemInfo) -> NormalItemData {
    NormalItemData {
        index: i.index,
        item_id: i.itid,
        item_type: i.atype,
        is_identified: i.flag & ITEM_FLAG_IDENTIFIED != 0,
        count: i.count,
        wear_state: i.wear_state,
        slot: cards(&i.slot),
    }
}

fn equip_item_from(i: &EquipitemInfo) -> EquipmentItemData {
    EquipmentItemData {
        grade: i.grade,
        options: item_options(&i.option_data),
        index: i.index,
        item_id: i.itid,
        item_type: i.atype,
        is_identified: i.flag & ITEM_FLAG_IDENTIFIED != 0,
        location: i.location,
        wear_state: i.wear_state,
        is_damaged: i.flag & ITEM_FLAG_DAMAGED != 0,
        refining_level: i.refining_level,
        slot: cards(&i.slot),
    }
}

fn autospell_list(skids: &[i32]) -> Vec<GameEvent> {
    let skills = skids
        .iter()
        .filter(|&&s| s != 0)
        .filter_map(|&s| known_skill(s as u32))
        .collect();
    vec![GameEvent::AutoSpellList { skills }]
}

/// rAthena's `e_damage_type`; a value past the known ones reads as a plain hit instead of panicking, which would
/// drop the damage
fn action_type(action: u8) -> ActionType {
    ActionType::try_from_value(action as usize).unwrap_or(ActionType::Attack)
}

/// rAthena's `clr_type`; a value past the known ones reads as leaving sight
fn vanish_type(atype: u8) -> VanishType {
    VanishType::try_from_value(atype as usize).unwrap_or(VanishType::OutOfSight)
}

/// A skill's target type; one the client does not know reads as the default instead of panicking, which would drop
/// the whole skill list
fn skill_target_type(atype: i32) -> SkillTargetType {
    SkillTargetType::try_from_value(atype as usize).unwrap_or_default()
}

fn refine_row_from(info: &RepairitemInfo) -> nostalre_game::event::RefineItemRow {
    nostalre_game::event::RefineItemRow {
        index: info.index,
        item_id: info.itid_32,
        refine: info.refining_level,
        cards: cards(&info.slot),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::cast_possible_truncation)] // test packets are built from small literals
    use super::*;

    /// The 20250716 item lists at rAthena's offsets (`NORMALITEM_INFO` 34 bytes, `EQUIPITEM_INFO` 68), with ids above
    /// 16 bits and `invType` choosing the inventory, cart or storage.
    #[test]
    fn item_lists_at_20250716_land_by_inventory_type() {
        let packetver = 20250716;
        let list = |id: u16, inv_type: u8, item: Vec<u8>| {
            let mut wire = id.to_le_bytes().to_vec();
            wire.extend(((5 + item.len()) as u16).to_le_bytes());
            wire.push(inv_type);
            wire.extend(item);
            wire
        };

        let mut equip = vec![0u8; 68];
        equip[0..2].copy_from_slice(&7i16.to_le_bytes());
        equip[2..6].copy_from_slice(&490_000u32.to_le_bytes());
        equip[6] = 4;
        equip[7..11].copy_from_slice(&2u32.to_le_bytes());
        equip[15..19].copy_from_slice(&4001u32.to_le_bytes());
        equip[65] = 7;
        equip[66] = 1;
        equip[67] = ITEM_FLAG_IDENTIFIED | ITEM_FLAG_DAMAGED;
        let packet = packets::packets_parser::parse(&list(0x0b39, INVTYPE_CART, equip), packetver);
        match dispatch_packet(packet.as_ref()).as_slice() {
            [GameEvent::CartEquipmentItems { items }] => {
                let i = &items[0];
                assert_eq!(
                    (i.index, i.item_id, i.item_type, i.location),
                    (7, 490_000, 4, 2)
                );
                assert_eq!((i.refining_level, i.slot), (7, [4001, 0, 0, 0]));
                assert!(i.is_identified && i.is_damaged);
            }
            other => panic!("unexpected events: {other:?}"),
        }

        let mut normal = vec![0u8; 34];
        normal[0..2].copy_from_slice(&3i16.to_le_bytes());
        normal[2..6].copy_from_slice(&1_000_001u32.to_le_bytes());
        normal[6] = 3;
        normal[7..9].copy_from_slice(&25i16.to_le_bytes());
        normal[33] = ITEM_FLAG_IDENTIFIED;
        let packet =
            packets::packets_parser::parse(&list(0x0b09, INVTYPE_STORAGE, normal), packetver);
        match dispatch_packet(packet.as_ref()).as_slice() {
            [GameEvent::StorageNormalItems { items }] => {
                let i = &items[0];
                assert_eq!(
                    (i.index, i.item_id, i.item_type, i.count),
                    (3, 1_000_001, 3, 25)
                );
                assert!(i.is_identified);
            }
            other => panic!("unexpected events: {other:?}"),
        }
    }

    /// The wire carries a skill name we deliberately ignore: the id alone must
    /// resolve the skill, and the display name must come out of the GRF table.
    #[test]
    fn a_skill_list_packet_lands_in_the_skill_list_and_renders_its_table_name() {
        use nostalre_game::data_table::skill_name_table::{
            SkillNameTable, format_skill_display_name,
        };
        use nostalre_game::skill::SkillList;

        let packetver = 20250716;
        let mut wire = vec![0x01u8, 0x0f, 0x00, 0x00];
        for (skid, level, spcost, atype) in [
            (SkillEnum::McMammonite.id() as i16, 5i16, 20i16, 1i32),
            (SkillEnum::McDiscount.id() as i16, 10, 0, 0),
        ] {
            let mut row = Vec::new();
            row.extend_from_slice(&skid.to_le_bytes());
            row.extend_from_slice(&atype.to_le_bytes());
            row.extend_from_slice(&level.to_le_bytes());
            row.extend_from_slice(&spcost.to_le_bytes());
            row.extend_from_slice(&1i16.to_le_bytes());
            row.extend_from_slice(&[0u8; 24]);
            row.push(1);
            assert_eq!(row.len(), 37);
            wire.extend_from_slice(&row);
        }
        let len = wire.len() as i16;
        wire[2..4].copy_from_slice(&len.to_le_bytes());

        let pkt = PacketZcSkillinfoList::from(&wire, packetver);
        let events = dispatch_packet(&pkt);
        let GameEvent::SkillListReceived { skills } = &events[0] else {
            panic!("expected SkillListReceived, got {events:?}");
        };

        let mut list = SkillList::new();
        let icons = list.apply_skill_list(skills.clone());
        assert!(icons[0].ends_with("/item/mc_mammonite.bmp"), "{}", icons[0]);

        let mammonite = list.get_skill(SkillEnum::McMammonite).expect("learned");
        assert_eq!((mammonite.level, mammonite.sp_cost), (5, 20));
        assert_eq!(mammonite.skill_target_type, SkillTargetType::from_value(1));

        let mut entries = std::collections::HashMap::new();
        entries.insert("MC_MAMMONITE".to_string(), "Mammonite".to_string());
        let names = SkillNameTable::from_entries(entries);
        assert_eq!(
            format_skill_display_name(mammonite.skill, Some(&names)),
            "Mammonite"
        );
        // Absent from the table: the internal id is what shows.
        let discount = list.get_skill(SkillEnum::McDiscount).expect("learned");
        assert_eq!(
            format_skill_display_name(discount.skill, Some(&names)),
            "MC_DISCOUNT"
        );
    }

    #[test]
    fn dispatch_unknown_packet_returns_empty() {
        let packetver = 20250716;
        let mut pkt = PacketZcRefuseEnter::new(packetver);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert!(result.is_empty());
    }

    #[test]
    fn dispatch_self_config_packets_map_to_kinds() {
        let packetver = 20250716;

        let mut party = PacketZcPartyConfig::new(packetver);
        party.set_b_refuse_join_msg(true);
        party.fill_raw();
        assert!(matches!(
            dispatch_packet(&party).as_slice(),
            [GameEvent::SelfConfigChanged {
                kind: SelfConfigKind::RefusePartyInvite,
                enabled: true
            }]
        ));

        let mut notify = PacketZcConfigNotify::new(packetver);
        notify.set_b_open_equipment_win(true);
        notify.fill_raw();
        assert!(matches!(
            dispatch_packet(&notify).as_slice(),
            [GameEvent::SelfConfigChanged {
                kind: SelfConfigKind::OpenEquipmentWindow,
                enabled: true
            }]
        ));

        let mut homun = PacketZcConfig::new(packetver);
        homun.set_config(3);
        homun.set_value(1);
        homun.fill_raw();
        assert!(matches!(
            dispatch_packet(&homun).as_slice(),
            [GameEvent::SelfConfigChanged {
                kind: SelfConfigKind::HomunculusAutofeed,
                enabled: true
            }]
        ));

        // CONFIG_DISABLE_SHOWCOSTUMES, sent on each map change from 20230419
        let mut costumes = PacketZcConfig::new(20250716);
        costumes.set_config(5);
        costumes.set_value(1);
        costumes.fill_raw();
        assert!(matches!(
            dispatch_packet(&costumes).as_slice(),
            [GameEvent::SelfConfigChanged {
                kind: SelfConfigKind::HideCostumes,
                enabled: true
            }]
        ));

        let mut unknown = PacketZcConfig::new(packetver);
        unknown.set_config(99);
        unknown.fill_raw();
        assert!(dispatch_packet(&unknown).is_empty());
    }

    #[test]
    fn dispatch_broadcast_packets_carry_message_and_color() {
        let packetver = 20250716;

        let mut styled = PacketZcBroadcast2::new(packetver);
        styled.set_font_color(0x00ff00);
        styled.set_msg("Server restart soon".to_string());
        styled.fill_raw();
        assert!(matches!(
            dispatch_packet(&styled).as_slice(),
            [GameEvent::BroadcastMessage { message, color, banner: BannerKind::None }]
                if message == "Server restart soon" && *color == [0.0, 1.0, 0.0, 1.0]
        ));

        let mut plain = PacketZcBroadcast::new(packetver);
        plain.set_msg("Welcome".to_string());
        plain.fill_raw();
        assert!(matches!(
            dispatch_packet(&plain).as_slice(),
            [GameEvent::BroadcastMessage { message, color, banner: BannerKind::None }]
                if message == "Welcome" && *color == BROADCAST_YELLOW
        ));

        let mut blue = PacketZcBroadcast::new(packetver);
        blue.set_msg("blueGvG starts".to_string());
        blue.fill_raw();
        assert!(matches!(
            dispatch_packet(&blue).as_slice(),
            [GameEvent::BroadcastMessage { message, color, banner: BannerKind::None }]
                if message == "GvG starts" && *color == BROADCAST_BLUE
        ));

        let mut tool = PacketZcBroadcast::new(packetver);
        tool.set_msg("toolff0000red alert".to_string());
        tool.fill_raw();
        assert!(matches!(
            dispatch_packet(&tool).as_slice(),
            [GameEvent::BroadcastMessage { message, color, banner: BannerKind::None }]
                if message == "red alert" && *color == [1.0, 0.0, 0.0, 1.0]
        ));

        let mut banner = PacketZcBroadcast::new(packetver);
        banner.set_msg("@Server maintenance".to_string());
        banner.fill_raw();
        assert!(matches!(
            dispatch_packet(&banner).as_slice(),
            [GameEvent::BroadcastMessage { message, banner: BannerKind::Once, .. }]
                if message == "Server maintenance"
        ));
    }

    #[test]
    fn dispatch_channel_chat_packets_map_to_events() {
        let packetver = 20250716;

        let mut guild = PacketZcGuildChat::new(packetver);
        guild.set_msg("Leader : rally at emp".to_string());
        guild.fill_raw();
        assert!(matches!(
            dispatch_packet(&guild).as_slice(),
            [GameEvent::GuildChatMessage { message }] if message == "Leader : rally at emp"
        ));
    }

    #[test]
    fn dispatch_guild_emblem_img_carries_blob() {
        let packetver = 20250716;
        let blob = vec![0x78, 0x9c, 0x01, 0x02, 0x03, 0x04, 0x05];
        let mut pkt = PacketZcGuildEmblemImg::new(packetver);
        pkt.set_gdid(42);
        pkt.set_emblem_version(7);
        pkt.set_img_raw(blob.clone());
        let result = dispatch_packet(&pkt);
        let [
            GameEvent::GuildEmblem {
                gdid,
                version,
                bmp,
                from_map_server: true,
            },
        ] = result.as_slice()
        else {
            panic!("expected GuildEmblem, got {result:?}");
        };
        assert_eq!((*gdid, *version), (42, 7));
        assert_eq!(*bmp, blob);
    }

    #[test]
    fn dispatch_map_property_agitzone_is_siege() {
        let packetver = 20250716;
        let mut agit = PacketZcNotifyMapproperty2::new(packetver);
        agit.set_atype(3);
        let agit_events = dispatch_packet(&agit);
        let [GameEvent::MapPropertyChanged(props)] = agit_events.as_slice() else {
            panic!("expected MapPropertyChanged");
        };
        assert!(props.is_siege());

        let mut normal = PacketZcNotifyMapproperty2::new(packetver);
        normal.set_atype(0);
        let normal_events = dispatch_packet(&normal);
        let [GameEvent::MapPropertyChanged(props)] = normal_events.as_slice() else {
            panic!("expected MapPropertyChanged");
        };
        assert!(!props.is_siege());
    }

    #[test]
    fn dispatch_pvp_ranking_on_a_free_pvp_map() {
        let packetver = 20250716;

        let mut map = PacketZcNotifyMapproperty2::new(packetver);
        map.set_atype(1);
        let map_events = dispatch_packet(&map);
        let [GameEvent::MapPropertyChanged(props)] = map_events.as_slice() else {
            panic!("expected MapPropertyChanged");
        };
        assert!(props.is_pk_zone());

        let mut rank = PacketZcNotifyRanking::new(packetver);
        rank.set_aid(2000000);
        rank.set_ranking(3);
        rank.set_total(130);
        rank.fill_raw();
        assert!(matches!(
            dispatch_packet(&rank).as_slice(),
            [GameEvent::PvpRankingChanged {
                account_id: 2000000,
                ranking: 3,
                total: 130
            }]
        ));

        // Hidden subjects come through as UINT32_MAX, which the game layer
        // must not treat as a rank.
        let mut hidden = PacketZcNotifyRanking::new(packetver);
        hidden.set_ranking(-1);
        hidden.fill_raw();
        assert!(matches!(
            dispatch_packet(&hidden).as_slice(),
            [GameEvent::PvpRankingChanged { ranking: -1, .. }]
        ));
    }

    #[test]
    fn dispatch_change_members_ack_yields_position_changes() {
        let packetver = 20250716;
        let mut row = MemberPositionInfo::new(packetver);
        row.set_aid(2000000);
        row.set_gid(150001);
        row.set_position_id(3);
        row.fill_raw();
        let mut pkt = PacketZcAckReqChangeMembers::new(packetver);
        pkt.set_packet_length(4 + 12);
        pkt.set_member_info(vec![row]);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        let [GameEvent::GuildMemberPositionsChanged { entries }] = result.as_slice() else {
            panic!("expected GuildMemberPositionsChanged, got {result:?}");
        };
        assert_eq!(entries.as_slice(), &[(2000000, 150001, 3)]);
    }

    #[test]
    fn the_item_list_goes_to_the_cart_or_storage_by_its_type() {
        let packetver = 20250716;
        let mut pkt = PacketZcInventoryItemlistNormal::new(packetver);
        pkt.set_inv_type(INVTYPE_CART);
        pkt.fill_raw();
        assert!(matches!(
            dispatch_packet(&pkt).as_slice(),
            [GameEvent::CartNormalItems { .. }]
        ));
        pkt.set_inv_type(INVTYPE_STORAGE);
        assert!(matches!(
            dispatch_packet(&pkt).as_slice(),
            [GameEvent::StorageNormalItems { .. }]
        ));
    }

    #[test]
    fn dispatch_storage_open_add_remove_close() {
        let packetver = 20250716;

        let mut count_info = PacketZcNotifyStoreitemCountinfo::new(packetver);
        count_info.set_cur_count(42);
        count_info.set_max_count(600);
        count_info.fill_raw();
        assert!(matches!(
            dispatch_packet(&count_info).as_slice(),
            [GameEvent::StorageOpened { cur: 42, max: 600 }]
        ));

        let mut add = PacketZcAddItemToStoreV3::new(packetver);
        add.set_index(5);
        add.set_amount(3);
        add.set_itid(501);
        add.fill_raw();
        assert!(matches!(
            dispatch_packet(&add).as_slice(),
            [GameEvent::StorageItemAdded {
                index: 5,
                count: 3,
                item_id: 501,
                ..
            }]
        ));

        let mut del = PacketZcDeleteItemFromStore::new(packetver);
        del.set_index(5);
        del.set_count(2);
        del.fill_raw();
        assert!(matches!(
            dispatch_packet(&del).as_slice(),
            [GameEvent::StorageItemRemoved {
                index: 5,
                amount: 2
            }]
        ));

        let mut close = PacketZcCloseStore::new(packetver);
        close.fill_raw();
        assert!(matches!(
            dispatch_packet(&close).as_slice(),
            [GameEvent::StorageClosed]
        ));
    }

    #[test]
    fn dispatch_mail_window_inbox_and_open() {
        let packetver = 20120307;

        let mut win = PacketZcMailWindows::new(packetver);
        win.set_atype(0);
        win.fill_raw();
        assert!(matches!(
            dispatch_packet(&win).as_slice(),
            [GameEvent::MailWindow { open: true }]
        ));

        // 0x240: header(8) + one 73-byte entry.
        let mut inbox = vec![0x40u8, 0x02];
        inbox.extend_from_slice(&(81i16).to_le_bytes());
        inbox.extend_from_slice(&1i32.to_le_bytes()); // mail_number
        let mut entry = Vec::new();
        entry.extend_from_slice(&1001u32.to_le_bytes());
        let mut title = [0u8; 40];
        title[..5].copy_from_slice(b"Hello");
        entry.extend_from_slice(&title);
        entry.push(0); // is_open = unread
        let mut sender = [0u8; 24];
        sender[..5].copy_from_slice(b"Alice");
        entry.extend_from_slice(&sender);
        entry.extend_from_slice(&1_615_680_000u32.to_le_bytes());
        inbox.extend_from_slice(&entry);
        let pkt = PacketZcMailReqGetList::from(&inbox, packetver);
        let events = dispatch_packet(&pkt);
        let GameEvent::MailInboxReceived { entries } = &events[0] else {
            panic!("expected inbox, got {events:?}");
        };
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mail_id, 1001);
        assert_eq!(entries[0].title, "Hello");
        assert_eq!(entries[0].sender, "Alice");
        assert!(!entries[0].read);

        // 0x242: fixed header up to offset 100, then body + NUL.
        let body = b"hi there";
        let mut open = vec![0x42u8, 0x02];
        open.extend_from_slice(&((101 + body.len()) as i16).to_le_bytes());
        open.extend_from_slice(&1001i32.to_le_bytes());
        open.extend_from_slice(&title); // reuse "Hello"
        open.extend_from_slice(&sender); // reuse "Alice"
        open.extend_from_slice(&0i32.to_le_bytes()); // delete_time (unused)
        open.extend_from_slice(&5000u32.to_le_bytes()); // zeny
        open.extend_from_slice(&3i32.to_le_bytes()); // count
        open.extend_from_slice(&501u16.to_le_bytes()); // itid
        open.extend_from_slice(&0u16.to_le_bytes()); // atype
        open.push(1); // identified
        open.push(0); // damaged
        open.push(0); // refine
        open.extend_from_slice(&[0u8; 8]); // 4 card slots
        open.push(body.len() as u8);
        open.extend_from_slice(body);
        open.push(0); // NUL
        let pkt = PacketZcMailReqOpen::from(&open, packetver);
        let events = dispatch_packet(&pkt);
        let GameEvent::MailOpened { mail } = &events[0] else {
            panic!("expected opened, got {events:?}");
        };
        assert_eq!(mail.mail_id, 1001);
        assert_eq!(mail.zeny, 5000);
        assert_eq!(mail.body, "hi there");
        let item = mail.item.as_ref().expect("attachment present");
        assert_eq!(item.nameid, 501);
        assert_eq!(item.amount, 3);
        assert!(item.identified);
    }

    #[test]
    fn mail_send_packet_encodes_body_len_byte() {
        let body = "z".repeat(199);
        let raw = crate::sender::build_mail_send_packet("Bob", "Subject", &body, 20120307);

        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x0248);
        assert_eq!(u16::from_le_bytes([raw[2], raw[3]]) as usize, 69 + 199);
        assert_eq!(&raw[4..7], b"Bob");
        assert_eq!(&raw[28..35], b"Subject");
        assert_eq!(raw[68], 199); // 1-byte body length
        assert_eq!(raw.len(), 69 + 199);
    }

    #[test]
    fn disconnect_ack_maps_result_and_request_carries_quit_type() {
        let packetver = 20250716;

        let mut allowed = PacketZcAckReqDisconnect::new(packetver);
        allowed.set_result(0);
        allowed.fill_raw();
        assert!(matches!(
            dispatch_packet(&allowed).as_slice(),
            [GameEvent::DisconnectAck { allowed: true }]
        ));

        let mut refused = PacketZcAckReqDisconnect::new(packetver);
        refused.set_result(1);
        refused.fill_raw();
        assert!(matches!(
            dispatch_packet(&refused).as_slice(),
            [GameEvent::DisconnectAck { allowed: false }]
        ));

        let raw = crate::sender::build_req_disconnect_packet(packetver);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        let request = parsed
            .as_any()
            .downcast_ref::<PacketCzReqDisconnect>()
            .expect("parsed as disconnect request");
        assert_eq!(request.atype, 0);
    }

    #[test]
    fn change_cart_packet_carries_model_number() {
        let packetver = 20250716;
        let raw = crate::sender::build_change_cart_packet(3, packetver);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        let p = parsed
            .as_any()
            .downcast_ref::<PacketCzReqChangecart>()
            .expect("parsed as change-cart request");
        assert_eq!(p.num, 3);
    }

    #[test]
    fn storage_password_prompt_result_and_reply() {
        let packetver = 20250716;

        let prompt: Vec<u8> = vec![0x3a, 0x02, 0x00, 0x00];
        assert!(matches!(
            dispatch_packet(&*packets::packets_parser::parse(&prompt, packetver)).as_slice(),
            [GameEvent::StoragePasswordRequest {
                prompt: StoragePasswordPrompt::NotSetYet
            }]
        ));

        let mut result = PacketZcResultStorePassword::new(packetver);
        result.set_result(7);
        result.set_error_count(2);
        result.fill_raw();
        assert!(matches!(
            dispatch_packet(&result).as_slice(),
            [GameEvent::StoragePasswordResult {
                outcome: StoragePasswordOutcome::CheckFailed,
                error_count: 2
            }]
        ));

        let mut unknown = PacketZcResultStorePassword::new(packetver);
        unknown.set_result(99);
        unknown.fill_raw();
        assert!(dispatch_packet(&unknown).is_empty());

        // 0x023b <type>.W <password>.16B <new password>.16B
        let reply = crate::sender::build_ack_store_password_packet(false, "1234", "", packetver);
        assert_eq!(reply.len(), 36);
        assert_eq!(u16::from_le_bytes([reply[0], reply[1]]), 0x023b);
        assert_eq!(i16::from_le_bytes([reply[2], reply[3]]), 3);
        assert_eq!(&reply[4..8], b"1234");
        assert!(reply[8..].iter().all(|b| *b == 0));

        let change = crate::sender::build_ack_store_password_packet(true, "", "5678", packetver);
        assert_eq!(i16::from_le_bytes([change[2], change[3]]), 2);
        assert!(change[4..20].iter().all(|b| *b == 0));
        assert_eq!(&change[20..24], b"5678");
    }

    #[test]
    fn storage_send_packets_use_the_servers_current_ids() {
        let packetver = 20250716;

        let close = crate::sender::build_close_store_packet(packetver);
        assert_eq!(close.len(), 2);
        assert_eq!(u16::from_le_bytes([close[0], close[1]]), 0x0193);

        let deposit = crate::sender::build_move_item_body_to_store_packet(7, 42, packetver);
        assert_eq!(deposit.len(), 8);
        assert_eq!(u16::from_le_bytes([deposit[0], deposit[1]]), 0x0364);
        assert_eq!(i16::from_le_bytes([deposit[2], deposit[3]]), 7);
        assert_eq!(
            i32::from_le_bytes([deposit[4], deposit[5], deposit[6], deposit[7]]),
            42
        );

        let withdraw = crate::sender::build_move_item_store_to_body_packet(3, 5, packetver);
        assert_eq!(u16::from_le_bytes([withdraw[0], withdraw[1]]), 0x0365);
    }

    #[test]
    fn dispatch_skill_unit_entry_and_disappear_round_trip() {
        let packetver = 20250716;
        let mut entry = PacketZcSkillEntry::new(packetver);
        entry.set_aid(7001);
        entry.set_creator_aid(42);
        entry.set_x_pos(150);
        entry.set_y_pos(200);
        entry.set_job(0x83);
        entry.set_is_visible(true);
        entry.fill_raw();
        match &dispatch_packet(&entry)[..] {
            [
                GameEvent::SkillUnitEntered {
                    aid,
                    x,
                    y,
                    unit_id,
                    is_visible,
                    ..
                },
            ] => {
                assert_eq!(*aid, 7001);
                assert_eq!((*x, *y), (150, 200));
                assert_eq!(*unit_id, 0x83);
                assert!(*is_visible);
            }
            other => panic!("expected SkillUnitEntered, got {other:?}"),
        }

        // 0x011f wire layout at this packetver is `job · isVisible` (16 bytes total).
        let wire: Vec<u8> = vec![
            0x1f, 0x01, // packet id
            0x59, 0x1b, 0x00, 0x00, // aid
            0x2a, 0x00, 0x00, 0x00, // creator aid
            0x96, 0x00, // x
            0xc8, 0x00, // y
            0x90, // job (UNT_SKIDTRAP)
            0x01, // isVisible
        ];
        let parsed = PacketZcSkillEntry::from(&wire, packetver);
        assert_eq!(parsed.job, 0x90);
        assert!(parsed.is_visible);

        let mut gone = PacketZcSkillDisappear::new(packetver);
        gone.set_aid(7001);
        gone.fill_raw();
        match &dispatch_packet(&gone)[..] {
            [GameEvent::SkillUnitDisappeared { aid }] => assert_eq!(*aid, 7001),
            other => panic!("expected SkillUnitDisappeared, got {other:?}"),
        }
    }

    #[test]
    fn makable_item_list_decodes_all_entries() {
        let packetver = 20250716;
        // id(0x8d,0x01) + len(36) + two 16-byte entries {itemId.L, material[3].L}
        let mut buf: Vec<u8> = vec![0x8d, 0x01, 36, 0x00];
        buf.extend_from_slice(&501u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 12]);
        buf.extend_from_slice(&502u32.to_le_bytes());
        buf.extend_from_slice(&[0u8; 12]);
        let pkt = packets::packets_parser::parse(&buf, packetver);
        match &dispatch_packet(pkt.as_ref())[..] {
            [GameEvent::MakableItemList { item_ids }] => {
                assert_eq!(item_ids, &vec![501u32, 502u32]);
            }
            other => panic!("expected MakableItemList, got {other:?}"),
        }
    }

    #[test]
    fn production_cz_builders_round_trip() {
        let packetver = 20250716;

        let raw = crate::sender::build_req_itemidentify_packet(7, packetver);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        assert_eq!(
            parsed
                .as_any()
                .downcast_ref::<PacketCzReqItemidentify>()
                .unwrap()
                .index,
            7
        );

        // 32-bit item ids at 20250716: CZ_REQ_MAKINGARROW id.W itemId.L
        let raw = crate::sender::build_req_makingarrow_packet(1750, 20250716);
        assert_eq!(raw.len(), 6);
        assert_eq!(u32::from_le_bytes(raw[2..6].try_into().unwrap()), 1750);

        let raw = crate::sender::build_req_weaponrefine_packet(42, packetver);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        assert_eq!(
            parsed
                .as_any()
                .downcast_ref::<PacketCzReqWeaponrefine>()
                .unwrap()
                .index,
            42
        );

        // CZ_REQ_ITEMREPAIR2 at 20250716: id.W index.W itemId.L card.4L refine.B grade.B = 26 bytes
        let raw = crate::sender::build_req_itemrepair_packet(3, 1201, 4, [10, 20, 0, 0], 20250716);
        assert_eq!(raw.len(), 26);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x0b66);
        assert_eq!(i16::from_le_bytes([raw[2], raw[3]]), 3);
        assert_eq!(u32::from_le_bytes(raw[4..8].try_into().unwrap()), 1201);
        assert_eq!(u32::from_le_bytes(raw[8..12].try_into().unwrap()), 10);
        assert_eq!(u32::from_le_bytes(raw[12..16].try_into().unwrap()), 20);
        assert_eq!(raw[24], 4);
    }

    // CZ_REQMAKINGITEM (0x018e) at 20250716: id.W, itemId.L, material[3].L = 18 bytes
    #[test]
    fn making_item_builder_wire_bytes() {
        let raw = crate::sender::build_req_makingitem_packet(501, [1000, 990, 5], 20250716);
        let long = |at: usize| u32::from_le_bytes(raw[at..at + 4].try_into().unwrap());
        assert_eq!(raw.len(), 18);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x018e);
        assert_eq!((long(2), long(6), long(10), long(14)), (501, 1000, 990, 5));
    }

    #[test]
    fn vending_open_store_builder_sets_packet_length() {
        let packetver = 20250716;
        let raw = crate::sender::build_req_openstore2_packet(
            "Cheap Potions",
            &[(5, 10, 1000), (6, 1, 5000)],
            packetver,
        );
        // header(2) + len(2) + name(80) + result(1) + 2*8 = 101
        let len = u16::from_le_bytes([raw[2], raw[3]]);
        assert_eq!(len, 101);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        let p = parsed
            .as_any()
            .downcast_ref::<PacketCzReqOpenstore2>()
            .unwrap();
        assert_eq!(p.store_list.len(), 2);
        assert_eq!(p.store_list[0].price, 1000);
    }

    #[test]
    fn vending_cancel_builder_clears_result_with_empty_list() {
        let packetver = 20250716;
        let raw = crate::sender::build_req_cancel_openstore_packet(packetver);
        let len = u16::from_le_bytes([raw[2], raw[3]]);
        assert_eq!(len, 85);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        let p = parsed
            .as_any()
            .downcast_ref::<PacketCzReqOpenstore2>()
            .unwrap();
        assert!(!p.result);
        assert!(p.store_list.is_empty());
    }

    #[test]
    fn dispatch_notify_time_returns_server_tick() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyTime::new(packetver);
        pkt.set_time(42000);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::ServerTick { server_tick, .. } => assert_eq!(*server_tick, 42000),
            other => panic!("expected ServerTick, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_refuse_login_returns_error_code() {
        let packetver = 20250716;
        let mut pkt = PacketAcRefuseLogin::new(packetver);
        pkt.set_error_code(1);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::LoginRefused { error_code } => assert_eq!(*error_code, 1),
            other => panic!("expected LoginRefused, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_char_server_refuse_enter_returns_error_code() {
        let packetver = 20250716;
        let mut pkt = PacketHcRefuseEnter::new(packetver);
        pkt.set_error_code(0);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::CharServerConnectRefused { error_code } => assert_eq!(*error_code, 0),
            other => panic!("expected CharServerConnectRefused, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_refuse_login_r2_returns_error_code() {
        let packetver = 20250716;
        let mut pkt = PacketAcRefuseLoginR2::new(packetver);
        pkt.set_error_code(6);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::LoginRefused { error_code } => assert_eq!(*error_code, 6),
            other => panic!("expected LoginRefused, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_mapmove_returns_map_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcNpcackMapmove::new(packetver);
        let mut map_name = [0 as char; 16];
        for (i, c) in "prt_fild08.gat".chars().enumerate() {
            map_name[i] = c;
        }
        pkt.set_map_name(map_name);
        pkt.set_x_pos(150);
        pkt.set_y_pos(200);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::MapChanged { map_name, x, y } => {
                assert_eq!(map_name, "prt_fild08.gat");
                assert_eq!(*x, 150);
                assert_eq!(*y, 200);
            }
            other => panic!("expected MapChanged, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_notify_playermove_decodes_positions() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyPlayermove::new(packetver);
        pkt.set_move_start_time(5000);
        let x1: u16 = 100;
        let y1: u16 = 200;
        let x2: u16 = 110;
        let y2: u16 = 210;
        let b0 = (x1 >> 2) as u8;
        let b1 = ((x1 << 6) as u8) | ((y1 >> 4) as u8);
        let b2 = ((y1 << 4) as u8) | ((x2 >> 6) as u8);
        let b3 = ((x2 << 2) as u8) | ((y2 >> 8) as u8);
        let b4 = y2 as u8;
        let b5 = 0u8;
        pkt.set_move_data([b0, b1, b2, b3, b4, b5]);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::PlayerMoved {
                start_x,
                start_y,
                dest_x,
                dest_y,
                start_time,
            } => {
                assert_eq!((*start_x, *start_y), (100, 200));
                assert_eq!((*dest_x, *dest_y), (110, 210));
                assert_eq!(*start_time, 5000);
            }
            other => panic!("expected PlayerMoved, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_accept_enter_decodes_position() {
        let packetver = 20250716;
        let mut pkt = PacketZcAcceptEnter2::new(packetver);
        pkt.set_start_time(1000);
        let encoded = crate::helpers::encode_pos(100, 200, 3);
        pkt.set_pos_dir(encoded);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::MapEntered { x, y, dir, tick } => {
                assert_eq!((*x, *y, *dir, *tick), (100, 200, 3, 1000));
            }
            other => panic!("expected MapEntered, got {other:?}"),
        }
    }

    #[test]
    fn units_carry_their_clothes_dye() {
        let mut stand = PacketZcNotifyStandentry6::new(20250716);
        stand.set_aid(7);
        stand.set_headpalette(2);
        stand.set_bodypalette(5);
        stand.fill_raw_with_packetver(Some(20250716));
        assert!(matches!(
            dispatch_packet(&stand)[0],
            GameEvent::EntitySpawned {
                hair_color: 2,
                cloth_color: 5,
                ..
            }
        ));
    }

    #[test]
    fn units_carry_their_garment() {
        let mut unit = PacketZcNotifyStandentry6::new(20250716);
        unit.set_aid(7);
        unit.set_robe(3);
        unit.fill_raw_with_packetver(Some(20250716));
        let events = dispatch_packet(&unit);
        assert!(
            matches!(events[0], GameEvent::EntitySpawned { robe: 3, .. }),
            "{:?}",
            events[0]
        );
    }

    #[test]
    fn dispatch_spawn_unit_returns_standing_entity_spawn() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyNewentry8::new(packetver);
        pkt.set_aid(123456);
        pkt.set_job(1002);
        pkt.set_pos_dir(crate::helpers::encode_pos(100, 200, 3));
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntitySpawned {
                gid,
                job,
                x,
                y,
                direction,
                posture,
                is_new_entry,
                ..
            } => {
                assert_eq!(*gid, 123456);
                assert_eq!(*job, 1002);
                assert_eq!((*x, *y, *direction), (100, 200, 3));
                assert_eq!(*posture, 0, "a freshly spawned entity is standing");
                assert!(*is_new_entry, "newentry marks a fresh appearance");
            }
            other => panic!("expected EntitySpawned, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_standentry_is_not_a_new_entry() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyStandentry6::new(packetver);
        pkt.set_aid(123456);
        pkt.set_job(1002);
        pkt.set_pos_dir(crate::helpers::encode_pos(100, 200, 3));
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        match &result[0] {
            GameEvent::EntitySpawned { is_new_entry, .. } => {
                assert!(
                    !is_new_entry,
                    "an already-present entity entering view is not a new entry"
                );
            }
            other => panic!("expected EntitySpawned, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_vanish_returns_entity_vanished() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyVanish::new(packetver);
        pkt.set_gid(42);
        pkt.set_atype(0);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityVanished { gid, vanish_type } => {
                assert_eq!(*gid, 42);
                assert!(matches!(vanish_type, VanishType::OutOfSight));
            }
            other => panic!("expected EntityVanished, got {other:?}"),
        }
    }

    #[test]
    fn a_walking_unit_returns_entity_moved() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyMoveentry8::new(packetver);
        pkt.set_aid(99);
        pkt.set_move_start_time(7000);
        let x1: u16 = 50;
        let y1: u16 = 60;
        let x2: u16 = 55;
        let y2: u16 = 65;
        let b0 = (x1 >> 2) as u8;
        let b1 = ((x1 << 6) as u8) | ((y1 >> 4) as u8);
        let b2 = ((y1 << 4) as u8) | ((x2 >> 6) as u8);
        let b3 = ((x2 << 2) as u8) | ((y2 >> 8) as u8);
        let b4 = y2 as u8;
        let b5 = 0u8;
        pkt.set_move_data([b0, b1, b2, b3, b4, b5]);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        match result
            .iter()
            .find(|e| matches!(e, GameEvent::EntityMoved { .. }))
            .unwrap()
        {
            GameEvent::EntityMoved {
                gid,
                start_x,
                start_y,
                dest_x,
                dest_y,
                start_time,
            } => {
                assert_eq!(*gid, 99);
                assert_eq!((*start_x, *start_y), (50, 60));
                assert_eq!((*dest_x, *dest_y), (55, 65));
                assert_eq!(*start_time, 7000);
            }
            other => panic!("expected EntityMoved, got {other:?}"),
        }
    }

    #[test]
    fn autospell_list_reads_every_skill_offered() {
        let packetver = 20250716;
        // 0x0afb: header(2) + length(2) + one int32 per skill offered
        let mut wire = vec![0xfbu8, 0x0a, 20, 0];
        for skill_id in [11i32, 14, 19, 20] {
            wire.extend_from_slice(&skill_id.to_le_bytes());
        }
        let pkt = PacketZcAutospelllist2::from(&wire, packetver);
        let events = dispatch_packet(&pkt);
        match &events[0] {
            GameEvent::AutoSpellList { skills } => assert_eq!(
                skills,
                &vec![
                    SkillEnum::MgNapalmbeat,
                    SkillEnum::MgColdbolt,
                    SkillEnum::MgFirebolt,
                    SkillEnum::MgLightningbolt,
                ]
            ),
            other => panic!("expected AutoSpellList, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_stopmove_returns_entity_stop_move() {
        let packetver = 20250716;
        let mut pkt = PacketZcStopmove::new(packetver);
        pkt.set_aid(77);
        pkt.set_x_pos(120);
        pkt.set_y_pos(130);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityStopMove { gid, x, y } => {
                assert_eq!(*gid, 77);
                assert_eq!((*x, *y), (120, 130));
            }
            other => panic!("expected EntityStopMove, got {other:?}"),
        }
    }

    fn skill_damage(skill_id: u16) -> Vec<GameEvent> {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifySkill2::new(packetver);
        pkt.set_skid(skill_id);
        pkt.set_aid(50);
        pkt.set_target_id(99);
        pkt.set_damage(42);
        pkt.fill_raw();
        dispatch_packet(&pkt)
    }

    #[test]
    fn unknown_skill_damage_is_still_shown_as_a_hit() {
        assert!(matches!(
            skill_damage(9999)[..], // no skill in rAthena's db
            [GameEvent::EntityAction {
                gid: 50,
                target_gid: 99,
                damage: 42,
                ..
            }]
        ));
    }

    #[test]
    fn third_and_fourth_job_skill_damage_names_its_skill() {
        for (id, skill) in [
            (2002, SkillEnum::RkSonicwave),
            (5208, SkillEnum::DkHackandslasher),
        ] {
            assert!(matches!(
                skill_damage(id)[..],
                [GameEvent::SkillDamage {
                    skill: s,
                    src_gid: 50,
                    target_gid: 99,
                    damage: 42,
                    ..
                }] if s == skill
            ));
        }
    }

    #[test]
    fn dispatch_notify_act_returns_entity_action() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyAct3::new(packetver);
        pkt.set_gid(50);
        pkt.set_target_gid(99);
        pkt.set_action(8);
        pkt.set_damage(42);
        pkt.set_attack_mt(500);
        pkt.set_attacked_mt(300);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityAction {
                gid,
                target_gid,
                action,
                damage,
                attack_mt,
                attacked_mt,
                ..
            } => {
                assert_eq!(*gid, 50);
                assert_eq!(*target_gid, 99);
                assert_eq!(*action, ActionType::AttackMultiple);
                assert_eq!(*damage, 42);
                assert_eq!(*attack_mt, 500);
                assert_eq!(*attacked_mt, 300);
            }
            other => panic!("expected EntityAction, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_change_direction_returns_entity_direction_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcChangeDirection::new(packetver);
        pkt.set_aid(60);
        pkt.set_head_dir(1);
        pkt.set_dir(3);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityDirectionChanged { gid, head_dir, dir } => {
                assert_eq!(*gid, 60);
                assert_eq!(*head_dir, 1);
                assert_eq!(*dir, 3);
            }
            other => panic!("expected EntityDirectionChanged, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_notify_chat_returns_chat_message() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyChat::new(packetver);
        pkt.set_gid(42);
        pkt.set_msg("Player : Hello".to_string());
        pkt.set_msg_raw("Player : Hello".as_bytes().to_vec());
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::ChatMessage { gid, message } => {
                assert_eq!(*gid, 42);
                assert_eq!(message, "Player : Hello");
            }
            other => panic!("expected ChatMessage, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_notify_playerchat_returns_own_chat() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyPlayerchat::new(packetver);
        pkt.set_msg("Me : Hi there".to_string());
        pkt.set_msg_raw("Me : Hi there".as_bytes().to_vec());
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::OwnChatMessage { message } => {
                assert_eq!(message, "Me : Hi there");
            }
            other => panic!("expected OwnChatMessage, got {other:?}"),
        }
    }

    #[test]
    fn build_action_request_packet_has_correct_format() {
        let raw = crate::sender::build_action_request_packet(0, 2, 20250716);
        assert_eq!(raw.len(), 7);
        assert_eq!(raw[0], 0x37);
        assert_eq!(raw[1], 0x04);
        assert_eq!(&raw[2..6], &[0, 0, 0, 0]);
        assert_eq!(raw[6], 2);
    }

    #[test]
    fn build_chat_packet_has_correct_format() {
        let raw = crate::sender::build_chat_packet("Player : hello", 20250716);
        assert_eq!(raw.len(), 19);
        assert_eq!(raw[0], 0xF3);
        assert_eq!(raw[1], 0x00);
        let pkt_len = i16::from_le_bytes([raw[2], raw[3]]);
        assert_eq!(pkt_len, 19);
        assert_eq!(&raw[4..], b"Player : hello\0");

        for (packetver, id) in [
            (20040705u32, [0x8c, 0x00]),
            (20040726, [0xf3, 0x00]),
            (20040906, [0x9f, 0x00]),
            (20041129, [0x85, 0x00]),
            (20050110, [0xf3, 0x00]),
            (20080910, [0xf3, 0x00]),
        ] {
            let raw = crate::sender::build_chat_packet("hi", packetver);
            assert_eq!([raw[0], raw[1]], id, "wrong chat id at {packetver}");
            assert_eq!(i16::from_le_bytes([raw[2], raw[3]]) as usize, raw.len());
        }
    }

    #[test]
    fn build_emotion_packet_has_correct_id_and_type() {
        let raw = crate::sender::build_emotion_packet(23, 20250716);
        assert_eq!(raw.len(), 3);
        assert_eq!(raw[0], 0xBF);
        assert_eq!(raw[1], 0x00);
        assert_eq!(raw[2], 23);
    }

    #[test]
    fn build_request_time_packet_contains_client_time() {
        let raw = crate::sender::build_request_time_packet(12345, 20250716);
        assert_eq!(raw.len(), 6);
        let client_time = u32::from_le_bytes([raw[2], raw[3], raw[4], raw[5]]);
        assert_eq!(client_time, 12345);
    }

    #[test]
    fn build_char_ping_packet_contains_account_id() {
        let raw = crate::sender::build_char_ping_packet(200_000, 20250716);
        assert_eq!(raw.len(), 6);
        let aid = u32::from_le_bytes([raw[2], raw[3], raw[4], raw[5]]);
        assert_eq!(aid, 200_000);
    }

    #[test]
    fn build_reqname_packet_has_correct_format() {
        let raw = crate::sender::build_reqname_packet(12345, 20250716);
        assert_eq!(raw.len(), 6);
        let aid = u32::from_le_bytes([raw[2], raw[3], raw[4], raw[5]]);
        assert_eq!(aid, 12345);
    }

    #[test]
    fn a_name_reply_for_a_monster_or_npc_returns_entity_name_received() {
        let packetver = 20250716;
        let mut pkt = PacketZcAckReqnameallNpc::new(packetver);
        pkt.set_gid(42);
        let mut name = ['\0'; 24];
        for (i, c) in "Poring".chars().enumerate() {
            name[i] = c;
        }
        pkt.set_name(name);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityNameReceived { gid, name } => {
                assert_eq!(*gid, 42);
                assert_eq!(name, "Poring");
            }
            other => panic!("expected EntityNameReceived, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_ack_reqname_bygid_returns_char_and_entity_name() {
        let packetver = 20250716;
        let mut pkt = PacketZcAckReqnameBygid::new(packetver);
        pkt.set_gid(77);
        let mut name = ['\0'; 24];
        for (i, c) in "Lidia".chars().enumerate() {
            name[i] = c;
        }
        pkt.set_cname(name);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 2);
        match &result[0] {
            GameEvent::CharNameReceived { char_id, name } => {
                assert_eq!(*char_id, 77);
                assert_eq!(name, "Lidia");
            }
            other => panic!("expected CharNameReceived, got {other:?}"),
        }
        assert!(matches!(
            &result[1],
            GameEvent::EntityNameReceived { gid: 77, .. }
        ));
    }

    #[test]
    fn dispatch_ack_reqnameall_returns_entity_names_received() {
        let packetver = 20250716;
        let mut pkt = PacketZcAckReqnameall2::new(packetver);
        pkt.set_gid(42);
        let fill = |s: &str| {
            let mut buf = ['\0'; 24];
            for (i, c) in s.chars().enumerate() {
                buf[i] = c;
            }
            buf
        };
        pkt.set_name(fill("Alice"));
        pkt.set_party_name(fill("HP: 100/200"));
        pkt.set_guild_name(fill("Knights"));
        pkt.set_position_name(fill("Leader"));
        pkt.set_title_id(1034);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityNamesReceived {
                gid,
                name,
                party_name,
                guild_name,
                position_name,
                title_id,
            } => {
                assert_eq!(*gid, 42);
                assert_eq!(name, "Alice");
                assert_eq!(party_name, "HP: 100/200");
                assert_eq!(guild_name, "Knights");
                assert_eq!(position_name, "Leader");
                assert_eq!(*title_id, 1034);
            }
            other => panic!("expected EntityNamesReceived, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_par_change_returns_parameter_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcParChange::new(packetver);
        pkt.set_var_id(5);
        pkt.set_count(441);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::ParameterChanged { var_id, value } => {
                assert_eq!(*var_id, 5);
                assert_eq!(*value, 441);
            }
            other => panic!("expected ParameterChanged, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_stat_up_ack_and_cost_update_character() {
        let packetver = 20250716;
        let mut character = nostalre_game::character::Character::new();

        let mut initial = PacketZcStatus::new(packetver);
        initial.set_point(48);
        initial.set_str(110);
        initial.set_standard_str(19);
        initial.set_luk(1);
        initial.set_standard_luk(2);
        initial.fill_raw();

        let mut ack = PacketZcStatusChangeAck::new(packetver);
        ack.set_status_id(StatusTypes::Str.value() as u16);
        ack.set_result(true);
        ack.set_value(11);
        ack.fill_raw();

        let mut cost = PacketZcStatusChange::new(packetver);
        cost.set_status_id(StatusTypes::StrNextLevelIncreaseCost.value() as u16);
        cost.set_value(3);
        cost.fill_raw();

        for event in dispatch_packet(&initial) {
            match event {
                GameEvent::ParameterChanged { var_id, value } => {
                    character.apply_parameter_changed(var_id, value);
                }
                other => panic!("expected ParameterChanged, got {other:?}"),
            }
        }
        assert_eq!(character.status_point, 48);
        assert_eq!(character.str_cost, 19);
        assert_eq!(character.luk_cost, 2);

        for event in dispatch_packet(&ack)
            .into_iter()
            .chain(dispatch_packet(&cost))
        {
            match event {
                GameEvent::ParameterChanged { var_id, value } => {
                    character.apply_parameter_changed(var_id, value);
                }
                other => panic!("expected ParameterChanged, got {other:?}"),
            }
        }
        assert_eq!(character.str, 11);
        assert_eq!(character.str_cost, 3);

        let mut rejected = PacketZcStatusChangeAck::new(packetver);
        rejected.set_status_id(StatusTypes::Str.value() as u16);
        rejected.set_result(false);
        rejected.set_value(0);
        rejected.fill_raw();
        assert!(dispatch_packet(&rejected).is_empty());
        assert_eq!(character.str, 11);
    }

    #[test]
    fn dispatch_sprite_change2_returns_entity_sprite_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcSpriteChange2::new(packetver);
        pkt.set_gid(150000);
        pkt.set_atype(2);
        pkt.set_value_32(1);
        pkt.set_value2_32(0);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntitySpriteChanged {
                gid,
                sprite_type,
                value,
                value2,
            } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*sprite_type, 2);
                assert_eq!(*value, 1);
                assert_eq!(*value2, 0);
            }
            other => panic!("expected EntitySpriteChanged, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_req_baby_returns_adoption_requested() {
        let packetver = 20250716;
        let mut pkt = PacketZcReqBaby::new(packetver);
        pkt.set_aid(111);
        pkt.set_gid(222);
        let mut name = ['\0'; 24];
        for (i, c) in "Daddy".chars().enumerate() {
            name[i] = c;
        }
        pkt.set_name(name);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::AdoptionRequested {
                father_aid,
                mother_aid,
                name,
            } => {
                assert_eq!(*father_aid, 111);
                assert_eq!(*mother_aid, 222);
                assert_eq!(name, "Daddy");
            }
            other => panic!("expected AdoptionRequested, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_npcsprite_change_returns_class_sprite_change() {
        let packetver = 20250716;
        let mut pkt = PacketZcNpcspriteChange::new(packetver);
        pkt.set_gid(150000);
        pkt.set_atype(0);
        pkt.set_value(1160);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntitySpriteChanged {
                gid,
                sprite_type,
                value,
                value2,
            } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*sprite_type, 0);
                assert_eq!(*value, 1160);
                assert_eq!(*value2, 0);
            }
            other => panic!("expected EntitySpriteChanged, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_ack_itemrefining_returns_item_refining_result() {
        let packetver = 20250716;
        let mut pkt = PacketZcAckItemrefining::new(packetver);
        pkt.set_result(1);
        pkt.set_item_index(7);
        pkt.set_refining_level(4);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::ItemRefiningResult {
                index,
                refine,
                result,
            } => {
                assert_eq!(*index, 7);
                assert_eq!(*refine, 4);
                assert_eq!(*result, 1);
            }
            other => panic!("expected ItemRefiningResult, got {other:?}"),
        }
    }

    #[test]
    fn the_refine_window_opens_lists_its_materials_and_announces() {
        let packetver = 20250716;
        let open = packets::packets_parser::parse(&[0xa0, 0x0a], packetver);
        assert!(matches!(
            dispatch_packet(open.as_ref()).as_slice(),
            [GameEvent::RefineWindowOpened]
        ));

        // 0x0aa2: index 5, 1 blessing, Phracon (1010) at 100% for 50z and Oridecon (984) at 90% for 1000z
        let mut list = vec![0xa2, 0x0a, 0, 0, 5, 0, 1];
        for (item, chance, zeny) in [(1010u32, 100u8, 50u32), (984, 90, 1000)] {
            list.extend_from_slice(&item.to_le_bytes());
            list.push(chance);
            list.extend_from_slice(&zeny.to_le_bytes());
        }
        let len = list.len() as u16;
        list[2..4].copy_from_slice(&len.to_le_bytes());
        let parsed = packets::packets_parser::parse(&list, packetver);
        match dispatch_packet(parsed.as_ref()).as_slice() {
            [
                GameEvent::RefineMaterials {
                    index,
                    blessings,
                    materials,
                },
            ] => {
                assert_eq!((*index, *blessings), (5, 1));
                assert_eq!(
                    materials,
                    &[
                        nostalre_game::event::RefineMaterial {
                            item_id: 1010,
                            chance: 100,
                            zeny: 50
                        },
                        nostalre_game::event::RefineMaterial {
                            item_id: 984,
                            chance: 90,
                            zeny: 1000
                        },
                    ]
                );
            }
            other => panic!("expected RefineMaterials, got {other:?}"),
        }

        // 0x0ada: name, item, level, status
        let mut news = vec![0xda, 0x0a];
        let mut name = [0u8; 24];
        name[..5].copy_from_slice(b"Spike");
        news.extend_from_slice(&name);
        news.extend_from_slice(&1201u32.to_le_bytes());
        news.extend_from_slice(&[10, 1]);
        let parsed = packets::packets_parser::parse(&news, packetver);
        match dispatch_packet(parsed.as_ref()).as_slice() {
            [
                GameEvent::RefineAnnounced {
                    name,
                    item_id,
                    refine,
                    success,
                },
            ] => assert_eq!(
                (name.as_str(), *item_id, *refine, *success),
                ("Spike", 1201, 10, true)
            ),
            other => panic!("expected RefineAnnounced, got {other:?}"),
        }
    }

    #[test]
    fn the_stylist_opens_by_its_ui_type_and_answers_with_a_flag() {
        let packetver = 20250716;
        let events = |bytes: &[u8]| {
            let parsed = packets::packets_parser::parse(bytes, packetver);
            dispatch_packet(parsed.as_ref())
        };
        assert!(matches!(
            events(&[0xe2, 0x0a, 1, 0, 0, 0, 0]).as_slice(),
            [GameEvent::StylistOpened]
        ));
        assert!(
            events(&[0xe2, 0x0a, 0, 0, 0, 0, 0]).is_empty(),
            "the bank is not ours yet"
        );
        assert!(matches!(
            events(&[0x47, 0x0a, 0]).as_slice(),
            [GameEvent::StylistResult { success: true }]
        ));
        assert!(matches!(
            events(&[0x47, 0x0a, 1]).as_slice(),
            [GameEvent::StylistResult { success: false }]
        ));
    }

    #[test]
    fn barter_lists_carry_their_costs() {
        let packetver = 20250716;
        let framed = |mut bytes: Vec<u8>| {
            let len = bytes.len() as u16;
            bytes[2..4].copy_from_slice(&len.to_le_bytes());
            packets::packets_parser::parse(&bytes, packetver)
        };
        let lists = |parsed: Box<dyn Packet>| match dispatch_packet(parsed.as_ref()).as_slice() {
            [GameEvent::NpcBarterList { extended, items }] => (*extended, items.clone()),
            other => panic!("expected NpcBarterList, got {other:?}"),
        };

        // 0x0b78: Oridecon for 5 Rough Oridecon, no stock kept (-1), shop index 0
        let mut simple = vec![0x78, 0x0b, 0, 0];
        simple.extend_from_slice(&984u32.to_le_bytes());
        simple.push(3);
        simple.extend_from_slice(&(-1i32).to_le_bytes());
        simple.extend_from_slice(&756u32.to_le_bytes());
        simple.extend_from_slice(&5i32.to_le_bytes());
        simple.extend_from_slice(&20i32.to_le_bytes());
        simple.extend_from_slice(&0i32.to_le_bytes());
        simple.extend_from_slice(&0u16.to_le_bytes());
        simple.extend_from_slice(&0u32.to_le_bytes());
        let (extended, items) = lists(framed(simple));
        assert!(!extended);
        assert_eq!(
            items,
            [BarterItem {
                item_id: 984,
                item_type: 3,
                shop_index: 0,
                stock: None,
                zeny: 0,
                refine: 0,
                costs: vec![BarterCost {
                    item_id: 756,
                    refine: 0,
                    amount: 5
                }],
            }]
        );

        // 0x0b79: Bradium (7 left, 90000z) for an Oridecon and a +4 Blade; then Carnium with no costs
        let head =
            |bytes: &mut Vec<u8>, item: u32, stock: i32, index: i32, zeny: u32, costs: u32| {
                bytes.extend_from_slice(&item.to_le_bytes());
                bytes.extend_from_slice(&3u16.to_le_bytes());
                bytes.extend_from_slice(&stock.to_le_bytes());
                bytes.extend_from_slice(&10i32.to_le_bytes());
                bytes.extend_from_slice(&index.to_le_bytes());
                bytes.extend_from_slice(&zeny.to_le_bytes());
                bytes.extend_from_slice(&0u16.to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
                bytes.extend_from_slice(&costs.to_le_bytes());
                // the item's own refine
                bytes.extend_from_slice(&0u32.to_le_bytes());
            };
        let cost = |bytes: &mut Vec<u8>, item: u32, refine: u16, amount: i32| {
            bytes.extend_from_slice(&item.to_le_bytes());
            bytes.extend_from_slice(&refine.to_le_bytes());
            bytes.extend_from_slice(&amount.to_le_bytes());
            bytes.extend_from_slice(&3u16.to_le_bytes());
        };
        let mut expanded = vec![0x79, 0x0b, 0, 0];
        expanded.extend_from_slice(&2i32.to_le_bytes());
        head(&mut expanded, 6224, 7, 2, 90_000, 2);
        cost(&mut expanded, 984, 0, 1);
        cost(&mut expanded, 1108, 4, 1);
        head(&mut expanded, 6223, -1, 3, 0, 0);
        let (extended, items) = lists(framed(expanded));
        assert!(extended);
        assert_eq!(items.len(), 2);
        assert_eq!(
            (
                items[0].item_id,
                items[0].stock,
                items[0].shop_index,
                items[0].zeny
            ),
            (6224, Some(7), 2, 90_000)
        );
        assert_eq!(
            items[0].costs,
            [
                BarterCost {
                    item_id: 984,
                    refine: 0,
                    amount: 1
                },
                BarterCost {
                    item_id: 1108,
                    refine: 4,
                    amount: 1
                },
            ]
        );
        assert_eq!((items[1].item_id, items[1].stock), (6223, None));
        assert!(items[1].costs.is_empty());
    }

    /// 0x0b79's item head carries the refine the item comes with, between the cost count and the costs: a +7 Blade
    /// for two Oridecon, its cost read after the refine
    #[test]
    fn an_expanded_barter_item_comes_with_its_refine() {
        let packetver = 20250716;
        let mut bytes = vec![0x79, 0x0b, 0, 0];
        bytes.extend_from_slice(&1i32.to_le_bytes());
        bytes.extend_from_slice(&1108u32.to_le_bytes());
        bytes.extend_from_slice(&5u16.to_le_bytes());
        bytes.extend_from_slice(&(-1i32).to_le_bytes());
        bytes.extend_from_slice(&600i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&2u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&7u32.to_le_bytes());
        bytes.extend_from_slice(&984u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&2i32.to_le_bytes());
        bytes.extend_from_slice(&3u16.to_le_bytes());
        let len = bytes.len() as u16;
        bytes[2..4].copy_from_slice(&len.to_le_bytes());
        let parsed = packets::packets_parser::parse(&bytes, packetver);
        match dispatch_packet(parsed.as_ref()).as_slice() {
            [GameEvent::NpcBarterList { items, .. }] => {
                assert_eq!((items[0].item_id, items[0].refine), (1108, 7));
                assert_eq!(
                    items[0].costs,
                    [BarterCost {
                        item_id: 984,
                        refine: 0,
                        amount: 2
                    }]
                );
            }
            other => panic!("expected NpcBarterList, got {other:?}"),
        }
    }

    /// A script's dialog placement and a map's popup (from 20220504 and 20221005), each field where rAthena puts it
    #[test]
    fn dialog_placements_and_popups_carry_their_values() {
        let pv = 20250716;
        let event = |id: u16, a: i32, b: Option<i32>| {
            let mut bytes = id.to_le_bytes().to_vec();
            bytes.extend_from_slice(&a.to_le_bytes());
            bytes.extend(b.map(i32::to_le_bytes).into_iter().flatten());
            dispatch_packet(packets::packets_parser::parse(&bytes, pv).as_ref())
        };
        // 0x0ba2 is height, then width
        assert!(matches!(
            event(0x0ba2, 300, Some(500)).as_slice(),
            [GameEvent::NpcDialogPlacement(DialogPlacement::Size {
                width: 500,
                height: 300
            })]
        ));
        assert!(matches!(
            event(0x0ba3, 10, Some(20)).as_slice(),
            [GameEvent::NpcDialogPlacement(DialogPlacement::Pixels {
                x: 10,
                y: 20
            })]
        ));
        assert!(matches!(
            event(0x0bb5, 50, Some(75)).as_slice(),
            [GameEvent::NpcDialogPlacement(DialogPlacement::Percent {
                x: 50,
                y: 75
            })]
        ));
        assert!(matches!(
            event(0x0bbe, 3, None).as_slice(),
            [GameEvent::SpecialPopup { id: 3 }]
        ));
    }

    /// 0x07fd as rAthena sends it: a box item with its refine field, a monster's drop and an NPC's
    /// gift (both with the byte rAthena's length counts past their field); an unknown type makes no event
    #[test]
    fn a_special_item_is_announced_from_a_box_a_monster_or_an_npc() {
        let name = |text: &str| {
            let mut field = vec![24];
            let mut bytes = text.as_bytes().to_vec();
            bytes.resize(24, 0);
            field.extend(bytes);
            field
        };
        let packet = |kind: u8, tail: Vec<u8>| {
            let mut bytes = vec![0xfd, 0x07, 0, 0, kind];
            bytes.extend_from_slice(&1599u32.to_le_bytes());
            bytes.extend(name("Spike"));
            bytes.extend(tail);
            let len = bytes.len() as u16;
            bytes[2..4].copy_from_slice(&len.to_le_bytes());
            bytes
        };
        let announced = |bytes: Vec<u8>| {
            let parsed = packets::packets_parser::parse(&bytes, 20250716);
            match dispatch_packet(parsed.as_ref()).as_slice() {
                [
                    GameEvent::SpecialItemObtained {
                        player,
                        item_id: 1599,
                        source,
                        refine,
                    },
                ] => Some((player.clone(), source.clone(), *refine)),
                [] => None,
                other => panic!("{other:?}"),
            }
        };
        let mut boxed = vec![4];
        boxed.extend_from_slice(&616u32.to_le_bytes());
        boxed.push(4);
        boxed.extend_from_slice(&7u32.to_le_bytes());
        assert_eq!(
            announced(packet(0, boxed)),
            Some(("Spike".into(), SpecialItemSource::Box(616), 7))
        );
        let mut monster = name("Baphomet");
        monster.push(0xcc);
        assert_eq!(
            announced(packet(1, monster)),
            Some((
                "Spike".into(),
                SpecialItemSource::Monster("Baphomet".into()),
                0
            ))
        );
        assert_eq!(
            announced(packet(2, vec![0, 0])),
            Some(("Spike".into(), SpecialItemSource::Npc, 0))
        );
        assert_eq!(announced(packet(3, vec![0, 0])), None);
    }

    /// An NPC's music: 0x0b8c's play type and open name; a play type other than rAthena's 0 makes no event
    #[test]
    fn an_npc_plays_its_music() {
        for (play_type, music) in [(0u8, true), (1, false)] {
            let mut new = vec![0x8c, 0x0b, 8, 0, play_type];
            new.extend_from_slice(b"07\0");
            let parsed = packets::packets_parser::parse(&new, 20250716);
            let events = dispatch_packet(parsed.as_ref());
            assert_eq!(
                matches!(events.as_slice(), [GameEvent::NpcMusic { name }] if name == "07"),
                music,
                "play type {play_type}"
            );
        }
    }

    /// 0x0bae answers an unequip-all: 0 done, 1 refused (rAthena, when the player cannot act), 2 refused too in kRO's
    /// exe; any other value makes no event
    #[test]
    fn an_unequip_all_is_answered_done_or_refused() {
        for (result, refused) in [
            (0, Some(false)),
            (1, Some(true)),
            (2, Some(true)),
            (3, None),
        ] {
            let parsed = packets::packets_parser::parse(&[0xae, 0x0b, result], 20250716);
            let events = dispatch_packet(parsed.as_ref());
            let got = match events.as_slice() {
                [GameEvent::UnequipAllAnswered { refused }] => Some(*refused),
                [] => None,
                other => panic!("result {result}: {other:?}"),
            };
            assert_eq!(got, refused, "result {result}");
        }
    }

    /// The hand-read offsets of 0x0b79 (the crate keeps its items as raw bytes) are rAthena's
    #[test]
    fn expanded_barter_offsets_are_rathenas() {
        let layout = crate::rathena_table::table().layout(0x0b79, true).unwrap();
        // the items start at 8, after the id, length and count
        let at = |name: &str| layout.iter().find(|f| f.name == name).map(|f| f.offset - 8);
        assert_eq!(at("items[].currency_count"), Some(28));
        assert_eq!(at("items[].currencies[].nameid"), Some(BARTER_ITEM_HEAD));
        assert_eq!(at("items[].refine_level"), Some(BARTER_ITEM_REFINE_AT));
    }

    #[test]
    fn a_market_opens_with_its_stock_and_answers_a_purchase() {
        let packetver = 20250716;
        // 0x0b7a: Phracon, etc, 200z, 999999 left, weight 10, no location; then Arrow with no limit (-1, sent
        // as 0xffffffff in the unsigned qty: the Extended Ammunition markets)
        let mut list = vec![0x7a, 0x0b, 0, 0];
        for (item, item_type, stock) in [(1010u32, 3u8, 999_999u32), (1750, 10, u32::MAX)] {
            list.extend_from_slice(&item.to_le_bytes());
            list.push(item_type);
            list.extend_from_slice(&200u32.to_le_bytes());
            list.extend_from_slice(&stock.to_le_bytes());
            list.extend_from_slice(&10u16.to_le_bytes());
            list.extend_from_slice(&0u32.to_le_bytes());
        }
        let len = list.len() as u16;
        list[2..4].copy_from_slice(&len.to_le_bytes());
        let parsed = packets::packets_parser::parse(&list, packetver);
        match dispatch_packet(parsed.as_ref()).as_slice() {
            [GameEvent::NpcMarketList { items }] => assert_eq!(
                items,
                &[
                    nostalre_game::event::MarketItem {
                        item_id: 1010,
                        item_type: 3,
                        price: 200,
                        stock: Some(999_999)
                    },
                    nostalre_game::event::MarketItem {
                        item_id: 1750,
                        item_type: 10,
                        price: 200,
                        stock: None
                    }
                ]
            ),
            other => panic!("expected NpcMarketList, got {other:?}"),
        }

        // 0x0b4e: 0 is success from 20190807, 0xffff failure; 3 Phracon at 200z
        for (result, success) in [(0u16, true), (0xffff, false)] {
            let mut answer = vec![0x4e, 0x0b, 0, 0];
            answer.extend_from_slice(&result.to_le_bytes());
            answer.extend_from_slice(&1010u32.to_le_bytes());
            answer.extend_from_slice(&3u16.to_le_bytes());
            answer.extend_from_slice(&200u32.to_le_bytes());
            let len = answer.len() as u16;
            answer[2..4].copy_from_slice(&len.to_le_bytes());
            let parsed = packets::packets_parser::parse(&answer, packetver);
            match dispatch_packet(parsed.as_ref()).as_slice() {
                [GameEvent::NpcMarketBought { success: s, items }] => {
                    assert_eq!((*s, items.as_slice()), (success, &[(1010, 3)][..]));
                }
                other => panic!("expected NpcMarketBought, got {other:?}"),
            }
        }
    }

    #[test]
    fn dispatch_notify_effect2_returns_play_effect_on_entity() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyEffect2::new(packetver);
        pkt.set_aid(150000);
        pkt.set_effect_id(28);
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::PlayEffectOnEntity {
                gid,
                effect_id,
                value,
            } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*effect_id, 28);
                assert_eq!(*value, None);
            }
            other => panic!("expected PlayEffectOnEntity, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_spirits_returns_spirits_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcSpirits::new(packetver);
        pkt.set_aid(150000);
        pkt.set_num(5);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::SpiritsChanged { gid, count } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*count, 5);
            }
            other => panic!("expected SpiritsChanged, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_devotion_keeps_the_filled_slots_in_order() {
        let packetver = 20250716;
        let mut pkt = PacketZcDevotionlist::new(packetver);
        pkt.set_my_aid(2_000_000);
        pkt.set_aid([2_000_001, 0, 2_000_002, 0, 0]);
        pkt.set_range(7);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::DevotionLinks { gid, targets } => {
                assert_eq!(*gid, 2_000_000);
                assert_eq!(targets, &[2_000_001, 2_000_002]);
            }
            other => panic!("expected DevotionLinks, got {other:?}"),
        }
        pkt.set_aid([0; 5]);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::DevotionLinks { targets, .. } => {
                assert!(targets.is_empty(), "every link gone")
            }
            other => panic!("expected DevotionLinks, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_elemental_init_carries_its_unit_and_gauges() {
        let packetver = 20250716;
        let mut pkt = PacketZcElInit::new(packetver);
        pkt.set_aid(110_000_123);
        pkt.set_hp(900);
        pkt.set_max_hp(1000);
        pkt.set_sp(80);
        pkt.set_max_sp(100);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::ElementalInit {
                gid,
                hp,
                max_hp,
                sp,
                max_sp,
            } => assert_eq!(
                (*gid, *hp, *max_hp, *sp, *max_sp),
                (110_000_123, 900, 1000, 80, 100)
            ),
            other => panic!("expected ElementalInit, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_elemental_par_takes_each_gauge_rathena_sends_and_no_other() {
        use nostalre_game::companion::ElementalPar;
        let packetver = 20250716;
        for (var, par) in [
            (5, ElementalPar::Hp),
            (6, ElementalPar::MaxHp),
            (7, ElementalPar::Sp),
            (8, ElementalPar::MaxSp),
        ] {
            let mut pkt = PacketZcElParChange::new(packetver);
            pkt.set_var(var);
            pkt.set_value(42);
            match dispatch_packet(&pkt).as_slice() {
                [
                    GameEvent::ElementalParChanged {
                        par: got,
                        value: 42,
                    },
                ] => assert_eq!(*got, par),
                other => panic!("var {var}: {other:?}"),
            }
        }
        let mut pkt = PacketZcElParChange::new(packetver);
        pkt.set_var(9);
        assert!(dispatch_packet(&pkt).is_empty());
    }

    #[test]
    fn dispatch_summon_hp_init_carries_its_unit_and_hp() {
        let packetver = 20250716;
        let mut pkt = PacketZcSummonHpInit::new(packetver);
        pkt.set_summon_aid(110_000_456);
        pkt.set_current_hp(3_000_000_000);
        pkt.set_max_hp(4_000_000_000);
        assert!(matches!(
            dispatch_packet(&pkt).as_slice(),
            [GameEvent::SummonHpInit {
                gid: 110_000_456,
                hp: 3_000_000_000,
                max_hp: 4_000_000_000,
            }]
        ));
    }

    #[test]
    fn dispatch_summon_hp_takes_hp_and_max_hp_and_no_other() {
        use nostalre_game::companion::SummonPar;
        let packetver = 20250716;
        for (var, par) in [(5, SummonPar::Hp), (6, SummonPar::MaxHp)] {
            let mut pkt = PacketZcSummonHpUpdate::new(packetver);
            pkt.set_summon_aid(7);
            pkt.set_var_id(var);
            pkt.set_value(42);
            match dispatch_packet(&pkt).as_slice() {
                [
                    GameEvent::SummonHpChanged {
                        gid: 7,
                        par: got,
                        value: 42,
                    },
                ] => assert_eq!(*got, par),
                other => panic!("var {var}: {other:?}"),
            }
        }
        let mut pkt = PacketZcSummonHpUpdate::new(packetver);
        pkt.set_var_id(7);
        assert!(dispatch_packet(&pkt).is_empty());
    }

    #[test]
    fn dispatch_crimson_marker_keeps_the_cell_and_the_removal() {
        let packetver = 20250716;
        for (x, y) in [(150, 40), (-1, -1)] {
            let mut pkt = PacketZcCMarkerinfo::new(packetver);
            pkt.set_aid(110_000_789);
            pkt.set_x_pos(x);
            pkt.set_y_pos(y);
            match dispatch_packet(&pkt).as_slice() {
                [
                    GameEvent::CrimsonMarker {
                        aid: 110_000_789,
                        x: got_x,
                        y: got_y,
                    },
                ] => assert_eq!((*got_x, *got_y), (x, y)),
                other => panic!("{x},{y}: {other:?}"),
            }
        }
    }

    #[test]
    fn dispatch_shadow_scars_names_the_unit_and_its_count() {
        let packetver = 20250716;
        let mut pkt = PacketZcTargetSpirits::new(packetver);
        pkt.set_gid(110_000_321);
        pkt.set_amount(100);
        assert!(matches!(
            dispatch_packet(&pkt).as_slice(),
            [GameEvent::ShadowScars {
                gid: 110_000_321,
                amount: 100,
            }]
        ));
    }

    #[test]
    fn dispatch_item_list_window_keeps_its_kind() {
        let packetver = 20250716;
        for kind in [0, 1, 2] {
            let mut pkt = PacketZcItemlistwinOpen::new(packetver);
            pkt.set_atype(kind);
            assert!(matches!(
                dispatch_packet(&pkt).as_slice(),
                [GameEvent::ItemListWindowOpened { kind: k }] if *k == kind
            ));
        }
    }

    #[test]
    fn dispatch_cart_decoration_keeps_every_cart_offered() {
        let packetver = 20250716;
        let mut pkt = PacketZcSelectcart::new(packetver);
        pkt.set_identity(2_000_000);
        pkt.set_carts(vec![10, 11, 12]);
        pkt.set_packet_length(11);
        pkt.fill_raw_with_packetver(Some(packetver));
        let parsed = packets::packets_parser::parse(&pkt.raw, packetver);
        assert!(matches!(
            dispatch_packet(parsed.as_ref()).as_slice(),
            [GameEvent::CartDecorationList { identity: 2_000_000, carts }] if carts == &vec![10, 11, 12]
        ));
    }

    #[test]
    fn dispatch_navigation_reads_rathenas_kinds_and_drops_any_other() {
        use nostalre_game::navigation::{NavServices, NavTarget};
        let packetver = 20250716;
        let mut pkt = PacketZcNavigationActive::new(packetver);
        pkt.set_atype(0);
        pkt.set_flag(101);
        pkt.set_hide_window(1);
        let mut map = [0u8; 16];
        map[..5].copy_from_slice(b"payon");
        pkt.set_map_name_raw(map);
        pkt.set_x_pos(252);
        pkt.set_y_pos(297);
        match dispatch_packet(&pkt).as_slice() {
            [GameEvent::NavigationGuide { navigation }] => {
                assert_eq!(navigation.map, "payon");
                assert_eq!(navigation.target, NavTarget::Cell { x: 252, y: 297 });
                assert_eq!(navigation.services, NavServices::from_flag(101));
                assert!(navigation.hide_window);
            }
            other => panic!("{other:?}"),
        }
        pkt.set_atype(2);
        assert!(dispatch_packet(&pkt).is_empty());
    }

    #[test]
    fn dispatch_combo_delay_names_the_unit_and_its_window() {
        let packetver = 20250716;
        let mut pkt = PacketZcCombodelay::new(packetver);
        pkt.set_aid(150000);
        pkt.set_delay_time(994);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::ComboDelay { gid, delay_ms } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*delay_ms, 994);
            }
            other => panic!("expected ComboDelay, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_bladestop_toggles_root_by_flag() {
        let packetver = 20250716;
        let mut pkt = PacketZcBladestop::new(packetver);
        pkt.set_src_aid(150000);
        pkt.set_dest_aid(160000);
        pkt.set_flag(1);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::BladeStop {
                src_gid,
                dest_gid,
                active,
            } => {
                assert_eq!(*src_gid, 150000);
                assert_eq!(*dest_gid, 160000);
                assert!(*active);
            }
            other => panic!("expected BladeStop, got {other:?}"),
        }
        pkt.set_flag(0);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::BladeStop { active, .. } => assert!(!*active),
            other => panic!("expected BladeStop, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_resurrection_and_mvp_return_lifecycle_events() {
        let packetver = 20250716;
        let mut res = PacketZcResurrection::new(packetver);
        res.set_aid(150000);
        match &dispatch_packet(&res)[0] {
            GameEvent::EntityResurrected { gid } => assert_eq!(*gid, 150000),
            other => panic!("expected EntityResurrected, got {other:?}"),
        }

        let mut mvp = PacketZcMvp::new(packetver);
        mvp.set_aid(150001);
        match &dispatch_packet(&mvp)[0] {
            GameEvent::MvpReward { gid } => assert_eq!(*gid, 150001),
            other => panic!("expected MvpReward, got {other:?}"),
        }
    }

    // ZC_EXTEND_BODYITEM_SIZE 0x0b18: expansion.W past the 100 slots; 0x0ade: natural_heal_weight_rate.L
    #[test]
    fn inventory_slots_and_overweight_at_20250716() {
        assert!(matches!(
            dispatch_packet(&*raw(0x0b18, &[&le(20, 2)]))[..],
            [GameEvent::InventorySlots { slots: 120 }]
        ));
        assert!(matches!(
            dispatch_packet(&*raw(0x0ade, &[&le(70, 4)]))[..],
            [GameEvent::OverweightPercent { percent: 70 }]
        ));
    }

    /// One 50-byte achievement entry: id.L completed.B objectives 10×L completed-at.L rewarded.B
    fn achievement_entry(id: i64, rewarded: bool) -> Vec<u8> {
        [
            le(id, 4),
            vec![1],
            vec![0; 40],
            le(1_700_000_000, 4),
            vec![rewarded as u8],
        ]
        .concat()
    }

    // ZC_ALL_ACH_LIST 0x0a23: count.L score.L level.W exp.L next.L, then 50-byte entries
    #[test]
    fn the_achievement_list_gives_the_claimed_ones() {
        let entries = [
            achievement_entry(200_000, true),
            achievement_entry(120_001, false),
            achievement_entry(130_000, true),
        ]
        .concat();
        let packet = raw_var(
            0x0a23,
            &[
                &le(3, 4),
                &le(30, 4),
                &le(1, 2),
                &le(30, 4),
                &le(80, 4),
                &entries,
            ],
        );
        match &dispatch_packet(&*packet)[..] {
            [GameEvent::AchievementsRewarded { achievement_ids }] => {
                assert_eq!(achievement_ids, &[200_000, 130_000]);
            }
            other => panic!("{other:?}"),
        }
    }

    // ZC_ACK_CHANGE_TITLE 0x0a2f: result.B title.L; 0 worn, 1 not one of the character's
    #[test]
    fn a_title_change_answer_says_whether_it_was_worn() {
        for (result, ok) in [(0, true), (1, false)] {
            let packet = raw(0x0a2f, &[&[result], &le(1034, 4)]);
            assert!(matches!(
                dispatch_packet(&*packet)[..],
                [GameEvent::TitleChangeAnswered { ok: o, title_id: 1034 }] if o == ok
            ));
        }
    }

    // The swap equipment set 0x0a9b (index.W position.L each) and the answers: add 0x0a98 (index.W position.L
    // flag.W), removal 0x0a9a (index.W position.L failed.W), CHANGE 0x0a9d (failed.W)
    #[test]
    fn the_swap_set_and_its_answers() {
        let list = raw_var(0x0a9b, &[&le(5, 2), &le(0x10, 4), &le(7, 2), &le(0x22, 4)]);
        assert!(matches!(
            &dispatch_packet(&*list)[..],
            [GameEvent::EquipSwitchList { entries }] if entries == &[(5, 0x10), (7, 0x22)]
        ));
        assert!(matches!(
            dispatch_packet(&*raw(0x0a98, &[&le(5, 2), &le(0x10, 4), &le(2, 2)]))[..],
            [GameEvent::EquipSwitchAdded {
                index: 5,
                location: 0x10,
                flag: 2
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*raw(0x0a9a, &[&le(7, 2), &le(0x22, 4), &le(0, 2)]))[..],
            [GameEvent::EquipSwitchRemoved {
                index: 7,
                failed: 0
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*raw(0x0a9d, &[&le(1, 2)]))[..],
            [GameEvent::EquipSwitchAnswered { failed: 1 }]
        ));
    }

    // Systems with no window here: the achievement update 0x0a24 and a claim's answer 0x0a26 (result.B id.L),
    // reputation 0x0b8d (success.B, then type.Q points.Q each), navigation 0x08e2 (type.B flag.B hide.B map.16B x.W
    // y.W mob.W)
    #[test]
    fn systems_without_a_window_are_parsed_and_dropped() {
        let achievement = achievement_entry(120_001, false);
        let packets = [
            raw(
                0x0a24,
                &[&le(10, 4), &le(1, 2), &le(10, 4), &le(80, 4), &achievement],
            ),
            raw(0x0a26, &[&[1], &le(120_001, 4)]),
            raw_var(0x0b8d, &[&[1], &le(1, 8), &le(0, 8)]),
        ];
        for packet in packets {
            assert!(
                matches!(dispatch_packet(&*packet)[..], [GameEvent::Acknowledged]),
                "{}",
                packet.name()
            );
        }
    }

    // ZC_CLEAR_DIALOG 0x08d6: npc.L; ZC_DIALOG_TEXT_ALIGN 0x0ba1: align.B
    #[test]
    fn dialog_clear_and_align_at_20250716() {
        assert!(matches!(
            dispatch_packet(&*raw(0x08d6, &[&le(110_022_079, 4)]))[..],
            [GameEvent::NpcDialogClear {
                npc_id: 110_022_079
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*raw(0x0ba1, &[&[2]]))[..],
            [GameEvent::Acknowledged]
        ));
    }

    // ZC_UPDATE_RANKING_POINT 0x097e: type.W points.L total.L; type 3 (/pk) has no fame
    #[test]
    fn fame_points_at_20250716_say_their_ranking() {
        let lines: Vec<String> = [(0, 1, 12), (1, 2, 24), (2, 3, 36)]
            .iter()
            .map(|&(kind, point, total)| {
                let packet = raw(0x097e, &[&le(kind, 2), &le(point, 4), &le(total, 4)]);
                match &dispatch_packet(&*packet)[..] {
                    [GameEvent::FamePointsGained { kind, point, total }] => {
                        kind.point_line(*point, *total)
                    }
                    other => panic!("expected FamePointsGained, got {other:?}"),
                }
            })
            .collect();
        assert_eq!(
            lines,
            vec![
                "[Blacksmith Point] You gained 1 point(s), for a total of 12.",
                "[Alchemist Point] You gained 2 point(s), for a total of 24.",
                "[TaeKwon Point] You gained 3 point(s), for a total of 36.",
            ]
        );
        let pk = raw(0x097e, &[&le(3, 2), &le(1, 4), &le(1, 4)]);
        assert!(matches!(
            dispatch_packet(&*pk)[..],
            [GameEvent::Acknowledged]
        ));
    }

    // ZC_ACK_RANKING2 0x0af6: type.W, ten character ids.L, ten points.L, my points.L; empty places are 0
    #[test]
    fn ranking_at_20250716_carries_character_ids() {
        let mut cids = vec![le(150_001, 4), le(150_005, 4)];
        cids.extend(std::iter::repeat_n(le(0, 4), 8));
        let mut points = vec![le(40, 4), le(7, 4)];
        points.extend(std::iter::repeat_n(le(0, 4), 8));
        let mut parts: Vec<&[u8]> = vec![&[1, 0]];
        parts.extend(cids.iter().map(Vec::as_slice));
        parts.extend(points.iter().map(Vec::as_slice));
        let mine = le(7, 4);
        parts.push(&mine);
        match &dispatch_packet(&*raw(0x0af6, &parts))[..] {
            [GameEvent::RankingReceived { ranking }] => {
                assert_eq!(ranking.kind, FameKind::Alchemist);
                assert_eq!(ranking.entries, vec![(150_001, 40), (150_005, 7)]);
            }
            other => panic!("expected RankingReceived, got {other:?}"),
        }
    }

    /// A packet at 20250716 from rAthena's bytes: `id`, then `fields` as little-endian (value, width) pairs.
    fn wire(id: u16, fields: &[(i64, usize)]) -> Box<dyn Packet> {
        let mut raw = id.to_le_bytes().to_vec();
        for &(value, width) in fields {
            raw.extend_from_slice(&value.to_le_bytes()[..width]);
        }
        packets::packets_parser::parse(&raw, 20250716)
    }

    // Trait stats at 20250716 (rAthena's clif_updatestatus): POW to CRT on 0x0141 with their bonus, P.ATK to
    // C.RATE, trait points, AP and max AP on 0x00b0, the costs on 0x00be (SP_UPOW is 247), a raise on 0x00bc
    #[test]
    fn trait_stats_at_20250716_fill_the_character() {
        let packets = [
            wire(0x0141, &[(219, 4), (5, 4), (2, 4)]),
            wire(0x0141, &[(224, 4), (3, 4), (0, 4)]),
            wire(0x00b0, &[(225, 2), (7, 4)]),
            wire(0x00b0, &[(231, 2), (12, 4)]),
            wire(0x00b0, &[(232, 2), (40, 4)]),
            wire(0x00b0, &[(233, 2), (200, 4)]),
            wire(0x00be, &[(247, 2), (1, 1)]),
            wire(0x00bc, &[(220, 2), (1, 1), (9, 1)]),
            wire(0x00bc, &[(221, 2), (0, 1), (0, 1)]),
        ];
        let mut character = nostalre_game::character::Character::new();
        for packet in &packets {
            for event in dispatch_packet(&**packet) {
                match event {
                    GameEvent::ParameterChanged { var_id, value } => {
                        character.apply_parameter_changed(var_id, value);
                    }
                    GameEvent::StatusChanged {
                        status_type,
                        base,
                        bonus,
                    } => character.apply_status_changed(status_type, base, bonus),
                    other => panic!("unexpected {other:?}"),
                }
            }
        }
        let t = &character.traits;
        assert_eq!(
            (t.stats[0].base, t.stats[0].bonus, t.stats[0].cost),
            (5, 2, 1)
        );
        assert_eq!(t.stats[1].base, 9);
        assert_eq!(t.stats[2].base, 0, "a refused raise changes nothing");
        assert_eq!(t.stats[5].base, 3);
        assert_eq!((t.p_atk, t.points, t.ap, t.max_ap), (7, 12, 40, 200));
    }

    // CHARACTER_INFO at 20250716: exp and job exp are 64-bit from 20170830, HP and SP from 20220330
    #[test]
    fn character_info_at_20250716_reads_the_wide_fields() {
        let mut info = CharacterInfoNeoUnion::new(20250716);
        info.set_exp_64(5_000_000_000);
        info.set_hp_64(16_753);
        info.set_maxhp_64(16_753);
        // SP past 16 bits (rAthena allows up to 1,000,000)
        info.set_sp_64(120_000);
        info.set_maxsp_64(120_000);
        info.set_body_color(4);
        let character = character_info_from_neo_union(&info);
        assert_eq!(character.base_exp, 5_000_000_000);
        assert_eq!((character.hp, character.max_hp), (16_753, 16_753));
        assert_eq!((character.sp, character.max_sp), (120_000, 120_000));
        assert_eq!(
            character.cloth_color, 4,
            "a clothes dye comes back with the character"
        );
    }

    // ZC_NOTIFY_EXP 0x0acc: AID.L, exp.Q, varID.W, expType.W (64-bit from 20170830)
    #[test]
    fn exp_gain_is_64_bit_at_20250716() {
        let base = StatusTypes::Baseexp.value() as i64;
        let pkt = wire(
            0x0acc,
            &[(2_000_000, 4), (5_000_000_000, 8), (base, 2), (1, 2)],
        );
        assert!(matches!(
            dispatch_packet(&*pkt)[..],
            [GameEvent::ExpGained {
                aid: 2_000_000,
                amount: 5_000_000_000,
                is_base: true,
                is_quest: true,
            }]
        ));
    }

    // ZC_LONGLONGPAR_CHANGE 0x0acb: varID.W, amount.Q (experience and the next level's, from 20170830)
    #[test]
    fn experience_parameters_are_64_bit_at_20250716() {
        let next = StatusTypes::Nextbaseexp.value() as i64;
        let pkt = wire(0x0acb, &[(next, 2), (9_000_000_000, 8)]);
        assert!(matches!(
            dispatch_packet(&*pkt)[..],
            [GameEvent::ParameterChanged {
                var_id: 22,
                value: 9_000_000_000,
            }]
        ));
    }

    // ZC_RECOVERY 0x0a27: type.W, amount.L (32-bit from 20141022)
    #[test]
    fn recovery_is_32_bit_at_20250716() {
        let pkt = wire(0x0a27, &[(5, 2), (70_000, 4)]);
        assert!(matches!(
            dispatch_packet(&*pkt)[..],
            [GameEvent::Recovery {
                var_id: 5,
                amount: 70_000,
            }]
        ));
    }

    // ZC_USESKILL_ACK 0x0b1a: srcId.L dstId.L x.W y.W skillId.W element.L delayTime.L disposable.B attackMT.L
    #[test]
    fn skill_cast_at_20250716() {
        let cast = |skill| {
            wire(
                0x0b1a,
                &[
                    (150_000, 4),
                    (200_000, 4),
                    (7, 2),
                    (9, 2),
                    (skill, 2),
                    (3, 4),
                    (2000, 4),
                    (1, 1),
                    (500, 4),
                ],
            )
        };
        assert!(matches!(
            dispatch_packet(&*cast(10))[..],
            [GameEvent::SkillCasting {
                gid: 150_000,
                target_gid: 200_000,
                skill: SkillEnum::MgSight,
                property: 3,
                delay_ms: 2000,
                x: 7,
                y: 9,
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*cast(2001))[..],
            [GameEvent::SkillCasting {
                skill: SkillEnum::RkEnchantblade,
                ..
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*cast(9999))[..],
            [GameEvent::Acknowledged]
        ));
    }

    // ZC_SKILL_ENTRY5 0x09ca: length.W id.L creator.L x.W y.W unitId.L range.B visible.B level.B; Renewal unit ids
    // pass 0xff (rAthena's `UNT_MAX` is 0x190)
    #[test]
    fn skill_unit_at_20250716_has_a_wide_unit_id() {
        let fields = [
            (23, 2),
            (7001, 4),
            (42, 4),
            (150, 2),
            (200, 2),
            (0x10a, 4),
            (2, 1),
            (1, 1),
            (5, 1),
        ];
        assert!(matches!(
            dispatch_packet(&*wire(0x09ca, &fields))[..],
            [GameEvent::SkillUnitEntered {
                aid: 7001,
                creator_aid: 42,
                x: 150,
                y: 200,
                unit_id: 0x10a,
                is_visible: true,
            }]
        ));
    }

    // ZC_USE_SKILL 0x09cb: SKID.W level.L targetAID.L srcAID.L result.B; for a heal, `level` is the amount
    #[test]
    fn no_damage_skill_at_20250716_carries_a_32_bit_amount() {
        let heal = 28; // AL_HEAL
        let fields = [(heal, 2), (40_000, 4), (200_000, 4), (150_000, 4), (1, 1)];
        assert!(matches!(
            dispatch_packet(&*wire(0x09cb, &fields))[..],
            [GameEvent::SkillNoDamage {
                skill: SkillEnum::AlHeal,
                src_gid: 150_000,
                target_gid: 200_000,
                level: 40_000,
            }]
        ));
    }

    // ZC_MSG_COLOR 0x09cd: msg.W color.L (BGR); ZC_MSG_VALUE 0x07e2: msg.W value.L; ZC_MSG_SKILL 0x07e6: skill.W
    // msg.L; ZC_SKILLINFO_DELETE 0x0441: skill.W
    #[test]
    fn server_messages_and_skill_removal_at_20250716() {
        let can_not_equip = 372;
        assert!(matches!(
            dispatch_packet(&*wire(0x09cd, &[(can_not_equip, 2), (0xff, 4)]))[..],
            [GameEvent::ServerMsgColor {
                msg_id: 372,
                color: 0xff,
            }]
        ));
        let reuse_seconds = 1862;
        assert!(matches!(
            dispatch_packet(&*wire(0x07e2, &[(reuse_seconds, 2), (12, 4)]))[..],
            [GameEvent::ServerMsgValue {
                msg_id: 1862,
                value: 12,
            }]
        ));
        // GN_CHANGEMATERIAL: the id goes through raw, the chat line names it
        let skill_fail = 1575;
        assert!(matches!(
            dispatch_packet(&*wire(0x07e6, &[(2494, 2), (skill_fail, 4)]))[..],
            [GameEvent::ServerSkillMsg {
                skill_id: 2494,
                msg_id: 1575,
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*wire(0x0441, &[(142, 2)]))[..],
            [GameEvent::SkillRemoved {
                skill: SkillEnum::NvFirstaid,
            }]
        ));
    }

    // ZC_MSG_STATE_CHANGE3 0x0983: index.W AID.L state.B total.L left.L val.3L (the total from 20120618);
    // ZC_EFST_SET_ENTER2 0x0984: AID.L type.W total.L left.L val.3L, a unit coming into view with the status on
    #[test]
    fn status_icons_at_20250716_read_the_time_left() {
        let change = [
            (32, 2),
            (150_000, 4),
            (1, 1),
            (60_000, 4),
            (45_000, 4),
            (7, 4),
            (0, 4),
            (0, 4),
        ];
        let enter = [
            (150_001, 4),
            (32, 2),
            (60_000, 4),
            (45_000, 4),
            (7, 4),
            (0, 4),
            (0, 4),
        ];
        for (id, fields, gid) in [
            (0x0983, &change[..], 150_000),
            (0x0984, &enter[..], 150_001),
        ] {
            match &dispatch_packet(&*wire(id, fields))[..] {
                [
                    GameEvent::StatusEffectChanged {
                        gid: g,
                        efst: 32,
                        active: true,
                        remain_ms: 45_000,
                        val1: 7,
                    },
                ] => {
                    assert_eq!(*g, gid)
                }
                other => panic!("{id:#06x}: expected StatusEffectChanged, got {other:?}"),
            }
        }
    }

    // ZC_AUTOSPELLLIST 0x0afb: length.W, then skill ids.L, as many as offered (rAthena's Renewal list has nine)
    #[test]
    fn autospell_list_at_20250716_is_variable() {
        // MG_FIREBOLT 19, MG_COLDBOLT 14, ..., WZ_HEAVENDRIVE 91
        let ids = [19, 14, 20, 13, 17, 90, 15, 21, 91];
        let mut fields = vec![(4 + 4 * ids.len() as i64, 2)];
        fields.extend(ids.iter().map(|&id| (id, 4)));
        match &dispatch_packet(&*wire(0x0afb, &fields))[..] {
            [GameEvent::AutoSpellList { skills }] => {
                assert_eq!(skills.len(), 9);
                assert_eq!(skills[0], SkillEnum::MgFirebolt);
                assert_eq!(skills[8], SkillEnum::WzHeavendrive);
            }
            other => panic!("expected AutoSpellList, got {other:?}"),
        }
    }

    // ZC_MILLENNIUMSHIELD 0x0440: AID.L num.W state.W; rAthena's state is always 0, a broken shield the count left
    #[test]
    fn millennium_shields_carry_the_count_left() {
        for (num, count) in [(2, 2), (0, 0)] {
            assert!(matches!(
                &dispatch_packet(&*wire(0x0440, &[(150_000, 4), (num, 2), (0, 2)]))[..],
                [GameEvent::MillenniumShieldChanged { gid: 150_000, count: c }] if *c == count
            ));
        }
    }

    // ZC_SPIRITS_ATTRIBUTE 0x08cf: AID.L spiritsType.W num.W; type 3 fire, and 0 with 0 when the last charm went
    #[test]
    fn charms_carry_their_element_and_count() {
        for (element, num) in [(3u8, 10u8), (0, 0)] {
            let packet = wire(
                0x08cf,
                &[(150_000, 4), (element as i64, 2), (num as i64, 2)],
            );
            assert!(matches!(
                &dispatch_packet(&*packet)[..],
                [GameEvent::CharmsChanged { gid: 150_000, element: e, count: c }] if (*e, *c) == (element, num)
            ));
        }
    }

    // ZC_REQ_WEAR_EQUIP_ACK 0x0999: index.W wearLocation.L viewid.W result.B; shadow gear sits at 0x10000 and up
    #[test]
    fn a_shadow_slot_keeps_its_high_bits() {
        let fields = [(7, 2), (0x01_0000, 4), (0, 2), (0, 1)];
        assert!(matches!(
            &dispatch_packet(&*wire(0x0999, &fields))[..],
            [GameEvent::InventoryEquipResult {
                index: 7,
                wear_location: 0x01_0000,
                ..
            }]
        ));
    }

    #[test]
    fn a_combo_skill_update_carries_its_target_type() {
        // ZC_SKILLINFO_UPDATE2 0x07e1: skid.W type.L level.W sp.W range.W upgradable.B;
        // Asura Strike made self-targeted (4) while a combo window is open
        let fields = [(271, 2), (4, 4), (5, 2), (0, 2), (2, 2), (0, 1)];
        assert!(matches!(
            &dispatch_packet(&*wire(0x07e1, &fields))[..],
            [GameEvent::SkillUpdated {
                skill: SkillEnum::MoExtremityfist,
                target_type: Some(SkillTargetType::MySelf),
                ..
            }]
        ));
    }

    // ZC_SOULENERGY 0x0b73: AID.L num.W, a Soul Reaper's souls (0 to 20)
    #[test]
    fn soul_energy_carries_the_count() {
        for count in [20u8, 0] {
            assert!(matches!(
                &dispatch_packet(&*wire(0x0b73, &[(150_000, 4), (count as i64, 2)]))[..],
                [GameEvent::SoulEnergyChanged { gid: 150_000, count: c }] if *c == count
            ));
        }
    }

    // ZC_MAKINGITEM_LIST 0x025a: length.W makeItem.W, then item ids.L (from 20181121): Mix Cooking's list (6)
    #[test]
    fn cooking_list_at_20250716_carries_its_kind_and_32_bit_items() {
        let ids = [12062, 500001];
        let mut fields = vec![(6 + 4 * ids.len() as i64, 2), (6, 2)];
        fields.extend(ids.iter().map(|&id| (id, 4)));
        assert!(matches!(
            &dispatch_packet(&*wire(0x025a, &fields))[..],
            [GameEvent::CookingList { mk_type: 6, item_ids }] if item_ids[..] == [12062, 500001]
        ));
    }

    // ZC_SKILL_SELECT_REQUEST 0x0442: length.W why.L, then skill ids.W: Auto Shadow Spell's copied skills (why 1)
    #[test]
    fn skill_select_list_at_20250716_names_the_copied_skills() {
        // MG_FIREBOLT 19, SC_FATALMENACE 2284, and an id no skill has
        let ids = [19, 2284, 9999];
        let mut fields = vec![(8 + 2 * ids.len() as i64, 2), (1, 4)];
        fields.extend(ids.iter().map(|&id| (id, 2)));
        assert!(matches!(
            &dispatch_packet(&*wire(0x0442, &fields))[..],
            [GameEvent::SkillSelectList { why: 1, skills }]
                if skills[..] == [SkillEnum::MgFirebolt, SkillEnum::ScFatalmenace]
        ));
    }

    // ZC_SPRITE_CHANGE2 0x01d7: AID.L type.B val.L val2.L. The hands (type 2) carry item ids when the item has no
    // view id, and Renewal weapons pass 16 bits (Boost Sword 500001)
    #[test]
    fn hand_look_at_20250716_keeps_32_bit_item_ids() {
        let fields = [(150_000, 4), (2, 1), (500_001, 4), (2101, 4)];
        assert!(matches!(
            dispatch_packet(&*wire(0x01d7, &fields))[..],
            [GameEvent::EntitySpriteChanged {
                gid: 150_000,
                sprite_type: 2,
                value: 500_001,
                value2: 2101,
            }]
        ));
    }

    // ZC_SHORTCUT_KEY_LIST 0x0b20: rotate.B, tab.W, 38 of isSkill.B id.L count.W. The server sends tab 0 (the bar)
    // and tab 1 (a second bar, rAthena's `hotkey_rowshift2`, which this client has no window for).
    #[test]
    fn hotkey_bar_at_20250716_is_tab_zero() {
        let list = |tab| {
            let mut fields = vec![
                (0, 1),
                (tab, 2),
                (1, 1),
                (5, 4),
                (10, 2),
                (0, 1),
                (501, 4),
                (3, 2),
            ];
            fields.extend((2..38).flat_map(|_| [(0, 1), (0, 4), (0, 2)]));
            wire(0x0b20, &fields)
        };
        match &dispatch_packet(&*list(0))[..] {
            [GameEvent::HotkeyListReceived { slots }] => {
                assert_eq!(slots.len(), 38);
                assert_eq!(slots[..2], [(1, 5, 10), (0, 501, 3)]);
            }
            other => panic!("expected HotkeyListReceived, got {other:?}"),
        }
        assert!(matches!(
            dispatch_packet(&*list(1))[..],
            [GameEvent::Acknowledged]
        ));
    }

    #[test]
    fn dispatch_mvp_feedback_and_pvp_points() {
        let packetver = 20250716;
        let mut item = PacketZcMvpGettingItem::new(packetver);
        item.set_itid_32(603);
        match &dispatch_packet(&item)[0] {
            GameEvent::MvpFeedback {
                kind: MvpFeedbackKind::Item { item_id },
            } => assert_eq!(*item_id, 603),
            other => panic!("expected MvpFeedback item, got {other:?}"),
        }

        let thrown = PacketZcThrowMvpitem::new(packetver);
        match &dispatch_packet(&thrown)[0] {
            GameEvent::MvpFeedback {
                kind: MvpFeedbackKind::ItemDropped,
            } => {}
            other => panic!("expected MvpFeedback dropped, got {other:?}"),
        }

        let mut pvp = PacketZcAckPvppoint::new(packetver);
        let mut info = PVPINFO::new(packetver);
        info.set_win_point(7);
        info.set_lose_point(2);
        info.set_point(50);
        pvp.set_pvp(info);
        match &dispatch_packet(&pvp)[0] {
            GameEvent::PvpPointsReceived { win, lose, point } => {
                assert_eq!((*win, *lose, *point), (7, 2, 50));
            }
            other => panic!("expected PvpPointsReceived, got {other:?}"),
        }
    }

    #[test]
    fn skillmsg_resolves_known_ids_and_stays_silent_otherwise() {
        let packetver = 20250716;
        let mut pkt = PacketZcSkillmsg::new(packetver);
        pkt.set_msg_no(0x17);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::SkillMsg { msg_no } => assert_eq!(
                nostalre_game::skill_msg::skill_msg_line(*msg_no),
                Some("Max HP +100%.")
            ),
            other => panic!("expected SkillMsg, got {other:?}"),
        }

        pkt.set_msg_no(0x1a);
        match &dispatch_packet(&pkt)[0] {
            GameEvent::SkillMsg { msg_no } => {
                assert_eq!(nostalre_game::skill_msg::skill_msg_line(*msg_no), None)
            }
            other => panic!("expected SkillMsg, got {other:?}"),
        }
    }

    #[test]
    fn talkbox_contents_trim_at_the_first_nul_and_decode_euc_kr() {
        let packetver = 20250716;
        let mut pkt = PacketZcTalkboxChatcontents::new(packetver);
        pkt.set_aid(4000);
        let mut contents = [0xffu8; 21];
        // "프론테라" followed by a NUL and trailing junk.
        let text: &[u8] = &[
            0xc7, 0xc1, 0xb7, 0xd0, 0xc5, 0xd7, 0xb6, 0xf3, 0x00, b'j', b'u', b'n', b'k',
        ];
        contents[..text.len()].copy_from_slice(text);
        pkt.set_contents_21_raw(contents);

        match &dispatch_packet(&pkt)[0] {
            GameEvent::TalkboxContents { aid, message } => {
                assert_eq!(*aid, 4000);
                assert_eq!(message, "프론테라");
            }
            other => panic!("expected TalkboxContents, got {other:?}"),
        }
    }

    #[test]
    fn showdigit_and_boss_info_reject_unknown_types() {
        let packetver = 20250716;
        let mut digit = PacketZcShowdigit::new(packetver);
        digit.set_atype(3);
        digit.set_value(60);
        match &dispatch_packet(&digit)[0] {
            GameEvent::ShowDigit { mode, value } => {
                assert_eq!(*mode, ShowDigitMode::FastCountDown);
                assert_eq!(*value, 60);
            }
            other => panic!("expected ShowDigit, got {other:?}"),
        }
        digit.set_atype(9);
        assert!(dispatch_packet(&digit).is_empty());

        let mut boss = PacketZcBossInfo::new(packetver);
        boss.set_info_type(3);
        boss.set_min_hour(1);
        boss.set_min_minute(30);
        match &dispatch_packet(&boss)[0] {
            GameEvent::BossInfoReceived {
                kind,
                respawn_hour,
                respawn_minute,
                ..
            } => {
                assert_eq!(*kind, BossInfoKind::Dead);
                assert_eq!((*respawn_hour, *respawn_minute), (1, 30));
            }
            other => panic!("expected BossInfoReceived, got {other:?}"),
        }
        boss.set_info_type(7);
        assert!(dispatch_packet(&boss).is_empty());
    }

    #[test]
    fn equip_ack_succeeds_on_result_zero() {
        let packetver = 20250716;
        // 0x0999: index, 32-bit position, sprite id, result (0 is ITEM_EQUIP_ACK_OK)
        let ok = [
            0x99, 0x09, 0x0c, 0x00, 0x00, 0x01, 0x00, 0x00, 0x2a, 0x00, 0x00,
        ];
        match &dispatch_packet(&*packets::packets_parser::parse(&ok, packetver))[0] {
            GameEvent::InventoryEquipResult {
                index,
                wear_location,
                view_id,
                success,
            } => {
                assert_eq!((*index, *wear_location, *view_id), (12, 0x0100, 42));
                assert!(*success);
            }
            other => panic!("expected InventoryEquipResult, got {other:?}"),
        }

        for code in [1u8, 2] {
            let fail = [
                0x99, 0x09, 0x0c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, code,
            ];
            match &dispatch_packet(&*packets::packets_parser::parse(&fail, packetver))[0] {
                GameEvent::InventoryEquipResult { success, .. } => {
                    assert!(!*success, "result {code} is a refusal")
                }
                other => panic!("expected InventoryEquipResult, got {other:?}"),
            }
        }
    }

    #[test]
    fn dispatch_attack_range_returns_attack_range_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcAttackRange::new(packetver);
        pkt.set_current_att_range(2);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::AttackRangeChanged { range } => assert_eq!(*range, 2),
            other => panic!("expected AttackRangeChanged, got {other:?}"),
        }
    }

    #[test]
    fn who_request_and_its_answer_round_trip() {
        let packetver = 20250716;
        let request = crate::sender::build_user_count_packet(packetver);
        assert_eq!(request.len(), 2);
        assert_eq!(u16::from_le_bytes([request[0], request[1]]), 0x00c1);

        let mut pkt = PacketZcUserCount::new(packetver);
        pkt.set_count(137);
        pkt.fill_raw();
        match &dispatch_packet(&pkt)[0] {
            GameEvent::UserCount { count } => assert_eq!(*count, 137),
            other => panic!("expected UserCount, got {other:?}"),
        }
    }

    #[test]
    fn manner_report_request_and_its_two_answers_round_trip() {
        let packetver = 20250716;

        let request = crate::sender::build_give_manner_point_packet(2000042, false, 60, packetver);
        assert_eq!(u16::from_le_bytes([request[0], request[1]]), 0x0149);
        assert_eq!(request.len(), 9);
        assert_eq!(
            u32::from_le_bytes([request[2], request[3], request[4], request[5]]),
            2000042
        );
        assert_eq!(request[6], nostalre_game::gm::MANNER_TYPE_MINUS);
        assert_eq!(i16::from_le_bytes([request[7], request[8]]), 60);

        let by_name = crate::sender::build_give_manner_byname_packet("Hunter", packetver);
        assert_eq!(u16::from_le_bytes([by_name[0], by_name[1]]), 0x0212);
        assert_eq!(by_name.len(), 26);

        let mut ack = PacketZcAckGiveMannerPoint::new(packetver);
        ack.set_result(0);
        ack.fill_raw();
        assert!(matches!(
            dispatch_packet(&ack).as_slice(),
            [GameEvent::MannerPointResult { result: 0 }]
        ));

        let mut given = PacketZcNotifyMannerPointGiven::new(packetver);
        given.set_atype(1);
        let mut gm_name = [0 as char; 24];
        for (i, c) in "Sysop".chars().enumerate() {
            gm_name[i] = c;
        }
        given.set_other_char_name(gm_name);
        given.fill_raw();
        match &dispatch_packet(&given)[0] {
            GameEvent::MannerPointGiven {
                positive,
                other_name,
            } => {
                assert!(!positive);
                assert_eq!(other_name, "Sysop");
            }
            other => panic!("expected MannerPointGiven, got {other:?}"),
        }
    }

    #[test]
    fn check_request_and_its_stat_block_round_trip() {
        let packetver = 20250716;

        let request = crate::sender::build_status_gm_packet("Hunter", packetver);
        assert_eq!(u16::from_le_bytes([request[0], request[1]]), 0x0213);
        assert_eq!(request.len(), 26);

        let mut pkt = PacketZcAckStatusGm::new(packetver);
        pkt.set_str(90);
        pkt.set_standard_str(15);
        pkt.set_luk(7);
        pkt.set_att_power(120);
        pkt.set_refining_power(30);
        pkt.set_min_matt_power(40);
        pkt.set_max_matt_power(55);
        pkt.set_aspd(180);
        pkt.fill_raw();
        match &dispatch_packet(&pkt)[0] {
            GameEvent::GmStatusReceived { status } => {
                assert_eq!(status.str, 90);
                assert_eq!(status.str_cost, 15);
                assert_eq!(status.luk, 7);
                assert_eq!(status.aspd, 180);
                let lines = status.lines();
                assert!(lines[0].starts_with("STR 90"));
                assert!(lines[2].contains("ATK 120+30"));
                assert!(lines[2].contains("MATK 40~55"));
            }
            other => panic!("expected GmStatusReceived, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_attack_failure_for_distance_carries_positions_and_range() {
        let packetver = 20250716;
        let mut pkt = PacketZcAttackFailureForDistance::new(packetver);
        pkt.set_target_aid(150000);
        pkt.set_target_xpos(120);
        pkt.set_target_ypos(80);
        pkt.set_x_pos(115);
        pkt.set_y_pos(78);
        pkt.set_current_att_range(3);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::AttackFailedForDistance {
                target_gid,
                target_x,
                target_y,
                x,
                y,
                range,
            } => {
                assert_eq!(*target_gid, 150000);
                assert_eq!((*target_x, *target_y), (120, 80));
                assert_eq!((*x, *y), (115, 78));
                assert_eq!(*range, 3);
            }
            other => panic!("expected AttackFailedForDistance, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_notify_hp_to_groupm_returns_entity_hp_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyHpToGroupmR2::new(packetver);
        pkt.set_aid(42);
        pkt.set_hp(350);
        pkt.set_maxhp(500);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        match result.as_slice() {
            [
                GameEvent::EntityHpChanged { gid, hp, max_hp },
                GameEvent::PartyMemberHp {
                    aid,
                    hp: php,
                    max_hp: pmax,
                },
            ] => {
                assert_eq!((*gid, *hp, *max_hp), (42, 350, 500));
                assert_eq!((*aid, *php, *pmax), (42, 350, 500));
            }
            other => panic!("expected EntityHpChanged + PartyMemberHp, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_autorun_skill_returns_auto_cast() {
        let packetver = 20250716;
        let mut pkt = PacketZcAutorunSkill::new(packetver);
        pkt.data.skid = SkillEnum::AllResurrection.id() as i16;
        pkt.data.level = 1;
        pkt.data.atype = SkillTargetType::Friend.value() as i32;
        pkt.data.spcost = 60;
        pkt.data.attack_range = 9;
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::AutoCastSkill {
                skill,
                level,
                sp_cost,
                attack_range,
                skill_target_type,
            } => {
                assert_eq!(*skill, SkillEnum::AllResurrection);
                assert_eq!(*level, 1);
                assert_eq!(*sp_cost, 60);
                assert_eq!(*attack_range, 9);
                assert_eq!(*skill_target_type, SkillTargetType::Friend);
            }
            other => panic!("expected AutoCastSkill, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_emotion_returns_entity_emotion() {
        let packetver = 20250716;
        let mut pkt = PacketZcEmotion::new(packetver);
        pkt.set_gid(42);
        pkt.set_atype(1);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityEmotion { gid, emotion_type } => {
                assert_eq!(*gid, 42);
                assert_eq!(*emotion_type, 1);
            }
            other => panic!("expected EntityEmotion, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_say_dialog_returns_npc_dialog_text() {
        let packetver = 20250716;
        let mut pkt = PacketZcSayDialog::new(packetver);
        pkt.set_naid(500);
        pkt.set_msg("Hello traveler!\0".to_string());
        pkt.set_msg_raw("Hello traveler!\0".as_bytes().to_vec());
        pkt.set_packet_length((8 + 16) as i16);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::NpcDialogText { npc_id, text } => {
                assert_eq!(*npc_id, 500);
                assert_eq!(text, "Hello traveler!");
            }
            other => panic!("expected NpcDialogText, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_menu_list_splits_items() {
        let packetver = 20250716;
        let mut pkt = PacketZcMenuList::new(packetver);
        pkt.set_naid(500);
        let msg = "Buy:Sell:Cancel\0";
        pkt.set_msg(msg.to_string());
        pkt.set_msg_raw(msg.as_bytes().to_vec());
        pkt.set_packet_length((8 + msg.len()) as i16);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::NpcDialogMenu { npc_id, items } => {
                assert_eq!(*npc_id, 500);
                assert_eq!(items, &["Buy", "Sell", "Cancel"]);
            }
            other => panic!("expected NpcDialogMenu, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_wait_and_close_dialog() {
        let packetver = 20250716;

        let mut pkt = PacketZcWaitDialog::new(packetver);
        pkt.set_naid(500);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            GameEvent::NpcDialogNext { npc_id: 500 }
        ));

        let mut pkt = PacketZcCloseDialog::new(packetver);
        pkt.set_naid(500);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0],
            GameEvent::NpcDialogClose { npc_id: 500 }
        ));
    }

    #[test]
    fn show_image2_wire_bytes_parse_to_npc_cutin() {
        let packetver = 20250716;
        let name = b"wedding_marry0";
        let mut raw = vec![0xb3, 0x01];
        raw.extend_from_slice(name);
        raw.resize(2 + 64, 0);
        raw.push(2);
        let parsed = packets::packets_parser::parse(&raw, packetver);
        assert!(
            parsed.as_any().is::<PacketZcShowImage2>(),
            "0x01b3 must parse as ZC_SHOW_IMAGE2, got {}",
            parsed.name()
        );
        match dispatch_packet(parsed.as_ref()).as_slice() {
            [GameEvent::NpcCutin { image, position }] => {
                assert_eq!(image, "wedding_marry0");
                assert_eq!(*position, 2);
            }
            other => panic!("expected NpcCutin, got {other:?}"),
        }
    }

    #[test]
    fn open_editdlgstr_wire_bytes_parse_to_npc_input_string() {
        let packetver = 20250716;
        let mut raw = vec![0xd4, 0x01];
        raw.extend_from_slice(&110002361u32.to_le_bytes());
        let parsed = packets::packets_parser::parse(&raw, packetver);
        assert!(
            parsed.as_any().is::<PacketZcOpenEditdlgstr>(),
            "0x01d4 must parse as ZC_OPEN_EDITDLGSTR, got {}",
            parsed.name()
        );
        match dispatch_packet(parsed.as_ref()).as_slice() {
            [GameEvent::NpcInputString { npc_id }] => assert_eq!(*npc_id, 110002361),
            other => panic!("expected NpcInputString, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_state_change3_returns_entity_option_changed() {
        let packetver = 20250716;
        let mut pkt = PacketZcStateChange3::new(packetver);
        pkt.set_aid(150000);
        pkt.set_body_state(3);
        pkt.set_health_state(0x1);
        pkt.set_effect_state(0x20);
        pkt.set_is_pkmode_on(false);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::EntityOptionChanged {
                gid,
                body_state,
                health_state,
                effect_state,
            } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*body_state, 3);
                assert_eq!(*health_state, 0x1);
                assert_eq!(*effect_state, 0x20);
            }
            other => panic!("expected EntityOptionChanged, got {other:?}"),
        }
    }

    #[test]
    fn a_walking_actor_entering_sight_carries_its_opt3_buffs() {
        let packetver = 20250716;
        let mut pkt = PacketZcNotifyMoveentry8::new(packetver);
        pkt.set_aid(150000);
        pkt.set_clevel(88);
        pkt.set_virtue(nostalre_game::effect::opt3::OPT3_BERSERK);
        pkt.fill_raw();
        match dispatch_packet(&pkt).as_slice() {
            [
                GameEvent::EntitySpawned { gid, .. },
                GameEvent::EntityMoved { .. },
                GameEvent::EntityOpt3Changed {
                    gid: opt3_gid,
                    base_level,
                    opt3,
                    ..
                },
            ] => {
                assert_eq!(*gid, 150000);
                assert_eq!(*opt3_gid, 150000);
                assert_eq!(*base_level, 88);
                assert_eq!(*opt3, nostalre_game::effect::opt3::OPT3_BERSERK);
            }
            other => panic!("expected spawn + move + opt3, got {other:?}"),
        }
    }

    #[test]
    fn skill_entry2_splits_graffiti_from_ordinary_ground_units() {
        let packetver = 20250716;

        let mut graffiti = PacketZcSkillEntry2::new(packetver);
        graffiti.set_aid(4000);
        graffiti.set_creator_aid(150000);
        graffiti.set_x_pos(120);
        graffiti.set_y_pos(80);
        graffiti.set_job(0xb0);
        graffiti.set_is_visible(true);
        graffiti.set_is_contens(true);
        let mut msg = ['\0'; 80];
        for (slot, c) in msg.iter_mut().zip("HELLO".chars()) {
            *slot = c;
        }
        graffiti.set_msg(msg);
        graffiti.fill_raw();
        match dispatch_packet(&graffiti).as_slice() {
            [
                GameEvent::GraffitiEntered {
                    aid,
                    creator_aid,
                    x,
                    y,
                    message,
                },
            ] => {
                assert_eq!((*aid, *creator_aid), (4000, 150000));
                assert_eq!((*x, *y), (120, 80));
                assert_eq!(message, "HELLO");
            }
            other => panic!("expected GraffitiEntered, got {other:?}"),
        }

        let mut unit = PacketZcSkillEntry2::new(packetver);
        unit.set_aid(4001);
        unit.set_job(0x7f);
        unit.set_is_visible(true);
        unit.set_is_contens(false);
        unit.fill_raw();
        match dispatch_packet(&unit).as_slice() {
            [GameEvent::SkillUnitEntered { aid, unit_id, .. }] => {
                assert_eq!((*aid, *unit_id), (4001, 0x7f));
            }
            other => panic!("expected SkillUnitEntered, got {other:?}"),
        }
    }

    #[test]
    fn half_wired_replies_now_reach_the_game() {
        let packetver = 20250716;

        let mut whisper_pc = PacketZcSettingWhisperPc::new(packetver);
        whisper_pc.set_atype(0);
        whisper_pc.set_result(2);
        whisper_pc.fill_raw();
        match dispatch_packet(&whisper_pc).as_slice() {
            [GameEvent::WhisperSettingResult { allow, result, all }] => {
                assert!(!*allow && *result == 2 && !*all);
            }
            other => panic!("expected WhisperSettingResult, got {other:?}"),
        }

        let mut whisper_all = PacketZcSettingWhisperState::new(packetver);
        whisper_all.set_atype(1);
        whisper_all.set_result(0);
        whisper_all.fill_raw();
        match dispatch_packet(&whisper_all).as_slice() {
            [GameEvent::WhisperSettingResult { allow, all, .. }] => assert!(*allow && *all),
            other => panic!("expected WhisperSettingResult, got {other:?}"),
        }

        let mut memo = PacketZcAckRememberWarppoint::new(packetver);
        memo.set_error_code(1);
        memo.fill_raw();
        match dispatch_packet(&memo).as_slice() {
            [GameEvent::MemoResult { result }] => assert_eq!(*result, 1),
            other => panic!("expected MemoResult, got {other:?}"),
        }

        let mut progress = PacketZcProgress::new(packetver);
        progress.set_color(0x00ff_ff00);
        progress.set_time(7);
        progress.fill_raw();
        match dispatch_packet(&progress).as_slice() {
            [GameEvent::ProgressBarStarted { duration_secs }] => assert_eq!(*duration_secs, 7),
            other => panic!("expected ProgressBarStarted, got {other:?}"),
        }
    }

    #[test]
    fn refusal_replies_carry_their_reason_to_the_game() {
        let packetver = 20250716;

        let mut mapinfo = PacketZcNotifyMapinfo::new(packetver);
        mapinfo.set_atype(2);
        mapinfo.fill_raw();
        match dispatch_packet(&mapinfo).as_slice() {
            [GameEvent::MapInfoNotice { atype }] => assert_eq!(*atype, 2),
            other => panic!("expected MapInfoNotice, got {other:?}"),
        }

        let mut arrow = PacketZcActionFailure::new(packetver);
        arrow.set_error_code(0);
        arrow.fill_raw();
        match dispatch_packet(&arrow).as_slice() {
            [GameEvent::ActionFailure { error_code }] => assert_eq!(*error_code, 0),
            other => panic!("expected ActionFailure, got {other:?}"),
        }

        let mut fail = PacketZcAckTouseskill::new(packetver);
        fail.set_skid(SkillEnum::AcDouble.id() as u16);
        fail.set_num(2);
        fail.set_itid(1177);
        fail.set_result(false);
        fail.set_cause(71);
        fail.fill_raw();
        match dispatch_packet(&fail).as_slice() {
            [
                GameEvent::SkillFailed {
                    skill,
                    cause,
                    btype,
                    item_id,
                },
            ] => {
                assert_eq!(
                    (*skill, *cause, *btype, *item_id),
                    (Some(SkillEnum::AcDouble), 71, 2, 1177)
                );
            }
            other => panic!("expected SkillFailed, got {other:?}"),
        }

        let mut no_bullet = PacketZcAckTouseskill::new(packetver);
        no_bullet.set_skid(0);
        no_bullet.set_num(0);
        no_bullet.set_result(false);
        no_bullet.set_cause(84);
        no_bullet.fill_raw();
        match dispatch_packet(&no_bullet).as_slice() {
            [
                GameEvent::SkillFailed {
                    skill,
                    cause,
                    btype,
                    ..
                },
            ] => {
                assert_eq!((*skill, *cause, *btype), (None, 84, 0));
            }
            other => panic!("expected SkillFailed, got {other:?}"),
        }

        let mut npc_chat = PacketZcNpcChat::new(packetver);
        npc_chat.set_account_id(2000000);
        npc_chat.set_color(0x0000_00ff);
        npc_chat.set_msg("Skill Failed.".to_string());
        npc_chat.fill_raw();
        match dispatch_packet(&npc_chat).as_slice() {
            [
                GameEvent::ServerColoredMessage {
                    account_id,
                    color,
                    message,
                },
            ] => {
                assert_eq!((*account_id, *color), (2000000, 0x0000_00ff));
                assert_eq!(message, "Skill Failed.");
            }
            other => panic!("expected ServerColoredMessage, got {other:?}"),
        }
    }

    #[test]
    fn update_mapinfo_reports_the_new_cell_type() {
        let packetver = 20250716;
        let mut pkt = PacketZcUpdateMapinfo::new(packetver);
        pkt.set_x_pos(55);
        pkt.set_y_pos(66);
        pkt.set_atype(5);
        pkt.fill_raw();
        match dispatch_packet(&pkt).as_slice() {
            [GameEvent::MapCellChanged { x, y, cell_type }] => {
                assert_eq!((*x, *y, *cell_type), (55, 66, 5));
            }
            other => panic!("expected MapCellChanged, got {other:?}"),
        }
    }

    #[test]
    fn guild_member_status_reports_online_and_appearance() {
        let packetver = 20250716;
        let mut notify = PacketZcUpdateCharstat2::new(packetver);
        notify.set_aid(2000000);
        notify.set_gid(150000);
        notify.set_status(0);
        notify.set_sex(1);
        notify.set_head(7);
        notify.set_head_palette(3);
        notify.fill_raw();
        match dispatch_packet(&notify).as_slice() {
            [
                GameEvent::GuildMemberOnline {
                    online, appearance, ..
                },
            ] => {
                assert!(!*online);
                let a = appearance.expect("0x01f2 carries the member appearance");
                assert_eq!((a.sex, a.head, a.head_palette), (1, 7, 3));
            }
            other => panic!("expected GuildMemberOnline, got {other:?}"),
        }
    }

    // ZC_NPCACK_SERVERMOVE 0x0ac7: map.16B x.W y.W ip.L port.W domain.128B (empty)
    #[test]
    fn servermove_at_20250716_carries_the_new_zone_address() {
        let packet = raw(
            0x0ac7,
            &[
                &name("prontera.gat", 16),
                &le(150, 2),
                &le(100, 2),
                &le(0x7f00_0001, 4),
                &le(5121, 2),
                &[0; 128],
            ],
        );
        match dispatch_packet(&*packet).as_slice() {
            [GameEvent::ZoneServerChanged { map_name, ip, port }] => {
                assert_eq!(map_name, "prontera.gat");
                assert_eq!((*ip, *port), (0x7f00_0001, 5121));
            }
            other => panic!("expected ZoneServerChanged, got {other:?}"),
        }
    }

    // ZC_WARPLIST 0x0abe: length.W skill.W, then one map.16B per destination, more than the old four
    #[test]
    fn warp_list_at_20250716_holds_every_destination() {
        let maps: Vec<Vec<u8>> = [
            "prontera.gat",
            "geffen.gat",
            "payon.gat",
            "izlude.gat",
            "morocc.gat",
        ]
        .iter()
        .map(|m| name(m, 16))
        .collect();
        let mut parts: Vec<&[u8]> = vec![&[27, 0]];
        parts.extend(maps.iter().map(Vec::as_slice));
        match dispatch_packet(&*raw_var(0x0abe, &parts)).as_slice() {
            [
                GameEvent::WarpList {
                    skill: SkillEnum::AlWarp,
                    destinations,
                },
            ] => assert_eq!(destinations.len(), 5),
            other => panic!("expected WarpList, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_npc_showefst_update_returns_opt3() {
        let packetver = 20250716;
        let mut pkt = PacketZcNpcShowefstUpdate::new(packetver);
        pkt.set_aid(150000);
        pkt.set_effect_state(0x20);
        pkt.set_clevel(88);
        pkt.set_show_efst(nostalre_game::effect::opt3::OPT3_STEELBODY);
        pkt.fill_raw();
        match dispatch_packet(&pkt).as_slice() {
            [
                GameEvent::EntityOpt3Changed {
                    gid,
                    effect_state,
                    base_level,
                    opt3,
                },
            ] => {
                assert_eq!(*gid, 150000);
                assert_eq!(*effect_state, 0x20);
                assert_eq!(*base_level, 88);
                assert_eq!(*opt3, nostalre_game::effect::opt3::OPT3_STEELBODY);
            }
            other => panic!("expected EntityOpt3Changed, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_msg_state_change_routes_buff_and_keeps_postdelay() {
        let packetver = 20250716;

        let mut pkt = PacketZcMsgStateChange2::new(packetver);
        pkt.set_index(192);
        pkt.set_aid(150000);
        pkt.set_state(true);
        pkt.set_remain_ms(60000);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::StatusEffectChanged {
                gid,
                efst,
                active,
                remain_ms,
                ..
            } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*efst, 192);
                assert!(*active);
                assert_eq!(*remain_ms, 60000);
            }
            other => panic!("expected StatusEffectChanged, got {other:?}"),
        }

        let mut pkt = PacketZcMsgStateChange2::new(packetver);
        pkt.set_index(46);
        pkt.set_aid(150000);
        pkt.set_state(true);
        pkt.set_remain_ms(500);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert!(matches!(
            &result[0],
            GameEvent::AfterCastDelay { delay_ms: 500 }
        ));

        let mut pkt = PacketZcMsgStateChange::new(packetver);
        pkt.set_index(192);
        pkt.set_aid(150000);
        pkt.set_state(false);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::StatusEffectChanged {
                gid,
                efst,
                active,
                remain_ms,
                ..
            } => {
                assert_eq!(*gid, 150000);
                assert_eq!(*efst, 192);
                assert!(!*active, "0x196 off-packet must deactivate");
                assert_eq!(*remain_ms, 0);
            }
            other => panic!("expected StatusEffectChanged, got {other:?}"),
        }
    }

    #[test]
    fn party_invite_carries_the_target_account() {
        let packetver = 20250716;

        let invite = crate::sender::build_req_join_party_packet(101, packetver);
        assert_eq!(invite[0], 0xFC);
        let aid = u32::from_le_bytes([invite[2], invite[3], invite[4], invite[5]]);
        assert_eq!(aid, 101);
    }

    #[test]
    fn offline_shop_and_room_titles_keep_portuguese_accents() {
        for title in ["Poções e asas".as_bytes(), b"Po\xe7\xf5es e asas"] {
            let mut fixed = [0u8; 80];
            fixed[..title.len()].copy_from_slice(title);
            let shop = raw(0x0131, &[&le(95_000_001, 4), &fixed]);
            assert!(matches!(&dispatch_packet(shop.as_ref())[..],
                [GameEvent::VendingBoardShown { name, .. }] if name == "Poções e asas"));
            for id in [0x00d7, 0x00df] {
                // rAthena sends room titles without NUL; accept both forms.
                for nul in [false, true] {
                    let mut payload = title.to_vec();
                    if nul { payload.push(0); }
                    let room = raw_var(id, &[&le(95_000_001, 4), &le(7, 4),
                        &le(6, 2), &le(1, 2), &[1], &payload]);
                    assert!(matches!(&dispatch_packet(room.as_ref())[..],
                        [GameEvent::ChatRoomUpsert { title, .. }] if title == "Poções e asas"));
                }
            }
        }
    }

    #[test]
    fn dispatch_room_newentry_returns_chat_room_upsert() {
        let packetver = 20250716;
        let mut pkt = PacketZcRoomNewentry::new(packetver);
        pkt.set_aid(150000);
        pkt.set_room_id(7);
        pkt.set_maxcount(20);
        pkt.set_curcount(3);
        pkt.set_atype(2);
        let title = "Arena Entrance\0";
        pkt.set_title(title.to_string());
        pkt.set_title_raw(title.as_bytes().to_vec());
        pkt.set_packet_length((17 + title.len()) as i16);
        pkt.fill_raw();
        let result = dispatch_packet(&pkt);
        assert_eq!(result.len(), 1);
        match &result[0] {
            GameEvent::ChatRoomUpsert {
                owner_aid,
                room_id,
                max_count,
                cur_count,
                atype,
                title,
            } => {
                assert_eq!(*owner_aid, 150000);
                assert_eq!(*room_id, 7);
                assert_eq!(*max_count, 20);
                assert_eq!(*cur_count, 3);
                assert_eq!(*atype, 2);
                assert_eq!(title, "Arena Entrance");
            }
            other => panic!("expected ChatRoomUpsert, got {other:?}"),
        }
    }

    #[test]
    fn create_chatroom_round_trips_with_ack() {
        let packetver = 20250716;
        let raw =
            crate::sender::build_create_chatroom_packet("Trade", 15, false, "secret", packetver);
        assert_eq!(u16::from_le_bytes([raw[0], raw[1]]), 0x00d5);
        assert_eq!(
            i16::from_le_bytes([raw[2], raw[3]]),
            (15 + "Trade".len()) as i16
        );
        assert_eq!(i16::from_le_bytes([raw[4], raw[5]]), 15); // limit
        assert_eq!(raw[6], 0); // private
        assert_eq!(&raw[7..13], b"secret");
        assert_eq!(&raw[15..], b"Trade"); // title, not null-terminated

        let mut ack = PacketZcAckCreateChatroom::new(packetver);
        ack.set_result(0);
        ack.fill_raw();
        assert!(matches!(
            dispatch_packet(&ack).as_slice(),
            [GameEvent::ChatRoomCreateResult { flag: 0 }]
        ));
    }

    #[test]
    fn enter_room_parses_members_and_member_events() {
        let packetver = 20250716;
        let name24 = |n: &str| {
            let mut b = [0u8; 24];
            b[..n.len()].copy_from_slice(n.as_bytes());
            b
        };
        let mut raw = Vec::new();
        raw.extend_from_slice(&0x00dbu16.to_le_bytes());
        raw.extend_from_slice(&(8 + 28 * 2i16).to_le_bytes());
        raw.extend_from_slice(&42u32.to_le_bytes()); // room id
        raw.extend_from_slice(&0u32.to_le_bytes()); // role owner
        raw.extend_from_slice(&name24("Alice"));
        raw.extend_from_slice(&1u32.to_le_bytes()); // role normal
        raw.extend_from_slice(&name24("Bob"));
        let parsed = packets::packets_parser::parse(&raw, packetver);
        match dispatch_packet(&*parsed).as_slice() {
            [GameEvent::ChatRoomEntered { room_id, members }] => {
                assert_eq!(*room_id, 42);
                assert_eq!(members.len(), 2);
                assert_eq!(members[0].name, "Alice");
                assert!(members[0].is_owner);
                assert!(!members[1].is_owner);
            }
            other => panic!("expected ChatRoomEntered, got {other:?}"),
        }

        let mut exit = PacketZcMemberExit::new(packetver);
        exit.set_curcount(1);
        exit.set_name(name24("Bob").map(|b| b as char));
        exit.set_atype(1); // kicked
        exit.fill_raw();
        assert!(matches!(
            dispatch_packet(&exit).as_slice(),
            [GameEvent::ChatRoomMemberLeft { kicked: true, .. }]
        ));
    }

    #[test]
    fn guild_positioninfo_decodes_rights_bits() {
        let packetver = 20250716;

        let position = |id: i32, right: i32, ranking: i32, pay: i32| {
            let mut buf = Vec::with_capacity(16);
            buf.extend_from_slice(&id.to_le_bytes());
            buf.extend_from_slice(&right.to_le_bytes());
            buf.extend_from_slice(&ranking.to_le_bytes());
            buf.extend_from_slice(&pay.to_le_bytes());
            buf
        };
        // Master rank: invite (0x1) + expel (0x10) + storage (0x100); grunt rank: none.
        let p0 = position(0, 0x111, 0, 50);
        let p1 = position(1, 0x000, 1, 0);

        let mut raw = Vec::new();
        raw.extend_from_slice(&[0x60, 0x01]);
        let len = (4 + p0.len() + p1.len()) as i16;
        raw.extend_from_slice(&len.to_le_bytes());
        raw.extend_from_slice(&p0);
        raw.extend_from_slice(&p1);

        let parsed = packets::packets_parser::parse(&raw, packetver);
        match &dispatch_packet(parsed.as_ref())[..] {
            [GameEvent::GuildPositions { positions }] => {
                assert_eq!(positions.len(), 2);
                assert_eq!(positions[0].id, 0);
                assert_eq!(positions[0].right, 0x111);
                assert_eq!(positions[0].pay_rate, 50);
                assert_eq!(positions[1].right, 0);
            }
            other => panic!("expected GuildPositions, got {other:?}"),
        }
    }

    #[test]
    fn dispatch_pet_hatch_burst_populates_state() {
        use nostalre_game::pet::PetState;
        let packetver = 20250716;
        let pet_gid: u32 = 400123;
        let accessory_view: i32 = 10001;
        let mut pet = PetState::default();

        let apply = |pet: &mut PetState, pkt: &dyn Packet| match &dispatch_packet(pkt)[..] {
            [GameEvent::PetStateChanged { ty, gid, data }] => {
                pet.apply_state_changed(*ty, *gid, *data)
            }
            [GameEvent::PetProperty { property }] => pet.apply_property(property),
            other => panic!("unexpected pet event: {other:?}"),
        };

        let mut init = PacketZcChangestatePet::new(packetver);
        init.set_atype(0);
        init.set_gid(pet_gid as i32);
        init.fill_raw();
        apply(&mut pet, &init);

        let mut marker = PacketZcChangestatePet::new(packetver);
        marker.set_atype(5);
        marker.set_gid(pet_gid as i32);
        marker.set_data(100);
        marker.fill_raw();
        apply(&mut pet, &marker);

        let mut accessory = PacketZcChangestatePet::new(packetver);
        accessory.set_atype(3);
        accessory.set_gid(pet_gid as i32);
        accessory.set_data(accessory_view);
        accessory.fill_raw();
        apply(&mut pet, &accessory);

        let mut prop = PacketZcPropertyPet::new(packetver);
        let mut name = [0 as char; 24];
        for (i, c) in "Poring".chars().enumerate() {
            name[i] = c;
        }
        prop.set_sz_name(name);
        prop.set_b_modified(0);
        prop.set_n_level(1);
        prop.set_n_fullness(80);
        prop.set_n_relationship(920);
        prop.set_itid(accessory_view as u16);
        prop.set_job(1002);
        prop.fill_raw();
        apply(&mut pet, &prop);

        assert_eq!(pet.gid, Some(pet_gid));
        assert_eq!(pet.accessory, accessory_view as u16);
        assert_eq!(pet.name, "Poring");
        assert_eq!(pet.hunger, 80);
        assert_eq!(pet.intimacy, 920);
        assert_eq!(pet.job, 1002);
        assert!(!pet.renamed);
        assert_eq!(
            pet.hunger_state(),
            nostalre_game::pet::HungerState::Satisfied
        );
        assert_eq!(
            pet.intimacy_state(),
            nostalre_game::pet::IntimacyState::Loyal
        );
    }

    #[test]
    fn dispatch_petegg_list_yields_indices() {
        let packetver = 20250716;
        let mut e0 = PeteggitemInfo::new(packetver);
        e0.set_index(7);
        e0.fill_raw();
        let mut e1 = PeteggitemInfo::new(packetver);
        e1.set_index(9);
        e1.fill_raw();
        let mut pkt = PacketZcPeteggList::new(packetver);
        pkt.set_packet_length(4 + 4);
        pkt.set_egg_list(vec![e0, e1]);
        pkt.fill_raw();
        match &dispatch_packet(&pkt)[..] {
            [GameEvent::PetEggList { indices }] => assert_eq!(indices.as_slice(), &[7, 9]),
            other => panic!("expected PetEggList, got {other:?}"),
        }
    }

    // rAthena's clif_quest_*: ZC_ALL_QUEST_LIST3 0x09f8, ZC_ADD_QUEST_EX 0x09f9 then ZC_HUNTING_QUEST_INFO 0x08fe,
    // ZC_UPDATE_MISSION_HUNT_EX 0x09fa, ZC_DEL_QUEST 0x02b4, ZC_ACTIVE_QUEST 0x02b7, ZC_QUEST_NOTIFY_EFFECT 0x0446
    #[test]
    fn quests_at_20250716_build_the_log_and_markers() {
        use nostalre_game::quest::{QuestLog, QuestMarker};
        use std::collections::HashMap;

        let mut log = QuestLog::default();
        let mut markers: HashMap<u32, QuestMarker> = HashMap::new();
        let apply =
            |log: &mut QuestLog, markers: &mut HashMap<u32, QuestMarker>, pkt: Box<dyn Packet>| {
                for ev in dispatch_packet(&*pkt) {
                    match ev {
                        GameEvent::QuestListReceived { quests } => log.replace(quests),
                        GameEvent::QuestAdded { quest } => log.add(quest),
                        GameEvent::QuestRemoved { quest_id } => {
                            log.remove(quest_id);
                        }
                        GameEvent::QuestHuntUpdated { entries } => {
                            entries.into_iter().for_each(|e| log.update_hunt(e))
                        }
                        GameEvent::QuestActiveChanged { quest_id, active } => {
                            log.set_active(quest_id, active)
                        }
                        GameEvent::QuestNpcMarker {
                            npc_id,
                            x,
                            y,
                            effect,
                            color,
                        } if color != 0 => {
                            markers.insert(
                                npc_id,
                                QuestMarker {
                                    x,
                                    y,
                                    effect,
                                    color,
                                },
                            );
                        }
                        GameEvent::QuestNpcMarker { npc_id, .. } => {
                            markers.remove(&npc_id);
                        }
                        other => panic!("unexpected quest event: {other:?}"),
                    }
                }
            };
        // hunt id.L type.L mob id.L min level.W max level.W, then killed.W total.W (the list) or killed.W (an add)
        let objective = |hunt: i64, killed: i64, total: Option<i64>, mob: &str| {
            let mut bytes = [
                le(hunt, 4),
                le(0, 4),
                le(1002, 4),
                le(1, 2),
                le(99, 2),
                le(killed, 2),
            ]
            .concat();
            bytes.extend(total.map(|t| le(t, 2)).unwrap_or_default());
            bytes.extend(name(mob, 24));
            bytes
        };

        // The login list: two objectives that both name a Poring, a second quest inactive and time-limited.
        apply(
            &mut log,
            &mut markers,
            raw_var(
                0x09f8,
                &[
                    &le(2, 4),
                    &le(1000, 4),
                    &[1],
                    &le(0, 4),
                    &le(0, 4),
                    &le(2, 2),
                    &objective(1_000_000, 3, Some(10), "Poring"),
                    &objective(1_000_001, 0, Some(5), "Brute"),
                    &le(1001, 4),
                    &[0],
                    &le(1_699_000_000, 4),
                    &le(1_700_000_000, 4),
                    &le(0, 2),
                ],
            ),
        );
        let quest = log.get(1000).unwrap();
        assert!(quest.active);
        assert_eq!(quest.objectives.len(), 2);
        assert_eq!(
            (
                quest.objectives[0].name.as_str(),
                quest.objectives[0].current,
                quest.objectives[0].required
            ),
            ("Poring", 3, 10)
        );
        assert_eq!(quest.objectives[1].hunt_id, 1_000_001);
        let inactive = log.get(1001).unwrap();
        assert_eq!(
            (inactive.active, inactive.end_time),
            (false, Some(1_700_000_000))
        );

        // An added quest: 143 bytes whatever its objectives (three slots), the totals in 0x08fe after it.
        apply(
            &mut log,
            &mut markers,
            raw(
                0x09f9,
                &[
                    &le(2000, 4),
                    &[1],
                    &le(0, 4),
                    &le(0, 4),
                    &le(1, 2),
                    &objective(2_000_000, 0, None, "Drops"),
                    &[0; 84],
                ],
            ),
        );
        assert_eq!(log.get(2000).unwrap().objectives[0].required, 0);
        apply(
            &mut log,
            &mut markers,
            raw_var(
                0x08fe,
                &[&le(2_000_000, 4), &le(1113, 4), &le(7, 2), &le(0, 2)],
            ),
        );
        assert_eq!(log.get(2000).unwrap().objectives[0].required, 7);

        // A kill: quest id.L hunt id.L total.W killed.W, matched by the hunt id.
        apply(
            &mut log,
            &mut markers,
            raw_var(
                0x09fa,
                &[
                    &le(1, 2),
                    &le(1000, 4),
                    &le(1_000_001, 4),
                    &le(5, 2),
                    &le(2, 2),
                ],
            ),
        );
        let quest = log.get(1000).unwrap();
        assert_eq!(
            (quest.objectives[0].current, quest.objectives[1].current),
            (3, 2)
        );

        apply(&mut log, &mut markers, raw(0x02b4, &[&le(1000, 4)]));
        assert!(log.get(1000).is_none());
        apply(&mut log, &mut markers, raw(0x02b7, &[&le(2000, 4), &[0]]));
        assert!(!log.get(2000).unwrap().active);

        // A marker: npc.L x.W y.W effect.W color.W; color 0 (QMARK_NONE) takes it off.
        apply(
            &mut log,
            &mut markers,
            raw(
                0x0446,
                &[&le(555, 4), &le(30, 2), &le(40, 2), &le(0, 2), &le(1, 2)],
            ),
        );
        assert_eq!(markers.get(&555).map(|m| m.color), Some(1));
        apply(
            &mut log,
            &mut markers,
            raw(
                0x0446,
                &[&le(555, 4), &le(30, 2), &le(40, 2), &le(0, 2), &le(0, 2)],
            ),
        );
        assert!(markers.get(&555).is_none());
    }

    #[test]
    fn marriage_packets_carry_names_effect_and_trim_nulls() {
        let packetver = 20250716;
        let name24 = |n: &str| {
            let mut b = [0u8; 24];
            b[..n.len()].copy_from_slice(n.as_bytes());
            b.map(|c| c as char)
        };

        let mut couple = PacketZcCouplename::new(packetver);
        couple.set_couple_name(name24("Juliet"));
        couple.fill_raw();
        assert!(matches!(
            dispatch_packet(&couple).as_slice(),
            [GameEvent::CoupleNameReceived { name }] if name == "Juliet"
        ));

        let mut congrats = PacketZcCongratulation::new(packetver);
        congrats.set_aid(654321);
        congrats.fill_raw();
        assert!(matches!(
            dispatch_packet(&congrats).as_slice(),
            [GameEvent::WeddingCelebration { account_id: 654321 }]
        ));

        let mut divorce = PacketZcDivorce::new(packetver);
        divorce.set_name(name24("Romeo"));
        divorce.fill_raw();
        assert!(matches!(
            dispatch_packet(&divorce).as_slice(),
            [GameEvent::Divorced { name }] if name == "Romeo"
        ));
    }

    #[test]
    fn marriage_proposal_arms_cursor_then_replies_to_the_proposer() {
        let packetver = 20120307;

        let mut start = PacketZcStartCouple::new(packetver);
        start.fill_raw();
        assert!(matches!(
            dispatch_packet(&start).as_slice(),
            [GameEvent::MarriageProposalArmed]
        ));

        // 0x1e5 <target aid>.L — the click that picks the partner.
        let request = crate::sender::build_marry_request_packet(4001, packetver);
        assert_eq!(request.len(), 6);
        assert_eq!(u16::from_le_bytes([request[0], request[1]]), 0x01e5);
        assert_eq!(
            u32::from_le_bytes([request[2], request[3], request[4], request[5]]),
            4001
        );

        let mut proposal = PacketZcReqCouple::new(packetver);
        proposal.set_aid(111);
        proposal.set_gid(222);
        let mut name = [0u8; 24];
        name[.."Romeo".len()].copy_from_slice(b"Romeo");
        proposal.set_name(name.map(|c| c as char));
        proposal.fill_raw();
        let (aid, gid) = match &dispatch_packet(&proposal)[..] {
            [
                GameEvent::MarriageProposed {
                    proposer_aid,
                    proposer_gid,
                    name,
                },
            ] => {
                assert_eq!(name, "Romeo");
                (*proposer_aid, *proposer_gid)
            }
            other => panic!("expected MarriageProposed, got {other:?}"),
        };

        // 0x1e3 <proposer aid>.L <proposer gid>.L <answer>.L
        let reply = crate::sender::build_marry_reply_packet(aid, gid, true, packetver);
        assert_eq!(reply.len(), 14);
        assert_eq!(u16::from_le_bytes([reply[0], reply[1]]), 0x01e3);
        assert_eq!(
            u32::from_le_bytes([reply[2], reply[3], reply[4], reply[5]]),
            111
        );
        assert_eq!(
            u32::from_le_bytes([reply[6], reply[7], reply[8], reply[9]]),
            222
        );
        assert_eq!(
            i32::from_le_bytes([reply[10], reply[11], reply[12], reply[13]]),
            1
        );
    }

    #[test]
    fn friend_list_starts_offline_until_state_packet_arrives() {
        use nostalre_game::friends::{Friend, FriendList};

        let packetver = 20250716;
        let name24 = |n: &str| {
            let mut b = [0u8; 24];
            b[..n.len()].copy_from_slice(n.as_bytes());
            b.map(|c| c as char)
        };
        let entry = |aid: u32, gid: u32, n: &str| {
            let mut f = FRIEND::new(packetver);
            f.set_aid(aid);
            f.set_gid(gid);
            f.set_name(name24(n));
            f
        };

        let mut list = PacketZcFriendsList::new(packetver);
        list.set_friend_list(vec![entry(1, 10, "Alice"), entry(2, 20, "Bob")]);
        list.fill_raw();

        let mut friends = FriendList::default();
        match dispatch_packet(&list).as_slice() {
            [GameEvent::FriendListReceived { friends: received }] => friends.set_all(
                received
                    .iter()
                    .map(|f| Friend {
                        aid: f.aid,
                        gid: f.gid,
                        name: f.name.clone(),
                        online: f.online,
                    })
                    .collect(),
            ),
            other => panic!("unexpected events: {other:?}"),
        }
        assert!(friends.friends.iter().all(|f| !f.online));

        let mut state = PacketZcFriendsState::new(packetver);
        state.set_aid(2);
        state.set_gid(20);
        state.set_state(false);
        state.fill_raw();
        match dispatch_packet(&state).as_slice() {
            [GameEvent::FriendStateChanged { aid, gid, online }] => {
                friends.set_state(*aid, *gid, *online)
            }
            other => panic!("unexpected events: {other:?}"),
        }
        assert!(!friends.friends[0].online);
        assert!(friends.friends[1].online);
    }

    /// A packet as the 20250716 server writes it: the id, then each part as is (`le` numbers, `name` strings).
    fn raw(id: u16, parts: &[&[u8]]) -> Box<dyn Packet> {
        let mut bytes = id.to_le_bytes().to_vec();
        parts.iter().for_each(|p| bytes.extend_from_slice(p));
        packets::packets_parser::parse(&bytes, 20250716)
    }

    /// A variable packet: the id, its length, then the parts.
    fn raw_var(id: u16, parts: &[&[u8]]) -> Box<dyn Packet> {
        let len = 4 + parts.iter().map(|p| p.len()).sum::<usize>();
        let mut with_len = vec![(len as u16).to_le_bytes().to_vec()];
        with_len.extend(parts.iter().map(|p| p.to_vec()));
        raw(id, &with_len.iter().map(Vec::as_slice).collect::<Vec<_>>())
    }

    fn le(value: i64, width: usize) -> Vec<u8> {
        value.to_le_bytes()[..width].to_vec()
    }

    /// A NUL-padded string field of `width` bytes.
    fn name(s: &str, width: usize) -> Vec<u8> {
        let mut bytes = s.as_bytes().to_vec();
        bytes.resize(width, 0);
        bytes
    }

    // ZC_WHISPER 0x09de: length.W senderGID.L sender.24B isAdmin.B message; ZC_ACK_WHISPER 0x09df: result.B CID.L
    #[test]
    fn whisper_at_20250716_carries_the_sender_id() {
        let whisper = raw_var(
            0x09de,
            &[&le(2_000_001, 4), &name("Alice", 24), &[1], b"hi there\0"],
        );
        assert!(matches!(
            &dispatch_packet(&*whisper)[..],
            [GameEvent::WhisperReceived { sender, message }]
                if sender == "Alice" && message == "hi there"
        ));
        assert!(matches!(
            dispatch_packet(&*raw(0x09df, &[&[1], &le(150_001, 4)]))[..],
            [GameEvent::WhisperAck { result: 1 }]
        ));
    }

    // ZC_GROUP_LIST 0x0ae5: length.W name.24B, members of AID.L GID.L name.24B map.16B leader.B offline.B class.W
    // level.W (0 is the leader and online)
    #[test]
    fn party_list_at_20250716_has_54_bytes_per_member() {
        let member = |aid, gid, nick, leader| {
            [
                le(aid, 4),
                le(gid, 4),
                name(nick, 24),
                name("prontera.gat", 16),
                vec![leader, 0],
                le(4252, 2),
                le(250, 2),
            ]
            .concat()
        };
        let list = raw_var(
            0x0ae5,
            &[
                &name("Heroes", 24),
                &member(2_000_001, 150_001, "Alice", 0),
                &member(2_000_002, 150_002, "Bob", 1),
            ],
        );
        match &dispatch_packet(&*list)[..] {
            [GameEvent::PartyMemberList { name, members }] => {
                assert_eq!(name, "Heroes");
                assert!(
                    members
                        .iter()
                        .all(|m| (m.class, m.base_level) == (4252, 250))
                );
                let m: Vec<_> = members
                    .iter()
                    .map(|m| (m.aid, m.name.as_str(), m.map.as_str(), m.leader, m.online))
                    .collect();
                assert_eq!(
                    m,
                    [
                        (2_000_001, "Alice", "prontera.gat", true, true),
                        (2_000_002, "Bob", "prontera.gat", false, true)
                    ]
                );
            }
            other => panic!("expected PartyMemberList, got {other:?}"),
        }
    }

    // ZC_ADD_MEMBER_TO_GROUP 0x0ae4 (89): AID.L GID.L leader.L class.W level.W x.W y.W offline.B party.24B
    // name.24B map.16B pickup.B share.B
    #[test]
    fn party_member_at_20250716() {
        let member = raw(
            0x0ae4,
            &[
                &le(2_000_002, 4),
                &le(150_002, 4),
                &le(1, 4),
                &le(4008, 2),
                &le(99, 2),
                &le(150, 2),
                &le(180, 2),
                &[0],
                &name("Heroes", 24),
                &name("Bob", 24),
                &name("prontera.gat", 16),
                &[1, 0],
            ],
        );
        assert!(matches!(
            &dispatch_packet(&*member)[..],
            [GameEvent::PartyMemberAdded { aid: 2_000_002, name, map, leader: false, online: true, x: 150, y: 180,
                class: 4008, base_level: 99 }]
                if name == "Bob" && map == "prontera.gat"
        ));
    }

    // ZC_MEMBERMGR_INFO 0x0b7d: length.W, members of AID.L GID.L head.W palette.W sex.W job.W level.W exp.L
    // state.L position.L lastLogin.L name.24B (62 bytes)
    #[test]
    fn guild_members_at_20250716_carry_names_and_last_login() {
        let member = |aid, gid, nick, job, state, position| {
            [
                le(aid, 4),
                le(gid, 4),
                le(0, 2),
                le(0, 2),
                le(1, 2),
                le(job, 2),
                le(99, 2),
                le(500, 4),
                le(state, 4),
                le(position, 4),
                le(1_760_000_000, 4),
                name(nick, 24),
            ]
            .concat()
        };
        let list = raw_var(
            0x0b7d,
            &[
                &member(101, 201, "Master", 4008, 1, 0),
                &member(102, 202, "Grunt", 1, 0, 2),
            ],
        );
        match &dispatch_packet(&*list)[..] {
            [GameEvent::GuildMembers { members }] => {
                assert_eq!(members.len(), 2);
                let m = &members[0];
                assert_eq!(
                    (m.aid, m.gid, m.name.as_str(), m.job),
                    (101, 201, "Master", 4008)
                );
                assert!(m.online && m.position_id == 0 && m.last_login == 1_760_000_000);
                assert_eq!((members[1].gid, members[1].name.as_str()), (202, "Grunt"));
                assert!(!members[1].online && members[1].position_id == 2);
            }
            other => panic!("expected GuildMembers, got {other:?}"),
        }
    }

    // ZC_BAN_LIST 0x0b7c: length.W, entries of char_id.L message.40B name.24B
    #[test]
    fn guild_ban_list_at_20250716_names_after_the_reason() {
        let entry = |id, nick, reason| [le(id, 4), name(reason, 40), name(nick, 24)].concat();
        let list = raw_var(
            0x0b7c,
            &[
                &entry(150_001, "Traitor", "Left mid-WoE"),
                &entry(150_002, "Spy", "Enemy alt"),
            ],
        );
        match &dispatch_packet(&*list)[..] {
            [GameEvent::GuildBanList { entries }] => {
                let e: Vec<_> = entries
                    .iter()
                    .map(|e| (e.char_name.as_str(), e.reason.as_str()))
                    .collect();
                assert_eq!(e, [("Traitor", "Left mid-WoE"), ("Spy", "Enemy alt")]);
            }
            other => panic!("expected GuildBanList, got {other:?}"),
        }
    }

    // ZC_GUILD_INFO 0x0b7b (118): GDID.L level.L members.L max.L average.L exp.L maxExp.L point.L honor.L
    // virtue.L emblem.L name.24B land.16B zeny.L masterGID.L masterName.24B
    #[test]
    fn guild_info_at_20250716_names_the_master_last() {
        let numbers: Vec<u8> = [7, 3, 2, 16, 88, 1000, 5000, 0, 0, 0, 4]
            .iter()
            .flat_map(|&v| le(v, 4))
            .collect();
        let info = raw(
            0x0b7b,
            &[
                &numbers,
                &name("Heroes", 24),
                &name("None Taken", 16),
                &le(0, 4),
                &le(150_001, 4),
                &name("Master", 24),
            ],
        );
        assert!(matches!(
            &dispatch_packet(&*info)[..],
            [GameEvent::GuildInfo { gdid: 7, level: 3, member_num: 2, max_member_num: 16, avg_level: 88,
                exp: 1000, max_exp: 5000, emblem_version: 4, name, master_name, manage_land, .. }]
                if name == "Heroes" && master_name == "Master" && manage_land == "None Taken"
        ));
    }

    // ZC_ACK_LEAVE_GUILD 0x0a83: GID.L reason.40B; ZC_ACK_BAN_GUILD 0x0a82: reason.40B GID.L. No names
    #[test]
    fn guild_leave_and_expulsion_at_20250716_carry_the_character_id() {
        assert!(matches!(
            &dispatch_packet(&*raw(0x0a83, &[&le(150_002, 4), &name("bye", 40)]))[..],
            [GameEvent::GuildMemberLeft { gid: 150_002, reason }] if reason == "bye"
        ));
        assert!(matches!(
            &dispatch_packet(&*raw(0x0a82, &[&name("spy", 40), &le(150_003, 4)]))[..],
            [GameEvent::GuildMemberExpelled { gid: 150_003, reason }] if reason == "spy"
        ));
    }

    // ZC_CHANGE_GUILD 0x0b47: guild_id.L emblem_id.L AID.L (the unit last, a 32-bit emblem version)
    #[test]
    fn unit_guild_at_20250716_names_the_unit_last() {
        assert!(matches!(
            dispatch_packet(&*raw(
                0x0b47,
                &[&le(7, 4), &le(70_000, 4), &le(2_000_001, 4)]
            ))[..],
            [GameEvent::EntityGuildChanged {
                aid: 2_000_001,
                gdid: 7,
                emblem_version: 70_000
            }]
        ));
    }

    // ZC_UPDATE_GDID 0x02f7 (47): guildId.L emblem.L mode.L isMaster.B interSid.L name.24B masterGID.L
    #[test]
    fn guild_belonging_at_20250716() {
        let packet = raw(
            0x02f7,
            &[
                &le(7, 4),
                &le(4, 4),
                &le(0x11, 4),
                &[1],
                &le(0, 4),
                &name("Heroes", 24),
                &le(150_001, 4),
            ],
        );
        assert!(matches!(
            &dispatch_packet(&*packet)[..],
            [GameEvent::GuildIdentityUpdated { gdid: 7, emblem_version: 4, right: 0x11, is_master: true, name }]
                if name == "Heroes"
        ));
    }

    // ZC_GUILD_POSITION 0x0afd: length.W AID.L position (empty when the unit has none)
    #[test]
    fn guild_position_title_at_20250716() {
        let packet = raw_var(0x0afd, &[&le(2_000_001, 4), &name("Captain", 24)]);
        assert!(matches!(
            &dispatch_packet(&*packet)[..],
            [GameEvent::EntityGuildPositionChanged { aid: 2_000_001, position }] if position == "Captain"
        ));
    }

    // ZC_PROPERTY_HOMUN 0x0ba4 (85): name.24B flags.B level.W hunger.W intimacy.W atk.W matk.W hit.W crit.W def.W
    // mdef.W flee.W amotion.W hp.L maxHp.L sp.L maxSp.L exp.Q expNext.Q skillPoints.W range.W; no accessory
    #[test]
    fn homunculus_at_20250716_has_wide_sp_and_experience() {
        let stats: Vec<u8> = [12, 34, 56, 7, 8, 9, 10, 450]
            .iter()
            .flat_map(|&v| le(v, 2))
            .collect();
        let packet = raw(
            0x0ba4,
            &[
                &name("Lif", 24),
                &[0b01],
                &le(150, 2),
                &le(80, 2),
                &le(900, 2),
                &stats,
                &le(80_000, 4),
                &le(90_000, 4),
                &le(70_000, 4),
                &le(75_000, 4),
                &le(5_000_000_000, 8),
                &le(6_000_000_000, 8),
                &le(3, 2),
                &le(1, 2),
            ],
        );
        match &dispatch_packet(&*packet)[..] {
            [GameEvent::HomunPropertyReceived { property: h }] => {
                assert_eq!(
                    (h.name.as_str(), h.renamed, h.vaporized),
                    ("Lif", true, false)
                );
                assert_eq!((h.level, h.hunger, h.intimacy), (150, 80, 900));
                assert_eq!((h.atk, h.matk, h.aspd), (12, 34, 450));
                assert_eq!(
                    (h.hp, h.max_hp, h.sp, h.max_sp),
                    (80_000, 90_000, 70_000, 75_000)
                );
                assert_eq!((h.exp, h.max_exp), (5_000_000_000, 6_000_000_000));
                assert_eq!((h.skill_points, h.atk_range, h.accessory), (3, 1, 0));
            }
            other => panic!("expected HomunPropertyReceived, got {other:?}"),
        }
    }

    // ZC_HO_PAR_CHANGE 0x0ba5: type.W value.Q (SP_BASEEXP 1)
    #[test]
    fn homunculus_parameter_at_20250716_is_64_bit() {
        assert!(matches!(
            dispatch_packet(&*raw(0x0ba5, &[&le(1, 2), &le(5_000_000_000, 8)]))[..],
            [GameEvent::HomunParamChanged {
                var: 1,
                value: 5_000_000_000
            }]
        ));
    }

    // ZC_NOTIFY_UNREADMAIL 0x09e7: result.B, RODEX's icon at login
    #[test]
    fn unread_mail_notice_is_acknowledged() {
        assert!(matches!(
            dispatch_packet(&*raw(0x09e7, &[&[1]]))[..],
            [GameEvent::Acknowledged]
        ));
    }

    // ZC_ITEM_PICKUP_PARTY 0x0b67 (33): AID.L itemId.L identified.B damaged.B cards.16B location.W type.B refine.B
    // grade.B (the refine moved after the type)
    #[test]
    fn party_pickup_at_20250716_has_the_refine_last() {
        let packet = raw(
            0x0b67,
            &[
                &le(2_000_001, 4),
                &le(500_067, 4),
                &[1, 0],
                &[0; 16],
                &le(2, 2),
                &[5, 7, 1],
            ],
        );
        assert!(matches!(
            dispatch_packet(&*packet)[..],
            [GameEvent::PartyItemPickup {
                aid: 2_000_001,
                item_id: 500_067,
                identified: true,
                refine: 7
            }]
        ));
    }

    // ZC_GROUP_ISALIVE 0x0ab2: AID.L isDead.B; ZC_NOTIFY_MEMBERINFO_TO_GROUPM 0x0abd: AID.L job.W level.W
    #[test]
    fn party_member_death_and_job_at_20250716() {
        assert!(matches!(
            dispatch_packet(&*raw(0x0ab2, &[&le(2_000_001, 4), &[1]]))[..],
            [GameEvent::PartyMemberDead {
                aid: 2_000_001,
                dead: true
            }]
        ));
        assert!(matches!(
            dispatch_packet(&*raw(
                0x0abd,
                &[&le(2_000_001, 4), &le(4252, 2), &le(200, 2)]
            ))[..],
            [GameEvent::PartyMemberJob {
                aid: 2_000_001,
                class: 4252,
                base_level: 200
            }]
        ));
    }
}

#[cfg(test)]
mod campaign_quest_ui_tests {
 use super::*;
 #[test]
 fn server_quest_ui_opens_the_journal_or_the_requested_entry() {
  let mut packet=PacketZcUiOpen::new(crate::rathena_table::PACKETVER);
  packet.set_uitype(6);packet.set_data(0);packet.fill_raw();
  assert!(matches!(dispatch_packet(&packet).as_slice(),[GameEvent::ShowQuestWindow]));
  packet.set_data(900004);packet.fill_raw();
  assert!(matches!(dispatch_packet(&packet).as_slice(),[GameEvent::OpenQuestDetail {quest_id:900004}]));
 }
}
