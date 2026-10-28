//! The title kRO shows on entering a map: a banner (`display_mapname\<background>.png`) with the map's subtitle small
//! over its main title large, from the pinned client's `mapInfo.lub` (`mapTbl`: `displayName`, `notifyEnter`,
//! `signName.subTitle/mainTitle`, `backgroundBmp`). Read from the 2022 exe: made on map load for a map with
//! `notifyEnter` (0x9293e0; background `noname` when none, the display name as the main title when it has no
//! titles), drawn whole for 5 seconds and faded out over 255 milliseconds (0x929ab0), laid out by 0x9285e0.

use std::collections::HashMap;

use nostalre_formats::lua_source::{LuaValue, globals};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapTitle {
    /// The banner's texture
    pub background: String,
    pub sub_title: String,
    pub main_title: String,
}

/// The titles of the maps that show one on entering, by map name (lowercase, no `.rsw`)
#[derive(Debug, Default)]
pub struct MapTitles {
    by_map: HashMap<String, MapTitle>,
}

/// Whole until then, in seconds
const SHOWN_SECS: f32 = 5.0;
/// Then gone over this
const FADE_SECS: f32 = 0.255;

impl MapTitles {
    /// `mapInfo.lub`'s source
    pub fn parse(source: &[u8]) -> Self {
        let globals = globals(source);
        let Some(maps) = globals.get("mapTbl").and_then(LuaValue::as_table) else {
            return Self::default();
        };
        let text = |table: Option<&nostalre_formats::lua_source::LuaTable>, field: &str| {
            table
                .and_then(|t| t.get(field))
                .and_then(LuaValue::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let by_map = maps
            .entries
            .iter()
            .filter_map(|(key, info)| {
                let nostalre_formats::lua_source::LuaKey::Name(name) = key else {
                    return None;
                };
                let info = info.as_table()?;
                if info.get("notifyEnter").and_then(LuaValue::as_bool) != Some(true) {
                    return None;
                }
                let sign = info.get("signName").and_then(LuaValue::as_table);
                let mut main_title = text(sign, "mainTitle");
                let sub_title = text(sign, "subTitle");
                if main_title.is_empty() && sub_title.is_empty() {
                    main_title = text(Some(info), "displayName");
                }
                let background = info
                    .get("backgroundBmp")
                    .and_then(LuaValue::as_str)
                    .unwrap_or("noname");
                Some((
                    map_key(name),
                    MapTitle {
                        background: format!(
                            "{}display_mapname/{background}.png",
                            nostalre_resources::dir::UI_TEXTURE
                        ),
                        sub_title,
                        main_title,
                    },
                ))
            })
            .collect();
        Self { by_map }
    }

    /// The title `map` shows on entering it, if it shows one
    pub fn on_enter(&self, map: &str) -> Option<&MapTitle> {
        self.by_map.get(&map_key(map))
    }

    pub fn len(&self) -> usize {
        self.by_map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_map.is_empty()
    }
}

fn map_key(map: &str) -> String {
    let map = map.trim().to_ascii_lowercase();
    map.strip_suffix(".rsw")
        .or_else(|| map.strip_suffix(".gat"))
        .unwrap_or(&map)
        .to_string()
}

/// A title being shown
#[derive(Debug, Clone, PartialEq)]
pub struct MapTitleBanner {
    pub title: MapTitle,
    age: f32,
}

impl MapTitleBanner {
    pub fn new(title: MapTitle) -> Self {
        Self { title, age: 0.0 }
    }

    /// Ages it; false once it is gone
    pub fn tick(&mut self, dt: f32) -> bool {
        self.age += dt;
        self.age < SHOWN_SECS + FADE_SECS
    }

    pub fn alpha(&self) -> f32 {
        if self.age <= SHOWN_SECS {
            1.0
        } else {
            (1.0 - (self.age - SHOWN_SECS) / FADE_SECS).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &[u8] = br#"
mapTbl = {
	["hero_in1.rsw"] = {
		displayName = "Herosria Spring Garden",
		notifyEnter = true,
		signName = { subTitle = "Herosria", mainTitle = "Spring Garden" },
		backgroundBmp = "siege"
	},
	["prt_in.rsw"] = { displayName = "Prontera Indoors", notifyEnter = true },
	["quiet.rsw"] = { displayName = "Quiet", notifyEnter = false, backgroundBmp = "field" },
}
main = function() return true end
"#;

    #[test]
    fn a_map_with_notify_enter_shows_its_titles_on_its_banner() {
        let titles = MapTitles::parse(SOURCE);
        assert_eq!(titles.len(), 2);
        let hero = titles.on_enter("HERO_IN1.gat").unwrap();
        assert_eq!(
            hero,
            &MapTitle {
                background: "data/texture/유저인터페이스/display_mapname/siege.png".into(),
                sub_title: "Herosria".into(),
                main_title: "Spring Garden".into(),
            }
        );
        assert!(titles.on_enter("quiet").is_none());
    }

    #[test]
    fn without_titles_the_display_name_on_the_noname_banner() {
        let titles = MapTitles::parse(SOURCE);
        let indoors = titles.on_enter("prt_in").unwrap();
        assert_eq!(indoors.main_title, "Prontera Indoors");
        assert_eq!(indoors.sub_title, "");
        assert!(indoors.background.ends_with("display_mapname/noname.png"));
    }

    #[test]
    fn a_banner_shows_5_seconds_then_fades_over_255_ms() {
        let mut banner =
            MapTitleBanner::new(MapTitles::parse(SOURCE).on_enter("prt_in").unwrap().clone());
        assert!(banner.tick(5.0));
        assert_eq!(banner.alpha(), 1.0);
        assert!(banner.tick(0.1275));
        assert!((banner.alpha() - 0.5).abs() < 0.01);
        assert!(!banner.tick(0.2));
    }
}
