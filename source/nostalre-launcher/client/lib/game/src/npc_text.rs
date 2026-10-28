//! The markup in an NPC's text (rAthena's `doc/script_commands.txt`, "HTML-like labels", and `mesitemicon`): what
//! shows and what a click on it does. kRO's exe knows these tags (its tag table at 0xd01884: URL, NAVI, ITEM, ITEML,
//! MSG, NAVIL, TIPBOX, QUEST; each `<TAG>label<INFO>value</INFO></TAG>`, split by 0xa89190). Beside the 2025 rig
//! (2026-10-19) its client also draws `<B>` bold and hides `<FONT ...>`, and draws a TIPBOX's label as a link.

use crate::data_table::DataTable;
use crate::item::Item;
use crate::navigation::{NavServices, NavTarget, Navigation};
use models::enums::item::ItemType;

/// What a click on a link does
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcLink {
    /// `<ITEM>label<INFO>item id</INFO></ITEM>` (`mesitemlink`): the item's description
    Item(u32),
    /// `<NAVI>label<INFO>map,x,y,0,000,flag</INFO></NAVI>`: navigation to that cell (rAthena's `flag`: 1 opens the
    /// navigation window, 0 not)
    Navigation(Navigation),
    /// `<URL>`, `<TIPBOX>`, `<QUEST>`: a link kRO opens a web page, its tip box or a quest with; nothing of ours yet
    Inert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcTextPart<'a> {
    /// Text as written, with its `^RRGGBB` colours
    Text(&'a str),
    Link {
        label: &'a str,
        link: NpcLink,
    },
    /// `^i[item id]` (`mesitemicon`, 2023 on): the item's icon, a link to its description
    ItemIcon(u32),
    /// `<B>` (true) and `</B>` (false): the text between is bold
    Bold(bool),
}

/// Tags whose label is a link nothing of ours opens yet (a web page, kRO's tip box and quest windows)
const INERT: [&str; 3] = ["URL", "TIPBOX", "QUEST"];
const ITEM_ICON: &str = "^i[";

/// `text` in parts; a tag missing its `<INFO>` or its end stays as written
pub fn parse(text: &str) -> Vec<NpcTextPart<'_>> {
    let mut parts = Vec::new();
    let mut plain_from = 0;
    let mut at = 0;
    while let Some(offset) = text[at..].find(['<', '^']) {
        let start = at + offset;
        match markup_at(&text[start..]) {
            Some((part, len)) => {
                if plain_from < start {
                    parts.push(NpcTextPart::Text(&text[plain_from..start]));
                }
                if let Some(part) = part {
                    parts.push(part);
                }
                at = start + len;
                plain_from = at;
            }
            None => at = start + 1,
        }
    }
    if plain_from < text.len() {
        parts.push(NpcTextPart::Text(&text[plain_from..]));
    }
    parts
}

/// The markup `rest` starts with and its length in bytes; `Some((None, _))` for one that shows nothing
fn markup_at(rest: &str) -> Option<(Option<NpcTextPart<'_>>, usize)> {
    if let Some(after) = rest.strip_prefix(ITEM_ICON) {
        let end = after.find(']')?;
        let id = after[..end].parse().ok()?;
        return Some((Some(NpcTextPart::ItemIcon(id)), ITEM_ICON.len() + end + 1));
    }
    let tag_len = rest.find('>')? + 1;
    let name = rest.get(1..tag_len - 1)?;
    match name {
        "B" => return Some((Some(NpcTextPart::Bold(true)), tag_len)),
        "/B" => return Some((Some(NpcTextPart::Bold(false)), tag_len)),
        // the size and colour not read: the rig showed only that the tag is not drawn
        "/FONT" => return Some((None, tag_len)),
        _ if name.starts_with("FONT ") => return Some((None, tag_len)),
        "ITEM" | "NAVI" => {}
        _ if INERT.contains(&name) => {}
        _ => return None,
    }
    let (label, info, len) = tagged(rest, name)?;
    let part = match name {
        "ITEM" => NpcTextPart::Link {
            label,
            link: NpcLink::Item(info.trim().parse().ok()?),
        },
        "NAVI" => NpcTextPart::Link {
            label,
            link: NpcLink::Navigation(navigation(info)?),
        },
        _ => NpcTextPart::Link {
            label,
            link: NpcLink::Inert,
        },
    };
    Some((Some(part), len))
}

/// `<name>label<INFO>info</INFO></name>` at the start of `rest`: its label, its info and its length
fn tagged<'a>(rest: &'a str, name: &str) -> Option<(&'a str, &'a str, usize)> {
    let body = &rest[name.len() + 2..];
    let info_at = body.find("<INFO>")?;
    let info_end = body.find("</INFO>")?;
    let closing = format!("</{name}>");
    let close_at = body.get(info_end..)?.find(&closing)? + info_end;
    let label = &body[..info_at];
    let info = body.get(info_at + "<INFO>".len()..info_end)?;
    Some((label, info, name.len() + 2 + close_at + closing.len()))
}

