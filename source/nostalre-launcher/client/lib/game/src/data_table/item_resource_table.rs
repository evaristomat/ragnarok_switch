use std::collections::{HashMap, HashSet};

use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_table::{self, ItemInfo};

use crate::data_table::fill_missing;

pub struct ItemResourceTable {
    identified_entries: HashMap<u32, String>,
    unidentified_entries: HashMap<u32, String>,
}

/// kRO's icon for most cards
const BLANK_CARD: &str = "이름없는카드";
const IDENTIFIED_PATH: &str = nostalre_resources::table::IDENTIFIED_ITEM_RESOURCE;
const UNIDENTIFIED_PATH: &str = nostalre_resources::table::UNIDENTIFIED_ITEM_RESOURCE;

impl ItemResourceTable {
    pub fn from_entries(
        identified_entries: HashMap<u32, String>,
        unidentified_entries: HashMap<u32, String>,
    ) -> Self {
        Self {
            identified_entries,
            unidentified_entries,
        }
    }

    pub fn load(grf: &GrfArchive) -> Self {
        let mut identified_entries = grf
            .read_file(IDENTIFIED_PATH)
            .map(|data| lua_table::parse_item_res_table(&data))
            .unwrap_or_default();
        let mut unidentified_entries = grf
            .read_file(UNIDENTIFIED_PATH)
            .map(|data| lua_table::parse_item_res_table(&data))
            .unwrap_or_default();

        if let Some(resource) = identified_entries.get(&601).cloned() {
            identified_entries.insert(12887, resource.clone());
            unidentified_entries.insert(12887, resource);
        }

        tracing::info!(
            "Loaded item resource tables from GRF: {} identified, {} unidentified",
            identified_entries.len(),
            unidentified_entries.len(),
        );
        if unidentified_entries.is_empty() {
            tracing::warn!(
                "{UNIDENTIFIED_PATH} is missing: unidentified items will show their real icon"
            );
        }

        Self {
            identified_entries,
            unidentified_entries,
        }
    }

    /// Icons and drop sprites for the items the text tables do not have (newer than them: the 32-bit ids)
    pub fn fill_from_item_info(&mut self, items: &HashMap<u32, ItemInfo>) {
        fill_missing(&mut self.identified_entries, items, |i| {
            &i.identified_resource
        });
        fill_missing(&mut self.unidentified_entries, items, |i| {
            &i.unidentified_resource
        });
    }

    /// Icons for the headgear no table names (rAthena's views, `views`): another headgear's of the same look that has
    /// one, else the look's own sprite name (`look_name`, accname without its `_`), which kRO also names icons by
    pub fn fill_headgear_icons(
        &mut self,
        views: &HashMap<u32, u16>,
        look_name: impl Fn(u16) -> Option<String>,
        has_icon: impl Fn(&str) -> bool,
    ) {
        let mut by_view: HashMap<u16, Vec<u32>> = HashMap::new();
        for (&id, &view) in views {
            by_view.entry(view).or_default().push(id);
        }
        for ids in by_view.values_mut() {
            ids.sort_unstable();
        }
        let mut found = Vec::new();
        for (&id, &view) in views {
            if self.identified_entries.contains_key(&id) {
                continue;
            }
            let same_look = by_view[&view]
                .iter()
                .filter_map(|other| self.identified_entries.get(other))
                .find(|resource| has_icon(resource))
                .cloned();
            let resource = same_look.or_else(|| {
                look_name(view)
                    .map(|name| name.trim_start_matches('_').to_string())
                    .filter(|name| has_icon(name))
            });
            if let Some(resource) = resource {
                found.push((id, resource));
            }
        }
        tracing::info!(headgear = found.len(), "icons from headgear looks");
        self.identified_entries.extend(found);
    }

    /// kRO's blank card icon for the cards no table names (`cards`, rAthena's): kRO draws most cards with it, and
    /// rAthena's enchant entries (31xxxx) are cards too
    pub fn fill_card_icons(&mut self, cards: &HashSet<u32>) {
        let mut filled = 0;
        for &id in cards {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.identified_entries.entry(id)
            {
                entry.insert(BLANK_CARD.to_string());
                filled += 1;
            }
        }
        tracing::info!(cards = filled, "blank card icons");
    }

