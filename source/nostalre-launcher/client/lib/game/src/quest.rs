/// A single hunt objective. From 20150513 the server names it by a hunt id (`quest id * 1000 + index`), not by
/// its monster: an objective on a race or size has no monster and arrives as a Poring.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestObjective {
    pub hunt_id: u32,
    pub mob_id: u32,
    pub name: String,
    pub current: i16,
    /// 0 until the server says: the list (0x09f8) carries it, an added quest (0x09f9) gets it from 0x08fe.
    pub required: i16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Quest {
    pub id: u32,
    pub active: bool,
    /// Unix expiry, `None` when the quest is not time-limited.
    pub end_time: Option<u32>,
    pub objectives: Vec<QuestObjective>,
}

/// A hunt objective's progress (0x09fa, 0x08fe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestHuntEntry {
    pub hunt_id: u32,
    pub current: i16,
    pub required: i16,
}

/// An over-NPC quest marker (ZC_QUEST_NOTIFY_EFFECT): `effect` is rAthena's icon (`e_questinfo_types`), drawn over the
/// NPC (`marker_sprite_action`); `color` its minimap dot, none for 0 (`QMARK_NONE`), whose icon kRO still draws
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestMarker {
    pub x: u16,
    pub y: u16,
    pub effect: i16,
    pub color: u8,
}

/// rAthena's `QTYPE_NONE`: the NPC's marker goes
pub const MARKER_NONE: i16 = 9999;

