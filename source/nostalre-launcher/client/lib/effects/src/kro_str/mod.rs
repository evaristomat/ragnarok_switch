//! kRO's registered effect classes that play one STR (the 2022 exe's newer effect system, made by unit through
//! 0x96aca0), and the skills whose packets make them, as the exe's runs give them (`generated.rs`,
//! `scripts/kro-exe/str_class_table.py`). They fill what our own tables leave empty: an effect we draw nothing for,
//! a skill's role in which ours shows nothing. What ours already draws stays ours.

#[rustfmt::skip]
mod generated;

use models::enums::EnumWithNumberValue;
use models::enums::effect_id::EffectId;
use models::enums::skill_enums::SkillEnum;

use crate::sfx::{SfxCue, SfxSchedule};

pub(crate) struct StrClass {
    id: u16,
    /// The STR under `texture/effect`, without `.str`
    file: &'static str,
    /// How long it lives: its updates, taken as 16.67 ms each
    ms: u32,
    /// It goes round until its owner takes it off (a skill unit's)
    repeat: bool,
    /// A floor under the units: kRO files its draws in the list it draws first, before the sprites (+0x184)
    under_sprites: bool,
    /// What it plays, at the update it plays it
    sounds: &'static [SfxCue],
}

/// The skill packet that makes an effect: a cast's start (0x0b1a), a hit (0x01de), a skill without damage (0x09cb),
/// a ground skill (0x0117)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packet {
    Cast,
    Damage,
    NoDamage,
    Ground,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum On {
    Caster,
    Target,
    /// A ground skill's cell (made on no unit)
    Cell,
}

/// The classes 0x09ca makes on a skill unit of `unit`, for as long as the unit lasts
pub(crate) struct UnitUses {
    unit: u16,
    effects: &'static [EffectId],
}

pub(crate) struct SkillUses {
    skill: u16,
    packet: Packet,
    on: On,
    /// Made when 0x01de's hit is due on the target, not when the packet comes
    at_hit: bool,
    effects: &'static [EffectId],
}

/// The spark a skill's hit puts on its target (0x01de's hit message, `effect`); none for most skills
pub(crate) struct HitSpark {
    skill: u16,
    effects: &'static [EffectId],
}

/// Classes a skill picks by the caster's facing on screen (kRO's pickers, `str_class_table.py`'s `FACING`): `made`
/// the one a facing of 0 picks, as the runs recorded; then a class a quarter, or a sector when nine long
pub(crate) struct Facing {
    skill: u16,
    made: EffectId,
    by_bucket: &'static [EffectId],
}

/// The eight-way pickers' sector starts past the first, in degrees: [0,23) [23,67) .. [292,337) [337,360]
const SECTORS: [f32; 8] = [23.0, 67.0, 112.0, 157.0, 202.0, 247.0, 292.0, 337.0];

fn class(id: EffectId) -> Option<&'static StrClass> {
    let id = u16::try_from(id.value()).ok()?;
    let at = generated::CLASSES
        .binary_search_by_key(&id, |c| c.id)
        .ok()?;
    Some(&generated::CLASSES[at])
}

/// The STR a kRO class plays, how long it lives and whether it goes round
pub fn str_effect(id: EffectId) -> Option<(&'static str, u32, bool)> {
    class(id).map(|c| (c.file, c.ms, c.repeat))
}

/// Whether `id` is one of kRO's registered classes (0x96aca0's: an STR class, or one ported with code of its own),
/// not an older effect. A cast's start tells an older effect the cast's length (message 0x50, 0x0b1a's handler), so
/// it ends with the cast; a class plays its own time
pub fn is_class(id: EffectId) -> bool {
    class(id).is_some() || crate::effects::kro_classes::ports(id)
}

/// Whether a kRO class lies under the units (its `_bottom` STR), drawn before the sprites
pub fn under_sprites(id: EffectId) -> bool {
    class(id).is_some_and(|c| c.under_sprites)
}

/// The effects kRO makes on a skill unit of `unit` (0x09ca)
pub fn unit_effects(unit: u16) -> &'static [EffectId] {
    generated::UNIT_USES
        .binary_search_by_key(&unit, |u| u.unit)
        .map_or(&[], |at| generated::UNIT_USES[at].effects)
}

/// The STR a kRO class plays, as the one name of a list (the STR preload's)
pub fn str_file(id: EffectId) -> Option<&'static [&'static str]> {
    class(id).map(|c| std::slice::from_ref(&c.file))
}

/// The sound a kRO class plays
pub fn sounds(id: EffectId) -> Option<SfxSchedule> {
    class(id).map(|c| c.sounds).filter(|s| !s.is_empty())
}

