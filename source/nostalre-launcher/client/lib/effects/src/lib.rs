pub mod buckets;
pub mod consumable_effects;
pub mod draw;
pub mod effect_queue;
pub mod effect_trait;
pub mod effects;
pub mod factory;
pub mod kro_prim;
pub mod kro_str;
pub mod opt3;
pub mod own_textures;
pub mod projectile;
pub mod radial_emitter;
pub mod sfx;
pub mod skill_effects;
pub mod skill_units;
pub mod spec;
pub mod spr_aliases;
pub mod spr_burst;
pub mod status_buff;
pub mod str_aliases;
pub mod table;
mod vec3;

pub use draw::{BILLBOARD_UV, BlendKind, EffectDrawList, EffectPrimitiveDraw, EffectStatus};
pub use effect_queue::{
    EffectQueue, ProjectileFlight, SpawnRequest, body_attached, is_caster_link_effect,
    is_count_point_effect, is_link_effect, is_trail_effect, link_status, status_link,
    trail_arrival_secs,
};
pub use effect_trait::{
    Afterimage, BodyAction, BodyCopy, BodyTint, BodyVertical, CameraShake, CameraView, Effect,
    EffectMessage, EffectRenderCtx, EffectUpdateCtx, GroundSampler, NumberRequest, StrPiece,
    StrTurn, WeaponAura, WeaponAuraStyle, WeaponLight,
};
pub use factory::make_effect;
pub use opt3::{
    OPT3_BLADESTOP, Opt3Reaction, monster_opt3_reaction, opt3_bit_for_icon, opt3_bits,
    player_opt3_reaction,
};
pub use skill_effects::{
    CasterSkillEffects, CastingSkill, TargetSkillEffects, begin_cast_effect,
    beginspell_for_element, caster_cast_on_use, caster_skill_effects, casting_skill,
    derive_hit_effect, fire_glyph_effect, ground_placed_effect, is_cast_circle, is_ground_cast,
    merc_skill_base, potion_throw_index, sevenwind_aura, suppresses_visuals_on_damage,
    target_skill_effects,
};
pub use skill_units::{
    SkillUnitEntrySound, UNT_USED_TRAPS, is_attackable_skill_unit, skill_unit_effect,
    skill_unit_entry_sound, skill_unit_sprite_paths, trap_idle_effect, trap_model_name,
    trap_trigger_effect,
};
pub use spec::{AlphaKeyframe, Attach, CurveParams, EffectSpec, SprBodyRecolor, SprBurstParams};
pub use spr_aliases::{SprDef, spr_def};
pub use spr_burst::spr_burst_params;
pub use status_buff::{
    EFST_SKE, StatusKind, StatusReaction, StatusSound, devil_blind_effect, persistent_aura,
    reaction_for_efst, status_reaction, status_reaction_by_efst,
};
pub use str_aliases::{str_aliases, str_variant};
pub use table::{
    StrLayer, custom_duration_ms, draws_over_ui, effect_spec, spawn_camera_shake, str_layer,
};

pub const ARROW_SPRITE: &str = nostalre_resources::sprite::monster::SKEL_ARCHER_ARROW;

pub const SPRITES: &[&str] = &[ARROW_SPRITE];

/// Whether a unit hides or cloaks as kRO's effects ask it: bits 1 and 2 of its option (actor +0x2c4, 0x8677c0 and
/// 0x867930), Chase Walk not among them. The sphere handler takes such a unit's spheres off at a count (0x805a80);
/// the draw skips another such player's effects (0x992480, 0x6c6ad0)
pub fn hidden_or_cloaked(option: i32) -> bool {
    option & 0x6 != 0
}

pub fn effect_texture_path(name: &str) -> String {
    if name.contains('/') {
        nostalre_resources::texture::named(name)
    } else {
        nostalre_resources::texture::effect::named(name)
    }
}

/// A texture an STR names: one named alone sits beside the STR, as kRO loads the 4th-job ones
/// (`effect\abyss_dagger\abyss_dagger_hit\aby_glow_01.bmp` for `abyss_dagger_hit.str`'s `aby_glow_01.bmp`); for an
/// STR at the top of `effect` that is `effect_texture_path`
pub fn str_texture_path(str_name: &str, texture: &str) -> String {
    match str_name.rsplit_once('/') {
        Some((dir, _)) if !texture.contains('/') => {
            nostalre_resources::texture::effect::named(&format!("{dir}/{texture}"))
        }
        _ => effect_texture_path(texture),
    }
}

