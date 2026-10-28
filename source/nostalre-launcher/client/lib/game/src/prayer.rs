//! The Super Novice's prayer to the guardian angel, as kRO's chat input (0x789f10) takes it: the last ten lines the
//! player says are kept, and when the 8th of them is msgstringtable line 790 ("Dear angel, can you hear my voice?")
//! and the 10th line 793 ("Help me out~ Please~ T_T") a Super Novice sends 0x01ed. kRO builds the line between ("I
//! am" name "Super Novice~", 791 and 792) but compares nothing against it. rAthena then grants Explosion Spirits at
//! each tenth of the base experience (`clif_parse_NoviceExplosionSpirits`).

use std::collections::VecDeque;

/// "Dear angel, can you hear my voice?", the prayer's first line
pub const MSG_PRAYER_ANGEL: u16 = 790;
/// "Help me out~ Please~ T_T", its last
pub const MSG_PRAYER_HELP: u16 = 793;
/// "[Protection of Guardian Angel] You can't use it when you reach the highest level.", in red (kRO 0x78e0a9)
pub const MSG_PRAYER_MAX_LEVEL: u16 = 2015;

/// How many lines kRO keeps (the oldest goes, +0x464 capped at 10)
const KEPT: usize = 10;

/// The player's last chat lines
#[derive(Default)]
pub struct PrayerLines {
    lines: VecDeque<String>,
}

impl PrayerLines {
    pub fn push(&mut self, line: &str) {
        if self.lines.len() == KEPT {
            self.lines.pop_front();
        }
        self.lines.push_back(line.to_string());
    }

    /// Whether the last lines are the prayer: ten kept, the 8th `angel` and the 10th `help`, word for word
    pub fn prayed(&self, angel: &str, help: &str) -> bool {
        self.lines.len() == KEPT && self.lines[7] == angel && self.lines[9] == help
    }
}

/// kRO's old Super Novices (0x8840d0: Super Novice, Super Baby and two ids rAthena does not send)
const OLD_SUPER_NOVICES: [u16; 4] = [23, 4045, 4128, 4172];
/// Its expanded ones (0x8840a0: 4190 and 4191 in rAthena, 4192 and 4193 its own)
const EXPANDED_SUPER_NOVICES: [u16; 4] = [4190, 4191, 4192, 4193];
/// The rest it hears with no level cap (the Hyper Novice, 4307, and 4314)
const UNCAPPED: [u16; 2] = [4307, 4314];

/// kRO's Super Novice family (0x884100), the jobs it hears the prayer from and gives the Super Novice's cart
/// (0x7522c0): the three lists above; rAthena sends 23, 4045, 4190, 4191 and 4307
pub fn is_super_novice(job: u16) -> bool { matches!(job, 23 | 4045) }

/// The base level from which kRO refuses the prayer with line 2015: the old Super Novices' highest (its global at
/// 0x113b550) and the expanded ones' (0x113b558), filled at run time; ours is rAthena's highest base level for the
/// job. The uncapped ones have none
pub fn level_cap(job: u16) -> Option<u16> { is_super_novice(job).then_some(99) }

#[cfg(test)]
mod tests {
    use super::*;

    const ANGEL: &str = "Dear angel, can you hear my voice?";
    const HELP: &str = "Help me out~ Please~ T_T";

    fn said(lines: &[&str]) -> PrayerLines {
        let mut kept = PrayerLines::default();
        for line in lines {
            kept.push(line);
        }
        kept
    }

    #[test]
    fn the_prayer_is_the_8th_and_10th_of_the_last_ten_lines() {
        let mut lines = vec!["hi"; 7];
        lines.extend([ANGEL, "I am Vendor Super Novice~", HELP]);
        assert!(said(&lines).prayed(ANGEL, HELP));
        lines[8] = "anything";
        assert!(
            said(&lines).prayed(ANGEL, HELP),
            "kRO compares nothing against the 9th"
        );
        assert!(
            !said(&lines[3..]).prayed(ANGEL, HELP),
            "fewer than ten lines"
        );
        let mut more = lines.clone();
        more.push("one more");
        assert!(
            !said(&more).prayed(ANGEL, HELP),
            "the prayer must end the ten"
        );
        let mut eleven = vec!["old"];
        eleven.extend(lines.iter());
        assert!(said(&eleven).prayed(ANGEL, HELP), "the oldest line goes");
        lines[7] = "dear angel, can you hear my voice?";
        assert!(!said(&lines).prayed(ANGEL, HELP), "word for word");
    }

    #[test]
    fn classic_super_novice_prayer_stops_at_99() {
        assert!(is_super_novice(23) && is_super_novice(4045));
        assert!(!is_super_novice(0) && !is_super_novice(4190) && !is_super_novice(4307));
        assert_eq!(level_cap(23), Some(99));
        assert_eq!(level_cap(4045), Some(99));
        assert_eq!(level_cap(4307), None);
    }
}