    pub fn get_resource_name(&self, item_id: u32) -> Option<&str> {
        self.get_resource_name_for(item_id, true)
    }

    /// An item no table has, newer than the client's iteminfo, shows a classic weapon of its type
    pub fn get_resource_name_for(&self, item_id: u32, is_identified: bool) -> Option<&str> {
        self.own_resource_name(item_id, is_identified).or_else(|| {
            crate::sprite_path::renewal_stand_in_item(item_id)
                .and_then(|classic| self.own_resource_name(classic, is_identified))
        })
    }

    fn own_resource_name(&self, item_id: u32, is_identified: bool) -> Option<&str> {
        if is_identified {
            self.identified_entries.get(&item_id).map(|s| s.as_str())
        } else {
            self.unidentified_entries
                .get(&item_id)
                .or_else(|| self.identified_entries.get(&item_id))
                .map(|s| s.as_str())
        }
    }

    pub fn item_icon_path(&self, item_id: u32) -> Option<String> {
        self.get_resource_name(item_id)
            .map(nostalre_resources::ui::item::icon)
    }

    pub fn item_sprite_path(&self, item_id: u32, is_identified: bool) -> Option<String> {
        self.get_resource_name_for(item_id, is_identified)
            .map(nostalre_resources::sprite::item::of)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headgear_without_a_name_takes_an_icon_of_its_look() {
        let mut table = ItemResourceTable::from_entries(
            HashMap::from([
                (5874, "크림힐트의왕관".to_string()),
                (5875, "no_icon".to_string()),
            ]),
            HashMap::new(),
        );
        // Phoenix Crown shares its look with 5874; the circlet's look is named after its own icon; 1 has neither
        let views = HashMap::from([
            (5874, 1056),
            (5875, 1056),
            (400_103, 1056),
            (400_143, 2100),
            (400_999, 1),
        ]);
        let look_name = |view| (view == 2100).then(|| "_C_Asgard_Circlet_TW".to_string());
        table.fill_headgear_icons(&views, look_name, |r| r != "no_icon");
        assert_eq!(table.get_resource_name(400_103), Some("크림힐트의왕관"));
        assert_eq!(
            table.get_resource_name(400_143),
            Some("C_Asgard_Circlet_TW")
        );
        assert_eq!(table.get_resource_name(5875), Some("no_icon"));
        assert_eq!(table.get_resource_name(400_999), None);
    }

    #[test]
    fn a_card_without_a_name_takes_the_blank_card() {
        let mut table = ItemResourceTable::from_entries(
            HashMap::from([
                (4001, "이름없는카드".to_string()),
                (4999, "용의보주_홍".to_string()),
            ]),
            HashMap::new(),
        );
        table.fill_card_icons(&HashSet::from([4001, 4999, 311_383]));
        assert_eq!(table.get_resource_name(311_383), Some("이름없는카드"));
        assert_eq!(table.get_resource_name(4999), Some("용의보주_홍"));
    }

    fn make_table() -> ItemResourceTable {
        let mut identified = HashMap::new();
        identified.insert(501, "빨간포션".to_string());
        identified.insert(1201, "단검".to_string());
        let mut unidentified = HashMap::new();
        unidentified.insert(1201, "무기".to_string());
        ItemResourceTable {
            identified_entries: identified,
            unidentified_entries: unidentified,
        }
    }

    #[test]
    fn get_resource_name_returns_identified() {
        let table = make_table();
        assert_eq!(table.get_resource_name(501), Some("빨간포션"));
        assert_eq!(table.get_resource_name(1201), Some("단검"));
        assert!(table.get_resource_name(999).is_none());
    }

    #[test]
    fn get_resource_name_for_dispatches_by_identified() {
        let table = make_table();
        assert_eq!(table.get_resource_name_for(1201, true), Some("단검"));
        assert_eq!(table.get_resource_name_for(1201, false), Some("무기"));
        assert_eq!(table.get_resource_name_for(501, true), Some("빨간포션"));
        assert_eq!(table.get_resource_name_for(501, false), Some("빨간포션"));
    }

    #[test]
    fn item_icon_path_builds_grf_path() {
        let table = make_table();
        assert_eq!(
            table.item_icon_path(501).unwrap(),
            "data/texture/유저인터페이스/item/빨간포션.bmp"
        );
        assert!(table.item_icon_path(999).is_none());
    }
}
