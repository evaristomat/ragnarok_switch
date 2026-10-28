pub use nostalre_ai as ai;
pub use nostalre_ai::{
    ActorView, AiContext, AiIntent, AiState, CommandKind, CompanionAi, Motion, OwnerCommand,
};

use crate::event::SkillInfo;

/// How full a gauge is, `cur` of `max`; empty with no maximum yet
fn gauge(cur: u32, max: u32) -> f32 {
    if max > 0 {
        cur as f32 / max as f32
    } else {
        0.0
    }
}

/// A gauge of the player's elemental that 0x081e changes (rAthena's `_sp`, the four `clif_elemental_updatestatus`
/// sends)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementalPar {
    Hp,
    MaxHp,
    Sp,
    MaxSp,
}

impl ElementalPar {
    /// rAthena's `SP_HP` to `SP_MAXSP` (`map.hpp`), as kRO's 0x081e handler (0x7bae50) switches over them
    pub fn from_var(var: u16) -> Option<Self> {
        match var {
            5 => Some(Self::Hp),
            6 => Some(Self::MaxHp),
            7 => Some(Self::Sp),
            8 => Some(Self::MaxSp),
            _ => None,
        }
    }
}

/// The player's elemental as kRO's client keeps it: its unit and its HP and SP (0x081d, 0x081e), for the gauges
/// under it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementalState {
    pub gid: u32,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
}

impl ElementalState {
    pub fn set(&mut self, par: ElementalPar, value: u32) {
        match par {
            ElementalPar::Hp => self.hp = value,
            ElementalPar::MaxHp => self.max_hp = value,
            ElementalPar::Sp => self.sp = value,
            ElementalPar::MaxSp => self.max_sp = value,
        }
    }

    pub fn hp_percentage(&self) -> f32 {
        gauge(self.hp, self.max_hp)
    }

    pub fn sp_percentage(&self) -> f32 {
        gauge(self.sp, self.max_sp)
    }
}

/// A gauge of one of the player's summons that 0x0b6c changes: kRO's handler (0x7f4330) stores `SP_HP` and
/// `SP_MAXHP` and only refreshes the gauge for any other; rAthena's `clif_summon_hp_bar` sends only `SP_HP`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummonPar {
    Hp,
    MaxHp,
}

impl SummonPar {
    /// rAthena's `SP_HP` and `SP_MAXHP` (`map.hpp`)
    pub fn from_var(var: u16) -> Option<Self> {
        match var {
            5 => Some(Self::Hp),
            6 => Some(Self::MaxHp),
            _ => None,
        }
    }
}

/// HP of a summon the player owns (a Meister's ABR, a Biolo's Bionic), kept by its unit as kRO's handlers keep it
/// (0x0b6b, 0x0b6c), for the one HP gauge kRO draws under it (0x82bed0, unit types 0xd and 0xe: 60 by 5)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SummonHp {
    pub hp: u32,
    pub max_hp: u32,
}

impl SummonHp {
    pub fn set(&mut self, par: SummonPar, value: u32) {
        match par {
            SummonPar::Hp => self.hp = value,
            SummonPar::MaxHp => self.max_hp = value,
        }
    }

