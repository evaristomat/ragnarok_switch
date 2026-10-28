//! Sign boards drawn where kRO's exe draws them: a window centred on each one's point, the icon alone (`CBmpObjWnd`,
//! the bitmap's size) or the board (`CSignBoardWnd`, 0xadfc90/0xadfdd0: `bg_signboard.bmp`, the icon at (5, 5), the
//! text in a 126 by 14 box at x 30, centred on the board's height)

use nostalre_game::sign_board::{SignBoard, SignKind};
use nostalre_renderer::font_atlas::FontAtlas;
use nostalre_ui::draw::{self, DrawCall, TextureRef};

pub const BOARD_TEX: &str = nostalre_resources::ui::SIGN_BOARD_BG;
/// `bg_signboard.bmp`'s size, for an archive without it
const BOARD_SIZE: (f32, f32) = (162.0, 36.0);
const ICON_AT: f32 = 5.0;
const TEXT_X: f32 = 30.0;
const TEXT_W: f32 = 126.0;
const TEXT_H: f32 = 14.0;
/// Half a board's width: how far off screen a sign's point may stand and still show
pub const REACH: f32 = BOARD_SIZE.0 / 2.0;

/// Draws each sign centred on its point; `size` gives a loaded texture's size, nothing for one not loaded (left
/// undrawn). The boards go first, then the icons, then the texts: the renderer merges neighbouring draws of one
/// texture, so the boards and the texts each take one.
pub fn draw_signs(
    draw_calls: &mut Vec<DrawCall>,
    atlas: &FontAtlas,
    size: impl Fn(&str) -> Option<(u32, u32)>,
    signs: &[(&SignBoard, f32, f32)],
) {
    let size = |path: &str| size(path).map(|(w, h)| (w as f32, h as f32));
    let board = size(BOARD_TEX);
    let (board_w, board_h) = board.unwrap_or(BOARD_SIZE);
    let board_at = |x: f32, y: f32| (x - board_w / 2.0, y - board_h / 2.0);
    let boards = || {
        signs
            .iter()
            .filter(|(sign, ..)| sign.kind == SignKind::Board)
    };

    if board.is_some() {
        for &(_, x, y) in boards() {
            let (bx, by) = board_at(x, y);
            quad(draw_calls, bx, by, board_w, board_h, BOARD_TEX);
        }
    }
    for &(sign, x, y) in signs {
        let Some((w, h)) = size(&sign.image) else {
            continue;
        };
        let (ix, iy) = match sign.kind {
            SignKind::Icon => (x - w / 2.0, y - h / 2.0),
            SignKind::Board => {
                let (bx, by) = board_at(x, y);
                (bx + ICON_AT, by + ICON_AT)
            }
        };
        quad(draw_calls, ix, iy, w, h, &sign.image);
    }
    for &(sign, x, y) in boards() {
        let (bx, by) = board_at(x, y);
        let text = crate::helper::format::truncate_to_width(&sign.text, TEXT_W, atlas);
        let [r, g, b] = sign.color.map(|c| c as f32 / 255.0);
        let text_top = by + board_h / 2.0 - TEXT_H / 2.0;
        let baseline = text_top + (TEXT_H + atlas.line_height) / 2.0;
        let (v, i) = draw::text_vertices(&text, bx + TEXT_X, baseline, [r, g, b, 1.0], atlas);
        if !v.is_empty() {
            draw_calls.push(DrawCall {
                vertices: v.into(),
                indices: i.into(),
                texture: TextureRef::FontAtlas,
            });
        }
    }
}

fn quad(draw_calls: &mut Vec<DrawCall>, x: f32, y: f32, w: f32, h: f32, path: &str) {
    let (v, i) = draw::quad_vertices(x, y, w, h, [1.0; 4]);
    draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: i.into(),
        texture: TextureRef::Named(path.to_string().into()),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_ui::test_support::atlas;

    fn sign(kind: SignKind, image: &str) -> SignBoard {
        SignBoard {
            map: "prontera".into(),
            cell: (0, 0),
            lift: 0.0,
            kind,
            image: image.into(),
            text: "Shop".into(),
            color: [255; 3],
        }
    }

    fn textures(calls: &[DrawCall]) -> Vec<String> {
        calls
            .iter()
            .map(|call| match &call.texture {
                TextureRef::Named(path) => path.to_string(),
                TextureRef::FontAtlas => "font".into(),
                _ => "other".into(),
            })
            .collect()
    }

    #[test]
    fn boards_then_icons_then_texts() {
        let (a, b) = (
            sign(SignKind::Board, "a.bmp"),
            sign(SignKind::Board, "b.bmp"),
        );
        let kafra = sign(SignKind::Icon, "kafra.bmp");
        let mut calls = Vec::new();
        draw_signs(
            &mut calls,
            atlas(),
            |_| Some((24, 24)),
            &[
                (&a, 100.0, 100.0),
                (&kafra, 300.0, 100.0),
                (&b, 500.0, 100.0),
            ],
        );
        assert_eq!(
            textures(&calls),
            [
                BOARD_TEX,
                BOARD_TEX,
                "a.bmp",
                "kafra.bmp",
                "b.bmp",
                "font",
                "font"
            ]
        );
    }

    #[test]
    fn what_is_not_loaded_is_not_drawn() {
        let board = sign(SignKind::Board, "a.bmp");
        let mut calls = Vec::new();
        draw_signs(&mut calls, atlas(), |_| None, &[(&board, 100.0, 100.0)]);
        assert_eq!(textures(&calls), ["font"]);
    }
}
