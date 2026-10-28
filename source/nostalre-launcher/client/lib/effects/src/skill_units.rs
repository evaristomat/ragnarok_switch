use models::enums::effect_id::EffectId;

const UNT_SAFETYWALL: u16 = 0x7e;
const UNT_FIREWALL: u16 = 0x7f;
const UNT_WARP_WAITING: u16 = 0x80;
const UNT_WARP_ACTIVE: u16 = 0x81;
const UNT_BENEDICTIO: u16 = 0x82;
const UNT_SANCTUARY: u16 = 0x83;
const UNT_MAGNUS: u16 = 0x84;
const UNT_FIREPILLAR_WAITING: u16 = 0x87;
const UNT_FIREPILLAR_ACTIVE: u16 = 0x88;
/// A trap that has just been sprung — the server changes the unit's look to this
/// (`clif_changetraplook`) at the moment a monster triggers it.
pub const UNT_USED_TRAPS: u16 = 0x8c;
const UNT_ICEWALL: u16 = 0x8d;
const UNT_QUAGMIRE: u16 = 0x8e;
const UNT_BLASTMINE: u16 = 0x8f;
const UNT_SKIDTRAP: u16 = 0x90;
const UNT_ANKLESNARE: u16 = 0x91;
const UNT_VENOMDUST: u16 = 0x92;
const UNT_LANDMINE: u16 = 0x93;
const UNT_SHOCKWAVE: u16 = 0x94;
const UNT_SANDMAN: u16 = 0x95;
const UNT_FLASHER: u16 = 0x96;
const UNT_FREEZINGTRAP: u16 = 0x97;
const UNT_CLAYMORETRAP: u16 = 0x98;
const UNT_TALKIEBOX: u16 = 0x99;
const UNT_VOLCANO: u16 = 0x9a;
const UNT_DELUGE: u16 = 0x9b;
const UNT_VIOLENTGALE: u16 = 0x9c;
const UNT_LANDPROTECTOR: u16 = 0x9d;
const UNT_LULLABY: u16 = 0x9e;
const UNT_RICHMANKIM: u16 = 0x9f;
const UNT_ETERNALCHAOS: u16 = 0xa0;
const UNT_DRUMBATTLEFIELD: u16 = 0xa1;
const UNT_RINGNIBELUNGEN: u16 = 0xa2;
const UNT_ROKISWEIL: u16 = 0xa3;
const UNT_INTOABYSS: u16 = 0xa4;
const UNT_SIEGFRIED: u16 = 0xa5;
const UNT_DISSONANCE: u16 = 0xa6;
const UNT_WHISTLE: u16 = 0xa7;
const UNT_ASSASSINCROSS: u16 = 0xa8;
const UNT_POEMBRAGI: u16 = 0xa9;
const UNT_APPLEIDUN: u16 = 0xaa;
const UNT_UGLYDANCE: u16 = 0xab;
const UNT_HUMMING: u16 = 0xac;
const UNT_DONTFORGETME: u16 = 0xad;
const UNT_FORTUNEKISS: u16 = 0xae;
const UNT_SERVICEFORYOU: u16 = 0xaf;
const UNT_DEMONSTRATION: u16 = 0xb1;
const UNT_CALLFAMILY: u16 = 0xb2;
const UNT_GOSPEL: u16 = 0xb3;
const UNT_BASILICA: u16 = 0xb4;
const UNT_FOGWALL: u16 = 0xb6;
const UNT_SPIDERWEB: u16 = 0xb7;
const UNT_GRAVITATION: u16 = 0xb8;
const UNT_HERMODE: u16 = 0xb9;
const UNT_SUITON: u16 = 0xbb;
const UNT_TATAMIGAESHI: u16 = 0xbc;
const UNT_KAEN: u16 = 0xbd;
const UNT_GROUNDDRIFT_WIND: u16 = 0xbe;
const UNT_GROUNDDRIFT_DARK: u16 = 0xbf;
const UNT_GROUNDDRIFT_POISON: u16 = 0xc0;
const UNT_GROUNDDRIFT_WATER: u16 = 0xc1;
const UNT_GROUNDDRIFT_FIRE: u16 = 0xc2;
const UNT_DEATHWAVE: u16 = 0xc3;
const UNT_WATERATTACK: u16 = 0xc4;
const UNT_WINDATTACK: u16 = 0xc5;
const UNT_EVILLAND: u16 = 0xc7;
const UNT_DARK_RUNNER: u16 = 0xc8;
const UNT_DARK_TRANSFER: u16 = 0xc9;