/// Textures the rotated ground grids sample: their quads run UVs outside
/// `[0, 1]`, so wrapping would tile the art back over the grid border.
pub const CLAMPED_TEXTURES: &[&str] = &[effects::ground_sample::TEXTURE];

pub fn texture_wraps(path: &str) -> bool {
    !CLAMPED_TEXTURES.iter().any(|name| path.ends_with(name))
}

pub fn effect_texture_paths() -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let texture_lists: &[&[&str]] = &[
        effects::begin_asura::TEXTURES,
        effects::warp::TEXTURES,
        effects::magnum_break::TEXTURES,
        effects::dome_ring::TEXTURES,
        effects::bottom_song::TEXTURES,
        effects::bottom_hermode::TEXTURES,
        effects::bottom_landprotector::TEXTURES,
        effects::bottom_light::TEXTURES,
        effects::forest_light::TEXTURES,
        effects::linelink::TEXTURES,
        effects::guard::TEXTURES,
        effects::bottom_magnus::TEXTURES,
        effects::bottom_out::TEXTURES,
        effects::bottom_vertical::TEXTURES,
        effects::hit::TEXTURES,
        effects::hit2::TEXTURES,
        effects::hit5_6::TEXTURES,
        effects::bottom_sanctuary_pillar::TEXTURES,
        effects::warp_zone::TEXTURES,
        effects::volcano::TEXTURES,
        effects::gumgang::TEXTURES,
        effects::floor_aura::TEXTURES,
        effects::casting_ring::TEXTURES,
        effects::color_casting::TEXTURES,
        effects::black_devil::TEXTURES,
        effects::electric::TEXTURES,
        effects::hit_line::TEXTURES,
        effects::sparkle_column::TEXTURES,
        effects::bash::TEXTURES,
        effects::flasher::TEXTURES,
        effects::hasteup::TEXTURES,
        effects::blessing::TEXTURES,
        effects::cast_circle::TEXTURES,
        effects::chemical::TEXTURES,
        effects::sightrasher::TEXTURES,
        effects::firesplashhit::TEXTURES,
        effects::coldhit::TEXTURES,
        effects::endure::TEXTURES,
        effects::enhance::TEXTURES,
        effects::entry::TEXTURES,
        effects::exit::TEXTURES,
        effects::glasswall::TEXTURES,
        effects::glasswall2::TEXTURES,
        effects::providence::TEXTURES,
        effects::healsp::TEXTURES,
        effects::frost_diver::TEXTURES,
        effects::fullscreen_overlay::TEXTURES,
        effects::begin_spell_6::TEXTURES,
        effects::begin_spell::TEXTURES,
        effects::beginspell_n::TEXTURES,
        kro_prim::TEXTURES,
        effects::aura_blade::TEXTURES,
        effects::couple_casting::TEXTURES,
        effects::stormgust::TEXTURES,
        effects::animated_texture_billboard::TEXTURES,
        effects::placeholder::TEXTURES,
        effects::kro_spheres::TEXTURES,
        effects::kro_classes::TEXTURES,
        effects::portal::TEXTURES,
        effects::portal2::TEXTURES,
        effects::portal_wind::TEXTURES,
        effects::ready_portal::TEXTURES,
        effects::teleportation::TEXTURES,
        effects::spraypond::TEXTURES,
        effects::status_up::TEXTURES,
        effects::magic_bolt::FIRE_TEXTURES,
        effects::napalmbeat::TEXTURES,
        effects::sandwind::TEXTURES,
        effects::yupitel::TEXTURES,
        effects::bowling_bash::TEXTURES,
        effects::overthrust::TEXTURES,
        effects::callzone::TEXTURES,
        effects::ground_sample::TEXTURES,
        effects::sonicblowhit::TEXTURES,
        effects::cartrevolution::TEXTURES,
        effects::barrier::TEXTURES,
        effects::turnundead::TEXTURES,
        effects::firepillaron::TEXTURES,
        effects::hitdark::TEXTURES,
        effects::pierce::TEXTURES,
        effects::gumgang2::TEXTURES,
        effects::defender::TEXTURES,
        effects::heal::TEXTURES,
        effects::big_portal::TEXTURES,
        effects::attack_energy::TEXTURES,
        effects::wind::TEXTURES,
        effects::bash3d::TEXTURES,
        effects::blitzbeat::TEXTURES,
        effects::curseattack::TEXTURES,
        effects::detecting::TEXTURES,
        effects::earthspike::TEXTURES,
        effects::heavensdrive::TEXTURES,
        effects::bottom_box::TEXTURES,
        effects::flowercast::TEXTURES,
        effects::fireivy::TEXTURES,
        effects::grimtooth::TEXTURES,
        effects::grimtooth_atk::TEXTURES,
        effects::icewall::TEXTURES,
        effects::party::TEXTURES,
        effects::foot::TEXTURES,
        effects::teihit::TEXTURES,
        effects::particle_up::TEXTURES,
        effects::peong_up::TEXTURES,
        effects::peong::TEXTURES,
        effects::heartcasting::TEXTURES,
        effects::colorpaper::TEXTURES,
        effects::gravitation::TEXTURES,
        effects::storm_kick::TEXTURES,
        effects::effect_texture::TEXTURES,
        effects::tarot_card::TEXTURES,
        effects::temp_result::TEXTURES,
        effects::toprank::TEXTURES,
        effects::lockon::TEXTURES,
        effects::waterball::TEXTURES,
        effects::yufitel_hit::TEXTURES,
        effects::texture_falling::TEXTURES,
        effects::aciddemon::TEXTURES,
        effects::rainbow::TEXTURES,
        effects::agiup::TEXTURES,
        effects::light_sphere::TEXTURES,
        effects::throw_item::TEXTURES,
        effects::rg_coin::TEXTURES,
        effects::cloud_projectile::TEXTURES,
        effects::twilight::TEXTURES,
        effects::pressure::TEXTURES,
        effects::mapzone::TEXTURES,
        effects::waterfall::TEXTURES,
        effects::cloud::TEXTURES,
        effects::stin::TEXTURES,
        effects::sma::TEXTURES,
        effects::soul_breaker::TEXTURES,
        effects::slash::TEXTURES,
        effects::thunderstorm2::TEXTURES,
        effects::summon_slave::TEXTURES,
        effects::bubble_drop::TEXTURES,
        effects::cartter::TEXTURES,
        effects::pokjuk::TEXTURES,
        effects::magic_bolt::ICE_TEXTURES,
        effects::body_aura::TEXTURES,
        &effects::body_aura::GREEN_FLAME,
        &effects::body_aura::RED_FLAME,
    ];
    for list in texture_lists {
        for name in *list {
            for candidate in name.split('|') {
                seen.insert(effect_texture_path(candidate));
            }
        }
    }
    seen.into_iter().collect()
}

