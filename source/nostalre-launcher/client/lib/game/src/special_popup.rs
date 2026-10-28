//! The special pop-ups rAthena's `specialpopup` mapflag sends (0x0bbe, an id): their kind, colour and lines, from the
//! pinned client's `spopup.lub` (`popuptbl`: `uitype` 0 chat, 1 floating, 2 both, after its `SPUI_IDS`; `textcolor`
//! as 0xRRGGBB, the 2025 rig showing 16763904 as #FFCC00; `Description`, its lines)

use std::collections::HashMap;

use nostalre_formats::lua_source::{LuaKey, LuaValue, globals};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupKind {
    Chat,
    Floating,
    FloatingChat,
}

impl PopupKind {
    pub fn in_chat(self) -> bool {
        matches!(self, PopupKind::Chat | PopupKind::FloatingChat)
    }

    pub fn floating(self) -> bool {
        matches!(self, PopupKind::Floating | PopupKind::FloatingChat)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpecialPopup {
    pub kind: PopupKind,
    pub color: [f32; 4],
    pub lines: Vec<String>,
}

/// `spopup.lub`'s pop-ups by id
pub fn parse(source: &[u8]) -> HashMap<u32, SpecialPopup> {
    let globals = globals(source);
    let Some(table) = globals.get("popuptbl").and_then(LuaValue::as_table) else {
        return HashMap::new();
    };
    table
        .entries
        .iter()
        .filter_map(|(key, popup)| {
            let LuaKey::Index(id) = key else {
                return None;
            };
            let popup = popup.as_table()?;
            let kind = match popup.get("uitype").and_then(LuaValue::as_number) {
                Some(1.0) => PopupKind::Floating,
                Some(2.0) => PopupKind::FloatingChat,
                // 0, and a kind the table's SPUI_IDS do not name
                _ => PopupKind::Chat,
            };
            let rgb = popup
                .get("textcolor")
                .and_then(LuaValue::as_number)
                .unwrap_or(0.0) as u32;
            let channel = |shift: u32| ((rgb >> shift) & 0xff) as f32 / 255.0;
            let lines = popup
                .get("Description")
                .and_then(LuaValue::as_table)
                .map(|lines| {
                    lines
                        .list()
                        .filter_map(LuaValue::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            Some((
                u32::try_from(*id).ok()?,
                SpecialPopup {
                    kind,
                    color: [channel(16), channel(8), channel(0), 1.0],
                    lines,
                },
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_popup_with_its_kind_colour_and_lines() {
        let popups = parse(
            br#"
SPUI_IDS = { SPU_CHAT = 0, SPU_FLOATING = 1, SPU_FLOATING_CHAT = 2 }
popuptbl = {
	[1] = { uitype = SPUI_IDS.SPU_CHAT, textcolor = 16763904, Description = { "One.", "Two." } },
	[7] = { uitype = SPUI_IDS.SPU_FLOATING_CHAT, textcolor = 255, Description = { "Deep." } },
}
function main() return true, "good" end
"#,
        );
        let herosria = &popups[&1];
        assert_eq!(herosria.kind, PopupKind::Chat);
        assert_eq!(herosria.color, [1.0, 0.8, 0.0, 1.0]);
        assert_eq!(herosria.lines, ["One.", "Two."]);
        assert!(popups[&7].kind.in_chat() && popups[&7].kind.floating());
        assert_eq!(popups[&7].color, [0.0, 0.0, 1.0, 1.0]);
    }
}
