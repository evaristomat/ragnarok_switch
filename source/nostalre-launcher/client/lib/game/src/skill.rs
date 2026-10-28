use std::collections::HashMap;

pub use models::enums::skill::SkillTargetType;
pub use models::enums::skill_enums::SkillEnum;

/// What fills a skill failure's message: nothing, the packet's `btype` as a count, the missing item's name (and
/// the amount, in `btype`), the name of the skill `btype` names, or the name of the skill that failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureArgs {
    None,
    Count,
    Item,
    ItemAndCount,
    Skill,
    FailedSkill,
}

/// `msgstringtable` id for a skill-use failure `cause` (`USESKILL_FAIL_*`) and what fills it, or `None` when no
/// message should be shown. Read out of kRO's exe (the rig's 2022-04-06 Ragexe, 0x0110's handler at 0x7b35d0: a
/// switch on the cause, table at 0x7b4244, and on `btype` for Basic Skill's cause 0, table at 0x7b4404). Causes
/// 4, 10 and cause 0's default show there only with `/skillfail` on; nostalRE shows them.
pub fn skill_failure_msg(
    cause: u8,
    skill: Option<SkillEnum>,
    btype: u16,
) -> Option<(u16, FailureArgs)> {
    use FailureArgs as A;
    let counted = |some: u16, none: u16| {
        if btype > 0 {
            (some, A::Count)
        } else {
            (none, A::None)
        }
    };
    Some(match cause {
        0 => (
            match skill {
                Some(SkillEnum::NvBasic) => match btype {
                    0 => 159,
                    1 => 160,
                    2 => 161,
                    3 => 162,
                    4 => 163,
                    5 => 164,
                    6 => 165,
                    7 => 383,
                    8 => 1304,
                    _ => return None,
                },
                Some(SkillEnum::AlWarp) => 214,
                Some(SkillEnum::TfSteal) => 205,
                Some(SkillEnum::TfPoison) => 207,
                _ => 204,
            },
            A::None,
        ),
        71 => (MSI_USESKILL_FAIL_NEED_ITEM, A::ItemAndCount),
        72 => (1538, A::Item),
        73 if btype > 0 => (1559, A::Skill),
        73 => (1560, A::None),
        74 => counted(1561, 1562),
        85 => counted(2553, 2552),
        4 if skill.is_some() => (1460, A::FailedSkill),
        4 => (219, A::None),
        // rAthena sends no message id (`btype` 0); kRO's exe shows the line `btype` names
        86 => (if btype > 0 { btype } else { 3969 }, A::None),
        90 => (2997, A::Count),
        91 => (3006, A::Count),
        98 => (3584, A::Count),
        101 => (3773, A::Count),
        _ => (
            match cause {
                1 => 202,
                2 => 203,
                3 => 808,
                5 => 233,
                6 => 239,
                7 => 246,
                8 => 247,
                9 => 580,
                10 => 285,
                11..=16 => 1396 + (cause as u16 - 11),
                17..=23 => 1411 + (cause as u16 - 17),
                24 => 1425,
                25 => 1864,
                26 => 1427,
                27 => 1428,
                28 => 1929,
                29 => 1932,
                30 => 1937,
                31..=33 => 1433 + (cause as u16 - 31),
                34 => 1948,
                35 => 1952,
                36 => 1951,
                37 => 1439,
                40 => 1450,
                43 => 1453,
                50 => 1462,
                51..=54 => 1474 + (cause as u16 - 51),
                57 => 1519,
                60 => 1522,
                78 => 2561,
                79 => 1748,
                80..=82 => 1823 + (cause as u16 - 80),
                83 => 1954,
                84 => 2466,
                87 => 2563,
                88 => 2626,
                89 => 2982,
                92 => 3458,
                93 => 3482,
                94 => 3483,
                95 => 3529,
                96 => 3530,
                97 => 3533,
                99 => 3604,
                100 => 3643,
                102..=110 => 3957 + (cause as u16 - 102),
                111 => 3968,
                _ => return None,
            },
            A::None,
        ),
    })
}

/// Cause 71: `"[%s] required '%d' amount."`, filled with the item name and the
/// amount the skill needs.
pub const MSI_USESKILL_FAIL_NEED_ITEM: u16 = 1536;