/// `effects_on` is the `/effect` toggle: the warp-portal-waiting unit is the
/// only unit that shows a different effect with effects turned off.
pub fn skill_unit_effect(unit_id: u16, effects_on: bool) -> Option<EffectId> {
    use EffectId as E;
    Some(match unit_id {
        UNT_SAFETYWALL => E::Glasswall2,
        UNT_FIREWALL => E::Firewall,
        UNT_WARP_WAITING if effects_on => E::Portal2,
        UNT_WARP_WAITING => E::Portal,
        UNT_WARP_ACTIVE => E::Readyportal2,
        UNT_BENEDICTIO => E::Benedictio,
        UNT_SANCTUARY => E::BottomSanc,
        UNT_MAGNUS => E::BottomMag,
        UNT_FIREPILLAR_WAITING => E::Firepillaron,
        UNT_FIREPILLAR_ACTIVE => E::Firepillarbomb,
        UNT_ICEWALL => E::Icewall,
        UNT_QUAGMIRE => E::Quagmire,
        UNT_VENOMDUST => E::Venomdust2,
        UNT_VOLCANO => E::BottomVo,
        UNT_DELUGE => E::BottomDe,
        UNT_VIOLENTGALE => E::BottomVi,
        UNT_LANDPROTECTOR => E::BottomLa,
        UNT_DEMONSTRATION => E::Demonstration,
        UNT_CALLFAMILY => E::Portal3,
        UNT_GOSPEL => E::BottomGospel,
        UNT_BASILICA => E::BottomBasilica,
        UNT_FOGWALL => E::BottomFogwall,
        UNT_SPIDERWEB => E::BottomSpider,
        UNT_GRAVITATION => E::Gravitation,
        UNT_HERMODE => E::BottomHermode,
        UNT_SUITON => E::BottomSuiton,
        UNT_TATAMIGAESHI => E::Tatami,
        UNT_KAEN => E::Kaen,
        UNT_GROUNDDRIFT_WIND => E::LightningS,
        UNT_GROUNDDRIFT_DARK => E::BlindS,
        UNT_GROUNDDRIFT_POISON => E::PoisonS,
        UNT_GROUNDDRIFT_WATER => E::FreezingS,
        UNT_GROUNDDRIFT_FIRE => E::FlareS,
        UNT_DEATHWAVE | UNT_WATERATTACK => E::Icewall,
        UNT_WINDATTACK => E::Firepillaron,
        UNT_EVILLAND => E::BottomEvilland,
        UNT_DARK_RUNNER => E::BottomRunner,
        UNT_DARK_TRANSFER => E::BottomTransfer,

        UNT_LULLABY => E::BottomLullaby,
        UNT_RICHMANKIM => E::BottomRichmankim,
        UNT_ETERNALCHAOS => E::BottomEternalchaos,
        UNT_DRUMBATTLEFIELD => E::BottomDrumbattlefield,
        UNT_RINGNIBELUNGEN => E::BottomRingnibelungen,
        UNT_ROKISWEIL => E::BottomRokisweil,
        UNT_INTOABYSS => E::BottomIntoabyss,
        UNT_SIEGFRIED => E::BottomSiegfried,
        UNT_DISSONANCE => E::BottomDissonance,
        UNT_WHISTLE => E::BottomWhistle,
        UNT_ASSASSINCROSS => E::BottomAssassincross,
        UNT_POEMBRAGI => E::BottomPoembragi,
        UNT_APPLEIDUN => E::BottomAppleidun,
        UNT_UGLYDANCE => E::BottomUglydance,
        UNT_HUMMING => E::BottomHumming,
        UNT_DONTFORGETME => E::BottomDontforgetme,
        UNT_FORTUNEKISS => E::BottomFortunekiss,
        UNT_SERVICEFORYOU => E::BottomServiceforyou,

        _ => return None,
    })
}

/// A positional one-shot a ground skill unit plays when it enters view.
/// `one_in` is a 1-in-N chance gate (1 = always).
pub struct SkillUnitEntrySound {
    pub wave: &'static str,
    pub one_in: u8,
}

pub fn skill_unit_entry_sound(unit_id: u16) -> Option<SkillUnitEntrySound> {
    Some(match unit_id {
        UNT_WARP_WAITING | UNT_WARP_ACTIVE => SkillUnitEntrySound {
            wave: "effect\\EF_Portal.wav",
            one_in: 1,
        },
        UNT_FIREWALL => SkillUnitEntrySound {
            wave: "effect\\EF_FireWall.wav",
            one_in: 1,
        },
        UNT_KAEN => SkillUnitEntrySound {
            wave: "effect\\화염진.wav",
            one_in: 8,
        },
        _ => return None,
    })
}