/// The effects kRO makes for `skill` at `packet` on `on`
pub fn skill_effects(
    skill: SkillEnum,
    packet: Packet,
    on: On,
    at_hit: bool,
) -> &'static [EffectId] {
    let Ok(id) = u16::try_from(skill.id()) else {
        return &[];
    };
    let uses = &generated::SKILL_USES;
    uses[uses.partition_point(|u| u.skill < id)..]
        .iter()
        .take_while(|u| u.skill == id)
        .find(|u| u.packet == packet && u.on == on && u.at_hit == at_hit)
        .map_or(&[], |u| u.effects)
}

/// The spark kRO's hit of `skill` puts on the target (Cold Bolt's `Coldhit`, Servant Weapon's own class), empty
/// where it puts none; `None` for a skill the 2022 exe does not know, or whose spark is a class ours cannot play
pub fn hit_spark(skill: SkillEnum) -> Option<&'static [EffectId]> {
    let id = u16::try_from(skill.id()).ok()?;
    let sparks = &generated::HIT_SPARKS;
    let at = sparks.binary_search_by_key(&id, |s| s.skill).ok()?;
    Some(sparks[at].effects)
}

/// Whether kRO picks any of `skill`'s classes by the caster's facing ([`faced`])
pub fn has_facing(skill: SkillEnum) -> bool {
    u16::try_from(skill.id()).is_ok_and(|id| generated::FACING.iter().any(|f| f.skill == id))
}

/// The class `skill` makes in place of `made` for the caster's `facing` on screen (`Camera::screen_facing`: 0 down,
/// 90 right): Dragon Breath's breath drawn toward its target, one of four; `made` itself where no picker reads it
pub fn faced(skill: SkillEnum, made: EffectId, facing: f32) -> EffectId {
    let Ok(id) = u16::try_from(skill.id()) else {
        return made;
    };
    let Some(set) = generated::FACING
        .iter()
        .find(|f| f.skill == id && f.made == made)
    else {
        return made;
    };
    let bucket = if set.by_bucket.len() == 4 {
        // (0,90) [90,180) [180,270) and the rest, 0 among the rest
        match facing {
            a if a > 0.0 && a < 90.0 => 0,
            a if (90.0..180.0).contains(&a) => 1,
            a if (180.0..270.0).contains(&a) => 2,
            _ => 3,
        }
    } else {
        SECTORS.iter().take_while(|&&start| facing >= start).count()
    };
    set.by_bucket[bucket]
}