/// The STR file an effect plays, its first name then the ones tried after it; `None` for an effect drawn
/// otherwise
pub fn str_names_of(id: models::enums::effect_id::EffectId) -> Option<&'static [&'static str]> {
    if !matches!(effect_spec(id), Some(spec::EffectSpec::Str { .. })) {
        return None;
    }
    let names = match str_aliases::str_aliases(id) {
        [] => kro_str::str_file(id).unwrap_or_default(),
        names => names,
    };
    (!names.is_empty()).then_some(names)
}

/// Every STR effect's names (see [`str_names_of`]), once each
pub fn effect_str_names() -> Vec<&'static [&'static str]> {
    use models::enums::effect_id::EffectId;

    let mut seen = std::collections::BTreeSet::new();
    EffectId::ALL
        .into_iter()
        .filter_map(str_names_of)
        .filter(|names| seen.insert(names[0]))
        .collect()
}

/// The effects a skill can show: its cast and begin-cast, on its target, before and at the hit, placed on the
/// ground at any level
pub fn skill_effect_ids(
    skill: models::enums::skill_enums::SkillEnum,
) -> Vec<models::enums::effect_id::EffectId> {
    let target = target_skill_effects(skill);
    let mut ids: Vec<_> = [
        caster_skill_effects(skill).cast,
        casting_skill(skill).begin,
        target.on_target,
        target.before_hit,
        target.hit,
    ]
    .concat();
    for level in 1..=10 {
        ids.extend_from_slice(skill_effects::ground_placed_effect(skill, level));
    }
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(*id));
    ids
}

