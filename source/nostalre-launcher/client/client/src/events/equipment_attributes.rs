//! Equipment Attributes: the character as the install's scripts read it, run again only while the window is open
//! and only when something they read changed (`docs/nostalre-equipment-attributes.md`, "When it runs").
use crate::App;
use models::enums::skill_enums::SkillEnum;
use nostalre_game::data_table::DataTable;
use nostalre_game::equipment_properties::{
    EquipmentProperties, SkillNames, Snapshot, WornItem, is_pet_egg, pet_relationship,
};
use std::hash::{Hash, Hasher};
use std::path::Path;

struct TableNames<'a>(&'a DataTable);

impl SkillNames for TableNames<'_> {
    fn skill_name(&self, skill: i32) -> String {
        u32::try_from(skill)
            .ok()
            .and_then(|id| SkillEnum::try_from_value(id).ok())
            .map(|skill| self.0.skill_name_for(skill).to_string())
            .unwrap_or_default()
    }
}

impl App {
    /// What the scripts read, hashed without building anything, to see whether a run is due; it must follow the
    /// snapshot's fields. SP and MaxSP stay out: the data never asks `get(7)` or `get(8)`, and they change with every
    /// regeneration tick
    fn equipment_attributes_fingerprint(&self) -> u64 {
        let mut hasher = std::hash::DefaultHasher::new();
        let character = &self.game.character;
        for item in character
            .inventory
            .all_items()
            .iter()
            .filter(|i| i.wear_state != 0)
        {
            (
                item.item_id,
                item.refining_level,
                item.grade,
                item.slot,
                item.wear_state,
            )
                .hash(&mut hasher);
            for option in &item.options {
                (option.id, option.value, option.param).hash(&mut hasher);
            }
        }
        for skill in character.skills.skills() {
            (skill.skill.id(), skill.level).hash(&mut hasher);
        }
        (
            self.game.world.entities.player_job(),
            character.base_level,
            character.job_level,
            [
                character.str,
                character.agi,
                character.vit,
                character.int,
                character.dex,
                character.luk,
            ],
            character.traits.stats.map(|s| s.base),
            &self.game.session.current_map,
            self.hatched_egg(),
            pet_relationship(self.game.companions.pet.intimacy),
        )
            .hash(&mut hasher);
        hasher.finish()
    }

    /// The pet that is out, by its egg: rAthena marks a hatched egg (`attribute`, the lists' damaged flag), clears
    /// it on the return to the egg and deletes it at intimacy 0, so the inventory alone says which egg counts
    fn hatched_egg(&self) -> Option<u32> {
        self.game
            .character
            .inventory
            .all_items()
            .iter()
            .find(|i| is_pet_egg(i.item_id) && i.is_damaged)
            .map(|i| i.item_id)
    }

    fn equipment_attributes_snapshot(&self) -> Snapshot {
        let character = &self.game.character;
        Snapshot {
            worn: character
                .inventory
                .all_items()
                .iter()
                .filter(|i| i.wear_state != 0)
                .map(|item| WornItem {
                    item_id: item.item_id,
                    refine: item.refining_level,
                    grade: item.grade,
                    cards: item.slot,
                    cards_are_maker: item.producer_char_id().is_some(),
                    options: item.options,
                    wear: item.wear_state,
                })
                .collect(),
            job: self.game.world.entities.player_job(),
            base_level: i32::from(character.base_level),
            job_level: character.job_level as i32,
            sp: character.sp as i32,
            max_sp: character.max_sp as i32,
            base_stats: [
                character.str,
                character.agi,
                character.vit,
                character.int,
                character.dex,
                character.luk,
            ]
            .map(i32::from),
            trait_stats: character.traits.stats.map(|s| i32::from(s.base)),
            skills: character
                .skills
                .skills()
                .iter()
                .map(|s| (s.skill.id() as i32, i32::from(s.level)))
                .collect(),
            map: self.game.session.current_map.clone().unwrap_or_default(),
            pet_egg: self.hatched_egg(),
            pet_intimacy: self.game.companions.pet.intimacy,
        }
    }

    /// Runs the scripts again when the window is open and what they read changed; the first opening loads them
    /// (18 MB of Lua state that a session that never opens the window does not need)
    pub(crate) fn refresh_equipment_attributes(&mut self) {
        if !self.windows.equipment_properties_window.is_open() {
            return;
        }
        if !self.equipment_properties_tried {
            self.equipment_properties_tried = true;
            self.equipment_properties = self.grf.as_deref().and_then(|grf| {
                EquipmentProperties::load(grf, self.config.install_dir.as_deref().map(Path::new))
            });
        }
        let window = &self.windows.equipment_properties_window;
        let Some(properties) = &self.equipment_properties else {
            return;
        };
        let fingerprint = self.equipment_attributes_fingerprint();
        if window.fingerprint == Some(fingerprint) {
            return;
        }
        let total = properties.compute(&self.equipment_attributes_snapshot());
        let names = TableNames(&self.game.data_table);
        let tabs = std::array::from_fn(|tab| {
            properties
                .tab_lines(&total, tab, &names)
                .iter()
                .map(|text| nostalre_game::equipment_properties::window_line(text))
                .collect()
        });
        let window = &mut self.windows.equipment_properties_window;
        window.set_lines(tabs);
        window.fingerprint = Some(fingerprint);
    }
}
