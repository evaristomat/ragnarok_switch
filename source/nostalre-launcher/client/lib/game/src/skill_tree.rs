//! A job's skill tree as rAthena merges it (`SkillTreeDatabase::loadingFinished`): the job's own skills, then
//! those of the jobs it inherits that it does not list itself (a later inheritance replacing an earlier one, an
//! excluded skill not passed on), skills at level 0 dropped.
use crate::skill::SkillEnum;
pub use crate::skill_tree_tables::TreeEntry;
use crate::skill_tree_tables::{inherits, own_tree};

/// A skill of the merged tree, and the job whose page it goes on: the first job in the line that has it, so a
/// skill a later job lists again (Rune Knight T's Berserk) stays with the job that brought it
pub struct TreeSkill {
    pub entry: &'static TreeEntry,
    pub page: u16,
}

pub fn tree(job: u16) -> Vec<TreeSkill> {
    let own = own_tree(job);
    let mut entries: Vec<&'static TreeEntry> = own.iter().collect();
    for &from in inherits(job) {
        for entry in own_tree(from).iter().filter(|e| !e.exclude) {
            if own.iter().any(|o| o.skill == entry.skill) {
                continue;
            }
            match entries.iter_mut().find(|e| e.skill == entry.skill) {
                Some(slot) => *slot = entry,
                None => entries.push(entry),
            }
        }
    }
    entries
        .into_iter()
        .filter(|e| e.max_level > 0)
        .map(|entry| TreeSkill {
            entry,
            page: page_of(job, entry.skill),
        })
        .collect()
}

/// The pages of `job`'s merged `skills` (`tree`), earliest job first: the inherited jobs and the job itself,
/// those with a skill
pub fn pages(job: u16, skills: &[TreeSkill]) -> Vec<u16> {
    inherits(job)
        .iter()
        .copied()
        .chain(std::iter::once(job))
        .filter(|&p| skills.iter().any(|s| s.page == p))
        .collect()
}

/// The first job in the line that passes `skill` on: one that excludes it or lists it at level 0 does not
fn page_of(job: u16, skill: SkillEnum) -> u16 {
    inherits(job)
        .iter()
        .copied()
        .find(|&from| {
            own_tree(from)
                .iter()
                .any(|e| e.skill == skill && !e.exclude && e.max_level > 0)
        })
        .unwrap_or(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNIGHT: u16 = 7;
    const SWORDMAN: u16 = 1;
    const LORD_KNIGHT: u16 = 4008;
    const RUNE_KNIGHT_T: u16 = 4060;

    fn find(job: u16, skill: SkillEnum) -> Option<TreeSkill> {
        tree(job).into_iter().find(|s| s.entry.skill == skill)
    }

    #[test]
    fn a_knight_has_the_swordman_skills_on_their_page() {
        let bash = find(KNIGHT, SkillEnum::SmBash).unwrap();
        assert_eq!(bash.page, SWORDMAN);
        let bowling = find(KNIGHT, SkillEnum::KnBowlingbash).unwrap();
        assert_eq!(bowling.page, KNIGHT);
        assert!(bowling.entry.requires.contains(&(SkillEnum::SmBash, 10)));
        assert_eq!(pages(KNIGHT, &tree(KNIGHT)), [0, SWORDMAN, KNIGHT]);
    }

    #[test]
    fn an_excluded_skill_is_not_passed_on() {
        assert!(find(0, SkillEnum::NvTrickdead).is_some());
        assert!(find(SWORDMAN, SkillEnum::NvTrickdead).is_none());
        // High Novice lists it itself; Novice, which excludes it, does not bring it
        let high_novice = 4001;
        assert_eq!(
            find(high_novice, SkillEnum::NvTrickdead).unwrap().page,
            high_novice
        );
    }

    #[test]
    fn transcendent_skills_stop_at_the_classic_tree() {
        let lk = find(LORD_KNIGHT, SkillEnum::LkBerserk).unwrap();
        assert_eq!(lk.page, LORD_KNIGHT);
        assert_eq!(lk.entry.job_level, 50);
        assert!(tree(RUNE_KNIGHT_T).is_empty());
    }
}
