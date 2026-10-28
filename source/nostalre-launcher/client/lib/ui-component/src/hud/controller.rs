//! Compact integrated controller HUD. Slot indices match input/gamepad.rs exactly.
use super::{HudInput, HudPlacement, PortraitPlacement, compact as s};
use crate::{BuildCtx, game::hotkey_bar::HotkeyDock};
use nostalre_game::hotkey::{HotkeySlotContent, PAD_SET_SLOTS, PAD_SETS};
use nostalre_game::quest_guidance;
use nostalre_game::skill::skill_icon_path;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

/// The compact identity block stays beside the existing controls, even at narrow UI scales.
struct ProfileLayout {
    width: f32,
    face: Rect,
    vitals_x: f32,
    column: f32,
}
impl ProfileLayout {
    fn new(width: f32, height: f32) -> Self {
        let band = 290.0_f32.min(width - 72.0);
        let left = (width - band) * 0.5;
        Self {
            width: band,
            face: Rect::new(left - 24.0, height - 70.0, 44.0, 44.0),
            vitals_x: left + 28.0,
            column: (band - 36.0) * 0.5,
        }
    }
}

fn caption(ui: &mut UiFrame, face: Rect, name: &str, job: &str) {
    use nostalre_ui::draw::{self, DrawCall, TextureRef};
    let scale = 0.60;
    for (i, content) in [name, job].into_iter().enumerate() {
        let text = s::fit(ui, content, face.w * 0.82 / scale);
        let width = ui.atlas.measure_text(&text) * scale;
        let x = face.x + (face.w - width) * 0.5;
        let y = face.y + face.h + 2.0 + i as f32 * 10.0 + ui.atlas.ascent * scale;
        for (dx, dy, color) in [(0.6, 0.6, [0.0, 0.0, 0.0, 0.8]), (0.0, 0.0, s::DIM)] {
            let (vertices, indices) =
                draw::text_vertices_scaled(&text, x + dx, y + dy, color, ui.atlas, scale);
            ui.draw_calls.push(DrawCall {
                vertices: vertices.into(),
                indices: indices.into(),
                texture: TextureRef::FontAtlas,
            });
        }
    }
}

fn slots(set: u8, group: Option<u8>) -> Vec<(usize, usize)> {
    let base = usize::from(set.min(PAD_SETS - 1)) * PAD_SET_SLOTS;
    let groups: &[usize] = match group {
        Some(0) => &[0],
        Some(1) => &[1],
        Some(2) => &[2],
        _ => &[0, 2],
    };
    groups
        .iter()
        .flat_map(|g| (0..4).map(move |f| (base + g * 4 + f, f)))
        .collect()
}

