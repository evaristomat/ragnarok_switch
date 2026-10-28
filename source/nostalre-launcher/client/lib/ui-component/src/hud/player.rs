//! Bottom centre, built up from the screen's edge as FF14's parameter bar: EXP, HP / SP (and AP), the
//! skill rows, buffs, the cast bar.
use super::HudInput;
use super::style::{self, Fill, Trail};
use crate::BuildCtx;
use crate::game::hotkey_bar::{DOCK_W, HotkeyDock, dock_height};
use crate::game::status_icon_bar::draw_status_icon;
use crate::helper::format::format_thousands;
use nostalre_game::character::is_fourth_job;
use nostalre_game::status_icon::status_icon_info;
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;

const BOTTOM: f32 = 6.0;
const GAP: f32 = 8.0;
const LABEL_GAP: f32 = 3.0;
const EXP_H: f32 = 7.0;
const BAR_H: f32 = 11.0;
const ICON: f32 = 32.0;
const ICON_GAP: f32 = 5.0;
const CAST_W: f32 = 280.0;
const CAST_H: f32 = 7.0;

struct Gauge {
    label: &'static str,
    cur: u32,
    max: u32,
    fill: Fill,
}

pub fn build(
    ui: &mut UiFrame,
    ctx: &BuildCtx,
    input: &HudInput,
    rows: usize,
    hp_trail: &mut Trail,
) -> HotkeyDock {
    let c = &*ctx.character;
    let line = style::line(ui);
    let w = DOCK_W;
    let x = ((ui.ctx.screen_width - w) / 2.0).floor();
    let mut y = ui.ctx.screen_height - BOTTOM - line;

    let half = ((w - GAP) / 2.0).floor();
    let baseline = style::baseline(ui, y);
    let base = format!(
        "Base Lv {} · {:.1}%",
        c.base_level,
        percent(c.base_exp_percentage())
    );
    let job = format!(
        "Job Lv {} · {:.1}%",
        c.job_level,
        percent(c.job_exp_percentage())
    );
    style::text(ui, x, baseline, &base, style::DIM);
    style::text_right(ui, x + w, baseline, &job, style::DIM);
    y -= LABEL_GAP + EXP_H;
    style::bar_light(
        ui,
        Rect::new(x, y, half, EXP_H),
        c.base_exp_percentage(),
        style::EXP,
    );
    style::bar_light(
        ui,
        Rect::new(x + w - half, y, half, EXP_H),
        c.job_exp_percentage(),
        style::JOB_EXP,
    );
    y -= GAP;

    let mut gauges = vec![
        Gauge {
            label: "HP",
            cur: c.hp,
            max: c.max_hp,
            fill: style::HP,
        },
        Gauge {
            label: "SP",
            cur: c.sp,
            max: c.max_sp,
            fill: style::SP,
        },
    ];
    // a 4th job before its first trait point has no AP to show
    if is_fourth_job(ctx.job_class) && c.traits.max_ap > 0 {
        gauges.push(Gauge {
            label: "AP",
            cur: c.traits.ap,
            max: c.traits.max_ap,
            fill: style::AP,
        });
    }
    let count = gauges.len() as f32;
    let gauge_w = ((w - GAP * (count - 1.0)) / count).floor();
    y -= BAR_H;
    let baseline = style::baseline(ui, y - LABEL_GAP - line);
    let labels_top = y - LABEL_GAP - line;
    for (i, g) in gauges.iter().enumerate() {
        let gx = x + i as f32 * (gauge_w + GAP);
        let bar = Rect::new(gx, y, gauge_w, BAR_H);
        let ratio = style::ratio(g.cur, g.max);
        if i == 0 {
            let trail = hp_trail.update(0, ratio, ui.elapsed_secs);
            style::bar_trailed(ui, bar, ratio, trail, g.fill);
        } else {
            style::bar(ui, bar, ratio, g.fill);
        }
        style::pill(ui, gx, labels_top, g.label, g.fill);
        let numbers = format!(
            "{} / {}",
            format_thousands(i64::from(g.cur)),
            format_thousands(i64::from(g.max))
        );
        style::text_right(ui, gx + gauge_w, baseline, &numbers, style::TEXT);
    }
    y -= LABEL_GAP + line + GAP;

    y -= dock_height(rows);
    let dock = HotkeyDock { x, y };
    if rows > 0 {
        y -= GAP;
    }

    y -= ICON;
    let pitch = ICON + ICON_GAP;
    let per_row = (((w + ICON_GAP) / pitch).floor() as usize).max(1);
    let now = ui.ctx.now_ms;
    let mut shown = 0;
    for (i, status) in c.active_statuses.iter().enumerate() {
        let Some(info) = status_icon_info(status.efst) else {
            continue;
        };
        let (row, col) = (shown / per_row, shown % per_row);
        let rect = Rect::new(x + col as f32 * pitch, y - row as f32 * pitch, ICON, ICON);
        draw_status_icon(ui, rect, status, info, now, i);
        shown += 1;
    }
    if shown == 0 {
        y += ICON;
    } else {
        y -= ((shown - 1) / per_row) as f32 * pitch;
    }
    y -= GAP;

    if let Some(cast) = &input.player_cast {
        let cx = x + ((w - CAST_W) / 2.0).floor();
        y -= CAST_H;
        style::bar(
            ui,
            Rect::new(cx, y, CAST_W, CAST_H),
            cast.progress(),
            style::CAST,
        );
        let baseline = style::baseline(ui, y - LABEL_GAP - line);
        style::text(ui, cx, baseline, &cast.name, style::TEXT);
        let left = format!("{:.1}s", cast.remaining.max(0.0));
        style::text_right(ui, cx + CAST_W, baseline, &left, style::DIM);
    }

    dock
}

/// A ratio as a percentage cut to one decimal, as kRO's basic info shows it
fn percent(ratio: f32) -> f32 {
    (ratio * 1000.0).floor() * 0.1
}