/// Ours when it has any, else kRO's
pub fn or_kro(
    ours: &'static [EffectId],
    skill: SkillEnum,
    packet: Packet,
    on: On,
    at_hit: bool,
) -> &'static [EffectId] {
    if ours.is_empty() {
        skill_effects(skill, packet, on, at_hit)
    } else {
        ours
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::EffectSpec;
    use models::enums::class::JobName;

    #[test]
    fn abyss_chaser_plays_what_kros_packets_make() {
        // the exe's runs: Abyss Dagger's 0x09cb makes 1966 on the caster, each hit 1967 on its target
        assert_eq!(
            skill_effects(
                SkillEnum::AbcAbyssDagger,
                Packet::NoDamage,
                On::Caster,
                false
            ),
            [EffectId::Kro1966]
        );
        let hit = crate::skill_effects::derive_hit_effect(
            Some(SkillEnum::AbcAbyssDagger),
            false,
            JobName::Novice,
            false,
            0,
        );
        assert_eq!(hit.skill, [EffectId::Kro1967]);
        // Omega Abyss Strike's 0x0117 makes four at the cell
        assert_eq!(
            skill_effects(SkillEnum::AbcAbyssStrike, Packet::Ground, On::Cell, false),
            [
                EffectId::Kro2087,
                EffectId::Kro2088,
                EffectId::Kro2091,
                EffectId::Kro2092
            ]
        );
        // Abyss Square's unit: its floor and its square, the square going round with its sound
        assert_eq!(unit_effects(0x115), [EffectId::Kro2130, EffectId::Kro2129]);
        assert!(str_effect(EffectId::Kro2129).is_some_and(|(_, _, repeat)| repeat));
        assert!(
            matches!(crate::effect_spec(EffectId::Kro2129), Some(EffectSpec::Str { duration_ms, .. }) if duration_ms > 60_000),
            "held until the unit goes"
        );
        assert!(
            under_sprites(EffectId::Kro2130) && !under_sprites(EffectId::Kro2129),
            "the floor, not the square"
        );
        use crate::{StrLayer, str_layer};
        assert_eq!(str_layer(EffectId::Kro2130), StrLayer::UnderSprites);
        assert_eq!(str_layer(EffectId::Kro2129), StrLayer::World);
        assert_eq!(
            str_layer(EffectId::Refineok),
            StrLayer::OverUi,
            "the windows' own over anything"
        );
        // Deft Stab's own class on its target at 0x09cb, with ABC_DEFT_STAB1 at its first update
        assert_eq!(
            skill_effects(SkillEnum::AbcDeftStab, Packet::NoDamage, On::Target, false),
            [EffectId::Kro2132]
        );
        assert!(sounds(EffectId::Kro2132).is_some());
        let Some(EffectSpec::Str {
            file, duration_ms, ..
        }) = crate::effect_spec(EffectId::Kro1967)
        else {
            panic!("1967 plays its STR");
        };
        assert_eq!(
            (file, duration_ms),
            ("abyss_dagger/abyss_dagger_hit/abyss_dagger_hit", 317),
            "19 updates"
        );
    }

    #[test]
    fn the_tables_stay_sorted_for_their_searches() {
        assert!(generated::CLASSES.is_sorted_by_key(|c| c.id));
        assert!(generated::SKILL_USES.is_sorted_by_key(|u| u.skill));
        assert!(generated::UNIT_USES.is_sorted_by_key(|u| u.unit));
        assert!(generated::HIT_SPARKS.is_sorted_by_key(|s| s.skill));
    }

    #[test]
    fn a_breath_is_picked_toward_where_the_caster_faces() {
        use EffectId as E;
        let breath = |facing| faced(SkillEnum::RkDragonbreath, E::Kro1382, facing);
        // the files' clock faces: 05 down-right, 01 up-right, 11 up-left, 07 down-left (and straight down)
        assert_eq!(
            [
                breath(45.0),
                breath(135.0),
                breath(225.0),
                breath(315.0),
                breath(0.0)
            ],
            [E::Kro1381, E::Kro1380, E::Kro1383, E::Kro1382, E::Kro1382]
        );
        // eight ways: Overbrand's 03 o'clock to the right, its 06 straight down either side of 0
        let brand = |facing| faced(SkillEnum::LgOverbrand, E::Kro1682, facing);
        assert_eq!(
            [brand(90.0), brand(10.0), brand(350.0)],
            [E::Kro1680, E::Kro1682, E::Kro1682]
        );
        assert_eq!(
            faced(SkillEnum::RkDragonbreath, E::Kro1378, 45.0),
            E::Kro1378,
            "not a picked class"
        );
        for set in &generated::FACING {
            assert!(matches!(set.by_bucket.len(), 4 | 9));
            assert_eq!(
                faced_by_id(set.skill, set.made, 0.0),
                set.made,
                "a facing of 0 picks the run's own"
            );
        }
    }

    fn faced_by_id(skill: u16, made: EffectId, facing: f32) -> EffectId {
        let skill = SkillEnum::try_from_value(u32::from(skill)).unwrap();
        faced(skill, made, facing)
    }

    #[test]
    fn what_ours_draws_stays_ours() {
        let ours: &'static [EffectId] = &[EffectId::Bash];
        assert_eq!(
            or_kro(
                ours,
                SkillEnum::AbcAbyssDagger,
                Packet::NoDamage,
                On::Caster,
                false
            ),
            ours
        );
        assert_eq!(
            skill_effects(SkillEnum::RkHundredspear, Packet::Damage, On::Target, true),
            [] as [EffectId; 0],
            "a role kRO draws nothing in"
        );
    }

    #[test]
    fn the_older_effects_kro_puts_on_a_unit_fill_ours_too() {
        // Hundred Spear's 0x01de: 723 and 835 made on the caster (effect_on_actor), not classes of the newer
        // system; both told the target's place (message 0xe), where 723 draws (the target's) and 835 aims
        assert_eq!(
            skill_effects(SkillEnum::RkHundredspear, Packet::Damage, On::Target, false),
            [EffectId::Chimto2]
        );
        assert_eq!(
            skill_effects(SkillEnum::RkHundredspear, Packet::Damage, On::Caster, false),
            [EffectId::Chemical2dash2]
        );
        assert_eq!(
            skill_effects(
                SkillEnum::RkPhantomthrust,
                Packet::NoDamage,
                On::Target,
                false
            ),
            [EffectId::Greenbody, EffectId::Bluecasting]
        );
        // the lock-on at a cast's target is ours (CastMark::Lockon), and the spark at a hit (Cold Bolt's Coldhit)
        // ours picks itself
        assert!(skill_effects(SkillEnum::MgColdbolt, Packet::Cast, On::Target, false).is_empty());
        assert!(skill_effects(SkillEnum::MgColdbolt, Packet::Damage, On::Target, true).is_empty());
    }
}
