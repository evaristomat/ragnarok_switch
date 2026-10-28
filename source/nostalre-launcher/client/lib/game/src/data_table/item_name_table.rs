use std::collections::HashMap;

use nostalre_formats::grf::GrfArchive;
use nostalre_formats::lua_table::{self, ItemInfo};

use crate::data_table::fill_missing;

pub struct ItemNameTable {
    identified_entries: HashMap<u32, String>,
    unidentified_entries: HashMap<u32, String>,
}

const IDENTIFIED_PATH: &str = nostalre_resources::table::IDENTIFIED_ITEM_NAME;
const UNIDENTIFIED_PATH: &str = nostalre_resources::table::UNIDENTIFIED_ITEM_NAME;

impl ItemNameTable {
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
            .map(|data| lua_table::parse_item_name_table(&data))
            .unwrap_or_default();
        let mut unidentified_entries = grf
            .read_file(UNIDENTIFIED_PATH)
            .map(|data| lua_table::parse_item_name_table(&data))
            .unwrap_or_default();

        identified_entries.insert(12887, "Asa de Mosca Infinita".into());
        unidentified_entries.insert(12887, "Asa de Mosca Infinita".into());

        tracing::info!(
            "Loaded item name tables from GRF: {} identified, {} unidentified",
            identified_entries.len(),
            unidentified_entries.len(),
        );
        if unidentified_entries.is_empty() {
            tracing::warn!(
                "{UNIDENTIFIED_PATH} is missing: unidentified items will show their real name"
            );
        }

        Self {
            identified_entries,
            unidentified_entries,
        }
    }

    /// Names for the items the text tables do not have (newer than them: the 32-bit ids)
    pub fn fill_from_item_info(&mut self, items: &HashMap<u32, ItemInfo>) {
        fill_missing(&mut self.identified_entries, items, |i| &i.identified_name);
        fill_missing(&mut self.unidentified_entries, items, |i| {
            &i.unidentified_name
        });
    }

    pub fn get_name(&self, item_id: u32) -> Option<&str> {
        self.identified_entries.get(&item_id).map(|s| s.as_str())
    }

    pub fn get_name_or_id(&self, item_id: u32) -> String {
        self.identified_entries
            .get(&item_id)
            .cloned()
            .unwrap_or_else(|| format!("Item #{item_id}"))
    }

    pub fn get_name_or_id_for(&self, item_id: u32, is_identified: bool) -> String {
        if is_identified {
            self.identified_entries
                .get(&item_id)
                .cloned()
                .unwrap_or_else(|| format!("Item #{item_id}"))
        } else {
            self.unidentified_entries
                .get(&item_id)
                .or_else(|| self.identified_entries.get(&item_id))
                .cloned()
                .unwrap_or_else(|| format!("Item #{item_id}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the private deployed classic GRF assets"]
    fn deployed_classic_item_tables_are_portuguese() {
        let root = std::env::var("RAGSWITCH_CLASSIC_ASSETS").expect("private asset path required");
        let paths = ["ragswitch-assets.grf", "data.grf", "official_data.grf"]
            .map(|name| format!("{root}/{name}"));
        for path in &paths {
            let archive = GrfArchive::open(std::path::Path::new(path)).unwrap();
            eprintln!("Catalog {path}: {:?}", ItemNameTable::load(&archive).get_name(501));
        }
        let grf = GrfArchive::open_layered(&paths, None).unwrap();
        let names = ItemNameTable::load(&grf);
        let description = crate::data_table::item_description_table::ItemDescriptionTable::load(&grf);
        for (id, expected) in [(501, "Poção Vermelha"), (1201, "Faca"), (601, "Asa de Mosca")] {
            let actual = names.get_name_or_id(id);
            eprintln!("deployed item {id}: {actual:?}; description={:?}", description.get(id, true));
            assert_eq!(actual, expected, "item {id} must use the Portuguese catalog");
        }
    }

    #[test]
    fn item_info_fills_only_the_missing_names() {
        let mut table = ItemNameTable::from_entries(
            HashMap::from([(1201, "Knife".to_string())]),
            HashMap::new(),
        );
        let info = |name: &str| ItemInfo {
            identified_name: Some(name.to_string()),
            ..ItemInfo::default()
        };
        table.fill_from_item_info(&HashMap::from([
            (1201, info("나이프")),
            (600025, info("각성 풀포스")),
        ]));
        assert_eq!(table.get_name(1201), Some("Knife"));
        assert_eq!(table.get_name(600025), Some("각성 풀포스"));
    }

    fn make_table() -> ItemNameTable {
        let mut identified = HashMap::new();
        identified.insert(501, "Red Potion".to_string());
        identified.insert(1201, "Knife".to_string());
        let mut unidentified = HashMap::new();
        unidentified.insert(1201, "Unknown Weapon".to_string());
        ItemNameTable {
            identified_entries: identified,
            unidentified_entries: unidentified,
        }
    }

    #[test]
    fn get_name_returns_identified() {
        let table = make_table();
        assert_eq!(table.get_name(501), Some("Red Potion"));
        assert_eq!(table.get_name_or_id(501), "Red Potion");
        assert!(table.get_name(999).is_none());
        assert_eq!(table.get_name_or_id(999), "Item #999");
    }

    #[test]
    fn get_name_or_id_for_dispatches_by_identified() {
        let table = make_table();
        assert_eq!(table.get_name_or_id_for(1201, true), "Knife");
        assert_eq!(table.get_name_or_id_for(1201, false), "Unknown Weapon");
        assert_eq!(table.get_name_or_id_for(501, true), "Red Potion");
        assert_eq!(table.get_name_or_id_for(501, false), "Red Potion");
    }
}