/// Ground skills whose cast carries a written message: the client collects the text
/// itself and sends it with the placement, so the server has nothing to prompt for.
pub fn skill_needs_talkbox(skill: SkillEnum) -> bool {
    matches!(skill, SkillEnum::HtTalkiebox | SkillEnum::RgGraffiti)
}

/// The skill window / hotkey icon for `skill`, named after its internal id.
pub fn skill_icon_path(skill: SkillEnum) -> String {
    nostalre_resources::ui::item::icon(&skill.to_name().to_lowercase())
}

pub const TALKBOX_MESSAGE_MAX_LEN: usize = 79;

/// The destination to answer a Teleport warp list with, when the list is the
/// single `Random` entry a level 1 cast produces. A level 2 cast adds the save
/// point, and any longer list has a real choice in it, so both return `None`.
pub fn teleport_lvl1_destination<'a>(
    skill: SkillEnum,
    destinations: &'a [String],
) -> Option<&'a str> {
    if skill != SkillEnum::AlTeleport {
        return None;
    }
    let [only] = destinations else {
        return None;
    };
    let name = only.strip_suffix(".gat").unwrap_or(only);
    name.eq_ignore_ascii_case("random").then_some(only.as_str())
}

pub struct SkillData {
    pub skill: SkillEnum,
    pub level: i16,
    pub selected_level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub upgradable: bool,
    pub skill_target_type: SkillTargetType,
}

impl SkillData {
    pub fn icon_path(&self) -> String {
        skill_icon_path(self.skill)
    }

    pub fn use_level(&self) -> i16 {
        if self.level <= 0 {
            0
        } else {
            self.selected_level.clamp(1, self.level)
        }
    }

    pub fn decrement_use_level(&mut self) {
        if self.level > 0 {
            self.selected_level = (self.selected_level - 1).max(1);
        }
    }

    pub fn increment_use_level(&mut self) {
        if self.level > 0 {
            self.selected_level = (self.selected_level + 1).min(self.level);
        }
    }
}

/// Cast metadata for a skill granted by a consumable, learned from
/// ZC_AUTORUN_SKILL rather than from the character's skill list. Kept apart from
/// [`SkillList`] so it never shows up in the skill window.
#[derive(Debug, Clone, PartialEq)]
pub struct ItemSkill {
    pub level: i16,
    pub sp_cost: i16,
    pub attack_range: i16,
    pub skill_target_type: SkillTargetType,
}

#[derive(Default)]
pub struct ItemSkills {
    skills: HashMap<u32, ItemSkill>,
}

impl ItemSkills {
    pub fn insert(&mut self, skill: SkillEnum, granted: ItemSkill) {
        self.skills.insert(skill.id(), granted);
    }

    pub fn get(&self, skill: SkillEnum) -> Option<&ItemSkill> {
        self.skills.get(&skill.id())
    }

    pub fn clear(&mut self) {
        self.skills.clear();
    }
}

pub struct SkillList {
    skills: Vec<SkillData>,
    open: bool,
}

impl Default for SkillList {
    fn default() -> Self {
        Self::new()
    }
}

