//! Status: the six stats with their raise buttons, the traits for a 4th job, what they add up to.
use super::frame::{self, COLUMN_GAP, Hint, Key, Keys};
use super::{ACTION_BASE_ID, ROW_BASE_ID};
use crate::BuildCtx;
use crate::game::status_window::{StatRow, combat, raise, stat_rows, trait_rows};
use crate::hud::style;
use nostalre_game::character::is_fourth_job;
use nostalre_game::event::GameEvent;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const STATS: [&str; 6] = ["STR", "AGI", "VIT", "INT", "DEX", "LUK"];
const TRAITS: [&str; 6] = ["POW", "STA", "WIS", "SPL", "CON", "CRT"];
const ABOUT: [&str; 12] = [
    "STR: melee attack and how much you can carry.",
    "AGI: attack speed and dodging.",
    "VIT: HP, defence and recovery.",
    "INT: magic attack, SP and magic defence.",
    "DEX: hit, ranged attack and casting speed.",
    "LUK: critical hits and perfect dodges.",
    "POW: P.ATK, raw attack power.",
    "STA: RES, resistance to physical damage.",
    "WIS: MRES, resistance to magic damage.",
    "SPL: S.MATK, raw magic power.",
    "CON: H.PLUS, more healing done.",
    "CRT: C.RATE, harder critical hits.",
];
const COMBAT: [&str; 8] = ["ATK", "MATK", "HIT", "CRIT", "DEF", "MDEF", "FLEE", "ASPD"];
/// Twelve rows and two headings fit a 720-high screen
const ROW_H: f32 = 34.0;
const LEFT_W: f32 = 380.0;
const NAME_W: f32 = 64.0;
const VALUE_W: f32 = 46.0;
const BONUS_W: f32 = 56.0;
const PLUS: f32 = 26.0;

#[derive(Default)]
pub(super) struct Status {
    /// STR to LUK, then POW to CRT
    picked: usize,
}

