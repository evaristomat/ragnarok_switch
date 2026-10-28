//! The Renewal status window (kRO 2020 and later, `statuswnd/`): the six stats and what they add up to, and under
//! the "Trait Status" strip the trait panel: POW to CRT, P.ATK to C.RATE and trait points. The trait panel is open
//! by default for 4th jobs, the only ones with trait points.

use crate::helper::window_chrome::{
    draw_sys_button, draw_textured_quad, draw_titlebar, label_color, push_quad, text_color,
};
use crate::{BuildCtx, InGameWindow, Window};
use nostalre_game::character::{Character, TRAIT_STAT_FIRST_ID, is_fourth_job};
use nostalre_game::event::GameEvent;
use nostalre_resources::ui::{basic, statuswnd};
use nostalre_ui::draw::strip_color_codes;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

pub const STATUS_WINDOW_ID: WidgetId = WidgetId(1500);
const CLOSE_BTN_ID: WidgetId = WidgetId(1501);
const MINI_BTN_ID: WidgetId = WidgetId(1502);
const TRAIT_TOGGLE_ID: WidgetId = WidgetId(1503);
/// STR to LUK raise arrows, 1510 to 1515
const UP_STAT_FIRST_ID: u32 = 1510;
/// POW to CRT raise arrows, 1520 to 1525
const UP_TRAIT_FIRST_ID: u32 = 1520;

const BG_TEX: &str = statuswnd::W_STATWIN_BG;
const BG_EX_TEX: &str = statuswnd::W_EX_STATWIN_BG;
const TITLEBAR_TEX: &str = basic::TITLEBAR_MID;

/// Default top-left corner, right of the basic info window and its menu
const DEFAULT_X: f32 = 230.0;
const DEFAULT_Y: f32 = 140.0;
const WIN_W: f32 = 280.0;
const TITLE_H: f32 = 17.0;
/// The panel without and with the trait rows (`w_statwin_bg`, `w_ex_statwin_bg`)
const PANEL_H: f32 = 124.0;
const PANEL_EX_H: f32 = 224.0;

const ROW_H: f32 = 16.0;
/// Baseline of the first stat row, from the top of the panel
const ROW_BASELINE: f32 = 15.0;
/// Top of the first raise arrow, from the top of the panel
const UP_TOP: f32 = 6.0;
/// The trait rows lie this far below the stat rows, in the same cells
const TRAIT_OFFSET: f32 = 121.0;

/// The value, arrow and cost cells (35..74, 75..86, 87..101)
const VALUE_LEFT: f32 = 37.0;
const BONUS_LEFT: f32 = 54.0;
const UP_LEFT: f32 = 75.0;
const COST_RIGHT: f32 = 98.0;
const COL1_RIGHT: f32 = 188.0;
const COL2_RIGHT: f32 = 275.0;

/// The "Trait Status" strip between the panels
const TOGGLE_LEFT: f32 = 2.0;
const TOGGLE_TOP: f32 = 104.0;
const TOGGLE_W: f32 = 276.0;
const TOGGLE_H: f32 = 18.0;
const TOGGLE_TEXT_LEFT: f32 = 20.0;

const STAT_ID_STR: u16 = 13;

/// msgstringtable: the toggle's label, then the tooltips (row order as drawn)
const MSG_TRAIT_STATUS: u16 = 3613;
const MSG_STAT_TIPS: u16 = 3112;
const MSG_TRAIT_TIPS: u16 = 3614;
/// P.ATK, S.MATK, H.PLUS, C.RATE; RES, MRES; trait points
const MSG_TRAIT_COL1_TIPS: [u16; 4] = [3620, 3621, 3624, 3625];
const MSG_TRAIT_COL2_TIPS: [u16; 2] = [3622, 3623];
const MSG_TRAIT_POINT_TIP: u16 = 3626;

