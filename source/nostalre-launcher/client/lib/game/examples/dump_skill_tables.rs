//! Our hand skill tables, one TSV row per skill id, for the comparison against kRO's exe
//! (`scripts/kro-exe/compare_ours.py`).
use models::enums::{EnumWithNumberValue, class::JobName, skill_enums::SkillEnum};
use nostalre_effects::{
    begin_cast_effect, caster_cast_on_use, caster_skill_effects, derive_hit_effect,
    fire_glyph_effect, ground_placed_effect, is_ground_cast, target_skill_effects,
};
use nostalre_game::skill_action::{
    ends_ready, hits_at_once, performance, skill_motion_type, skill_pose,
};
use nostalre_game::sound::tables::{
    skill_cast_begin_sound, skill_projectile_sound, skill_use_sound,
};

fn ids(effects: &[models::enums::effect_id::EffectId]) -> String {
    effects
        .iter()
        .map(|e| e.value().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn main() {
    println!(
        "id\tmotion\tpose\tends_ready\thits_at_once\tperformance\tbegin_cast\tcaster\ton_use\tglyph\tground\tground_cast\ton_target\tbefore_hit\thit\textra_secs\tuse_sound\tcast_sound\tprojectile_sound\thit_generic\thit_skill\tspins"
    );
    for id in 0..20000u32 {
        let Ok(s) = SkillEnum::try_from_value(id) else {
            continue;
        };
        let t = target_skill_effects(s);
        let h = derive_hit_effect(Some(s), false, JobName::Swordsman, false, 0);
        println!(
            "{id}\t{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            skill_motion_type(s),
            skill_pose(s).map(|p| format!("{p:?}")).unwrap_or_default(),
            ends_ready(s),
            hits_at_once(s),
            performance(s).map(|p| format!("{p:?}")).unwrap_or_default(),
            ids(begin_cast_effect(s)),
            ids(caster_skill_effects(s).cast),
            caster_cast_on_use(s),
            ids(fire_glyph_effect(s)),
            ids(ground_placed_effect(s, 1)),
            is_ground_cast(s),
            ids(t.on_target),
            ids(t.before_hit),
            ids(t.hit),
            t.hit_extra_delay_secs,
            skill_use_sound(s).map(|(f, _)| f).unwrap_or_default(),
            skill_cast_begin_sound(s)
                .map(|(f, _)| f)
                .unwrap_or_default(),
            skill_projectile_sound(s).unwrap_or_default(),
            ids(h.generic),
            ids(h.skill),
            h.spins_target,
        );
    }
}
