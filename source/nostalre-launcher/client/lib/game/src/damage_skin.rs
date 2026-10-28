//! Damage skins: the sprites damage numbers are drawn with, picked on the equipment window's Damage Indicator tab.
//! kRO's client counts them by `damageskinlist.lub`'s `DSList` (`GetDamageSkinSize`) and reads each from
//! `damageskininfo.lub`'s `DSLName[id]`: its name and icon for the tab, the number, message and blue-message sprites,
//! and the million unit (0xd620d0; `docs/nostalre-kro-exe-equip-window.md`, "Damage Indicator tab").

use std::collections::HashMap;

use nostalre_formats::lua_source::{LuaKey, LuaTable, LuaValue, globals_with};
use nostalre_formats::lub::{Key, LuaState, Value};

/// `damageskinlist.lub`'s `DSList`: each skin's name and id
pub type SkinIds = Vec<(String, f64)>;

#[derive(Debug, Clone, PartialEq)]
pub struct DamageSkin {
    /// `DSDisplayName`, under its cell
    pub name: String,
    /// `DSIconName` as a texture path
    pub icon: String,
    /// `BaseNumber`, `BaseMsg` and `BaseBlue` as sprite paths without the extension
    pub number: String,
    pub msg: String,
    pub blue: String,
    /// `DSMillionUnit`: 2 spaces the digits of 100,000,000 and more more tightly (0xc5fcb0)
    pub million_unit: u8,
}

/// `이팩트\숫자` → `data/sprite/이팩트/숫자`
fn sprite_path(name: &str) -> String {
    format!("data/sprite/{}", name.replace('\\', "/"))
}

/// `유저인터페이스\item\기본` → `data/texture/유저인터페이스/item/기본.bmp` (the tab asks for "%s.bmp")
fn icon_path(name: &str) -> String {
    format!("data/texture/{}.bmp", name.replace('\\', "/"))
}

/// `DSList` out of the compiled list's state; empty when it defines none
pub fn skin_ids(list: &LuaState) -> SkinIds {
    let Some(table) = list.global_table("DSList") else {
        return Vec::new();
    };
    table
        .iter()
        .filter_map(|(key, value)| match (key, value) {
            (Key::Str(name), Value::Number(id)) => {
                Some((String::from_utf8_lossy(name).into_owned(), *id))
            }
            _ => None,
        })
        .collect()
}

/// The skins in id order, from `DSList` and the info file's source; stops at the first id the info file lacks
pub fn parse(ids: &SkinIds, info_source: &[u8]) -> Vec<DamageSkin> {
    let count = ids.len();
    let ds_list = LuaValue::Table(LuaTable {
        entries: ids
            .iter()
            .map(|(name, id)| (LuaKey::Name(name.clone()), LuaValue::Number(*id)))
            .collect(),
    });
    let globals = globals_with(
        info_source,
        HashMap::from([("DSList".to_string(), ds_list)]),
    );
    let Some(names) = globals.get("DSLName").and_then(LuaValue::as_table) else {
        return Vec::new();
    };
    let by_id: HashMap<i64, &LuaTable> = names
        .entries
        .iter()
        .filter_map(|(key, value)| match key {
            LuaKey::Index(id) => Some((*id, value.as_table()?)),
            LuaKey::Name(_) => None,
        })
        .collect();
    let text = |entry: &LuaTable, field: &str| {
        entry
            .get(field)
            .and_then(LuaValue::as_str)
            .unwrap_or_default()
            .to_string()
    };
    (0..count as i64)
        .map_while(|id| by_id.get(&id))
        .map(|entry| DamageSkin {
            name: text(entry, "DSDisplayName"),
            icon: icon_path(&text(entry, "DSIconName")),
            number: sprite_path(&text(entry, "BaseNumber")),
            msg: sprite_path(&text(entry, "BaseMsg")),
            blue: sprite_path(&text(entry, "BaseBlue")),
            million_unit: entry
                .get("DSMillionUnit")
                .and_then(LuaValue::as_number)
                .map_or(1, |n| n as u8),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const INFO: &[u8] = br#"
DSLName = {
	[DSList.DT_Default] = {
		BaseNumber = "effect\\number",
		BaseMsg = "effect\\msg",
		BaseBlue = "effect\\bluemsg",
		DSDisplayName = "Normal",
		DSIconName = "ui\\item\\normal",
		DSMillionUnit = 1
	},
	[DSList.DT_Words] = {
		BaseNumber = "effect\\Words",
		BaseMsg = "effect\\Words_MSG",
		BaseBlue = "effect\\Words_BMSG",
		DSDisplayName = "Words",
		DSIconName = "ui\\item\\Words",
		DSMillionUnit = 2
	}
}
"#;

    fn ids(names: &[(&str, f64)]) -> SkinIds {
        names.iter().map(|(n, id)| (n.to_string(), *id)).collect()
    }

    #[test]
    fn skins_are_read_in_id_order_with_their_paths() {
        let skins = parse(&ids(&[("DT_Default", 0.0), ("DT_Words", 1.0)]), INFO);
        assert_eq!(skins.len(), 2);
        assert_eq!(skins[0].name, "Normal");
        assert_eq!(skins[0].number, "data/sprite/effect/number");
        assert_eq!(skins[0].icon, "data/texture/ui/item/normal.bmp");
        assert_eq!(skins[1].msg, "data/sprite/effect/Words_MSG");
        assert_eq!(skins[1].million_unit, 2);
    }

    #[test]
    fn a_skin_the_info_lacks_ends_the_list() {
        let skins = parse(
            &ids(&[("DT_Default", 0.0), ("DT_Gone", 1.0), ("DT_Words", 2.0)]),
            INFO,
        );
        assert_eq!(skins.len(), 1);
    }
}