/// The RSM model a deployed trap shows on the ground (path relative to
/// `data\model\`), or `None` for a non-trap unit. The trigger burst (freeze,
/// blast, …) fires separately via [`trap_trigger_effect`].
pub fn trap_model_name(unit_id: u16) -> Option<&'static str> {
    Some(match unit_id {
        UNT_ANKLESNARE => "외부소품\\트랩01.rsm",
        UNT_SKIDTRAP => "외부소품\\트랩02.rsm",
        UNT_LANDMINE => "외부소품\\트랩03.rsm",
        UNT_FREEZINGTRAP => "외부소품\\트랩03_2.rsm",
        UNT_BLASTMINE => "외부소품\\트랩03_3.rsm",
        UNT_SANDMAN => "외부소품\\트랩03_4.rsm",
        UNT_FLASHER => "외부소품\\트랩03_5.rsm",
        UNT_SHOCKWAVE => "외부소품\\트랩03_6.rsm",
        UNT_CLAYMORETRAP => "외부소품\\트랩04.rsm",
        UNT_TALKIEBOX => "외부소품\\트랩05.rsm",
        _ => return None,
    })
}

/// The ground units a plain attack can be swung at: the original game gives
/// these the attack cursor and sends an ordinary attack request for them, so a
/// trap laid by another player can be shot down.
pub fn is_attackable_skill_unit(unit_id: u16) -> bool {
    matches!(unit_id, UNT_ICEWALL | UNT_BLASTMINE | UNT_CLAYMORETRAP)
}

/// The effect a deployed trap keeps playing on top of its model. Shock Wave
/// Trap is the only trap that has one.
pub fn trap_idle_effect(unit_id: u16) -> Option<EffectId> {
    match unit_id {
        UNT_SHOCKWAVE => Some(EffectId::Shockwave),
        _ => None,
    }
}

/// The one-shot burst a trap plays when a monster springs it (the trap unit
/// becomes [`UNT_USED_TRAPS`]). Traps that only hold or teleport — Skid Trap,
/// Ankle Snare, Land Mine, Talkie Box, Shockwave — have no trigger burst.
pub fn trap_trigger_effect(unit_id: u16) -> Option<EffectId> {
    use EffectId as E;
    Some(match unit_id {
        UNT_BLASTMINE => E::Blastminebomb,
        UNT_FREEZINGTRAP => E::Freezing,
        UNT_SANDMAN => E::Sandman,
        UNT_FLASHER => E::Flasher,
        UNT_CLAYMORETRAP => E::Claymore,
        _ => return None,
    })
}

