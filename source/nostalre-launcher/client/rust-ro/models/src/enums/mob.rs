use enum_macro::{WithMaskValueU32, WithNumberValue, WithStringValue};

use crate::enums::{EnumWithMaskValueU32, EnumWithNumberValue, EnumWithStringValue};

#[derive(WithStringValue, WithNumberValue, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobClass {
    #[value = 0]
    #[value_string = "Class_Normal"]
    Normal,
    #[value_string = "Class_Boss"]
    Boss,
    #[value_string = "Class_Guardian"]
    Guardian,
    #[value_string = "Class_All"]
    All,
}

#[derive(WithStringValue, WithNumberValue, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobRace {
    #[value_string = "RC_All"]
    #[value_string = "All"]
    #[value = 0]
    All,
    #[value_string = "RC_Angel"]
    #[value_string = "Angel"]
    Angel,
    #[value_string = "RC_Brute"]
    #[value_string = "Brute"]
    Brute,
    #[value_string = "RC_DemiHuman"]
    #[value_string = "DemiHuman"]
    #[value_string = "Demihuman"]
    DemiHuman,
    #[value_string = "RC_Demon"]
    #[value_string = "Demon"]
    Demon,
    #[value_string = "RC_Dragon"]
    #[value_string = "Dragon"]
    Dragon,
    #[value_string = "RC_Fish"]
    #[value_string = "Fish"]
    Fish,
    #[value_string = "RC_Formless"]
    #[value_string = "Formless"]
    Formless,
    #[value_string = "RC_Insect"]
    #[value_string = "Insect"]
    Insect,
    #[value_string = "RC_Plant"]
    #[value_string = "Plant"]
    Plant,
    #[value_string = "RC_Player_Human"]
    PlayerHuman,
    #[value_string = "RC_Player_Doram"]
    PlayerDoram,
    #[value_string = "RC_Undead"]
    #[value_string = "Undead"]
    RUndead,
}

#[derive(WithStringValue, WithNumberValue, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobGroup {
    #[value_string = "RC2_Goblin"]
    #[value = 0]
    Goblin,
    #[value_string = "RC2_Kobold"]
    Kobold,
    #[value_string = "RC2_Golem"]
    Golem,
    #[value_string = "RC2_Orc"]
    Orc,
    #[value_string = "RC2_Guardian"]
    Guardian,
    #[value_string = "RC2_Ninja"]
    Ninja,
    #[value_string = "RC2_GVG"]
    GVG,
    #[value_string = "RC2_Battlefield"]
    Battlefield,
    #[value_string = "RC2_Treasure"]
    Treasure,
    #[value_string = "RC2_BioLab"]
    BioLab,
    #[value_string = "RC2_Manuk"]
    Manuk,
    #[value_string = "RC2_Splendide"]
    Splendide,
    #[value_string = "RC2_Scaraba"]
    Scaraba,
    #[value_string = "RC2_Clocktower"]
    Clocktower,
    #[value_string = "RC2_Thanatos"]
    Thanatos,
    #[value_string = "RC2_Faceworm"]
    Faceworm,
    #[value_string = "RC2_Hearthunter"]
    Hearthunter,
    #[value_string = "RC2_Rockridge"]
    Rockridge,
    #[value_string = "RC2_Werner_Lab"]
    WernerLab,
    #[value_string = "RC2_Temple_Demon"]
    TempleDemon,
    #[value_string = "RC2_Illusion_Vampire"]
    IllusionVampire,
    #[value_string = "RC2_Malangdo"]
    Malangdo,
    #[value_string = "RC2_Rachel_Sanctuary"]
    RachelSanctuary,
}