const STAT_LABELS: [&str; 6] = ["Str", "Agi", "Vit", "Int", "Dex", "Luk"];
const TRAIT_LABELS: [&str; 6] = ["Pow", "Sta", "Wis", "Spl", "Con", "Crt"];

/// One row of the stat or trait list: the value, its bonus, the cost of the next point and the status id to raise
pub(crate) struct StatRow {
    pub base: u16,
    pub bonus: i16,
    pub cost: u16,
    pub status_id: u16,
}

impl StatRow {
    /// Whether `points` cover the next point
    pub fn affordable(&self, points: u32) -> bool {
        self.cost > 0 && u32::from(self.cost) <= points
    }
}

/// STR to LUK
pub(crate) fn stat_rows(c: &Character) -> [StatRow; 6] {
    let stats = [
        (c.str, c.str_bonus, c.str_cost),
        (c.agi, c.agi_bonus, c.agi_cost),
        (c.vit, c.vit_bonus, c.vit_cost),
        (c.int, c.int_bonus, c.int_cost),
        (c.dex, c.dex_bonus, c.dex_cost),
        (c.luk, c.luk_bonus, c.luk_cost),
    ];
    std::array::from_fn(|i| StatRow {
        base: u16::from(stats[i].0),
        bonus: stats[i].1,
        cost: stats[i].2,
        status_id: STAT_ID_STR + i as u16,
    })
}

/// POW to CRT
pub(crate) fn trait_rows(c: &Character) -> [StatRow; 6] {
    std::array::from_fn(|i| StatRow {
        base: c.traits.stats[i].base,
        bonus: c.traits.stats[i].bonus,
        cost: c.traits.stats[i].cost,
        status_id: TRAIT_STAT_FIRST_ID + i as u16,
    })
}

/// The event that raises a stat or trait row by a point
pub(crate) fn raise(row: &StatRow, is_trait: bool) -> GameEvent {
    let status_id = row.status_id;
    if is_trait {
        GameEvent::RequestTraitStatChange {
            status_id,
            amount: 1,
        }
    } else {
        GameEvent::RequestStatChange {
            status_id,
            amount: 1,
        }
    }
}

/// What the stats add up to, as the window shows them: ATK, MATK, HIT, CRIT, then DEF, MDEF, FLEE, ASPD
pub(crate) fn combat(c: &Character) -> [String; 8] {
    let aspd = (200.0 - c.aspd as f32 / 10.0).max(0.0).floor() as i32;
    [
        format!("{} {}", c.atk1, fmt_signed(c.atk2)),
        format!("{} {}", c.matk2, fmt_signed(c.matk1)),
        c.hit.to_string(),
        c.critical.to_string(),
        format!("{} {}", c.def1, fmt_signed(c.def2)),
        format!("{} {}", c.mdef1, fmt_signed(c.mdef2)),
        format!("{} {}", c.flee1, fmt_signed(c.flee2)),
        aspd.to_string(),
    ]
}

fn fmt_signed(v: i32) -> String {
    if v >= 0 {
        format!("+ {}", v)
    } else {
        format!("- {}", -v)
    }
}

pub struct StatusWindow {
    pub has_grf_textures: bool,
    visible: bool,
    minimized: bool,
    /// The trait panel; `None` until the window first opens, then open for 4th jobs
    expanded: Option<bool>,
    arrow_size: (f32, f32),
    sys_btn_size: (f32, f32),
}

impl Default for StatusWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusWindow {
    pub fn new() -> Self {
        Self {
            has_grf_textures: false,
            visible: false,
            minimized: false,
            expanded: None,
            arrow_size: (11.0, 11.0),
            sys_btn_size: (11.0, 11.0),
        }
    }

    pub fn is_minimized(&self) -> bool {
        self.minimized
    }