/// Every sprite a ground skill unit can render — its placement sprite plus any
/// trigger burst — for effects backed by a `Spr`/`SprBurst` spec. These are not
/// referenced by any RSW or effect-module sprite list, so they must be collected
/// here to be preloaded (Str-backed units are already covered by `effect_str_names`).
pub fn skill_unit_sprite_paths() -> Vec<&'static str> {
    use crate::spec::EffectSpec;
    use crate::table::effect_spec;
    let mut out = Vec::new();
    // rAthena's unit ids end below 0x190 (`UNT_MAX`)
    for unit_id in 0u16..0x190 {
        for id in [
            skill_unit_effect(unit_id, true),
            skill_unit_effect(unit_id, false),
            trap_idle_effect(unit_id),
            trap_trigger_effect(unit_id),
        ]
        .into_iter()
        .flatten()
        {
            match effect_spec(id) {
                Some(EffectSpec::Spr { sprite, .. })
                | Some(EffectSpec::SprBurst { sprite, .. }) => out.push(sprite),
                _ => {}
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn representative_units_map_and_unknowns_are_none() {
        assert_eq!(
            skill_unit_effect(UNT_FIREWALL, true),
            Some(EffectId::Firewall)
        );
        assert_eq!(
            skill_unit_effect(UNT_ICEWALL, true),
            Some(EffectId::Icewall)
        );
        assert_eq!(
            skill_unit_effect(UNT_SANCTUARY, true),
            Some(EffectId::BottomSanc)
        );
        assert_eq!(
            skill_unit_effect(UNT_LANDPROTECTOR, true),
            Some(EffectId::BottomLa)
        );
        assert_eq!(
            skill_unit_effect(UNT_POEMBRAGI, true),
            Some(EffectId::BottomPoembragi)
        );
        assert_eq!(
            skill_unit_effect(UNT_CALLFAMILY, true),
            Some(EffectId::Portal3)
        );
        assert_eq!(
            skill_unit_effect(UNT_DARK_TRANSFER, true),
            Some(EffectId::BottomTransfer)
        );
        assert_eq!(skill_unit_effect(0x86, true), None);
        assert_eq!(skill_unit_effect(0x00, true), None);
    }

    #[test]
    fn warp_and_fire_pillar_split_by_unit_state() {
        assert_eq!(
            skill_unit_effect(UNT_WARP_WAITING, true),
            Some(EffectId::Portal2)
        );
        assert_eq!(
            skill_unit_effect(UNT_WARP_WAITING, false),
            Some(EffectId::Portal)
        );
        assert_eq!(
            skill_unit_effect(UNT_WARP_ACTIVE, true),
            Some(EffectId::Readyportal2)
        );
        assert_eq!(
            skill_unit_effect(UNT_WARP_ACTIVE, false),
            Some(EffectId::Readyportal2)
        );
        assert_eq!(
            skill_unit_effect(UNT_FIREPILLAR_WAITING, true),
            Some(EffectId::Firepillaron)
        );
        assert_eq!(
            skill_unit_effect(UNT_FIREPILLAR_ACTIVE, true),
            Some(EffectId::Firepillarbomb)
        );
    }

    #[test]
    fn traps_show_a_model_at_placement_and_burst_only_when_sprung() {
        // Placement: an RSM model, not a sprite effect.
        assert_eq!(skill_unit_effect(UNT_FREEZINGTRAP, true), None);
        assert_eq!(skill_unit_effect(UNT_ANKLESNARE, true), None);
        assert_eq!(
            trap_model_name(UNT_ANKLESNARE),
            Some("외부소품\\트랩01.rsm")
        );
        assert_eq!(
            trap_model_name(UNT_BLASTMINE),
            Some("외부소품\\트랩03_3.rsm")
        );
        assert_eq!(trap_model_name(UNT_SAFETYWALL), None);
        // Trigger: the burst fires for explosive traps, not for holders.
        assert_eq!(
            trap_trigger_effect(UNT_FREEZINGTRAP),
            Some(EffectId::Freezing)
        );
        assert_eq!(
            trap_trigger_effect(UNT_BLASTMINE),
            Some(EffectId::Blastminebomb)
        );
        assert_eq!(trap_trigger_effect(UNT_ANKLESNARE), None);
        assert_eq!(trap_trigger_effect(UNT_SKIDTRAP), None);
        // Idle: only Shock Wave Trap animates while it waits.
        assert_eq!(trap_idle_effect(UNT_SHOCKWAVE), Some(EffectId::Shockwave));
        assert_eq!(trap_trigger_effect(UNT_SHOCKWAVE), None);
        assert_eq!(trap_idle_effect(UNT_BLASTMINE), None);
        assert_eq!(trap_idle_effect(UNT_ANKLESNARE), None);
    }

    #[test]
    fn only_icewall_blastmine_and_claymore_take_a_plain_attack() {
        assert!(is_attackable_skill_unit(UNT_ICEWALL));
        assert!(is_attackable_skill_unit(UNT_BLASTMINE));
        assert!(is_attackable_skill_unit(UNT_CLAYMORETRAP));
        assert!(!is_attackable_skill_unit(UNT_ANKLESNARE));
        assert!(!is_attackable_skill_unit(UNT_SKIDTRAP));
        assert!(!is_attackable_skill_unit(UNT_SAFETYWALL));
        assert!(!is_attackable_skill_unit(UNT_USED_TRAPS));
    }

    #[test]
    fn warp_and_firewall_play_on_entry_kaen_is_chance_gated() {
        let warp = skill_unit_entry_sound(UNT_WARP_ACTIVE).expect("warp entry sound");
        assert_eq!(warp.wave, "effect\\EF_Portal.wav");
        assert_eq!(warp.one_in, 1);
        assert_eq!(
            skill_unit_entry_sound(UNT_WARP_WAITING).map(|s| s.wave),
            Some("effect\\EF_Portal.wav")
        );
        assert_eq!(
            skill_unit_entry_sound(UNT_FIREWALL).map(|s| s.wave),
            Some("effect\\EF_FireWall.wav")
        );
        assert_eq!(skill_unit_entry_sound(UNT_KAEN).map(|s| s.one_in), Some(8));
        assert!(skill_unit_entry_sound(UNT_ICEWALL).is_none());
    }
}
