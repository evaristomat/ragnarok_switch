use models::enums::skill_enums::SkillEnum;
use nostalre_effects::merc_skill_base;
use nostalre_formats::act::SpriteActionType;

/// Who owns a skill, derived purely from its id. Mercenary and homunculus skills
/// occupy fixed, disjoint id ranges, so the caster is known without any runtime
/// companion state — this must hold even for hotkeys restored at login before a
/// companion exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillCaster {
    Player,
    Mercenary,
    Homunculus,
}

/// rAthena's `HM_SKILLBASE` and `MAX_HOMUNSKILL`, `MC_SKILLBASE` and `MAX_MERCSKILL` (`mmo.hpp`).
const HOMUNCULUS_SKILLS: std::ops::Range<u32> = 8001..8001 + 59;
const MERCENARY_SKILLS: std::ops::Range<u32> = 8201..8201 + 41;

pub fn skill_caster(skill: SkillEnum) -> SkillCaster {
    let id = skill.id();
    if HOMUNCULUS_SKILLS.contains(&id) {
        SkillCaster::Homunculus
    } else if MERCENARY_SKILLS.contains(&id) {
        SkillCaster::Mercenary
    } else {
        SkillCaster::Player
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillMotionType {
    Attack,
    Throw,
    Attack2,
    Pickup,
    Sing,
    Dance,
    Stand,
    Walk,
    Skill,
    /// Freezes on a single frame, see [`skill_pose`].
    Pose,
}

/// A stance: frames `from..=frame` of one action group played once, then `frame` held still until the hold is up,
/// counted from the motion's start (kRO's `SetSkillMotion`, 0x75f0c0, sets its hold-until time as it starts).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkillPose {
    pub action: usize,
    pub from: usize,
    pub frame: usize,
    pub hold: PoseHold,
    /// A damage skill's: how long the frames play, as kRO times them whatever the server's delay; `None` plays
    /// them at the art's own speed
    pub play_secs: Option<f32>,
}

/// How long a pose holds its frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PoseHold {
    Secs(f32),
    /// The caster's combo window, as the server last sent it (0x01d2): kRO's combo motions take it from the table
    /// its 0x01d2 handler keeps per unit (0x875c80), and hold for no time while that has none for the unit.
    ComboDelay,
}

impl SkillPose {
    /// The hold in seconds, given the caster's last combo window.
    pub fn hold_secs(&self, combo_delay_ms: Option<u32>) -> f32 {
        match self.hold {
            PoseHold::Secs(secs) => secs,
            PoseHold::ComboDelay => combo_delay_ms.map_or(0.0, |ms| ms as f32 / 1000.0),
        }
    }
}

const STANCE_HOLD_SECS: f32 = 2.0;

/// The rig's screen rate: kRO steps an actor's motion once per drawn frame above 62.5 fps
/// (`docs/nostalre-kro-exe-skills.md`, "The update rate"), and its 144 Hz screen is where its timings were seen.
const RIG_UPDATES_PER_SEC: f32 = 144.0;

/// One update of kRO's actor motion on the rig: how long a hold of no time still shows its frame.
pub const ONE_UPDATE_SECS: f32 = 1.0 / RIG_UPDATES_PER_SEC;

/// How long kRO takes over `steps` frame steps at motion speed `speed` (`SetMotionSpeed`, 0x75ef00: a step every
/// `speed` updates).
const fn kro_motion_secs(steps: usize, speed: usize) -> f32 {
    (steps * speed) as f32 / RIG_UPDATES_PER_SEC
}

const fn stance(action: SpriteActionType, frame: usize) -> SkillPose {
    SkillPose {
        action: action as usize,
        from: frame,
        frame,
        hold: PoseHold::Secs(STANCE_HOLD_SECS),
        play_secs: None,
    }
}

/// A combo blow's motion from kRO's skill hit (0x767340): `from..=frame` of `action` at motion speed `speed`, then
/// `frame` held for the combo window. A single frame plays no steps, so its speed (0 where kRO sets none) is moot.
const fn combo(action: SpriteActionType, from: usize, frame: usize, speed: usize) -> SkillPose {
    SkillPose {
        action: action as usize,
        from,
        frame,
        hold: PoseHold::ComboDelay,
        play_secs: Some(kro_motion_secs(frame - from, speed)),
    }
}

