//! The menu's first page: the character, the companions, the bag and where you are.
use super::{frame, items};
use crate::BuildCtx;
use crate::helper::format::format_thousands;
use crate::hud::style::{self, Fill};
use crate::hud::{Place, place_name};
use nostalre_game::character::{is_fourth_job, job_class_name};
use nostalre_game::pet::{HungerState, IntimacyState};
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;

const LABEL_W: f32 = 96.0;
const NUMBER_W: f32 = 170.0;
const BAR_H: f32 = 10.0;
const EXP_H: f32 = 6.0;
const ROW_GAP: f32 = 8.0;
const SECTION_GAP: f32 = 30.0;
const COLUMN_GAP: f32 = 28.0;

pub(super) fn build(ui: &mut UiFrame, ctx: &BuildCtx, area: Rect, place: Option<&Place>) {
    let y = character(ui, ctx, area);
    let y = companions(ui, ctx, area, y + SECTION_GAP);
    session(ui, ctx, area, y + SECTION_GAP, place);
}

fn character(ui: &mut UiFrame, ctx: &BuildCtx, area: Rect) -> f32 {
    let c = &*ctx.character;
    let line = style::line(ui);
    let (x, mut y) = (area.x, area.y);
    let baseline = style::baseline(ui, y);
    style::text_bold(ui, x, baseline, &c.name, style::TEXT);
    let job_x = x + style::measure_bold(ui, &c.name) + 14.0;
    style::text(
        ui,
        job_x,
        baseline,
        job_class_name(ctx.job_class),
        style::DIM,
    );
    y += line + ROW_GAP;

    let facts = [
        (crate::pt!("Base Lv", "Nv. base"), c.base_level.to_string()),
        (crate::pt!("Job Lv", "Nv. classe"), c.job_level.to_string()),
        (crate::pt!("Status points", "Pontos de atributo"), c.status_point.to_string()),
        (crate::pt!("Skill points", "Pontos de habilidade"), c.skill_point.to_string()),
    ];
    let baseline = style::baseline(ui, y);
    let mut fx = x;
    for (label, value) in &facts {
        style::text(ui, fx, baseline, label, style::DIM);
        fx += style::measure(ui, label) + 6.0;
        style::text_bold(ui, fx, baseline, value, style::TEXT);
        fx += style::measure_bold(ui, value) + 22.0;
    }
    y += line + ROW_GAP;

    let mut gauges = vec![
        ("HP", c.hp, c.max_hp, style::HP),
        ("SP", c.sp, c.max_sp, style::SP),
    ];
    if is_fourth_job(ctx.job_class) && c.traits.max_ap > 0 {
        gauges.push(("AP", c.traits.ap, c.traits.max_ap, style::AP));
    }
    for (label, cur, max, fill) in gauges {
        let numbers = format!(
            "{} / {}",
            format_thousands(i64::from(cur)),
            format_thousands(i64::from(max))
        );
        y = gauge(
            ui,
            area,
            y,
            (label, &numbers),
            style::ratio(cur, max),
            BAR_H,
            fill,
        );
    }
    for (label, ratio, fill) in [
        (crate::pt!("Base EXP", "EXP base"), c.base_exp_percentage(), style::EXP),
        (crate::pt!("Job EXP", "EXP de classe"), c.job_exp_percentage(), style::JOB_EXP),
    ] {
        let pct = format!("{:.1}%", (ratio * 1000.0).floor() * 0.1);
        y = gauge(ui, area, y, (label, &pct), ratio, EXP_H, fill);
    }
    y - ROW_GAP
}

/// A label, a bar and its numbers on one line (`text`: the label, the numbers); a thin bar is an EXP one, on
/// the lighter track. Returns the y under it
fn gauge(
    ui: &mut UiFrame,
    area: Rect,
    y: f32,
    (label, numbers): (&str, &str),
    ratio: f32,
    h: f32,
    fill: Fill,
) -> f32 {
    let line = style::line(ui);
    let baseline = style::baseline(ui, y);
    style::text(ui, area.x, baseline, label, style::DIM);
    let bar_x = area.x + LABEL_W;
    let bar_w = (area.w - LABEL_W - NUMBER_W).max(40.0);
    let bar = Rect::new(bar_x, y + ((line - h) / 2.0).floor(), bar_w, h);
    if h < BAR_H {
        style::bar_light(ui, bar, ratio, fill);
    } else {
        style::bar(ui, bar, ratio, fill);
    }
    style::text_right(ui, area.x + area.w, baseline, numbers, style::TEXT);
    y + line + ROW_GAP
}

