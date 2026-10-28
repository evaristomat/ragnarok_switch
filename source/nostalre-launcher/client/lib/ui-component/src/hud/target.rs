//! Top centre: the target's name, level and HP; its cast above; whom it is after to the right.
use super::TargetInfo;
use super::style::{self, Trail};
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;

const TOP: f32 = 14.0;
const WIDTH: f32 = 460.0;
const HP_H: f32 = 12.0;
const CAST_W: f32 = 160.0;
const CAST_H: f32 = 6.0;
const AIM_W: f32 = 130.0;
const AIM_GAP: f32 = 16.0;
const AIM_H: f32 = 6.0;
const MIN_WIDTH: f32 = 300.0;
const EDGE_GAP: f32 = 24.0;
/// The minimap's corner
const RIGHT_RESERVE: f32 = 220.0;

/// Returns the plate's bottom edge, when there is a target
pub fn build(ui: &mut UiFrame, target: Option<&TargetInfo>, trail: &mut Trail) -> Option<f32> {
    let t = target?;
    let line = style::line(ui);
    let x = ((ui.ctx.screen_width - WIDTH) / 2.0)
        .floor()
        .max(super::party::RIGHT_EDGE + EDGE_GAP);
    let width = WIDTH
        .min(ui.ctx.screen_width - x - RIGHT_RESERVE)
        .max(MIN_WIDTH);
    let main_w = if t.target.is_some() {
        width - AIM_W - AIM_GAP
    } else {
        width
    };
    let mut y = TOP;

    if let Some(cast) = &t.cast {
        let bar_x = x + main_w - CAST_W;
        let baseline = style::baseline(ui, y);
        style::text_right(ui, bar_x - 6.0, baseline, &cast.name, style::TEXT);
        let bar_y = y + ((line - CAST_H) / 2.0).floor();
        style::bar(
            ui,
            Rect::new(bar_x, bar_y, CAST_W, CAST_H),
            cast.progress(),
            style::CAST,
        );
        y += line;
    }

    let baseline = style::baseline(ui, y);
    style::text_bold(ui, x, baseline, &t.unit.name, style::TEXT);
    if let Some(level) = t.level {
        let name_w = style::measure_bold(ui, &t.unit.name);
        style::text(
            ui,
            x + name_w + 8.0,
            baseline,
            &format!("Lv {level}"),
            style::DIM,
        );
    }
    let mut note = Vec::new();
    if t.boss {
        note.push("Boss".to_string());
    }
    if let Some(hp) = t.unit.hp {
        note.push(style::percent(hp));
    }
    let note_color = if t.boss { style::WARN } else { style::TEXT };
    style::text_right(ui, x + main_w, baseline, &note.join(" · "), note_color);
    let bar_y = y + line + 1.0;
    let hp = t.unit.hp.unwrap_or(1.0);
    let shown = trail.update(t.id, hp, ui.elapsed_secs);
    style::bar_trailed(
        ui,
        Rect::new(x, bar_y, main_w, HP_H),
        hp,
        shown,
        style::ENEMY_HP,
    );

    if let Some(aim) = &t.target {
        let ax = x + main_w + AIM_GAP;
        let label = format!("→ {}", aim.name);
        let label = style::fit(ui, &label, AIM_W);
        style::text(ui, ax, baseline, &label, style::DIM);
        style::bar(
            ui,
            Rect::new(ax, bar_y + HP_H - AIM_H, AIM_W, AIM_H),
            aim.hp.unwrap_or(1.0),
            style::HP,
        );
    }
    Some(bar_y + HP_H)
}