/// Monster behavior mode flags (matching Hercules MD_* flags)
/// See: https://github.com/HerculesWS/Hercules/blob/master/doc/mob_db_mode_list.md
#[derive(WithMaskValueU32, Debug, Copy, Clone, PartialEq, Eq)]
pub enum MobMode {
    /// Enables the mob to move/chase characters
    #[mask_value = 0x0001]
    CanMove,
    /// Loot nearby items on the ground when idle
    #[mask_value = 0x0002]
    Looter,
    /// Aggressive mob, will look for nearby players to attack
    #[mask_value = 0x0004]
    Aggressive,
    /// When a nearby mob of the same class attacks, join them
    #[mask_value = 0x0008]
    Assist,
    /// Chase characters who start casting on them (idle or walking)
    #[mask_value = 0x0010]
    CastSensorIdle,
    /// Immune to certain status changes and skills
    #[mask_value = 0x0020]
    Boss,
    /// Always receives 1 damage from attacks
    #[mask_value = 0x0040]
    Plant,
    /// Enables mob to attack/retaliate when within attack range
    #[mask_value = 0x0080]
    CanAttack,
    /// Can detect and attack characters in hiding/cloak
    #[mask_value = 0x0100]
    Detector,
    /// Chase characters who start casting on them (while chasing)
    #[mask_value = 0x0200]
    CastSensorChase,
    /// Allows chasing mobs to switch targets if another player is in attack range
    #[mask_value = 0x0400]
    ChangeChase,
    /// Hyper-active mob that auto-switches to closest target
    #[mask_value = 0x0800]
    Angry,
    /// Switch targets when attacked while attacking someone else
    #[mask_value = 0x1000]
    ChangeTargetMelee,
    /// Switch targets when attacked while chasing another character
    #[mask_value = 0x2000]
    ChangeTargetChase,
    /// Only aggressive against characters 5+ levels below its own level
    #[mask_value = 0x4000]
    TargetWeak,
    /// Immune to knockback effects
    #[mask_value = 0x8000]
    NoKnockback,
    /// Pick a new random target in range on each attack/skill
    #[mask_value = 0x10000]
    RandomTarget,
}

impl MobMode {
    /// Convert Hercules AI type to mode flags
    /// See: https://github.com/HerculesWS/Hercules/blob/master/doc/mob_db_mode_list.md
    pub fn from_ai_type(ai_type: i32) -> u32 {
        use MobMode::*;
        match ai_type {
            // AI 01 (0x0081): Passive
            1 => CanMove.as_flag() | CanAttack.as_flag(),
            // AI 02 (0x0083): Passive, looter
            2 => CanMove.as_flag() | Looter.as_flag() | CanAttack.as_flag(),
            // AI 03 (0x1089): Passive, assist, change-target melee
            3 => CanMove.as_flag() | Assist.as_flag() | CanAttack.as_flag() | ChangeTargetMelee.as_flag(),
            // AI 04 (0x3885): Angry, change-target melee/chase
            4 => CanMove.as_flag() | Aggressive.as_flag() | CanAttack.as_flag() | Angry.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag(),
            // AI 05 (0x2085): Aggressive, change-target chase
            5 => CanMove.as_flag() | Aggressive.as_flag() | CanAttack.as_flag() | ChangeTargetChase.as_flag(),
            // AI 06 (0x0000): Passive, immobile, can't attack (plants)
            6 => 0,
            // AI 07 (0x108B): Passive, looter, assist, change-target melee
            7 => CanMove.as_flag() | Looter.as_flag() | Assist.as_flag() | CanAttack.as_flag() | ChangeTargetMelee.as_flag(),
            // AI 08 (0x6085): Aggressive, change-target chase, target weak
            8 => CanMove.as_flag() | Aggressive.as_flag() | CanAttack.as_flag() | ChangeTargetChase.as_flag() | TargetWeak.as_flag(),
            // AI 09 (0x3095): Aggressive, change-target melee/chase, cast sensor idle (Guardian)
            9 => CanMove.as_flag() | Aggressive.as_flag() | CastSensorIdle.as_flag() | CanAttack.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag(),
            // AI 10 (0x0084): Aggressive, immobile (e.g. Hydra)
            10 => Aggressive.as_flag() | CanAttack.as_flag(),
            // AI 11 (0x0084): Aggressive, immobile (Guardian)
            11 => Aggressive.as_flag() | CanAttack.as_flag(),
            // AI 12 (0x2085): Aggressive, change-target chase (Guardian)
            12 => CanMove.as_flag() | Aggressive.as_flag() | CanAttack.as_flag() | ChangeTargetChase.as_flag(),
            // AI 13 (0x308D): Aggressive, change-target melee/chase, assist
            13 => CanMove.as_flag() | Aggressive.as_flag() | Assist.as_flag() | CanAttack.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag(),
            // AI 17 (0x0091): Passive, cast sensor idle
            17 => CanMove.as_flag() | CastSensorIdle.as_flag() | CanAttack.as_flag(),
            // AI 19 (0x3095): Aggressive, change-target melee/chase, cast sensor idle
            19 => CanMove.as_flag() | Aggressive.as_flag() | CastSensorIdle.as_flag() | CanAttack.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag(),
            // AI 20 (0x3295): Aggressive, change-target melee/chase, cast sensor idle/chase
            20 => CanMove.as_flag() | Aggressive.as_flag() | CastSensorIdle.as_flag() | CanAttack.as_flag() | CastSensorChase.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag(),
            // AI 21 (0x3695): Aggressive, change-target melee/chase, cast sensor idle/chase, change-chase
            21 => CanMove.as_flag() | Aggressive.as_flag() | CastSensorIdle.as_flag() | CanAttack.as_flag() | CastSensorChase.as_flag() | ChangeChase.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag(),
            // AI 25 (0x0001): Passive, can't attack (pet)
            25 => CanMove.as_flag(),
            // AI 26 (0xB695): Aggressive, change-target melee/chase, cast sensor idle/chase, change-chase, random target
            26 => CanMove.as_flag() | Aggressive.as_flag() | CastSensorIdle.as_flag() | CanAttack.as_flag() | CastSensorChase.as_flag() | ChangeChase.as_flag() | ChangeTargetMelee.as_flag() | ChangeTargetChase.as_flag() | RandomTarget.as_flag(),
            // AI 27 (0x8084): Aggressive, immobile, random target
            27 => Aggressive.as_flag() | CanAttack.as_flag() | RandomTarget.as_flag(),
            // Default: Passive (CanMove + CanAttack)
            _ => CanMove.as_flag() | CanAttack.as_flag(),
        }
    }