pub(super) fn build(
    ui: &mut UiFrame,
    ctx: &BuildCtx,
    input: &HudInput,
    trail: &mut super::style::Trail,
) -> HudPlacement {
    let c = &*ctx.character;
    let profile = ProfileLayout::new(ui.ctx.screen_width, ui.ctx.screen_height);
    let w = profile.width;
    let x = (ui.ctx.screen_width - w) * 0.5;
    let top = ui.ctx.screen_height - 77.0;
    let group = input.controller_group;
    let set = input.touch_slots.set.min(PAD_SETS - 1);
    let visible = slots(set, group);
    let pitch = 29.0;
    let grid_w = visible.len() as f32 * pitch - 3.0 + if group.is_none() { 12.0 } else { 0.0 };
    let start =
        (ui.ctx.screen_width * 0.5 - grid_w * 0.5).max(profile.face.x + profile.face.w + 6.0);
    if let Some(group) = group {
        s::center(
            ui,
            ui.ctx.screen_width * 0.5,
            s::baseline(ui, top),
            ["L", "L + R", "R"][usize::from(group.min(2))],
            s::DIM,
        );
    } else {
        for (i, label) in ["L", "R"].iter().enumerate() {
            s::center(
                ui,
                start + 56.5 + i as f32 * (pitch * 4.0 + 12.0),
                s::baseline(ui, top),
                label,
                s::DIM,
            );
        }
    }
    for (i, (index, face)) in visible.iter().enumerate() {
        let gap = if group.is_none() && i >= 4 { 12.0 } else { 0.0 };
        let r = Rect::new(start + i as f32 * pitch + gap, top + 15.0, 26.0, 26.0);
        s::slot(ui, r, group.is_some());
        let content = c.hotkeys.get_slot(*index);
        let (path, count) = match content {
            HotkeySlotContent::Skill { skill, .. } => (Some(skill_icon_path(skill)), None),
            HotkeySlotContent::Item { item_id } => (
                ctx.data.item_icon_path(item_id),
                Some(c.inventory.count_of(item_id)),
            ),
            HotkeySlotContent::Empty => (None, None),
        };
        if let Some(path) = path {
            crate::helper::window_chrome::draw_textured_quad(
                ui,
                r.x + 3.0,
                r.y + 2.0,
                20.0,
                20.0,
                path,
            );
            if let HotkeySlotContent::Skill { skill, .. } = content {
                let secs = c.cooldowns.remaining_secs(skill, ui.elapsed_secs);
                if secs > 0.1 {
                    s::fill(ui, r, [0.0, 0.0, 0.0, 0.5]);
                    s::center(ui, r.x + 13.0, r.y + 18.0, &format!("{secs:.0}"), s::TEXT);
                }
            }
        }
        if let Some(count) = count {
            s::text(ui, r.x + 1.0, r.y + 25.0, &count.to_string(), s::TEXT);
        }
        if group.is_some() {
            s::right(
                ui,
                r.x + 25.0,
                r.y + 25.0,
                ["Y", "X", "B", "A"][*face],
                s::ACCENT,
            );
        }
    }
    // Mark the layer after its background, before all later windows and menus.
    s::panel(ui, profile.face);
    let portrait = PortraitPlacement {
        rect: profile.face,
        insert_index: ui.draw_calls.len(),
    };
    let job = if ctx.job_class == 0 {
        "Aprendiz"
    } else {
        nostalre_game::character::job_class_name(ctx.job_class)
    };
    caption(ui, profile.face, &c.name, job);
    let y = top + 45.0;
    for (i, (name, cur, max, color)) in
        [("HP", c.hp, c.max_hp, s::HP), ("SP", c.sp, c.max_sp, s::SP)]
            .into_iter()
            .enumerate()
    {
        let bx = profile.vitals_x + i as f32 * (profile.column + 8.0);
        s::text(ui, bx, s::baseline(ui, y), name, s::DIM);
        s::right(
            ui,
            bx + profile.column,
            s::baseline(ui, y),
            &format!("{cur}/{max}"),
            s::DIM,
        );
        s::bar(
            ui,
            Rect::new(bx, y + 13.0, profile.column, 3.0),
            super::style::ratio(cur, max),
            color,
        );
    }
    for (i, (label, ratio, color)) in [
        (
            format!("Base {}", c.base_level),
            c.base_exp_percentage(),
            s::EXP,
        ),
        (
            format!("Job {}", c.job_level),
            c.job_exp_percentage(),
            s::JOB_EXP,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let left = profile.vitals_x + i as f32 * (profile.column + 8.0);
        let label_width = s::measure(ui, &label);
        s::text(
            ui,
            left,
            s::baseline(ui, ui.ctx.screen_height - 16.0),
            &label,
            s::DIM,
        );
        let bx = left + label_width + 7.0;
        s::bar(
            ui,
            nostalre_ui::rect::Rect::new(
                bx,
                ui.ctx.screen_height - 10.0,
                (profile.column - label_width - 7.0).max(8.0),
                4.0,
            ),
            ratio,
            color,
        );
    }
    let minimap = Rect::new(ui.ctx.screen_width - 96.0, 10.0, 86.0, 86.0);
    if let Some(place) = &input.place {
        let label = format!(
            "{}  {}, {}",
            super::place_name(ctx.data, &place.map),
            place.x,
            place.y
        );
        s::text(
            ui,
            12.0,
            s::baseline(ui, 11.0),
            &s::fit(ui, &label, 230.0),
            s::TEXT,
        );
    }
    quest_panel(ui, ctx, input, minimap);
    if let Some(t) = &input.target {
        let tw = 190.0;
        let tx = ui.ctx.screen_width * 0.5 - tw * 0.5;
        let name = if let Some(level) = t.level {
            format!("{} · Nv. {level}", t.unit.name)
        } else {
            t.unit.name.clone()
        };
        s::text(
            ui,
            tx,
            s::baseline(ui, 12.0),
            &s::fit(ui, &name, tw),
            s::TEXT,
        );
        if let Some(hp) = t.unit.hp {
            let shown = trail.update(t.id, hp, ui.elapsed_secs);
            super::style::bar_trailed(
                ui,
                Rect::new(tx, 27.0, tw, 4.0),
                hp,
                shown,
                super::style::ENEMY_HP,
            );
        }
        if let Some(cast) = &t.cast {
            s::bar(
                ui,
                Rect::new(tx, 35.0, tw, 3.0),
                cast.progress(),
                super::style::CAST,
            );
            s::text(
                ui,
                tx,
                s::baseline(ui, 40.0),
                &s::fit(ui, &cast.name, tw),
                s::DIM,
            );
        }
    }
    // Party/companions and active statuses retain live data, in the otherwise free left margin.
    let party_start = ui.draw_calls.len();
    super::party::build(ui, ctx, &input.attackers);
    for call in &mut ui.draw_calls[party_start..] {
        for v in &mut call.vertices {
            v.position[0] = v.position[0] * 0.65 + 2.0;
            v.position[1] = v.position[1] * 0.65 + 30.0;
        }
    }
    if let Some(camera) = &input.camera {
        let label = format!(
            "Câmera  {:.0}° · {:.0}° · {:.0}",
            camera.turn_deg, camera.tilt_deg, camera.distance
        );
        s::center(
            ui,
            ui.ctx.screen_width * 0.5,
            s::baseline(ui, 61.0),
            &label,
            s::DIM,
        );
    }
    for (i, status) in c.active_statuses.iter().enumerate() {
        if let Some(info) = nostalre_game::status_icon::status_icon_info(status.efst) {
            crate::game::status_icon_bar::draw_status_icon(
                ui,
                Rect::new(
                    12.0 + i as f32 * 21.0,
                    ui.ctx.screen_height - 107.0,
                    18.0,
                    18.0,
                ),
                status,
                info,
                ui.ctx.now_ms,
                i,
            );
        }
    }
    if let Some(cast) = &input.player_cast {
        let r = Rect::new(x, top - 13.0, w, 4.0);
        s::bar(ui, r, cast.progress(), super::style::CAST);
        s::text(ui, x, r.y - 4.0, &s::fit(ui, &cast.name, w), s::TEXT);
    }
    if let Some(name) = &input.touch_slots.talk {
        let talk = if name.is_empty() {
            "Y  Conversar".into()
        } else {
            format!("Y  Conversar · {name}")
        };
        s::text(
            ui,
            12.0,
            s::baseline(ui, ui.ctx.screen_height - 72.0),
            &s::fit(ui, &talk, 220.0),
            s::TEXT,
        );
    }
    let menu = Rect::new(
        ui.ctx.screen_width - 82.0,
        ui.ctx.screen_height - 27.0,
        72.0,
        20.0,
    );
    s::text(ui, menu.x, s::baseline(ui, menu.y), "+  Menu", s::TEXT);
    let menu_tapped = ui.interact(WidgetId(7040), menu).clicked();
    if set > 0 || group.is_some() {
        s::right(
            ui,
            ui.ctx.screen_width - 12.0,
            ui.ctx.screen_height - 32.0,
            &format!("Atalhos {}/{}", set + 1, PAD_SETS),
            s::DIM,
        );
    }
    HudPlacement {
        hotkeys: HotkeyDock { x, y: top },
        minimap,
        menu_tapped,
        portrait: Some(portrait),
    }
}

fn plain(text: &str) -> String {
    use nostalre_game::npc_text::{self, NpcTextPart};
    let mut out = String::new();
    for part in npc_text::parse(text) {
        match part {
            NpcTextPart::Text(t) => out.push_str(t),
            NpcTextPart::Link { label, .. } => out.push_str(label),
            _ => {}
        }
    }
    nostalre_ui::draw::strip_color_codes(&out)
}

fn quest_panel(ui: &mut UiFrame, ctx: &BuildCtx, input: &HudInput, map: Rect) {
    let width = 198.0_f32.min(ui.ctx.screen_width - 24.0);
    let left = ui.ctx.screen_width - 10.0 - width;
    let line = s::line(ui) + 2.0;
    let y = map.y + map.h + 5.0;
    s::text(
        ui,
        left,
        s::baseline(ui, y),
        "Amarelo: missão/NPC  |  Azul: navegação",
        s::DIM,
    );
    let quest = ctx.quest_log.followed();
    let guide = quest
        .and_then(|q| quest_guidance::for_quest(ctx.quest_log, q.id))
        .or_else(|| {
            if quest.is_none() {
                input.place.as_ref().and_then(|p| {
                    quest_guidance::novice_hint(&p.map, ctx.job_class, ctx.character.job_level)
                })
            } else {
                None
            }
        });
    if quest.is_none() && guide.is_none() {
        return;
    }
    let title = guide
        .as_ref()
        .map(|g| g.title.to_string())
        .unwrap_or_else(|| {
            quest
                .map(|q| {
                    ctx.data
                        .quest_display
                        .as_ref()
                        .map(|t| t.title(q.id))
                        .unwrap_or_else(|| format!("Missão {}", q.id))
                })
                .unwrap_or_default()
        });
    let resume_training=guide.as_ref().zip(input.place.as_ref()).is_some_and(|(g,p)|
        g.destination.map.starts_with("new_") && !p.map.starts_with("new_"));
    let objective = if resume_training {
        "Treinamento pendente. Use + > Campanha / Equipe > Retomar treinamento.".to_string()
    } else { guide
        .as_ref()
        .map(|g| g.objective.clone())
        .unwrap_or_else(|| {
            quest
                .and_then(|q| ctx.data.quest_display.as_ref().and_then(|t| t.get(q.id)))
                .map(|entry| {
                    plain(if entry.summary.trim().is_empty() {
                        &entry.description
                    } else {
                        &entry.summary
                    })
                })
                .unwrap_or_default()
        }) };
    let mut lines = Vec::new();
    lines.push((if quest.is_some_and(|q|(900000..900100).contains(&q.id)) {"ECOS DE RUNE-MIDGARD"} else {"MISSÃO ACOMPANHADA"}.to_string(), s::DIM));
    for text in s::wrap(ui, &title, width - 12.0).into_iter().take(2) {lines.push((text,s::ACCENT));}
    for text in s::wrap(ui, &objective, width - 12.0).into_iter().take(3) {
        lines.push((text, s::TEXT));
    }
    if let Some(quest) = quest {
        for objective in quest.objectives.iter().take(2) {
            let progress = if objective.required > 0 {
                format!(
                    "{}  {}/{}",
                    objective.name, objective.current, objective.required
                )
            } else {
                format!("{}  {}", objective.name, objective.current)
            };
            lines.push((s::fit(ui, &progress, width - 12.0), s::TEXT));
        }
    }
    if let Some(guide) = guide.filter(|_|!resume_training) {
        if let Some(place) = &input.place {
            let target = quest_guidance::next_on_map(&guide.destination, &place.map)
                .unwrap_or(guide.destination);
            lines.push((
                s::fit(
                    ui,
                    &format!("{}  {}, {}", target.name, target.x, target.y),
                    width - 12.0,
                ),
                [0.35, 0.88, 1.0, 1.0],
            ));
        }
    }
    lines.push(((if resume_training {"+ Menu > Campanha / Equipe"} else {"+ Menu > Missões: escolher"}).into(), s::DIM));
    let y = y + line + 4.0;
    s::fill(
        ui,
        Rect::new(left, y - 3.0, width, lines.len() as f32 * line + 7.0),
        [0.02, 0.04, 0.06, 0.36],
    );
    for (i, (text, color)) in lines.into_iter().enumerate() {
        s::text(
            ui,
            left + 6.0,
            s::baseline(ui, y + i as f32 * line),
            &text,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portrait_and_level_columns_fit_handheld_scales_without_covering_the_bar() {
        for (w, h) in [
            (320.0, 240.0),
            (640.0, 360.0),
            (853.0, 480.0),
            (1280.0, 720.0),
        ] {
            let p = ProfileLayout::new(w, h);
            assert!(p.face.x >= 12.0 && p.face.y >= 0.0);
            assert!(p.face.x + p.face.w + 8.0 <= p.vitals_x);
            assert!(p.vitals_x + p.column * 2.0 + 8.0 <= w - 12.0);
            // Eight unmodified hotkey slots stay to the right of the portrait.
            let start = (w * 0.5 - 241.0 * 0.5).max(p.face.x + p.face.w + 6.0);
            assert!(start + 241.0 <= w - 12.0);
        }
    }
    #[test]
    fn rest_and_modifiers_expose_the_same_live_slots_as_input() {
        assert_eq!(
            slots(0, None).iter().map(|s| s.0).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 8, 9, 10, 11]
        );
        for set in 0..PAD_SETS {
            for group in 0..3 {
                assert_eq!(
                    slots(set, Some(group)),
                    (0..4)
                        .map(|f| (usize::from(set) * 12 + usize::from(group) * 4 + f, f))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}
