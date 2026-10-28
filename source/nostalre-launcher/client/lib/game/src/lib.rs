pub mod ailment;
pub mod animation;
pub mod app_state;
pub mod arrow;
pub mod autocounter;
pub mod banner;
pub mod book;
pub mod boss_info;
pub mod cast_scope;
pub mod char_name;
pub mod character;
pub mod chat_command;
pub mod chat_repeat;
pub mod chat_room;
pub mod companion;
pub mod cooldown;
pub mod cursor;
pub mod damage_number;
pub mod damage_skin;
pub mod day_night;
pub mod display;
pub mod display_name;
pub mod doridori;
pub use nostalre_effects as effect;
pub mod effects;
pub mod emotion;
pub mod gm;
pub mod gr2_model;
pub mod graffiti;
pub mod inventory;
pub mod item;
#[rustfmt::skip] // generated (scripts/item-tables.py)
pub mod item_tables;
#[rustfmt::skip] // generated (scripts/job-tables.py)
pub mod job_tables;
pub mod skill_tree;
#[rustfmt::skip] // generated (scripts/skill-tree.py)
pub mod skill_tree_tables;
pub use cursor::RenderEntry;
pub mod data_table;
pub mod entity;
pub mod entity_collection;
pub mod equip_switch;
pub mod equipment_properties;
pub mod event;
pub mod floor_item;
pub mod friends;
pub mod guild;
pub mod hotkey;
pub mod keybinding;
pub mod level_aura;
pub mod lightmap;
pub mod map_title;
pub mod mail;
pub mod map_cloud;
pub mod map_loader;
pub mod minimap_mark;
pub mod mob_info;
pub mod monster_info;
pub mod movement;
pub mod navigation;
pub mod npc_dialog;
pub mod npc_shop;
pub mod npc_text;
pub mod party;
pub mod path;
pub mod pet;
pub mod pet_tables;
pub mod pk_rank;
pub mod notice;
pub mod poptip;
pub mod prayer;
pub mod progress_bar;
pub mod quest;
pub mod quest_guidance;
pub mod campaign_data;
pub mod refine;
pub mod scheduled_hit;
pub mod server_time;
pub mod shadow;
pub mod sign_board;
pub mod special_popup;
pub mod sign_board_text;
pub mod show_digit;
pub mod skill;
pub mod skill_action;
pub mod skill_msg;
pub mod sound;
pub mod sprite_loader;
pub mod sprite_path;
pub mod star_gladiator;
pub mod status_icon;
pub mod stylist;
pub mod title;
pub mod targeting;
pub mod trade;

/// Lookup key for a map: the bare name, lowercased. Map names reach us in three
/// forms — `prontera` from the world map tables, `prontera.rsw` from the data
/// tables, `prontera.gat` from the party and guild packets — and all three must
/// resolve to the same map.
pub fn map_key(name: &str) -> String {
    let lower = name.trim().to_lowercase();
    match lower.strip_suffix(".gat").or(lower.strip_suffix(".rsw")) {
        Some(bare) => bare.to_string(),
        None => lower,
    }
}

/// Whether two names are one map as [`map_key`] keys them (map names are ASCII), building neither key: for checks
/// made every frame
pub fn same_map(a: &str, b: &str) -> bool {
    fn bare(name: &str) -> &str {
        let name = name.trim();
        [".gat", ".rsw"]
            .iter()
            .find_map(|suffix| {
                let at = name.len().checked_sub(suffix.len())?;
                name.get(at..)
                    .filter(|end| end.eq_ignore_ascii_case(suffix))
                    .and_then(|_| name.get(..at))
            })
            .unwrap_or(name)
    }
    bare(a).eq_ignore_ascii_case(bare(b))
}

pub mod npc_localization;