fn companions(ui: &mut UiFrame, ctx: &BuildCtx, area: Rect, top: f32) -> f32 {
    let line = style::line(ui);
    let y = frame::section(ui, area.x, top, area.w, crate::pt!("Companions", "Companheiros"));
    // a companion's name, what it is or how it feels, its HP
    let mut cards: Vec<(String, String, Option<f32>)> = Vec::new();
    if ctx.pet.gid.is_some() {
        let note = format!(
            "{} · {}",
            IntimacyState::from_value(ctx.pet.intimacy).label(),
            HungerState::from_value(ctx.pet.hunger).label()
        );
        cards.push((ctx.pet.name.clone(), note, None));
    }
    if let Some(h) = ctx.homunculus {
        let note = if h.is_alive() {
            crate::pt!("Homunculus", "Homúnculo")
        } else {
            crate::pt!("Homunculus · resting", "Homúnculo · descansando")
        };
        let hp = style::ratio(h.hp, h.max_hp);
        cards.push((crate::pt_format!("{} Lv {}", "{} Nv. {}", h.name, h.level), note.into(), Some(hp)));
    }
    if let Some(m) = ctx.mercenary {
        let hp = style::ratio(m.hp, m.max_hp);
        cards.push((
            crate::pt_format!("{} Lv {}", "{} Nv. {}", m.name, m.level),
            crate::pt!("Mercenary", "Mercenário").into(),
            Some(hp),
        ));
    }
    if cards.is_empty() {
        style::text(
            ui,
            area.x,
            style::baseline(ui, y),
            crate::pt!("No companions", "Sem companheiros"),
            style::DIM,
        );
        return y + line;
    }
    let col_w = ((area.w - COLUMN_GAP * 2.0) / 3.0).floor();
    let mut bottom = y;
    for (i, (name, note, bar)) in cards.iter().enumerate() {
        let x = area.x + i as f32 * (col_w + COLUMN_GAP);
        let mut cy = y;
        let name = style::fit(ui, name, col_w);
        style::text_bold(ui, x, style::baseline(ui, cy), &name, style::TEXT);
        cy += line + 2.0;
        if let Some(ratio) = bar {
            style::bar(ui, Rect::new(x, cy + 2.0, col_w, 6.0), *ratio, style::HP);
            cy += 12.0;
        }
        let note = style::fit(ui, note, col_w);
        style::text(ui, x, style::baseline(ui, cy), &note, style::DIM);
        bottom = bottom.max(cy + line);
    }
    bottom
}

fn session(ui: &mut UiFrame, ctx: &BuildCtx, area: Rect, top: f32, place: Option<&Place>) {
    let line = style::line(ui);
    let y = frame::section(ui, area.x, top, area.w, crate::pt!("Bag and place", "Inventário e posição"));
    let inv = &ctx.character.inventory;
    let mut facts = vec![
        (
            "Zeny",
            format!("{} z", format_thousands(i64::from(inv.zeny))),
        ),
        (crate::pt!("Weight", "Peso"), items::weight(inv)),
        (crate::pt!("Slots", "Espaços"), items::slots(inv)),
    ];
    if let Some(p) = place {
        facts.push((crate::pt!("Map", "Mapa"), place_name(ctx.data, &p.map).to_string()));
    }
    let col_w = ((area.w - COLUMN_GAP * 3.0) / 4.0).floor();
    for (i, (label, value)) in facts.iter().enumerate() {
        let x = area.x + i as f32 * (col_w + COLUMN_GAP);
        style::text(ui, x, style::baseline(ui, y), label, style::DIM);
        let value = style::fit(ui, value, col_w);
        let color = if *label == crate::pt!("Weight", "Peso") && inv.is_overweight() {
            style::WARN
        } else {
            style::TEXT
        };
        style::text_bold(ui, x, style::baseline(ui, y + line + 2.0), &value, color);
    }
}