    pub fn set_minimized(&mut self, value: bool) {
        self.minimized = value;
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
    pub fn open(&mut self) {
        self.visible = true;
    }
    pub fn is_visible(&self) -> bool {
        self.visible
    }
    pub fn is_expanded(&self) -> bool {
        self.expanded.unwrap_or(false)
    }

    /// The rows of `rows` from `top` (panel y of the first row): value, bonus, a raise arrow while `points` cover
    /// the cost, the cost.
    #[allow(clippy::too_many_arguments)]
    fn build_rows(
        &self,
        ui: &mut UiFrame,
        events: &mut Vec<GameEvent>,
        x: f32,
        top: f32,
        rows: &[StatRow; 6],
        points: u32,
        first_up_id: u32,
        is_trait: bool,
    ) {
        let grf = self.has_grf_textures;
        let tc = text_color(grf);
        let (arr_w, arr_h) = self.arrow_size;
        for (i, row) in rows.iter().enumerate() {
            let baseline = top + ROW_BASELINE + i as f32 * ROW_H;
            ui.text(x + VALUE_LEFT, baseline, &row.base.to_string(), tc);
            if row.bonus != 0 {
                ui.text(x + BONUS_LEFT, baseline, &fmt_signed(row.bonus as i32), tc);
            }
            if row.affordable(points) {
                let arr_rect =
                    Rect::new(x + UP_LEFT, top + UP_TOP + i as f32 * ROW_H, arr_w, arr_h);
                let resp = ui.interact(WidgetId(first_up_id + i as u32), arr_rect);
                if resp.hovered() {
                    ui.any_interactive_hovered = true;
                }
                if resp.clicked() {
                    events.push(raise(row, is_trait));
                }
                if grf {
                    let tex = if resp.hovered() {
                        basic::ARW_RIGHT_ON
                    } else {
                        basic::ARW_RIGHT
                    };
                    draw_textured_quad(ui, arr_rect.x, arr_rect.y, arr_w, arr_h, tex);
                } else {
                    let c = if resp.hovered() {
                        [0.95, 0.95, 0.4, 1.0]
                    } else {
                        [0.7, 0.7, 0.3, 1.0]
                    };
                    push_quad(ui, arr_rect.x, arr_rect.y, arr_w, arr_h, c);
                }
            }
            ui.text_right(x + COST_RIGHT, baseline, &row.cost.to_string(), tc);
        }
    }

    /// Right-aligned values, one per row from `top`
    fn build_column(ui: &mut UiFrame, right: f32, top: f32, lines: &[String], grf: bool) {
        for (i, t) in lines.iter().enumerate() {
            ui.text_right(
                right,
                top + ROW_BASELINE + i as f32 * ROW_H,
                t,
                text_color(grf),
            );
        }
    }

    /// The labels the panel art carries, for drawing without it
    fn draw_fallback_labels(ui: &mut UiFrame, x: f32, top: f32, expanded: bool) {
        let lc = label_color(false);
        let columns: [(f32, f32, &[&str]); 3] = [
            (10.0, 0.0, &STAT_LABELS),
            (
                108.0,
                0.0,
                &["Atk", "Matk", "Hit", "Critical", "Status Point", crate::pt!("Guild", "Clã")],
            ),
            (195.0, 0.0, &["Def", "Mdef", "Flee", "Aspd"]),
        ];
        let trait_columns: [(f32, f32, &[&str]); 3] = [
            (10.0, TRAIT_OFFSET, &TRAIT_LABELS),
            (
                108.0,
                TRAIT_OFFSET,
                &["P.Atk", "S.Matk", "H.Plus", "C.Rate", "T.Status Point"],
            ),
            (195.0, TRAIT_OFFSET, &["Res", "Mres"]),
        ];
        let shown = if expanded { &trait_columns[..] } else { &[] };
        for (left, offset, labels) in columns.iter().chain(shown) {
            for (i, label) in labels.iter().enumerate() {
                ui.text(
                    x + left,
                    top + offset + ROW_BASELINE + i as f32 * ROW_H,
                    label,
                    lc,
                );
            }
        }
    }

    /// A tooltip from msgstringtable while the mouse is over `rect`
    fn tip(ui: &mut UiFrame, ctx: &BuildCtx, rect: Rect, msg_id: u16) {
        if !rect.contains(ui.ctx.mouse_x, ui.ctx.mouse_y) {
            return;
        }
        if let Some(text) = ctx.data.msg_string.as_ref().and_then(|t| t.get(msg_id)) {
            let text = strip_color_codes(text);
            ui.tooltip(ui.ctx.mouse_x + 10.0, ui.ctx.mouse_y, &text);
        }
    }

    fn row_rect(x: f32, left: f32, width: f32, top: f32, i: usize) -> Rect {
        Rect::new(x + left, top + 2.0 + i as f32 * ROW_H, width, ROW_H)
    }

    fn build_tooltips(ui: &mut UiFrame, ctx: &BuildCtx, x: f32, top: f32, expanded: bool) {
        for i in 0..6 {
            Self::tip(
                ui,
                ctx,
                Self::row_rect(x, 5.0, 28.0, top, i),
                MSG_STAT_TIPS + i as u16,
            );
        }
        if !expanded {
            return;
        }
        let trait_top = top + TRAIT_OFFSET;
        for i in 0..6 {
            Self::tip(
                ui,
                ctx,
                Self::row_rect(x, 5.0, 28.0, trait_top, i),
                MSG_TRAIT_TIPS + i as u16,
            );
        }
        for (i, msg) in MSG_TRAIT_COL1_TIPS.iter().enumerate() {
            Self::tip(ui, ctx, Self::row_rect(x, 106.0, 84.0, trait_top, i), *msg);
        }
        for (i, msg) in MSG_TRAIT_COL2_TIPS.iter().enumerate() {
            Self::tip(ui, ctx, Self::row_rect(x, 193.0, 84.0, trait_top, i), *msg);
        }
        Self::tip(
            ui,
            ctx,
            Self::row_rect(x, 106.0, 170.0, trait_top, 4),
            MSG_TRAIT_POINT_TIP,
        );
    }
}

impl Window for StatusWindow {
    fn has_grf_textures(&self) -> bool {
        self.has_grf_textures
    }
    fn set_has_grf_textures(&mut self, value: bool) {
        self.has_grf_textures = value;
    }