/// The stances that pose the body instead of animating it: the taekwon kicks, Critical Explosion, Scream, Aura Blade.
pub fn skill_pose(skill: SkillEnum) -> Option<SkillPose> {
    use SkillEnum as S;
    use SpriteActionType::{Attack3, Pickup, ReadyFight, Skill};

    match merc_skill_base(skill) {
        S::TkReadystorm => Some(stance(Skill, 0)),
        S::TkReadydown => Some(stance(Skill, 2)),
        S::TkReadyturn => Some(stance(Skill, 3)),
        S::TkReadycounter => Some(stance(Skill, 4)),
        S::TkDodge => Some(stance(Pickup, 1)),
        // draws back and crouches low, fist forward, and soon stands (kRO, by feel)
        S::MoExplosionspirits => Some(SkillPose {
            from: 1,
            hold: PoseHold::Secs(0.7),
            ..stance(Skill, 2)
        }),
        // the sword raised in the combat stance, then standing (kRO, by feel)
        S::LkAurablade => Some(SkillPose {
            from: 0,
            hold: PoseHold::Secs(1.0),
            ..stance(ReadyFight, 5)
        }),
        // kRO's combo blows (0x767340's `SetSkillMotion` and `SetMotionSpeed` calls, `motion_calls.py`). The third
        // attack's punches take 0.19 s on the rig and its last is held to 1 s, the 994 ms rAthena sent (October 15)
        S::MoTripleattack => Some(combo(Attack3, 0, 7, 4)),
        S::MoChaincombo | S::SrSkynetblow | S::SrTigercannon => Some(combo(Skill, 5, 5, 0)),
        S::MoCombofinish | S::SrEarthshaker => Some(combo(Skill, 1, 2, 6)),
        S::ChTigerfist | S::SrDragoncombo => Some(combo(Skill, 4, 4, 6)),
        S::SrFallenempire => Some(combo(Skill, 3, 3, 6)),
        // the Dancer's kick, held (kRO, by feel)
        S::DcScream => Some(SkillPose {
            hold: PoseHold::Secs(1.75),
            ..stance(Skill, 0)
        }),
        _ => None,
    }
}

/// A skill that leaves its caster in the combat stance; any other ends standing (kRO: Blessing, Absorb Spirit
/// Sphere, Energy Coat, Frost Weapon, Aura Blade stand)
pub fn ends_ready(skill: SkillEnum) -> bool {
    use SkillEnum as S;
    matches!(skill, S::MoCallspirits | S::LkHeadcrush | S::LkJointbeat)
}

/// A skill whose blow lands as it goes off, not on its swing's keyframe (kRO: Magnum Break's blast)
pub fn hits_at_once(skill: SkillEnum) -> bool {
    merc_skill_base(skill) == SkillEnum::SmMagnum
}

/// What a performer plays once a skill's motion ends, standing: `frames` of the skill group looped, for `secs` or,
/// without, until its next state change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Performance {
    pub frames: (usize, usize),
    pub secs: Option<f32>,
}

/// After a song: the instrument held up, strummed (the Bard's ACT; frames 0, 4 and 5 stretch the arm out)
const STRUM: Performance = Performance {
    frames: (1, 3),
    secs: None,
};

/// The performance a skill leaves its caster in (kRO's 2022 client): a song's strum (the Bard's, the Dancer's, an
/// ensemble's caster; the partner gets no cast to play), Frost Joker's singing, the arm out, for a while (by feel).
pub fn performance(skill: SkillEnum) -> Option<Performance> {
    if skill == SkillEnum::BaFrostjoker {
        return Some(Performance {
            frames: (4, 5),
            secs: Some(2.5),
        });
    }
    is_song(skill).then_some(STRUM)
}

/// A performance by rAthena's skill db (`IsSong`, `IsEnsemble`): Benediction is an ensemble but no song
fn is_song(skill: SkillEnum) -> bool {
    skill_motion_type(skill) == SkillMotionType::Sing
        || skill.is_song()
        || (skill.is_ensemble() && skill != SkillEnum::PrBenedictio)
}