impl Status {
    pub fn footer(&self) -> (String, Vec<Hint>) {
        (
            ABOUT[self.picked].to_string(),
            vec![
                Hint::MOVE,
                Hint::tap("Enter", crate::pt!("raise", "aumentar"), Key::Enter),
                Hint::BACK,
            ],
        )
    }

    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        area: Rect,
        keys: &Keys,
        events: &mut Vec<GameEvent>,
    ) {
        let c = &*ctx.character;
        let has_traits = is_fourth_job(ctx.job_class);
        let rows = if has_traits { 12 } else { 6 };
        self.picked = frame::step(keys, self.picked, rows);
        let left_w = LEFT_W.min(area.w * 0.5);
        let mut raised = None;

        let y = frame::section(ui, area.x, area.y, left_w, crate::pt!("Stats", "Atributos"));
        let stats = stat_rows(c);
        if let Some(i) = self.rows(ui, area.x, y, left_w, &stats, &STATS, 0, c.status_point) {
            raised = Some(i);
        }
        let mut y = y + 6.0 * ROW_H + 18.0;
        y = frame::section(ui, area.x, y, left_w, crate::pt!("Traits", "Características"));
        let traits = trait_rows(c);
        if has_traits {
            if let Some(i) = self.rows(ui, area.x, y, left_w, &traits, &TRAITS, 6, c.traits.points)
            {
                raised = Some(i);
            }
        } else {
            let line = crate::pt!("Trait points come with the 4th job.", "Pontos de característica são obtidos na 4ª classe.");
            style::text(ui, area.x, style::baseline(ui, y), line, style::DIM);
        }

        if keys.enter {
            raised = Some(self.picked);
        }
        if let Some(i) = raised {
            let (row, points, is_trait) = if i < 6 {
                (&stats[i], c.status_point, false)
            } else {
                (&traits[i - 6], c.traits.points, true)
            };
            if row.affordable(points) {
                events.push(raise(row, is_trait));
            }
        }

        let x = area.x + left_w + COLUMN_GAP;
        let w = area.w - left_w - COLUMN_GAP;
        let y = frame::section(ui, x, area.y, w, crate::pt!("Combat", "Combate"));
        let values = combat(c);
        grid(ui, x, y, w, &COMBAT, &values);
        if has_traits {
            let t = &c.traits;
            let y = frame::section(ui, x, y + 4.0 * ROW_H + 18.0, w, crate::pt!("Trait combat", "Combate avançado"));
            let labels = [
                "P.ATK",
                "S.MATK",
                "RES",
                "MRES",
                "H.PLUS",
                "C.RATE",
                crate::pt!("Trait points", "Pontos de característica"),
            ];
            let values = [
                t.p_atk,
                t.s_matk,
                t.res,
                t.mres,
                t.h_plus,
                t.c_rate,
                t.points as i32,
            ]
            .map(|v| v.to_string());
            grid(ui, x, y, w, &labels, &values);
        }
    }

    /// Six rows from `top`, numbered from `first`; returns the one whose raise was clicked
    #[allow(clippy::too_many_arguments)]
    fn rows(
        &mut self,
        ui: &mut UiFrame,
        x: f32,
        top: f32,
        w: f32,
        rows: &[StatRow; 6],
        names: &[&str; 6],
        first: usize,
        points: u32,
    ) -> Option<usize> {
        let line = style::line(ui);
        let mut raised = None;
        for (i, (row, name)) in rows.iter().zip(names).enumerate() {
            let index = first + i;
            let rect = Rect::new(x, top + i as f32 * ROW_H, w, ROW_H);
            let id = WidgetId(ROW_BASE_ID + index as u32);
            if frame::row(ui, id, rect, index == self.picked).clicked() {
                self.picked = index;
            }
            let baseline = frame::row_baseline(ui, rect);
            let mut cx = x + frame::ROW_TEXT;
            style::text_bold(ui, cx, baseline, name, style::TEXT);
            cx += NAME_W;
            style::text_right(
                ui,
                cx + VALUE_W,
                baseline,
                &row.base.to_string(),
                style::TEXT,
            );
            cx += VALUE_W + 10.0;
            if row.bonus != 0 {
                let bonus = format!("{:+}", row.bonus);
                style::text(ui, cx, baseline, &bonus, style::GOOD);
            }
            cx += BONUS_W;
            if row.cost > 0 {
                style::text(ui, cx, baseline, &crate::pt_format!("cost {}", "custo {}", row.cost), style::DIM);
            }
            if row.affordable(points) {
                let plus = Rect::new(
                    x + w - PLUS - 6.0,
                    rect.y + ((ROW_H - PLUS) / 2.0).floor(),
                    PLUS,
                    PLUS,
                );
                let resp = ui.interact(WidgetId(ACTION_BASE_ID + index as u32), plus);
                style::slot(ui, plus, resp.hovered());
                let pw = style::measure_bold(ui, "+");
                let pb = style::baseline(ui, plus.y + ((PLUS - line) / 2.0).floor());
                style::text_bold(
                    ui,
                    plus.x + ((PLUS - pw) / 2.0).floor(),
                    pb,
                    "+",
                    style::ACCENT,
                );
                if resp.clicked() {
                    self.picked = index;
                    raised = Some(index);
                }
            }
        }
        raised
    }
}

/// Labels and values in two columns
fn grid(ui: &mut UiFrame, x: f32, top: f32, w: f32, labels: &[&str], values: &[String]) {
    let line = style::line(ui);
    let col_w = ((w - COLUMN_GAP) / 2.0).floor();
    let rows = labels.len().div_ceil(2);
    for (i, (label, value)) in labels.iter().zip(values).enumerate() {
        let (col, row) = (i / rows, i % rows);
        let cx = x + col as f32 * (col_w + COLUMN_GAP);
        let baseline = style::baseline(
            ui,
            top + row as f32 * ROW_H + ((ROW_H - line) / 2.0).floor(),
        );
        style::text(ui, cx, baseline, label, style::DIM);
        style::text_right(ui, cx + col_w, baseline, value, style::TEXT);
    }
}
