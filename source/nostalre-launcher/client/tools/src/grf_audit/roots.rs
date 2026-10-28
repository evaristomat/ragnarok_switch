//! Everything the client can name without being told by another file: the
//! registry constants, plus a path per row of every data table it reads.

use super::{Need, Options, Origin, normalize, server::ServerData};
use models::enums::EnumWithNumberValue;
use models::enums::class::JobName;
use models::enums::skill_enums::SkillEnum;
use nostalre_effects::spec::{EffectAnchor, EffectSpec};
use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_table;
use nostalre_game::data_table::accessory_table::AccessoryTable;
use nostalre_game::data_table::item_resource_table::ItemResourceTable;
use nostalre_game::data_table::name_table::NameTable;
use nostalre_game::data_table::quest_display_table::QuestDisplayTable;
use nostalre_game::data_table::robe_table::RobeTable;
use nostalre_game::gr2_model as gr2;
use nostalre_game::sound::tables;
use nostalre_game::sprite_path as sp;
use nostalre_resources as res;

/// Job ids run to 6046 (mercenaries); scan a little past that.
const MAX_JOB_ID: u16 = 6100;
/// Shield and weapon view ids are small, dense and unlisted.
const MAX_VIEW_ID: u16 = 32;
/// Headgear view ids as sent in the look packets.
const MAX_HEADGEAR_VIEW_ID: u16 = 2000;

/// Highest EFST the status icon table has a row for; scan a little past it.
const MAX_EFST_ID: i16 = 800;

pub fn has_identity_tables(grf: &GrfArchive) -> bool {
    [
        res::lua::JOB_IDENTITY_514_LUB,
        res::lua::NPC_IDENTITY_514_LUB,
        res::lua::JOB_IDENTITY_LUB,
        res::lua::NPC_IDENTITY_LUB,
        res::lua::JOB_IDENTITY_LUA,
        res::lua::NPC_IDENTITY_LUA,
    ]
    .iter()
    .any(|p| grf.file_exists(p))
}

