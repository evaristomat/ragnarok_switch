//! Titles a character wears before its name: kRO's client lists the titles of the achievements whose reward was
//! claimed (`achievements.lub`'s `reward.title`, from the 0x0a23 list: 0xcfe3d0, 0xd7d6c0), sorted by name, and
//! names them from `titletable.lub`'s `title_tbl` (`GetTitleString`: "" for 0 or an id it lacks). Read from the
//! 2025 exe (`docs/nostalre-kro-exe-equip-window.md`, "Title").

use std::collections::HashMap;

use nostalre_formats::lua_source::{LuaKey, LuaValue, globals};

#[derive(Debug, Default)]
pub struct TitleTables {
    names: HashMap<i32, String>,
    by_achievement: HashMap<u32, i32>,
}

/// A table's entries keyed by number
fn numbered(
    table: &nostalre_formats::lua_source::LuaTable,
) -> impl Iterator<Item = (i64, &LuaValue)> {
    table.entries.iter().filter_map(|(key, value)| match key {
        LuaKey::Index(id) => Some((*id, value)),
        LuaKey::Name(_) => None,
    })
}

impl TitleTables {
    /// `titletable.lub`'s and `achievements.lub`'s sources; a missing one leaves its part empty
    pub fn parse(title_table: Option<&[u8]>, achievements: Option<&[u8]>) -> Self {
        let mut tables = Self::default();
        if let Some(source) = title_table
            && let Some(names) = globals(source)
                .get("title_tbl")
                .and_then(LuaValue::as_table)
        {
            tables.names = numbered(names)
                .filter_map(|(id, name)| {
                    Some((i32::try_from(id).ok()?, name.as_str()?.to_string()))
                })
                .collect();
        }
        if let Some(source) = achievements
            && let Some(list) = globals(source)
                .get("achievement_tbl")
                .and_then(LuaValue::as_table)
        {
            tables.by_achievement = numbered(list)
                .filter_map(|(id, achievement)| {
                    let title = achievement
                        .as_table()?
                        .get("reward")?
                        .as_table()?
                        .get("title")?
                        .as_number()?;
                    Some((u32::try_from(id).ok()?, title as i32))
                })
                .collect();
        }
        tables
    }

    /// The title's name, "" for none or one the table lacks
    pub fn name(&self, title_id: i32) -> &str {
        self.names.get(&title_id).map_or("", String::as_str)
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

/// The character's titles: those it may wear and the one it wears
#[derive(Debug, Default)]
pub struct Titles {
    owned: Vec<i32>,
    /// From the name packet of the character (0x0a30) or a change the server took (0x0a2f); 0 for none
    pub current: i32,
}

impl Titles {
    /// The titles of the claimed achievements, each once, sorted by name (0xd7ce10)
    pub fn set_rewarded(&mut self, tables: &TitleTables, achievement_ids: &[u32]) {
        self.owned.clear();
        for title in achievement_ids
            .iter()
            .filter_map(|id| tables.by_achievement.get(id))
        {
            if !self.owned.contains(title) {
                self.owned.push(*title);
            }
        }
        self.owned
            .sort_by(|a, b| tables.name(*a).cmp(tables.name(*b)));
    }

    pub fn owned(&self) -> &[i32] {
        &self.owned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITLES: &[u8] = br#"
title_tbl = {
	[1000] = "Junction of life",
	[1001] = "Specialist",
	[1034] = "VIP",
}
function GetTitleString(titleID) return title_tbl[titleID] end
"#;
    const ACHIEVEMENTS: &[u8] = br#"
achievement_tbl = {
	[200000] = { title = "First steps", reward = { title = 1000 }, score = 10 },
	[200001] = { title = "Getting there", reward = { title = 1001 } },
	[130000] = { title = "Royal visit", reward = { title = 1034 } },
	[110000] = { title = "A snack", reward = {} },
}
function main() end
"#;

    #[test]
    fn titles_are_named_and_found_by_their_achievement() {
        let tables = TitleTables::parse(Some(TITLES), Some(ACHIEVEMENTS));
        assert_eq!(tables.name(1034), "VIP");
        assert_eq!(tables.name(0), "");
        assert_eq!(tables.name(999), "");
        assert_eq!(tables.by_achievement.len(), 3);
    }

    #[test]
    fn claimed_achievements_list_their_titles_once_by_name() {
        let tables = TitleTables::parse(Some(TITLES), Some(ACHIEVEMENTS));
        let mut titles = Titles::default();
        titles.set_rewarded(&tables, &[200_001, 130_000, 110_000, 200_000, 200_001]);
        assert_eq!(titles.owned(), [1000, 1001, 1034]);
        titles.set_rewarded(&tables, &[130_000]);
        assert_eq!(titles.owned(), [1034]);
    }
}