    pub fn hp_percentage(&self) -> f32 {
        gauge(self.hp, self.max_hp)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionKind {
    Homunculus,
    Mercenary,
}

pub struct HomunculusState {
    pub gid: u32,
    pub job: u16,
    pub name: String,
    pub renamed: bool,
    pub level: i16,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub exp: i64,
    pub max_exp: i64,
    pub hunger: i16,
    pub intimacy: i16,
    pub accessory: u16,
    pub atk: i16,
    pub matk: i16,
    pub hit: i16,
    pub critical: i16,
    pub def: i16,
    pub mdef: i16,
    pub flee: i16,
    pub aspd: i16,
    pub atk_range: i16,
    pub skill_points: i16,
    pub skills: Vec<SkillInfo>,
    pub vaporized: bool,
    pub ai: CompanionAi,
}

impl HomunculusState {
    pub fn new(gid: u32) -> Self {
        Self {
            gid,
            job: 0,
            name: String::new(),
            renamed: false,
            level: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            exp: 0,
            max_exp: 0,
            hunger: 0,
            intimacy: 0,
            accessory: 0,
            atk: 0,
            matk: 0,
            hit: 0,
            critical: 0,
            def: 0,
            mdef: 0,
            flee: 0,
            aspd: 0,
            atk_range: 0,
            skill_points: 0,
            skills: Vec::new(),
            vaporized: false,
            ai: CompanionAi::new(false),
        }
    }

    pub fn hp_percentage(&self) -> f32 {
        gauge(self.hp, self.max_hp)
    }

    pub fn sp_percentage(&self) -> f32 {
        gauge(self.sp, self.max_sp)
    }

    pub fn exp_percentage(&self) -> f32 {
        if self.max_exp > 0 {
            self.exp as f32 / self.max_exp as f32
        } else {
            0.0
        }
    }

    pub fn is_alive(&self) -> bool {
        !self.vaporized && self.hp > 0
    }
}

pub struct MercenaryState {
    pub gid: u32,
    pub job: u16,
    pub name: String,
    pub level: i16,
    pub hp: u32,
    pub max_hp: u32,
    pub sp: u32,
    pub max_sp: u32,
    pub atk: i16,
    pub matk: i16,
    pub hit: i16,
    pub critical: i16,
    pub def: i16,
    pub mdef: i16,
    pub flee: i16,
    pub aspd: i16,
    pub atk_range: i16,
    pub faith: i16,
    pub expire_date: i32,
    pub calls: i32,
    pub kills: i32,
    pub skills: Vec<SkillInfo>,
    pub ai: CompanionAi,
}

impl MercenaryState {
    pub fn new(gid: u32) -> Self {
        Self {
            gid,
            job: 0,
            name: String::new(),
            level: 0,
            hp: 0,
            max_hp: 0,
            sp: 0,
            max_sp: 0,
            atk: 0,
            matk: 0,
            hit: 0,
            critical: 0,
            def: 0,
            mdef: 0,
            flee: 0,
            aspd: 0,
            atk_range: 0,
            faith: 0,
            expire_date: 0,
            calls: 0,
            kills: 0,
            skills: Vec::new(),
            ai: CompanionAi::new(true),
        }
    }

    pub fn hp_percentage(&self) -> f32 {
        gauge(self.hp, self.max_hp)
    }

    pub fn sp_percentage(&self) -> f32 {
        gauge(self.sp, self.max_sp)
    }

    pub fn is_alive(&self) -> bool {
        self.hp > 0
    }
}

/// Companion status packets carry the attack motion delay in milliseconds. Both
/// info windows show the rating derived from it.
pub fn aspd_display(amotion: i16) -> i16 {
    (2000 - amotion) / 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_elemental_gauge_change_sets_only_its_own_value() {
        let mut e = ElementalState {
            gid: 1,
            hp: 500,
            max_hp: 1000,
            sp: 10,
            max_sp: 0,
        };
        assert_eq!(e.hp_percentage(), 0.5);
        assert_eq!(
            e.sp_percentage(),
            0.0,
            "no max SP yet: empty, not a division by zero"
        );
        e.set(ElementalPar::MaxSp, 40);
        e.set(ElementalPar::Hp, 250);
        assert_eq!((e.hp, e.max_hp, e.sp, e.max_sp), (250, 1000, 10, 40));
        assert_eq!(e.sp_percentage(), 0.25);
    }

    #[test]
    fn a_summon_keeps_hp_and_max_hp_and_no_other_gauge() {
        assert_eq!(SummonPar::from_var(5), Some(SummonPar::Hp));
        assert_eq!(SummonPar::from_var(6), Some(SummonPar::MaxHp));
        assert_eq!(SummonPar::from_var(7), None, "SP: a summon has no SP gauge");
        let mut s = SummonHp {
            hp: 3000,
            max_hp: 12000,
        };
        assert_eq!(s.hp_percentage(), 0.25);
        s.set(SummonPar::Hp, 6000);
        assert_eq!(s.hp_percentage(), 0.5);
        s.set(SummonPar::MaxHp, 0);
        assert_eq!(s.hp_percentage(), 0.0, "no maximum: empty");
    }

    #[test]
    fn aspd_reads_as_a_rating_not_a_delay() {
        assert_eq!(aspd_display(700), 130);
        assert_eq!(aspd_display(100), 190);
    }
}