pub fn skill_motion_type(skill: SkillEnum) -> SkillMotionType {
    use SkillEnum as S;
    use SkillMotionType::*;

    let skill = merc_skill_base(skill);
    if skill_pose(skill).is_some() {
        return Pose;
    }
    match skill {
        S::SmBash
        | S::SmMagnum
        | S::McMammonite
        | S::AcDouble
        | S::AcShower
        | S::AcChargearrow
        | S::KnPierce
        | S::KnBrandishspear
        | S::KnSpearstab
        | S::KnBowlingbash
        | S::KnAutocounter
        | S::KnChargeatk
        | S::BsSkintemper
        | S::BsHammerfall
        | S::HtPower
        | S::HtPhantasmic
        | S::CrHolycross
        | S::RgBackstap
        | S::RgRaid
        | S::RgIntimidate
        | S::RgCloseconfine
        | S::AsSonicblow
        | S::MoInvestigate
        | S::MoFingeroffensive
        | S::PaPressure
        | S::PaSacrifice
        | S::ChPalmstrike
        | S::ChChaincrush
        | S::AscBreaker
        | S::AscMeteorassault
        | S::HwMagicpower
        | S::SnSharpshooting
        | S::LkSpiralpierce
        | S::LkHeadcrush
        | S::LkJointbeat => Attack,

        S::BaMusicalstrike | S::DcThrowarrow | S::CgArrowvulcan => Attack2,

        S::KnSpearboomerang
        | S::CrShieldcharge
        | S::CrShieldboomerang
        | S::PaShieldchain
        | S::AmPotionpitcher
        | S::AmAcidterror
        | S::AmDemonstration
        | S::AmCannibalize
        | S::TfThrowstone
        | S::TfSprinklesand
        | S::AsVenomknife => Throw,

        S::HtSkidtrap
        | S::HtLandmine
        | S::HtAnklesnare
        | S::HtShockwave
        | S::HtSandman
        | S::HtFlasher
        | S::HtFreezingtrap
        | S::HtBlastmine
        | S::HtClaymoretrap
        | S::HtRemovetrap
        | S::HtTalkiebox
        | S::BsGreed => Pickup,

        S::BaAppleidun | S::BaDissonance | S::BaWhistle | S::BaAssassincross | S::BaPoembragi => {
            Sing
        }

        S::DcWinkcharm
        | S::DcFortunekiss
        | S::DcUglydance
        | S::DcHumming
        | S::DcDontforgetme
        | S::DcServiceforyou
        | S::BdLullaby
        | S::BdRichmankim
        | S::BdEternalchaos
        | S::BdDrumbattlefield
        | S::BdSiegfried
        | S::CgHermode
        | S::BdRingnibelungen
        | S::BdRokisweil
        | S::BdIntoabyss
        | S::CgMoonlit
        | S::CgMarionette => Dance,

        S::AlIncagi
        | S::CrAutoguard
        | S::CrReflectshield
        | S::CrDefender
        | S::MoSteelbody
        | S::MoBladestop
        | S::BdAdaptation
        | S::LkParrying
        | S::PaGospel
        | S::SnSight
        | S::WsMeltdown
        | S::WsCartboost
        | S::ChSoulcollect => Stand,

        S::TkRun => Walk,

        _ => Skill,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attack_skills_return_attack() {
        assert_eq!(
            skill_motion_type(SkillEnum::SmBash),
            SkillMotionType::Attack
        );
        assert_eq!(
            skill_motion_type(SkillEnum::AcDouble),
            SkillMotionType::Attack
        );
        assert_eq!(
            skill_motion_type(SkillEnum::AsSonicblow),
            SkillMotionType::Attack
        );
        assert_eq!(
            skill_motion_type(SkillEnum::LkSpiralpierce),
            SkillMotionType::Attack
        );
    }

    #[test]
    fn bard_musical_strike_returns_attack2() {
        assert_eq!(
            skill_motion_type(SkillEnum::BaMusicalstrike),
            SkillMotionType::Attack2
        );
        assert_eq!(
            skill_motion_type(SkillEnum::CgArrowvulcan),
            SkillMotionType::Attack2
        );
    }

    #[test]
    fn throw_skills_return_throw() {
        assert_eq!(
            skill_motion_type(SkillEnum::KnSpearboomerang),
            SkillMotionType::Throw
        );
        assert_eq!(
            skill_motion_type(SkillEnum::TfThrowstone),
            SkillMotionType::Throw
        );
    }

    #[test]
    fn trap_skills_return_pickup() {
        assert_eq!(
            skill_motion_type(SkillEnum::HtLandmine),
            SkillMotionType::Pickup
        );
        assert_eq!(
            skill_motion_type(SkillEnum::HtAnklesnare),
            SkillMotionType::Pickup
        );
    }

    #[test]
    fn frost_joker_sings_arm_out_for_a_while() {
        assert_eq!(
            performance(SkillEnum::BaFrostjoker),
            Some(Performance {
                frames: (4, 5),
                secs: Some(2.5)
            })
        );
        assert_eq!(performance(SkillEnum::BaWhistle), Some(STRUM));
        assert_eq!(performance(SkillEnum::BaMusicalstrike), None);
    }

    #[test]
    fn every_song_and_ensemble_the_server_knows_strums_but_benediction() {
        assert_eq!(performance(SkillEnum::CgHermode), Some(STRUM));
        assert_eq!(performance(SkillEnum::CgMoonlit), Some(STRUM));
        assert_eq!(performance(SkillEnum::BdLullaby), Some(STRUM));
        assert_eq!(performance(SkillEnum::PrBenedictio), None);
    }

    #[test]
    fn triple_attack_plays_the_third_attack_to_its_last_punch() {
        let pose = skill_pose(SkillEnum::MoTripleattack).unwrap();
        assert_eq!(
            (pose.action, pose.from, pose.frame),
            (SpriteActionType::Attack3 as usize, 0, 7)
        );
        let play = pose.play_secs.unwrap();
        assert!((play - 0.19).abs() < 0.01, "the rig's 0.19 s, got {play}");
        // held from the motion's start for the window rAthena sent: a second in all, as the rig showed
        assert_eq!(pose.hold_secs(Some(994)), 0.994);
        assert_eq!(pose.hold_secs(None), 0.0, "no window yet: no hold");
        assert_eq!(
            skill_motion_type(SkillEnum::MoTripleattack),
            SkillMotionType::Pose
        );
    }

    #[test]
    fn combo_blows_take_kros_frames_and_hold_the_combo_window() {
        use SpriteActionType::{Attack3, Skill};
        // (skill, action, from, frame, speed) as kRO's skill hit 0x767340 sets them
        for (s, action, from, frame, speed) in [
            (SkillEnum::MoTripleattack, Attack3, 0, 7, 4),
            (SkillEnum::MoChaincombo, Skill, 5, 5, 0),
            (SkillEnum::MoCombofinish, Skill, 1, 2, 6),
            (SkillEnum::ChTigerfist, Skill, 4, 4, 6),
            (SkillEnum::SrDragoncombo, Skill, 4, 4, 6),
            (SkillEnum::SrSkynetblow, Skill, 5, 5, 0),
            (SkillEnum::SrEarthshaker, Skill, 1, 2, 6),
            (SkillEnum::SrFallenempire, Skill, 3, 3, 6),
            (SkillEnum::SrTigercannon, Skill, 5, 5, 0),
        ] {
            assert_eq!(skill_motion_type(s), SkillMotionType::Pose, "{s:?}");
            let pose = skill_pose(s).expect("{s:?} poses");
            assert_eq!(
                (pose.action, pose.from, pose.frame, pose.hold),
                (action as usize, from, frame, PoseHold::ComboDelay),
                "{s:?}"
            );
            assert_eq!(
                pose.play_secs,
                Some(((frame - from) * speed) as f32 / 144.0),
                "{s:?}"
            );
        }
    }

    #[test]
    fn magnum_break_hits_at_once() {
        assert!(hits_at_once(SkillEnum::SmMagnum));
        assert!(hits_at_once(SkillEnum::MsMagnum));
        assert!(!hits_at_once(SkillEnum::SmBash));
    }

    #[test]
    fn only_the_skills_seen_holding_it_end_ready() {
        assert!(ends_ready(SkillEnum::MoCallspirits));
        assert!(ends_ready(SkillEnum::LkHeadcrush));
        assert!(ends_ready(SkillEnum::LkJointbeat));
        assert!(!ends_ready(SkillEnum::MoAbsorbspirits));
        assert!(!ends_ready(SkillEnum::AlBlessing));
        assert!(!ends_ready(SkillEnum::LkAurablade));
    }

    #[test]
    fn bard_songs_return_sing() {
        assert_eq!(
            skill_motion_type(SkillEnum::BaPoembragi),
            SkillMotionType::Sing
        );
        assert_eq!(
            skill_motion_type(SkillEnum::BaAppleidun),
            SkillMotionType::Sing
        );
    }

    #[test]
    fn dancer_skills_return_dance() {
        assert_eq!(
            skill_motion_type(SkillEnum::DcFortunekiss),
            SkillMotionType::Dance
        );
        assert_eq!(
            skill_motion_type(SkillEnum::BdLullaby),
            SkillMotionType::Dance
        );
    }

    #[test]
    fn stand_skills_return_stand() {
        assert_eq!(
            skill_motion_type(SkillEnum::CrAutoguard),
            SkillMotionType::Stand
        );
        assert_eq!(
            skill_motion_type(SkillEnum::LkParrying),
            SkillMotionType::Stand
        );
    }

    #[test]
    fn stances_pose_the_body_and_running_walks() {
        use SpriteActionType::{Pickup, ReadyFight, Skill};
        for (s, action, from, frame, hold_secs) in [
            (SkillEnum::TkReadystorm, Skill, 0, 0, 2.0),
            (SkillEnum::TkReadydown, Skill, 2, 2, 2.0),
            (SkillEnum::TkReadyturn, Skill, 3, 3, 2.0),
            (SkillEnum::TkReadycounter, Skill, 4, 4, 2.0),
            (SkillEnum::TkDodge, Pickup, 1, 1, 2.0),
            (SkillEnum::MoExplosionspirits, Skill, 1, 2, 0.7),
            (SkillEnum::LkAurablade, ReadyFight, 0, 5, 1.0),
            (SkillEnum::DcScream, Skill, 0, 0, 1.75),
        ] {
            assert_eq!(skill_motion_type(s), SkillMotionType::Pose, "{s:?}");
            let pose = skill_pose(s).expect("{s:?} poses");
            assert_eq!(
                (pose.action, pose.from, pose.frame, pose.hold),
                (action as usize, from, frame, PoseHold::Secs(hold_secs)),
                "{s:?}"
            );
        }
        // Running plays the walk motion, not an idle stand or a cast.
        assert_eq!(skill_motion_type(SkillEnum::TkRun), SkillMotionType::Walk);
        assert!(skill_pose(SkillEnum::TkRun).is_none());
    }

    #[test]
    fn mercenary_bow_skills_animate_as_ranged_attack() {
        for s in [
            SkillEnum::MaDouble,
            SkillEnum::MaShower,
            SkillEnum::MaChargearrow,
        ] {
            assert_eq!(
                skill_motion_type(s),
                SkillMotionType::Attack,
                "{s:?} should animate as a ranged attack"
            );
        }
        assert_eq!(
            skill_motion_type(SkillEnum::MerQuicken),
            SkillMotionType::Skill
        );
    }

    #[test]
    fn heal_defaults_to_skill() {
        assert_eq!(skill_motion_type(SkillEnum::AlHeal), SkillMotionType::Skill);
    }

    #[test]
    fn every_homunculus_and_mercenary_skill_has_its_caster() {
        assert_eq!(skill_caster(SkillEnum::HlifHeal), SkillCaster::Homunculus);
        assert_eq!(
            skill_caster(SkillEnum::MhBlazingLava),
            SkillCaster::Homunculus
        );
        assert_eq!(skill_caster(SkillEnum::MsBash), SkillCaster::Mercenary);
        assert_eq!(
            skill_caster(SkillEnum::MerInvincibleoff2),
            SkillCaster::Mercenary
        );
        assert_eq!(skill_caster(SkillEnum::RkEnchantblade), SkillCaster::Player);
        for skill in SkillEnum::ALL {
            let name = skill.to_name();
            let named = |prefixes: &[&str]| prefixes.iter().any(|p| name.starts_with(p));
            let expected = if named(&["MH_", "HLIF_", "HAMI_", "HFLI_", "HVAN_"]) {
                SkillCaster::Homunculus
            } else if named(&["MS_", "MA_", "ML_", "MER_"]) {
                SkillCaster::Mercenary
            } else {
                SkillCaster::Player
            };
            assert_eq!(skill_caster(skill), expected, "{name}");
        }
    }
}