    fn set_texture_sizes(&mut self, size_fn: &dyn Fn(&str) -> Option<(u32, u32)>) {
        if let Some((w, h)) = size_fn(basic::SYS_CLOSE_OFF) {
            self.sys_btn_size = (w as f32, h as f32);
        }
        if let Some((w, h)) = size_fn(basic::ARW_RIGHT) {
            self.arrow_size = (w as f32, h as f32);
        }
    }

    fn window_size(&self) -> (f32, f32) {
        let panel = if self.is_expanded() {
            PANEL_EX_H
        } else {
            PANEL_H
        };
        (WIN_W, TITLE_H + panel)
    }

    fn grf_texture_paths() -> Vec<&'static str> {
        vec![
            BG_TEX,
            BG_EX_TEX,
            statuswnd::EXPAND_ON_NORMAL,
            statuswnd::EXPAND_ON_PRESS,
            statuswnd::EXPAND_OFF_NORMAL,
            statuswnd::EXPAND_OFF_PRESS,
            TITLEBAR_TEX,
            basic::SYS_CLOSE_OFF,
            basic::SYS_CLOSE_ON,
            basic::SYS_MINI_OFF,
            basic::SYS_MINI_ON,
            basic::ARW_RIGHT,
            basic::ARW_RIGHT_ON,
        ]
    }
}

impl InGameWindow for StatusWindow {
    fn wants_escape(&self, _ctx: &BuildCtx) -> bool {
        self.visible
    }

    fn on_escape(&mut self, _ctx: &mut BuildCtx) -> Vec<GameEvent> {
        self.visible = false;
        Vec::new()
    }

