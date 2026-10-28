use std::collections::HashMap;

/// `ZC_COMPASS` action: the server either shows a mark for 15 s, shows it until
/// it clears it, or clears it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkAction {
    ShowTimed,
    Show,
    Remove,
}

impl MarkAction {
    pub fn from_packet(atype: i32) -> Option<Self> {
        match atype {
            0 => Some(MarkAction::ShowTimed),
            1 => Some(MarkAction::Show),
            2 => Some(MarkAction::Remove),
            _ => None,
        }
    }
}

const TIMED_SECS: f32 = 15.0;

#[derive(Clone, Copy, Debug)]
pub struct MinimapMarkEntry {
    pub x: u16,
    pub y: u16,
    /// `0xRRGGBB`, as the server sends it.
    pub color: u32,
    expires_at: Option<f32>,
}

impl MinimapMarkEntry {
    pub fn rgb(&self) -> [f32; 3] {
        [
            ((self.color >> 16) & 0xff) as f32 / 255.0,
            ((self.color >> 8) & 0xff) as f32 / 255.0,
            (self.color & 0xff) as f32 / 255.0,
        ]
    }
}

/// Marks the server put on the minimap, keyed by the mark number it owns. The
/// town guide places one per facility and clears them by number, so several are
/// live at once and only the number identifies them.
#[derive(Default)]
pub struct MinimapMarks {
    marks: HashMap<u8, MinimapMarkEntry>,
}

impl MinimapMarks {
    pub fn apply(&mut self, id: u8, action: MarkAction, x: u16, y: u16, color: u32, now: f32) {
        match action {
            // A remove carries no meaningful position — the guide sends 1,1.
            MarkAction::Remove => {
                self.marks.remove(&id);
            }
            MarkAction::Show | MarkAction::ShowTimed => {
                let expires_at = matches!(action, MarkAction::ShowTimed).then(|| now + TIMED_SECS);
                self.marks.insert(
                    id,
                    MinimapMarkEntry {
                        x,
                        y,
                        color,
                        expires_at,
                    },
                );
            }
        }
    }

    pub fn prune(&mut self, now: f32) {
        self.marks
            .retain(|_, m| m.expires_at.is_none_or(|end| now < end));
    }

    pub fn iter(&self) -> impl Iterator<Item = &MinimapMarkEntry> {
        self.marks.values()
    }

    pub fn clear(&mut self) {
        self.marks.clear();
    }

    pub fn len(&self) -> usize {
        self.marks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }
}

/// How many minimap draws a Crimson Marker stays after its last 0x09c1: kRO's minimap (0x76d3a0) counts each mark
/// down by one a draw and drops it at 0
const CRIMSON_MARK_DRAWS: u32 = 120;

/// A unit a Rebellion's Crimson Marker marks, as the caster's minimap shows it (0x09c1, `clif_crimson_marker`)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrimsonMark {
    pub x: i16,
    pub y: i16,
    draws_left: u32,
}

impl CrimsonMark {
    /// kRO draws no mark at a negative cell (0x76e220): rAthena's removal sends -1, -1
    pub fn shown(&self) -> bool {
        self.x >= 0 && self.y >= 0
    }
}

/// The marked units by id, as kRO's 0x09c1 handler (0x7b8380) keeps them: each packet sets the unit's cell and
/// restarts its count; rAthena sends one a second while the mark lasts, and -1, -1 when it ends early
#[derive(Default)]
pub struct CrimsonMarks {
    marks: HashMap<u32, CrimsonMark>,
}

impl CrimsonMarks {
    pub fn apply(&mut self, aid: u32, x: i16, y: i16) {
        self.marks.insert(
            aid,
            CrimsonMark {
                x,
                y,
                draws_left: CRIMSON_MARK_DRAWS,
            },
        );
    }

    /// One minimap draw: every mark counts down, those at 0 go
    pub fn count_draw(&mut self) {
        self.marks.retain(|_, m| {
            m.draws_left -= 1;
            m.draws_left > 0
        });
    }

    pub fn iter(&self) -> impl Iterator<Item = &CrimsonMark> {
        self.marks.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_marks_expire_and_removals_are_by_id() {
        let mut marks = MinimapMarks::default();
        marks.apply(0, MarkAction::ShowTimed, 134, 221, 0xFF0000, 0.0);
        marks.apply(1, MarkAction::Show, 175, 220, 0x0A82FF, 0.0);
        assert_eq!(marks.len(), 2);

        marks.prune(14.9);
        assert_eq!(marks.len(), 2);
        marks.prune(15.1);
        assert_eq!(marks.len(), 1, "the timed mark should be gone");

        // The guide's clear form sends x = y = 1; only the id may be used.
        marks.apply(1, MarkAction::Remove, 1, 1, 0xFFFF00, 20.0);
        assert!(marks.is_empty());
    }

    #[test]
    fn a_crimson_mark_lasts_120_draws_after_its_last_packet() {
        let mut marks = CrimsonMarks::default();
        marks.apply(7, 120, 80);
        for _ in 0..119 {
            marks.count_draw();
        }
        assert_eq!(marks.iter().count(), 1);
        marks.apply(7, 121, 80);
        for _ in 0..119 {
            marks.count_draw();
        }
        let mark = *marks.iter().next().expect("refreshed by the second packet");
        assert_eq!((mark.x, mark.y), (121, 80));
        marks.count_draw();
        assert_eq!(marks.iter().count(), 0);
    }

    #[test]
    fn a_crimson_mark_at_minus_one_is_kept_but_not_shown() {
        let mut marks = CrimsonMarks::default();
        marks.apply(7, -1, -1);
        let mark = *marks.iter().next().expect("kept until its count runs out");
        assert!(!mark.shown());
        marks.apply(8, 0, 5);
        assert_eq!(marks.iter().filter(|m| m.shown()).count(), 1);
    }

    #[test]
    fn reusing_an_id_replaces_the_mark_and_its_colour() {
        let mut marks = MinimapMarks::default();
        marks.apply(3, MarkAction::Show, 10, 10, 0xFF0000, 0.0);
        marks.apply(3, MarkAction::Show, 20, 30, 0x00FF00, 1.0);

        let mark = marks.iter().next().copied().expect("one mark");
        assert_eq!((mark.x, mark.y), (20, 30));
        assert_eq!(mark.rgb(), [0.0, 1.0, 0.0]);
        assert_eq!(marks.len(), 1);
    }
}