pub fn effect_spr_paths() -> Vec<&'static str> {
    use models::enums::effect_id::EffectId;

    let mut seen = std::collections::BTreeSet::new();
    for id in EffectId::ALL {
        match effect_spec(id) {
            Some(
                spec::EffectSpec::Spr { sprite, .. } | spec::EffectSpec::SprBurst { sprite, .. },
            ) => {
                seen.insert(sprite);
            }
            _ => {}
        }
    }
    seen.into_iter().collect()
}

pub fn custom_effect_sprite_paths() -> Vec<&'static str> {
    let mut seen = std::collections::BTreeSet::new();
    let sprite_lists: &[&[&str]] = &[
        SPRITES,
        effects::hit::SPRITES,
        effects::sight::SPRITES,
        effects::exit::SPRITES,
        effects::hasteup::SPRITES,
        effects::magic_bolt::FIRE_SPRITES,
        effects::fireball::SPRITES,
        effects::soul_strike::SPRITES,
        effects::energy_drain::SPRITES,
        effects::sightrasher::SPRITES,
        effects::venomdust2::SPRITES,
        effects::healsp::SPRITES,
        effects::blessing::SPRITES,
        effects::cone::SPRITES,
        effects::dragonsmoke::SPRITES,
        effects::wink::SPRITES,
        effects::m_ef02::SPRITES,
        effects::ghost::SPRITES,
        effects::banjjakii::SPRITES,
        effects::orbit_burst::SPRITES,
        effects::hitdark::SPRITES,
        effects::spearbmr::SPRITES,
        effects::waterball2::SPRITES,
        effects::warp_zone::SPRITES,
        effects::peong::SPRITES,
        effects::super_angel::SPRITES,
        effects::summon_slave::SPRITES,
        effects::sakura::SPRITES,
        effects::bottom_song::SPRITES,
    ];
    for list in sprite_lists {
        for path in *list {
            seen.insert(*path);
        }
    }
    seen.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_str_in_a_folder_takes_its_textures_from_beside_it() {
        assert_eq!(
            str_texture_path(
                "abyss_dagger/abyss_dagger_hit/abyss_dagger_hit",
                "aby_glow_01.bmp"
            ),
            "data/texture/effect/abyss_dagger/abyss_dagger_hit/aby_glow_01.bmp"
        );
        assert_eq!(
            str_texture_path("tinder", "fire1.bmp"),
            effect_texture_path("fire1.bmp"),
            "a top-level STR as before"
        );
    }

    #[test]
    fn effect_texture_paths_returns_unique_names() {
        let paths = effect_texture_paths();
        let mut sorted = paths.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(paths.len(), sorted.len(), "no duplicates");
        assert!(paths.iter().all(|p| !p.is_empty()), "no empty entries");
        assert!(
            paths.iter().all(|p| p.starts_with("data/texture/")),
            "all entries are GRF texture paths",
        );
        assert!(
            paths
                .iter()
                .any(|p| p.starts_with("data/texture/유저인터페이스/item/")),
            "thrown item icons resolve under the item dir",
        );
        assert!(
            paths.iter().any(|p| p.ends_with("ring_yellow.tga")),
            "Warp's texture is included",
        );
    }

    #[test]
    fn the_rotated_ground_grid_texture_is_preloaded_clamped() {
        let grid = effect_texture_path(effects::ground_sample::TEXTURE);
        assert!(effect_texture_paths().contains(&grid));
        assert!(!texture_wraps(&grid));
        assert!(texture_wraps(&effect_texture_path("ring_yellow.tga")));
    }

    #[test]
    fn effect_spr_paths_cover_spr_def_effects() {
        let paths = effect_spr_paths();
        assert!(
            paths.contains(&"data/sprite/이팩트/스톱"),
            "NpcStop's sprite must be preloaded",
        );
        let mut sorted = paths.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(paths.len(), sorted.len(), "no duplicates");
    }
}