/// The `emotion.act` action an icon is drawn with, as kRO's NPC (0x829b30) has it made (0x7752b0): effect type
/// 0x24 + icon below 6, 0x36 + icon from 6, which take actions 80 to 85 and 93 to 97 of `이팩트\emotion.spr`. Icon 11
/// (rAthena's `QTYPE_JUMPING_PORING`) makes type 0x41, `이팩트\마도카트`'s first action, and a higher icon another
/// effect: not drawn.
pub fn marker_sprite_action(effect: i16) -> Option<usize> {
    match effect {
        0..=5 => Some(80 + effect as usize),
        6..=10 => Some(93 + (effect - 6) as usize),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
pub struct QuestLog {
    pub quests: Vec<Quest>,
    /// A local HUD preference; never sent as server quest state.
    pub followed_id: Option<u32>,
}

impl QuestLog {
    pub fn clear(&mut self) {
        self.quests.clear();
        self.followed_id = None;
    }

    pub fn followed(&self) -> Option<&Quest> {
        self.followed_id.and_then(|id| self.get(id)).filter(|q| q.active)
            // Without an explicit choice, the latest accepted stage wins.
            // Visit receipts are resolved as one tour by quest_guidance.
            .or_else(|| self.quests.iter().find(|q| q.active && (900000..900100).contains(&q.id)))
            .or_else(|| self.quests.iter().rev().find(|q| q.active))
    }

    pub fn follow(&mut self, id: u32) -> bool {
        if self.get(id).is_some_and(|q| q.active) {
            self.followed_id = Some(id);
            true
        } else { false }
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.quests.iter().position(|q| q.id == id)
    }

    pub fn get(&self, id: u32) -> Option<&Quest> {
        self.index_of(id).map(|i| &self.quests[i])
    }

    /// The quest list (0x09f8) replaces the log.
    pub fn replace(&mut self, quests: Vec<Quest>) {
        self.quests = quests;
        if self.followed_id.is_some_and(|id| !self.get(id).is_some_and(|q| q.active)) {
            self.followed_id = None;
        }
    }

    /// A new quest (0x09f9); a duplicate is ignored.
    pub fn add(&mut self, quest: Quest) {
        if self.index_of(quest.id).is_none() {
            self.quests.push(quest);
        }
    }

    pub fn remove(&mut self, id: u32) -> bool {
        if let Some(i) = self.index_of(id) {
            self.quests.remove(i);
            if self.followed_id == Some(id) { self.followed_id = None; }
            true
        } else {
            false
        }
    }

    pub fn set_active(&mut self, id: u32, active: bool) {
        if let Some(i) = self.index_of(id) {
            self.quests[i].active = active;
            if !active && self.followed_id == Some(id) { self.followed_id = None; }
        }
    }

    /// The quest id and objective of a hunt id.
    pub fn objective(&self, hunt_id: u32) -> Option<(u32, &QuestObjective)> {
        self.quests.iter().find_map(|q| {
            q.objectives
                .iter()
                .find(|o| o.hunt_id == hunt_id)
                .map(|o| (q.id, o))
        })
    }

    pub fn update_hunt(&mut self, entry: QuestHuntEntry) {
        if let Some(obj) = self
            .quests
            .iter_mut()
            .flat_map(|q| q.objectives.iter_mut())
            .find(|o| o.hunt_id == entry.hunt_id)
        {
            obj.current = entry.current;
            obj.required = entry.required;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn following_is_local_and_survives_list_refresh_but_not_logout_or_removal() {
        let mut log = QuestLog::default();
        log.replace(vec![quest(7474, vec![]), quest(1238, vec![])]);
        assert_eq!(log.followed().unwrap().id, 1238);
        assert!(log.follow(1238));
        log.replace(log.quests.clone());
        assert_eq!(log.followed().unwrap().id, 1238);
        log.set_active(1238, false);
        assert_eq!(log.followed().unwrap().id, 7474);
        assert!(!log.follow(1238) && !log.follow(999999));
        log.remove(7474);
        assert!(log.followed().is_none());
        log.clear();
        assert!(log.followed_id.is_none());
    }

    #[test]
    fn each_icon_takes_its_emotion_action_as_kro_has_it() {
        let actions: Vec<_> = (0..=11).map(marker_sprite_action).collect();
        assert_eq!(
            actions,
            [
                Some(80),
                Some(81),
                Some(82),
                Some(83),
                Some(84),
                Some(85),
                Some(93),
                Some(94),
                Some(95),
                Some(96),
                Some(97),
                None
            ]
        );
        assert_eq!(marker_sprite_action(-1), None);
        assert_eq!(marker_sprite_action(MARKER_NONE), None);
    }

    fn quest(id: u32, objectives: Vec<QuestObjective>) -> Quest {
        Quest {
            id,
            active: true,
            end_time: None,
            objectives,
        }
    }

    fn objective(hunt_id: u32, name: &str) -> QuestObjective {
        QuestObjective {
            hunt_id,
            mob_id: 1002,
            name: name.into(),
            ..Default::default()
        }
    }

    #[test]
    fn hunts_are_matched_by_hunt_id_not_by_monster() {
        // Two objectives without a monster both arrive as a Poring: only the hunt id tells them apart.
        let mut log = QuestLog::default();
        log.replace(vec![quest(
            1000,
            vec![objective(1_000_000, "Brute"), objective(1_000_001, "Small")],
        )]);
        log.update_hunt(QuestHuntEntry {
            hunt_id: 1_000_001,
            current: 2,
            required: 5,
        });
        let quest = log.get(1000).unwrap();
        assert_eq!(
            (quest.objectives[0].current, quest.objectives[0].required),
            (0, 0)
        );
        assert_eq!(
            (quest.objectives[1].current, quest.objectives[1].required),
            (2, 5)
        );
        assert_eq!(
            log.objective(1_000_001)
                .map(|(id, o)| (id, o.name.as_str())),
            Some((1000, "Small"))
        );
    }

    #[test]
    fn add_dedups_and_remove_drops() {
        let mut log = QuestLog::default();
        log.add(Quest {
            end_time: Some(123),
            ..quest(2000, vec![objective(2_000_000, "Drops")])
        });
        log.add(quest(2000, Vec::new()));
        assert_eq!(log.get(2000).unwrap().end_time, Some(123));
        assert!(log.remove(2000));
        assert!(log.get(2000).is_none());
    }
}