    pub fn is_aggressive(mode: u32) -> bool {
        mode & MobMode::Aggressive.as_flag() != 0
    }

    pub fn is_assist(mode: u32) -> bool {
        mode & MobMode::Assist.as_flag() != 0
    }

    pub fn can_move(mode: u32) -> bool {
        mode & MobMode::CanMove.as_flag() != 0
    }

    pub fn can_attack(mode: u32) -> bool {
        mode & MobMode::CanAttack.as_flag() != 0
    }

    pub fn is_looter(mode: u32) -> bool {
        mode & MobMode::Looter.as_flag() != 0
    }

    pub fn has_cast_sensor_idle(mode: u32) -> bool {
        mode & MobMode::CastSensorIdle.as_flag() != 0
    }

    pub fn has_cast_sensor_chase(mode: u32) -> bool {
        mode & MobMode::CastSensorChase.as_flag() != 0
    }

    pub fn can_change_target_melee(mode: u32) -> bool {
        mode & MobMode::ChangeTargetMelee.as_flag() != 0
    }

    pub fn can_change_target_chase(mode: u32) -> bool {
        mode & MobMode::ChangeTargetChase.as_flag() != 0
    }

    pub fn is_angry(mode: u32) -> bool {
        mode & MobMode::Angry.as_flag() != 0
    }

    pub fn targets_weak(mode: u32) -> bool {
        mode & MobMode::TargetWeak.as_flag() != 0
    }

    pub fn has_random_target(mode: u32) -> bool {
        mode & MobMode::RandomTarget.as_flag() != 0
    }

    pub fn is_detector(mode: u32) -> bool {
        mode & MobMode::Detector.as_flag() != 0
    }

    pub fn is_boss(mode: u32) -> bool {
        mode & MobMode::Boss.as_flag() != 0
    }

    pub fn is_plant(mode: u32) -> bool {
        mode & MobMode::Plant.as_flag() != 0
    }

    pub fn is_knockback_immune(mode: u32) -> bool {
        mode & MobMode::NoKnockback.as_flag() != 0
    }
}