/// The item a link shows: the client has no type for an item the player may not hold, so it is shown as an etc
/// item, without card slots
pub fn linked_item(item_id: u32, data_table: &DataTable) -> Item {
    Item::from_tables(item_id, ItemType::Etc, data_table)
}

/// The item icons `text` shows, for loading their textures
pub fn item_icons(text: &str) -> impl Iterator<Item = u32> + '_ {
    parse(text).into_iter().filter_map(|part| match part {
        NpcTextPart::ItemIcon(id) => Some(id),
        _ => None,
    })
}

/// rAthena's `map,x,y,0,000,flag`; only the map, the cell and the last flag are documented, the two between (`0,000`
/// or `0,101` in its scripts) are not read
fn navigation(info: &str) -> Option<Navigation> {
    let fields: Vec<&str> = info.split(',').map(str::trim).collect();
    let [map, x, y, ..] = fields[..] else {
        return None;
    };
    Some(Navigation {
        map: map.to_string(),
        target: NavTarget::Cell {
            x: x.parse().ok()?,
            y: y.parse().ok()?,
        },
        services: NavServices::default(),
        hide_window: fields.get(5) != Some(&"1"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(map: &str, x: i16, y: i16) -> NpcLink {
        NpcLink::Navigation(Navigation {
            map: map.into(),
            target: NavTarget::Cell { x, y },
            services: NavServices::default(),
            hide_window: true,
        })
    }

    #[test]
    fn an_item_link_shows_its_label_and_opens_the_item() {
        let parts = parse("Bring me <ITEM>[Shadowdecon Ore]<INFO>25728</INFO></ITEM>, please.");
        assert_eq!(
            parts,
            [
                NpcTextPart::Text("Bring me "),
                NpcTextPart::Link {
                    label: "[Shadowdecon Ore]",
                    link: NpcLink::Item(25728)
                },
                NpcTextPart::Text(", please."),
            ]
        );
    }

    #[test]
    fn a_navigation_link_goes_to_its_cell() {
        let parts = parse(
            "contact the <NAVI>[Woodworker]<INFO>payon,252,297,0,101,0</INFO></NAVI> near the factory.",
        );
        assert_eq!(
            parts[1],
            NpcTextPart::Link {
                label: "[Woodworker]",
                link: cell("payon", 252, 297)
            }
        );
        // rAthena's one with a field short (npc/re: the Illusion Enchanter)
        let short = parse("<NAVI>Illusion Enchanter<INFO>prontera,90,115,000,0,</INFO></NAVI>");
        assert_eq!(
            short,
            [NpcTextPart::Link {
                label: "Illusion Enchanter",
                link: cell("prontera", 90, 115)
            }]
        );
    }

    #[test]
    fn a_navigation_flag_of_1_opens_the_window() {
        let parts = parse("<NAVI>[Tool Shop]<INFO>alberta,98,154,0,000,1</INFO></NAVI>");
        let [
            NpcTextPart::Link {
                link: NpcLink::Navigation(nav),
                ..
            },
        ] = &parts[..]
        else {
            panic!("{parts:?}");
        };
        assert!(!nav.hide_window);
    }

    #[test]
    fn an_item_icon_stands_for_its_item() {
        assert_eq!(
            parse("^i[1001583] ^i[1001581]"),
            [
                NpcTextPart::ItemIcon(1001583),
                NpcTextPart::Text(" "),
                NpcTextPart::ItemIcon(1001581)
            ]
        );
    }

    #[test]
    fn links_nothing_of_ours_opens_are_inert_links() {
        let inert = |label| NpcTextPart::Link {
            label,
            link: NpcLink::Inert,
        };
        assert_eq!(
            parse("Read the <TIPBOX>[List]<INFO>9163</INFO></TIPBOX>."),
            [
                NpcTextPart::Text("Read the "),
                inert("[List]"),
                NpcTextPart::Text(".")
            ]
        );
        assert_eq!(
            parse("<URL>[guide]<INFO>http://ro.gnjoy.com/guide/,1024,768</INFO></URL>"),
            [inert("[guide]")]
        );
    }

    #[test]
    fn bold_shows_and_font_tags_go() {
        assert_eq!(
            parse("Prefer <FONT SIZE = 14>^dd5555<B>Summoner</B>^000000</FONT> class."),
            [
                NpcTextPart::Text("Prefer "),
                NpcTextPart::Text("^dd5555"),
                NpcTextPart::Bold(true),
                NpcTextPart::Text("Summoner"),
                NpcTextPart::Bold(false),
                NpcTextPart::Text("^000000"),
                NpcTextPart::Text(" class."),
            ]
        );
    }

    #[test]
    fn colours_and_unknown_or_broken_tags_stay_as_written() {
        for text in [
            "^FF0000ten^000000 Porings",
            "press the <INSERT> key",
            "<ITEM>[Knife]<INFO>1201</ITEM>",
            "<NAVI>[x]<INFO>payon</INFO></NAVI>",
            "^i[abc]",
            "a < b",
        ] {
            assert_eq!(parse(text), [NpcTextPart::Text(text)], "{text}");
        }
    }
}