    fn build(&mut self, ui: &mut UiFrame, ctx: &mut BuildCtx) -> Vec<GameEvent> {
        if !self.visible {
            return Vec::new();
        }
        self.expanded = Some(false);
        let expanded = false; // This edition has no traits.
        let prev_grf = ui.has_grf_textures;
        ui.has_grf_textures = self.has_grf_textures;
        let mut events = Vec::new();
        let grf = self.has_grf_textures;
        let tc = text_color(grf);

        let panel_h = if expanded { PANEL_EX_H } else { PANEL_H };
        let win_h = if self.minimized {
            TITLE_H
        } else {
            TITLE_H + panel_h
        };
        let win = ui.window_at(
            STATUS_WINDOW_ID,
            WIN_W,
            win_h,
            TITLE_H,
            DEFAULT_X,
            DEFAULT_Y,
        );
        ui.interact(STATUS_WINDOW_ID, Rect::new(win.x, win.y, WIN_W, win_h));
        let (x, y) = (win.x, win.y);

        draw_titlebar(ui, x, y, WIN_W, TITLE_H, grf);
        ui.text(x + 18.0, y + 13.0, "Status", tc);

        let (sys_w, sys_h) = self.sys_btn_size;
        let close_rect = Rect::new(x + WIN_W - 3.0 - sys_w, y + 3.0, sys_w, sys_h);
        let mini_rect = Rect::new(close_rect.x - sys_w - 1.0, y + 3.0, sys_w, sys_h);
        for (id, rect, on, off, glyph) in [
            (
                CLOSE_BTN_ID,
                close_rect,
                basic::SYS_CLOSE_ON,
                basic::SYS_CLOSE_OFF,
                'x',
            ),
            (
                MINI_BTN_ID,
                mini_rect,
                basic::SYS_MINI_ON,
                basic::SYS_MINI_OFF,
                '_',
            ),
        ] {
            let resp = ui.interact(id, rect);
            if resp.hovered() {
                ui.any_interactive_hovered = true;
            }
            if resp.clicked() {
                if id == CLOSE_BTN_ID {
                    self.visible = false;
                } else {
                    self.minimized = !self.minimized;
                }
            }
            draw_sys_button(
                ui,
                rect,
                (sys_w, sys_h),
                resp.hovered(),
                grf,
                on,
                off,
                Some(glyph),
            );
        }
        if self.minimized {
            ui.has_grf_textures = prev_grf;
            return events;
        }

        let top = y + TITLE_H;
        if grf {
            let bg = if expanded { BG_EX_TEX } else { BG_TEX };
            draw_textured_quad(ui, x, top, WIN_W, panel_h, bg);
        } else {
            crate::helper::fallback::window_body(ui, x, top, WIN_W, panel_h);
            Self::draw_fallback_labels(ui, x, top, expanded);
        }

        let character = &*ctx.character;
        let stat_rows = stat_rows(character);
        let trait_rows = trait_rows(character);
        let traits = &character.traits;
        let status_point = character.status_point;
        let [atk, matk, hit, crit, def, mdef, flee, aspd] = combat(character);
        let col1 = [atk, matk, hit, crit];
        let guild = ctx.guild.map(|g| g.name.clone()).unwrap_or_default();
        let col2 = [def, mdef, flee, aspd, status_point.to_string(), guild];

        self.build_rows(
            ui,
            &mut events,
            x,
            top,
            &stat_rows,
            status_point,
            UP_STAT_FIRST_ID,
            false,
        );
        Self::build_column(ui, x + COL1_RIGHT, top, &col1, grf);
        Self::build_column(ui, x + COL2_RIGHT, top, &col2, grf);

        // No trait toggle in the classic edition.
        if expanded {
            let trait_top = top + TRAIT_OFFSET;
            self.build_rows(
                ui,
                &mut events,
                x,
                trait_top,
                &trait_rows,
                traits.points,
                UP_TRAIT_FIRST_ID,
                true,
            );
            let t_col1 =
                [traits.p_atk, traits.s_matk, traits.h_plus, traits.c_rate].map(|v| v.to_string());
            Self::build_column(ui, x + COL1_RIGHT, trait_top, &t_col1, grf);
            let t_col2 = [
                traits.res.to_string(),
                traits.mres.to_string(),
                String::new(),
                String::new(),
                traits.points.to_string(),
            ];
            Self::build_column(ui, x + COL2_RIGHT, trait_top, &t_col2, grf);
        }
        Self::build_tooltips(ui, ctx, x, top, expanded);

        ui.has_grf_textures = prev_grf;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostalre_game::character::Character;
    use nostalre_game::data_table::DataTable;
    use nostalre_ui::context::UiContext;
    use nostalre_ui::state::StateCache;
    use nostalre_ui::test_support::test_frame;

    fn click(
        win: &mut StatusWindow,
        character: &mut Character,
        job: u16,
        at: Option<(f32, f32)>,
    ) -> Vec<GameEvent> {
        let data = DataTable::new();
        let mut ctx = UiContext::new(1024.0, 768.0);
        if let Some((mx, my)) = at {
            ctx.mouse_x = mx;
            ctx.mouse_y = my;
            ctx.mouse_clicked = true;
        }
        let mut state = StateCache::new();
        let mut ui = test_frame(&mut ctx, &mut state);
        let mut build_ctx = crate::BuildCtx::test(character, &data);
        build_ctx.job_class = job;
        win.build(&mut ui, &mut build_ctx)
    }

    /// The window's default top-left corner, and the panel's top
    const X: f32 = DEFAULT_X;
    const TOP: f32 = DEFAULT_Y + TITLE_H;

    fn trait_character() -> Character {
        let mut character = Character::new();
        character.traits.points = 3;
        character.traits.stats[0].cost = 1; // POW, affordable
        character.traits.stats[1].cost = 5; // STA, too dear
        character
    }

    #[test]
    fn the_classic_panel_never_opens_traits() {
        for (job, expanded) in [(0, false), (7, false), (4008, false), (4252, false)] {
            let mut win = StatusWindow::new();
            win.open();
            click(&mut win, &mut Character::new(), job, None);
            assert_eq!(win.is_expanded(), expanded, "job {job}");
            assert_eq!(
                win.window_size().1,
                TITLE_H + if expanded { PANEL_EX_H } else { PANEL_H }
            );
        }
    }

    #[test]
    fn the_former_trait_strip_is_inert() {
        let mut win = StatusWindow::new();
        win.open();
        let mut character = Character::new();
        let strip = (X + 100.0, TOP + TOGGLE_TOP + 5.0);
        click(&mut win, &mut character, 0, Some(strip));
        assert!(!win.is_expanded());
        click(&mut win, &mut character, 0, Some(strip));
        assert!(!win.is_expanded());
    }

    #[test]
    fn no_trait_packet_is_emitted_in_classic_mode() {
        let arrow = |row: usize| {
            (
                X + UP_LEFT + 5.0,
                TOP + TRAIT_OFFSET + UP_TOP + row as f32 * ROW_H + 5.0,
            )
        };
        let mut win = StatusWindow::new();
        win.open();
        let mut character = trait_character();
        let events = click(&mut win, &mut character, 4252, Some(arrow(0)));
        assert!(events.is_empty());
        assert!(click(&mut win, &mut character, 4252, Some(arrow(1))).is_empty());
    }

    #[test]
    fn a_stat_arrow_still_raises_the_stat() {
        let mut win = StatusWindow::new();
        win.open();
        let mut character = Character::new();
        character.status_point = 10;
        character.dex_cost = 2;
        let at = (X + UP_LEFT + 5.0, TOP + UP_TOP + 4.0 * ROW_H + 5.0);
        let events = click(&mut win, &mut character, 0, Some(at));
        assert!(matches!(
            events[..],
            [GameEvent::RequestStatChange {
                status_id: 17,
                amount: 1
            }]
        ));
    }
}