impl SkillList {
    pub fn new() -> Self {
        Self {
            skills: Vec::new(),
            open: false,
        }
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn clear(&mut self) {
        self.skills.clear();
        self.open = false;
    }

    pub fn set_skills(&mut self, skills: Vec<SkillData>) {
        self.skills = skills;
    }

    pub fn update_skill(
        &mut self,
        id: SkillEnum,
        level: i16,
        sp_cost: i16,
        attack_range: i16,
        upgradable: bool,
        target_type: Option<SkillTargetType>,
    ) {
        if let Some(skill) = self.skills.iter_mut().find(|s| s.skill == id) {
            if let Some(target_type) = target_type {
                skill.skill_target_type = target_type;
            }
            skill.level = level;
            skill.sp_cost = sp_cost;
            skill.attack_range = attack_range;
            skill.upgradable = upgradable;
            if level <= 0 {
                skill.selected_level = 0;
            } else if skill.selected_level > level {
                skill.selected_level = level;
            } else if skill.selected_level < 1 {
                skill.selected_level = 1;
            }
        }
    }

    pub fn add_skill(&mut self, skill: SkillData) {
        if let Some(existing) = self.skills.iter_mut().find(|s| s.skill == skill.skill) {
            existing.level = skill.level;
            existing.sp_cost = skill.sp_cost;
            existing.attack_range = skill.attack_range;
            existing.upgradable = skill.upgradable;
            existing.skill_target_type = skill.skill_target_type;
            if skill.level <= 0 {
                existing.selected_level = 0;
            } else if existing.selected_level > skill.level {
                existing.selected_level = skill.level;
            } else if existing.selected_level < 1 {
                existing.selected_level = 1;
            }
        } else {
            self.skills.push(skill);
        }
    }

    pub fn remove_skill(&mut self, id: SkillEnum) {
        self.skills.retain(|s| s.skill != id);
    }

    pub fn apply_skill_list(&mut self, skills: Vec<crate::event::SkillInfo>) -> Vec<String> {
        self.skills = skills
            .into_iter()
            .map(|s| SkillData {
                skill: s.skill,
                selected_level: s.level,
                level: s.level,
                sp_cost: s.sp_cost,
                attack_range: s.attack_range,
                upgradable: s.upgradable,
                skill_target_type: s.skill_target_type,
            })
            .collect();
        self.skills.iter().map(|s| s.icon_path()).collect()
    }

    pub fn apply_skill_added(&mut self, skill: crate::event::SkillInfo) -> String {
        let icon_path = skill_icon_path(skill.skill);
        self.add_skill(SkillData {
            skill: skill.skill,
            selected_level: skill.level,
            level: skill.level,
            sp_cost: skill.sp_cost,
            attack_range: skill.attack_range,
            upgradable: skill.upgradable,
            skill_target_type: skill.skill_target_type,
        });
        icon_path
    }

    pub fn get_skill(&self, id: SkillEnum) -> Option<&SkillData> {
        self.skills.iter().find(|s| s.skill == id)
    }

    /// Learned, not only listed: the server lists a skill the character could
    /// learn at level 0.
    pub fn knows(&self, id: SkillEnum) -> bool {
        self.get_skill(id).is_some_and(|s| s.level > 0)
    }

    pub fn get_skill_mut(&mut self, id: SkillEnum) -> Option<&mut SkillData> {
        self.skills.iter_mut().find(|s| s.skill == id)
    }

    pub fn skills(&self) -> &[SkillData] {
        &self.skills
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_lone_random_entry_on_teleport_is_answered_automatically() {
        let teleport = SkillEnum::AlTeleport;
        let random = || vec!["Random.gat".to_string()];

        assert_eq!(
            teleport_lvl1_destination(teleport, &random()),
            Some("Random.gat")
        );
        assert_eq!(
            teleport_lvl1_destination(
                teleport,
                &["Random.gat".to_string(), "prontera.gat".to_string()]
            ),
            None
        );
        assert_eq!(
            teleport_lvl1_destination(SkillEnum::AlWarp, &random()),
            None
        );
    }

    fn make_skill(skill: SkillEnum, level: i16) -> SkillData {
        SkillData {
            skill,
            level,
            selected_level: level,
            sp_cost: 10,
            attack_range: 1,
            upgradable: true,
            skill_target_type: SkillTargetType::Target,
        }
    }

    #[test]
    fn set_and_query_skills() {
        let mut list = SkillList::new();
        list.set_skills(vec![
            make_skill(SkillEnum::SmSword, 10),
            make_skill(SkillEnum::SmBash, 5),
        ]);
        assert_eq!(list.skills().len(), 2);
        assert_eq!(list.get_skill(SkillEnum::SmBash).unwrap().level, 5);
        assert!(list.get_skill(SkillEnum::AlHeal).is_none());
    }

    #[test]
    fn update_skill_modifies_existing() {
        let mut list = SkillList::new();
        list.set_skills(vec![make_skill(SkillEnum::SmBash, 5)]);
        list.update_skill(SkillEnum::SmBash, 6, 15, 1, true, None);
        let skill = list.get_skill(SkillEnum::SmBash).unwrap();
        assert_eq!(skill.level, 6);
        assert_eq!(skill.sp_cost, 15);
    }

    #[test]
    fn a_combo_window_makes_its_skill_self_targeted_and_back() {
        let mut list = SkillList::new();
        list.set_skills(vec![make_skill(SkillEnum::MoExtremityfist, 5)]);
        let target = |list: &SkillList| {
            list.get_skill(SkillEnum::MoExtremityfist)
                .unwrap()
                .skill_target_type
        };
        let before = target(&list);
        list.update_skill(
            SkillEnum::MoExtremityfist,
            5,
            0,
            2,
            false,
            Some(SkillTargetType::MySelf),
        );
        assert_eq!(target(&list), SkillTargetType::MySelf);
        list.update_skill(SkillEnum::MoExtremityfist, 5, 0, 2, false, Some(before));
        assert_eq!(target(&list), before);
        // 0x010e carries no type and leaves it be
        list.update_skill(SkillEnum::MoExtremityfist, 5, 0, 2, false, None);
        assert_eq!(target(&list), before);
    }

    #[test]
    fn add_skill_inserts_or_updates() {
        let mut list = SkillList::new();
        list.add_skill(make_skill(SkillEnum::SmBash, 5));
        assert_eq!(list.skills().len(), 1);
        list.add_skill(make_skill(SkillEnum::SmBash, 7));
        assert_eq!(list.skills().len(), 1);
        assert_eq!(list.get_skill(SkillEnum::SmBash).unwrap().level, 7);
        list.add_skill(make_skill(SkillEnum::SmMagnum, 3));
        assert_eq!(list.skills().len(), 2);
    }

    #[test]
    fn a_removed_skill_leaves_only_itself() {
        let mut list = SkillList::new();
        list.set_skills(vec![
            make_skill(SkillEnum::NvFirstaid, 1),
            make_skill(SkillEnum::SmBash, 5),
        ]);
        list.remove_skill(SkillEnum::NvFirstaid);
        list.remove_skill(SkillEnum::AlHeal);
        assert!(list.get_skill(SkillEnum::NvFirstaid).is_none());
        assert_eq!(list.skills().len(), 1);
    }

    #[test]
    fn toggle_open_close() {
        let mut list = SkillList::new();
        assert!(!list.is_open());
        list.toggle();
        assert!(list.is_open());
        list.close();
        assert!(!list.is_open());
        list.open();
        assert!(list.is_open());
    }

    #[test]
    fn skill_failure_msg_maps_every_reachable_cause() {
        let id = |cause, skill, btype| skill_failure_msg(cause, skill, btype).map(|(id, _)| id);
        let (bash, basic) = (Some(SkillEnum::SmBash), Some(SkillEnum::NvBasic));
        assert_eq!(id(0, basic, 3), Some(162));
        assert_eq!(id(0, basic, 7), Some(383));
        assert_eq!(id(0, basic, 9), None);
        assert_eq!(id(0, Some(SkillEnum::AlWarp), 0), Some(214));
        assert_eq!(id(0, Some(SkillEnum::TfSteal), 0), Some(205));
        assert_eq!(id(0, Some(SkillEnum::TfPoison), 0), Some(207));
        assert_eq!(id(0, bash, 0), Some(204));
        assert_eq!(id(0, None, 0), Some(204));
        for (cause, msg) in [
            (1, 202),
            (6, 239),
            (9, 580),
            (10, 285),
            (11, 1396),
            (16, 1401),
            (17, 1411),
            (23, 1417),
            (24, 1425),
            (27, 1428),
            (25, 1864),
            (34, 1948),
            (35, 1952),
            (36, 1951),
            (78, 2561),
            (79, 1748),
            (83, 1954),
            (84, 2466),
            (88, 2626),
            (89, 2982),
            (92, 3458),
            (93, 3482),
        ] {
            assert_eq!(id(cause, bash, 0), Some(msg), "cause {cause}");
        }
        assert_eq!(id(38, bash, 0), None);
        assert_eq!(id(70, bash, 0), None);
        assert_eq!(id(200, bash, 0), None);
    }

    // the 3rd and 4th jobs' causes, by the text of the line each gets
    #[test]
    fn third_and_fourth_job_failures_say_what_is_missing() {
        let msg = |cause, btype| skill_failure_msg(cause, Some(SkillEnum::DkVigor), btype);
        for (cause, line) in [
            (33, 1435),  // Item can only be used when Mado Gear is mounted. (rAthena's MSI)
            (43, 1453),  // Guillotine Cross Poison is required.
            (50, 1462),  // Can't be used while on Mado Gear.
            (54, 1477),  // You can't read this Magic Book.
            (57, 1519),  // Only available when cart is put on. (rAthena's MSI)
            (80, 1823),  // You lack of familiarity.
            (86, 3969), // You must be in Communication with at least one kind of Mystical Creature (ours: rAthena sends no id)
            (96, 3530), // Insufficient energy sphere.
            (100, 3643), // You don't have enough AP.
            (103, 3958), // Requires Soul Energy.
            (110, 3965), // You must be equipped with a Shield.
            (111, 3968), // The target's Shadow has no space to move to.
        ] {
            assert_eq!(
                msg(cause, 0),
                Some((line, FailureArgs::None)),
                "cause {cause}"
            );
        }
        assert_eq!(msg(74, 3), Some((1561, FailureArgs::Count)));
        assert_eq!(msg(74, 0), Some((1562, FailureArgs::None)));
        assert_eq!(msg(85, 2), Some((2553, FailureArgs::Count)));
        assert_eq!(msg(73, 8022), Some((1559, FailureArgs::Skill)));
        assert_eq!(msg(72, 0), Some((1538, FailureArgs::Item)));
        assert_eq!(
            msg(71, 2),
            Some((MSI_USESKILL_FAIL_NEED_ITEM, FailureArgs::ItemAndCount))
        );
        // kRO's exe: a cooldown names the skill, counts come in btype, 86's line is btype's
        assert_eq!(msg(4, 0), Some((1460, FailureArgs::FailedSkill)));
        assert_eq!(msg(90, 5), Some((2997, FailureArgs::Count)));
        assert_eq!(msg(101, 3), Some((3773, FailureArgs::Count)));
        assert_eq!(msg(86, 3970), Some((3970, FailureArgs::None)));
    }

    #[test]
    fn a_ninja_leaps_out_of_hiding_by_the_skill_db() {
        assert!(SkillEnum::NjShadowjump.allowed_when_hidden());
        assert!(SkillEnum::KoYamikumo.allowed_when_hidden());
        assert!(!SkillEnum::NjKasumikiri.allowed_when_hidden());
    }

    #[test]
    fn selected_level_defaults_to_learned() {
        let skill = make_skill(SkillEnum::SmBash, 10);
        assert_eq!(skill.use_level(), 10);
    }

    #[test]
    fn use_level_zero_for_unlearned() {
        let skill = make_skill(SkillEnum::SmBash, 0);
        assert_eq!(skill.use_level(), 0);
    }

    #[test]
    fn decrement_and_increment_use_level() {
        let mut skill = make_skill(SkillEnum::SmBash, 5);
        assert_eq!(skill.use_level(), 5);
        skill.decrement_use_level();
        assert_eq!(skill.use_level(), 4);
        skill.decrement_use_level();
        skill.decrement_use_level();
        skill.decrement_use_level();
        assert_eq!(skill.use_level(), 1);
        skill.decrement_use_level();
        assert_eq!(skill.use_level(), 1);
        skill.increment_use_level();
        assert_eq!(skill.use_level(), 2);
        skill.selected_level = 5;
        skill.increment_use_level();
        assert_eq!(skill.use_level(), 5);
    }

    #[test]
    fn update_skill_clamps_selected_level() {
        let mut list = SkillList::new();
        let mut skill = make_skill(SkillEnum::SmBash, 10);
        skill.selected_level = 8;
        list.set_skills(vec![skill]);
        list.update_skill(SkillEnum::SmBash, 5, 15, 1, true, None);
        assert_eq!(list.get_skill(SkillEnum::SmBash).unwrap().selected_level, 5);
    }

    #[test]
    fn add_skill_clamps_selected_level_on_update() {
        let mut list = SkillList::new();
        let mut skill = make_skill(SkillEnum::SmBash, 10);
        skill.selected_level = 8;
        list.set_skills(vec![skill]);
        list.add_skill(make_skill(SkillEnum::SmBash, 3));
        assert_eq!(list.get_skill(SkillEnum::SmBash).unwrap().selected_level, 3);
    }
}
