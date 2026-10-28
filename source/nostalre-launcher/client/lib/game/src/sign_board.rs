//! kRO's sign boards: an icon, or an icon and a line of text on a board, over a map cell (`signboardlist.lub`, read
//! by the exe's sign board manager 0xadefa0 and placed by 0xade5b0 as world points, `C3DCooRendering`, that draw a
//! window there). They stand at the cell, not on an NPC.

use nostalre_formats::lua_table::SignBoardRow;
use nostalre_resources::dir::{TEXTURE, UI_TEXTURE};

/// What stands over the cell
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignKind {
    /// Kind 1 (and any kind the exe does not know): the icon alone, at its bitmap's size (`CBmpObjWnd`)
    Icon,
    /// Kind 3: the board (`bg_signboard.bmp`), the icon at (5, 5) and the text beside it (`CSignBoardWnd`)
    Board,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignBoard {
    pub map: String,
    pub cell: (i32, i32),
    /// World units above the ground: the row's height less 3, as the exe stores it
    pub lift: f32,
    pub kind: SignKind,
    /// The icon's texture
    pub image: String,
    pub text: String,
    pub color: [u8; 3],
}

/// The rows as the exe takes them: the first holds the defaults (an image for any row without one, a text and a
/// colour for a board without them); kind 2 makes another object (0xadf390) no row of kRO's uses, so it is left out
pub fn sign_boards(rows: &[SignBoardRow]) -> Vec<SignBoard> {
    let Some((defaults, rows)) = rows.split_first() else {
        return Vec::new();
    };
    let or_default = |value: &Option<String>, default: &Option<String>| {
        value
            .clone()
            .or_else(|| default.clone())
            .unwrap_or_default()
    };
    rows.iter()
        .filter_map(|row| {
            let kind = match row.kind {
                2 => {
                    tracing::debug!(map = row.map, "sign board of kind 2 left out");
                    return None;
                }
                3 => SignKind::Board,
                _ => SignKind::Icon,
            };
            let (text, color) = match kind {
                SignKind::Board => (
                    shown_text(&or_default(&row.text, &defaults.text)),
                    board_color(&or_default(&row.color, &defaults.color)),
                ),
                SignKind::Icon => (String::new(), [0; 3]),
            };
            Some(SignBoard {
                map: row.map.clone(),
                cell: (row.x, row.y),
                lift: row.height.saturating_sub(3) as f32,
                kind,
                image: texture_path(&or_default(&row.image, &defaults.image)),
                text,
                color,
            })
        })
        .collect()
}

/// How far from its cell an NPC may stand for a sign to show (ours: kRO draws every sign of its table, its server
/// having every NPC; rAthena lacks a third of them, so their signs would float over nothing. 547 of the table's 895
/// stand on an rAthena NPC's cell, 18 more within 2 cells)
const NPC_REACH: i32 = 2;

/// Whether one of the `npc_cells` stands near enough the sign for it to show
pub fn stands_by_npc(sign: &SignBoard, mut npc_cells: impl Iterator<Item = (u16, u16)>) -> bool {
    let (x, y) = sign.cell;
    npc_cells.any(|(nx, ny)| (i32::from(nx) - x).abs().max((i32::from(ny) - y).abs()) <= NPC_REACH)
}

/// A board's text in our English (`sign_board_text`); the data's own, with a warning, for one we lack
fn shown_text(text: &str) -> String {
    crate::sign_board_text::english(text).unwrap_or_else(|| {
        tracing::warn!(text, "sign board text with no English");
        text.to_string()
    })
}

/// An image path as the exe completes it: under the UI folder when it does not name it
fn texture_path(image: &str) -> String {
    let image = image.replace('\\', "/");
    let ui_folder = UI_TEXTURE.trim_start_matches(TEXTURE).trim_end_matches('/');
    let root = if image.contains(ui_folder) {
        TEXTURE
    } else {
        UI_TEXTURE
    };
    format!("{root}{image}")
}

/// A board's text colour as the exe reads it (0xadf820): `r,g,b`, or a hex number after `#` (`#0x00RRGGBB`), which
/// wins when both are there; anything else black
fn board_color(color: &str) -> [u8; 3] {
    if let Some((_, hex)) = color.split_once('#') {
        let hex = hex.trim();
        let digits = hex
            .strip_prefix("0x")
            .or_else(|| hex.strip_prefix("0X"))
            .unwrap_or(hex);
        let end = digits
            .find(|c: char| !c.is_ascii_hexdigit())
            .unwrap_or(digits.len());
        let value = u32::from_str_radix(&digits[..end], 16).unwrap_or(0);
        return [(value >> 16) as u8, (value >> 8) as u8, value as u8];
    }
    if !color.contains(',') {
        return [0; 3];
    }
    let mut channels = color.split(',').map(|part| atoi(part) as u8);
    [0; 3].map(|_| channels.next().unwrap_or(0))
}

/// C's `atoi`: leading blanks, a sign, the digits that follow; 0 for none
fn atoi(text: &str) -> i32 {
    let text = text.trim_start();
    let (sign, digits) = match text.strip_prefix('-') {
        Some(rest) => (-1, rest),
        None => (1, text.strip_prefix('+').unwrap_or(text)),
    };
    let end = digits
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(digits.len());
    sign * digits[..end].parse::<i32>().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(
        map: &str,
        kind: i32,
        image: Option<&str>,
        text: Option<&str>,
        color: Option<&str>,
    ) -> SignBoardRow {
        SignBoardRow {
            map: map.into(),
            x: 146,
            y: 89,
            height: 10,
            kind,
            image: image.map(Into::into),
            text: text.map(Into::into),
            color: color.map(Into::into),
        }
    }

    fn defaults() -> SignBoardRow {
        row(
            "Default",
            1,
            Some("유저인터페이스\\collection\\꽃가지.bmp"),
            Some("입력안하셨습니다"),
            Some("#0x009EBDFF"),
        )
    }

    #[test]
    fn an_icon_stands_over_its_cell_its_height_less_3_up() {
        let signs = sign_boards(&[
            defaults(),
            row(
                "prontera",
                1,
                Some("유저인터페이스\\information\\over_kafra.bmp"),
                None,
                None,
            ),
        ]);
        assert_eq!(
            signs,
            [SignBoard {
                map: "prontera".into(),
                cell: (146, 89),
                lift: 7.0,
                kind: SignKind::Icon,
                image: "data/texture/유저인터페이스/information/over_kafra.bmp".into(),
                text: String::new(),
                color: [0; 3],
            }]
        );
    }

    #[test]
    fn a_board_takes_the_defaults_it_lacks_in_english_and_its_image_goes_under_the_ui_folder() {
        let signs = sign_boards(&[
            defaults(),
            row("prt_in", 3, Some("item\\콜오브네메시스.bmp"), None, None),
            row("prontera", 3, None, Some("소드 클랜"), Some("#0x00FFFFFF")),
        ]);
        assert_eq!(
            signs[0].image,
            "data/texture/유저인터페이스/item/콜오브네메시스.bmp"
        );
        assert_eq!(
            (signs[0].text.as_str(), signs[0].color),
            ("No text", [0x9e, 0xbd, 0xff])
        );
        assert_eq!(
            signs[1].image,
            "data/texture/유저인터페이스/collection/꽃가지.bmp"
        );
        assert_eq!(
            (signs[1].text.as_str(), signs[1].color),
            ("Sword Clan", [0xff; 3])
        );
    }

    #[test]
    fn kinds_the_exe_does_not_know_are_icons_and_kind_2_is_left_out() {
        let signs = sign_boards(&[
            defaults(),
            row("a", 0, None, None, None),
            row("b", 2, None, None, None),
            row("c", 7, None, None, None),
        ]);
        let kinds: Vec<_> = signs.iter().map(|s| (s.map.as_str(), s.kind)).collect();
        assert_eq!(kinds, [("a", SignKind::Icon), ("c", SignKind::Icon)]);
    }

    #[test]
    fn a_colour_is_hex_after_a_hash_or_three_numbers() {
        assert_eq!(board_color("#0x00FFC800"), [0xff, 0xc8, 0x00]);
        assert_eq!(board_color("#9EBDFF"), [0x9e, 0xbd, 0xff]);
        assert_eq!(board_color("255, 128,0"), [255, 128, 0]);
        assert_eq!(board_color("1,2,3#0x00102030"), [0x10, 0x20, 0x30]);
        assert_eq!(board_color("white"), [0; 3]);
        assert_eq!(board_color(""), [0; 3]);
    }

    #[test]
    fn a_sign_shows_only_by_an_npc() {
        let [sign] = &sign_boards(&[defaults(), row("prontera", 1, None, None, None)])[..] else {
            panic!();
        };
        assert!(stands_by_npc(sign, [(146, 89)].into_iter()));
        assert!(stands_by_npc(sign, [(10, 10), (148, 91)].into_iter()));
        assert!(!stands_by_npc(sign, [(149, 89)].into_iter()));
        assert!(!stands_by_npc(sign, std::iter::empty()));
    }

    #[test]
    fn no_rows_no_signs() {
        assert!(sign_boards(&[]).is_empty());
        assert!(sign_boards(&[defaults()]).is_empty());
    }
}