pub fn collect(
    grf: &GrfArchive,
    opts: &Options,
    server: Option<&ServerData>,
) -> Vec<(String, Origin, Need)> {
    let mut out: Vec<(String, Origin, Need)> = Vec::new();
    macro_rules! need {
        ($path:expr, $origin:expr $(,)?) => {
            out.push((normalize(&$path), $origin, Need::Required))
        };
    }
    macro_rules! probe {
        ($path:expr, $origin:expr $(,)?) => {
            out.push((normalize(&$path), $origin, Need::Probed))
        };
    }

    // Some registry entries are alternates rather than requirements: `.lua` and
    // `.lub` are two encodings of one table, `num2*` is the pre-rename spelling
    // of `idnum2*`, and pet artwork is cosmetic. The archive path is not an
    // entry at all.
    let alternates: &[&str] = &[
        res::table::UNIDENTIFIED_ITEM_DESC,
        res::table::UNIDENTIFIED_ITEM_NAME,
        res::table::UNIDENTIFIED_ITEM_RESOURCE,
    ];
    for path in res::all_static_paths() {
        if res::grf::ALL.contains(&path) {
            continue;
        }
        if res::lua::ALL.contains(&path)
            || alternates.contains(&path)
            || path.starts_with("data/texture/유저인터페이스/illust/")
        {
            probe!(path.to_string(), Origin::Registry);
        } else {
            need!(path.to_string(), Origin::Registry);
        }
    }

    // -- effects ------------------------------------------------------------
    for path in nostalre_effects::effect_texture_paths() {
        need!(path, Origin::Table("effect texture table"));
    }
    for path in nostalre_effects::effect_spr_paths() {
        need!(path.to_string(), Origin::Table("effect sprite table"));
    }
    for path in nostalre_effects::custom_effect_sprite_paths() {
        need!(path.to_string(), Origin::Table("effect sprite table"));
    }
    // An STR effect loads the first of its names the archive has (`StrEffects::load`), the rest probed; a custom
    // effect loads the one STR it lays over itself (`Effect::str_overlay`). Other effects' names are never loaded.
    for aliases in nostalre_effects::effect_str_names() {
        let paths: Vec<String> = aliases
            .iter()
            .map(|a| res::texture::effect::str_file(a))
            .collect();
        let path = paths
            .iter()
            .find(|p| grf.file_exists(p))
            .unwrap_or(&paths[0]);
        need!(path.clone(), Origin::Table("effect str alias table"));
    }
    for effect in models::enums::effect_id::EffectId::ALL {
        if !matches!(
            nostalre_effects::effect_spec(effect),
            Some(EffectSpec::Custom)
        ) {
            continue;
        }
        let point = EffectAnchor::Point([0.0; 3]);
        if let Some(name) = nostalre_effects::make_effect(effect, point, None, None, None)
            .and_then(|e| e.str_overlay())
        {
            need!(
                res::texture::effect::str_file(name),
                Origin::Table("custom effect str overlay"),
            );
        }
    }

    // -- maps ---------------------------------------------------------------
    // Without a server list every map in the archive is a root, so nothing map
    // shaped is ever called unused.
    let maps: Vec<String> = match server {
        Some(s) => s.maps.clone(),
        None => grf
            .layered_file_list()
            .iter()
            .filter(|f| f.name.to_lowercase().ends_with(".rsw"))
            .filter_map(|f| {
                let n = normalize(&f.name);
                let stem = n.strip_prefix("data/")?.strip_suffix(".rsw")?;
                (!stem.contains('/')).then(|| stem.to_string())
            })
            .collect(),
    };
    let map_origin = if server.is_some() {
        Origin::Table("server map list")
    } else {
        Origin::Table("archive map list")
    };
    for map in &maps {
        need!(res::map::rsw(map), map_origin.clone());
        need!(res::map::gnd(map), map_origin.clone());
        need!(res::map::gat(map), map_origin.clone());
        // Plenty of maps ship no minimap; the window just draws nothing.
        probe!(res::ui::minimap::of(map), map_origin.clone());
    }
    // -- items --------------------------------------------------------------
    let items = ItemResourceTable::load(grf);
    for id in 0..=u16::MAX {
        for identified in [true, false] {
            if let Some(name) = items.get_resource_name_for(id.into(), identified) {
                need!(
                    res::ui::item::icon(name),
                    Origin::Table("item resource table")
                );
                probe!(
                    res::sprite::item::of(name),
                    Origin::Table("item resource table")
                );
                probe!(
                    res::ui::collection::named(name),
                    Origin::Table("item resource table"),
                );
            }
        }
    }

    let quest_entries = lua_table::parse_questid2display(
        &grf.read_file(res::table::QUEST_DISPLAY).unwrap_or_default(),
    );
    let quest_ids: Vec<u32> = quest_entries.keys().copied().collect();
    let quests = QuestDisplayTable::from_entries(quest_entries);
    for id in quest_ids {
        need!(
            quests.icon_texture(id),
            Origin::Table("quest display table")
        );
        need!(
            quests.image_texture(id),
            Origin::Table("quest display table")
        );
    }
    // An id the table has no row for falls back to the default artwork, which
    // the server can put on screen for any quest it sends.
    need!(
        quests.icon_texture(u32::MAX),
        Origin::Table("quest display table")
    );
    need!(
        quests.image_texture(u32::MAX),
        Origin::Table("quest display table")
    );

    for skill in SkillEnum::ALL {
        probe!(
            nostalre_game::skill::skill_icon_path(skill),
            Origin::Table("skill list")
        );
    }

    for efst in 0..=MAX_EFST_ID {
        if let Some(info) = nostalre_game::status_icon::status_icon_info(efst) {
            need!(
                res::texture::effect::named(info.icon),
                Origin::Table("status icon table")
            );
        }
    }

    // -- actors -------------------------------------------------------------
    // the names the game draws by (`NameTable::load_from_grf`): every job below MAX_JOB_ID, and the NPCs and
    // monsters past it (kRO's newer ids run into the 20000s)
    let names = NameTable::load_from_grf(grf);
    let mut later: Vec<u16> = names.ids().filter(|&id| id >= MAX_JOB_ID).collect();
    later.sort_unstable();
    for job in (0..MAX_JOB_ID).chain(later) {
        if let Some(base) = sp::entity_sprite_base_path(&names, job) {
            need!(base, Origin::Table("job/npc identity table"));
        }
        if let Some(name) = names.get_name(job)
            && gr2::is_gr2_name(name)
        {
            need!(
                gr2::gr2_model_path(name),
                Origin::Table("job/npc identity table")
            );
            if let Some(bone) = gr2::bone_type_from_name(name) {
                for action in gr2::Gr2Action::ALL {
                    if let Some(path) = gr2::animation_file_path(bone, action) {
                        need!(path, Origin::Table("job/npc identity table"));
                    }
                }
            }
        }
        // Mercenaries carry a weapon sprite and an imf keyed off their body name.
        if (sp::MERCENARY_JOB_MIN..=sp::MERCENARY_JOB_MAX).contains(&job)
            && let Some(name) = names.get_name(job)
        {
            let name = name.to_string();
            if let Some(imf) = sp::mercenary_imf_path(&name) {
                probe!(imf, Origin::Table("job/npc identity table"));
            }
            if let Some(weapon) = sp::mercenary_weapon_sprite_path(&name) {
                probe!(weapon, Origin::Table("job/npc identity table"));
            }
        }
    }

    // -- players ------------------------------------------------------------
    // Every job rAthena sends and every body the client draws for one (mounts,
    // costumes, suits), each with its own sprite, palette set and imf.
    let mut jobs: Vec<u16> = (0..MAX_JOB_ID)
        .filter(|j| JobName::try_from_value(*j as usize).is_ok())
        .chain(sp::drawn_jobs())
        .collect();
    jobs.sort_unstable();
    jobs.dedup();

    for &job in &jobs {
        for sex in 0u8..2 {
            probe!(sp::body_sprite_path(job, sex), Origin::Table("job list"));
            probe!(sp::imf_path(job, sex), Origin::Table("job list"));
            for palette in 0..opts.max_palette_id {
                probe!(
                    sp::body_palette_path(job, sex, palette),
                    Origin::Table("job list"),
                );
            }
            for view in 0..MAX_VIEW_ID {
                if let Some(weapon) = sp::weapon_view_id_to_type(view) {
                    probe!(
                        sp::weapon_sprite_path(job, sex, weapon),
                        Origin::Table("job list"),
                    );
                }
                if let Some(path) = sp::shield_sprite_path(view, job, sex) {
                    probe!(path, Origin::Table("job list"));
                }
            }
        }
    }
    // garments: every one, for every job
    let robes = RobeTable::load(grf);
    for robe in robes.names() {
        for &job in &jobs {
            for sex in 0u8..2 {
                probe!(
                    sp::robe_sprite_path(robe, job, sex),
                    Origin::Table("garments")
                );
            }
        }
    }
    for sex in 0u8..2 {
        probe!(sp::gm_body_sprite_path(sex), Origin::Registry);
        probe!(sp::gm_weapon_sprite_path(sex), Origin::Registry);
        // a human's heads, then a Doram's
        for (head, job) in (0..opts.max_head_id).flat_map(|head| [(head, 0), (head, 4218)]) {
            probe!(
                sp::head_sprite_path(head, job, sex),
                Origin::Table("head ids")
            );
            for palette in 0..opts.max_palette_id {
                probe!(
                    sp::head_palette_path(head, job, sex, palette),
                    Origin::Table("head ids"),
                );
            }
        }
    }

    // -- headgear -----------------------------------------------------------
    let accessories = AccessoryTable::load_from_grf(grf);
    for sex in 0u8..2 {
        for view in 0..MAX_HEADGEAR_VIEW_ID {
            if let Some(suffix) = accessories.get_suffix(view) {
                probe!(
                    sp::headgear_sprite_path(suffix, sex),
                    Origin::Table("accessory table"),
                );
            }
        }
    }

    // -- sounds -------------------------------------------------------------
    for skill in SkillEnum::ALL {
        let wavs = [
            tables::skill_use_sound(skill).map(|(w, _)| w),
            tables::skill_cast_begin_sound(skill).map(|(w, _)| w),
            tables::skill_projectile_sound(skill),
        ];
        for wav in wavs.into_iter().flatten() {
            need!(res::sound::sfx(wav), Origin::Table("skill sound table"));
        }
    }

    // One path can arrive from several places; the strongest claim wins, so sort
    // Required ahead of Probed before collapsing duplicates.
    out.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then((a.2 == Need::Probed).cmp(&(b.2 == Need::Probed)))
    });
    out.dedup_by(|a, b| a.0 == b.0);
    out
}
