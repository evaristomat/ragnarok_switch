//! Top left, down the screen: the party's other members, the companions (homunculus, mercenary, pet
//! last), what is attacking you.
use super::Unit;
use super::style::{self, Fill};
use crate::BuildCtx;
use nostalre_game::character::job_class_name;
use nostalre_game::pet::IntimacyState;
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;

const LEFT: f32 = 16.0;
const TOP: f32 = 14.0;
const WIDTH: f32 = 270.0;
pub(super) const RIGHT_EDGE: f32 = LEFT + WIDTH;
const NUMBER_W: f32 = 16.0;
const BADGE: f32 = 36.0;
const BADGE_GAP: f32 = 8.0;
const TEXT_X: f32 = LEFT + NUMBER_W + BADGE + BADGE_GAP;
const NOTE_GAP: f32 = 8.0;
const ROW_GAP: f32 = 11.0;
const SECTION_GAP: f32 = 18.0;
const HP_H: f32 = 8.0;
const SP_H: f32 = 5.0;
const BAR_GAP: f32 = 2.0;
const MAX_ATTACKERS: usize = 6;

/// One row: who, a right-hand note, and the bars under the name
struct Row<'a> {
    number: Option<usize>,
    badge: String,
    name: &'a str,
    note: String,
    note_color: [f32; 4],
    bars: Vec<(f32, Fill, f32)>,
    dim: bool,
}

impl<'a> Row<'a> {
    /// A homunculus or a mercenary: its level, HP and SP
    fn companion(
        badge: &str,
        name: &'a str,
        level: i16,
        hp: [u32; 2],
        sp: [u32; 2],
        dim: bool,
    ) -> Self {
        Row {
            number: None,
            badge: badge.into(),
            name,
            note: format!("Lv {level}"),
            note_color: style::DIM,
            bars: vec![
                (style::ratio(hp[0], hp[1]), style::HP, HP_H),
                (style::ratio(sp[0], sp[1]), style::SP, SP_H),
            ],
            dim,
        }
    }
}

/// A job's badge: the initials of its name, or its first two letters
fn job_badge(job: u16) -> String {
    let name = job_class_name(job);
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() > 1 {
        words
            .iter()
            .filter_map(|w| w.chars().next())
            .take(3)
            .collect()
    } else {
        name.chars().take(2).collect()
    }
}

pub fn build(ui: &mut UiFrame, ctx: &BuildCtx, attackers: &[Unit]) {
    let mut members = Vec::new();
    if let Some(party) = ctx.party {
        let others = party.members.iter().filter(|m| m.aid != ctx.local_aid);
        for (i, m) in others.enumerate() {
            let (note, note_color) = if !m.online {
                ("offline".to_string(), style::DIM)
            } else if m.dead {
                ("dead".to_string(), style::WARN)
            } else {
                (format!("Lv {}", m.base_level), style::DIM)
            };
            let bars = match (m.hp, m.max_hp) {
                (Some(hp), Some(max)) if m.online => vec![(style::ratio(hp, max), style::HP, HP_H)],
                _ => Vec::new(),
            };
            members.push(Row {
                number: Some(i + 1),
                badge: job_badge(m.class),
                name: &m.name,
                note,
                note_color,
                bars,
                dim: !m.online,
            });
        }
    }

    let mut y = TOP - SECTION_GAP;
    if let Some(party) = ctx.party.filter(|_| !members.is_empty()) {
        y = section(ui, TOP, &format!("Party · {}", party.name), &members);
    }

    let mut companions = Vec::new();
    if let Some(h) = ctx.homunculus {
        let (hp, sp) = ([h.hp, h.max_hp], [h.sp, h.max_sp]);
        companions.push(Row::companion(
            "HOM",
            &h.name,
            h.level,
            hp,
            sp,
            !h.is_alive(),
        ));
    }
    if let Some(m) = ctx.mercenary {
        let (hp, sp) = ([m.hp, m.max_hp], [m.sp, m.max_sp]);
        companions.push(Row::companion("MER", &m.name, m.level, hp, sp, false));
    }
    if ctx.pet.gid.is_some() {
        let fullness = f32::from(ctx.pet.hunger.clamp(0, 100)) / 100.0;
        companions.push(Row {
            number: None,
            badge: "PET".into(),
            name: &ctx.pet.name,
            note: IntimacyState::from_value(ctx.pet.intimacy).label().into(),
            note_color: style::DIM,
            bars: vec![(fullness, style::HUNGER, SP_H)],
            dim: false,
        });
    }
    if !companions.is_empty() {
        y = section(ui, y + SECTION_GAP, crate::pt!("Companions", "Companheiros"), &companions);
    }

    if !attackers.is_empty() {
        y += SECTION_GAP;
        let line = style::line(ui);
        style::heading_rule(ui, LEFT, y, WIDTH, "Attacking you");
        y += line + 2.0;
        for unit in attackers.iter().take(MAX_ATTACKERS) {
            let baseline = style::baseline(ui, y);
            let pct = unit.hp.map(style::percent);
            let pct_w = pct
                .as_deref()
                .map_or(0.0, |p| style::measure(ui, p) + NOTE_GAP);
            let name = style::fit(ui, &unit.name, WIDTH - pct_w);
            style::text(ui, LEFT, baseline, &name, style::TEXT);
            if let Some(pct) = pct {
                style::text_right(ui, RIGHT_EDGE, baseline, &pct, style::DIM);
            }
            y += line;
        }
    }
}

/// A heading and its rows; returns the y under them
fn section(ui: &mut UiFrame, top: f32, heading: &str, rows: &[Row]) -> f32 {
    let line = style::line(ui);
    style::heading_rule(ui, LEFT, top, WIDTH, heading);
    let mut y = top + line + 4.0;
    for row in rows {
        let bars_h: f32 = row.bars.iter().map(|(_, _, h)| h + BAR_GAP).sum();
        let row_h = (line + bars_h).max(BADGE);
        let badge_y = y + ((row_h - BADGE) / 2.0).floor();
        let baseline = style::baseline(ui, y);
        if let Some(n) = row.number {
            let number_baseline = style::baseline(ui, badge_y + ((BADGE - line) / 2.0).floor());
            style::text(ui, LEFT, number_baseline, &n.to_string(), style::ACCENT);
        }
        style::badge(ui, LEFT + NUMBER_W, badge_y, BADGE, &row.badge);
        let name_color = if row.dim { style::DIM } else { style::TEXT };
        let note_w = style::measure(ui, &row.note) + NOTE_GAP;
        let name = style::fit(ui, row.name, RIGHT_EDGE - TEXT_X - note_w);
        style::text(ui, TEXT_X, baseline, &name, name_color);
        style::text_right(ui, RIGHT_EDGE, baseline, &row.note, row.note_color);
        let mut bar_y = y + line;
        for &(ratio, fill, h) in &row.bars {
            style::bar(
                ui,
                Rect::new(TEXT_X, bar_y, RIGHT_EDGE - TEXT_X, h),
                ratio,
                fill,
            );
            bar_y += h + BAR_GAP;
        }
        y += row_h + ROW_GAP;
    }
    y - ROW_GAP
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_badge_is_its_initials_or_first_letters() {
        assert_eq!(job_badge(4252), "DK");
        assert_eq!(job_badge(4218), "Su");
    }
}
