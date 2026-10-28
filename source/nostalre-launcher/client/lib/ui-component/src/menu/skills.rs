//! Skills: the job's tree (rAthena's skill tree, `nostalre_game::skill_tree`), one page per job of its line.
//! A skill sits a column right of everything it needs; what it needs from an earlier job sits faded in the
//! first column. Lines run from a skill to the ones it opens.
use super::bar_picker::ToBar;
use super::frame::{self, Hint, ICON, Key, Keys, ScrolledText, TABS_H, Textures};
use super::{NODE_BASE_ID, NODE_IDS};
use crate::BuildCtx;
use crate::game::hotkey_bar::skill_fits;
use crate::hud::style;
use nostalre_game::character::job_class_name;
use nostalre_game::data_table::skill_name_table::format_skill_display_name;
use nostalre_game::event::GameEvent;
use nostalre_game::skill::{SkillEnum, SkillList, skill_icon_path};
use nostalre_game::skill_tree::{self, TreeSkill};
use nostalre_ui::draw;
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const DETAIL_H: f32 = 160.0;
const MAX_PITCH: f32 = 190.0;
const NODE_GAP: f32 = 26.0;
const NODE_H: f32 = 40.0;
const ROW_PITCH: f32 = 48.0;
const MIN_ROW_GAP: f32 = 4.0;
const ELBOW: f32 = 12.0;
const LOCKED_ALPHA: f32 = 0.45;
/// The lines: faint, to the picked skill in the accent, from it in the picked row's blue
const LINE_DIM: [f32; 4] = style::faded(style::ACCENT, 0.18);
const LINE_OUT: [f32; 4] = style::PICKED_LINE;
const NODE_BG: [f32; 4] = [10.0 / 255.0, 16.0 / 255.0, 38.0 / 255.0, 0.7];
/// A node's ring: faint, the picked one in the accent, what the picked one needs in blue
const NODE_RING: [f32; 4] = style::faded(style::ACCENT, 0.3);
const NODE_RING_NEED: [f32; 4] = style::PICKED_LINE;

/// A skill on the page: from this job, or one an earlier job brings that a skill here needs
struct Node {
    skill: SkillEnum,
    max: u8,
    requires: &'static [(SkillEnum, u8)],
    /// The nodes of what it needs, on this page
    needs: Vec<usize>,
    /// From an earlier job
    earlier: bool,
    col: usize,
    row: usize,
    icon: String,
}

#[derive(Default)]
pub(super) struct Skills {
    /// The job the pages were laid out for
    job: Option<u16>,
    pages: Vec<(u16, Vec<Node>)>,
    /// The pages' tabs: their jobs' names
    labels: Vec<&'static str>,
    page: usize,
    picked: usize,
    /// The first row in sight
    offset: usize,
    /// Which rows are in sight (first, last, of how many), when the page is taller than the screen
    rows_shown: Option<(usize, usize, usize)>,
    /// The picked skill's description
    about: ScrolledText,
}

/// Lays out one page: earlier skills in the first column, then each skill one column right of what it needs,
/// near the rows of what it needs
fn layout(tree: &[TreeSkill], page: u16) -> Vec<Node> {
    let own: Vec<&TreeSkill> = tree.iter().filter(|s| s.page == page).collect();
    let mut nodes: Vec<Node> = Vec::new();
    for s in &own {
        for &(need, _) in s.entry.requires {
            let earlier = !own.iter().any(|o| o.entry.skill == need);
            if earlier && !nodes.iter().any(|n| n.skill == need) {
                let found = tree.iter().find(|t| t.entry.skill == need);
                nodes.push(Node {
                    skill: need,
                    max: found.map_or(0, |t| t.entry.max_level),
                    requires: &[],
                    needs: Vec::new(),
                    earlier: true,
                    col: 0,
                    row: nodes.len(),
                    icon: skill_icon_path(need),
                });
            }
        }
    }
    let base = usize::from(!nodes.is_empty());
    let first_own = nodes.len();
    for s in &own {
        nodes.push(Node {
            skill: s.entry.skill,
            max: s.entry.max_level,
            requires: s.entry.requires,
            needs: Vec::new(),
            earlier: false,
            col: base,
            row: 0,
            icon: skill_icon_path(s.entry.skill),
        });
    }
    for i in first_own..nodes.len() {
        let needs = nodes[i]
            .requires
            .iter()
            .filter_map(|(need, _)| nodes.iter().position(|n| n.skill == *need))
            .collect();
        nodes[i].needs = needs;
    }
    // columns: one right of the deepest need, until nothing moves (the tree has no cycles)
    for _ in 0..nodes.len() {
        let mut moved = false;
        for i in first_own..nodes.len() {
            let col = nodes[i]
                .needs
                .iter()
                .map(|&n| nodes[n].col + 1)
                .max()
                .unwrap_or(base)
                .max(base);
            if col != nodes[i].col {
                nodes[i].col = col;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    // rows: column by column, near the middle of what a skill needs
    let cols = nodes.iter().map(|n| n.col).max().unwrap_or(0);
    for col in base..=cols {
        let mut members: Vec<(usize, f32)> = (first_own..nodes.len())
            .filter(|&i| nodes[i].col == col)
            .map(|i| {
                let rows: Vec<f32> = nodes[i]
                    .needs
                    .iter()
                    .map(|&n| nodes[n].row as f32)
                    .collect();
                let mid = if rows.is_empty() {
                    f32::MAX
                } else {
                    rows.iter().sum::<f32>() / rows.len() as f32
                };
                (i, mid)
            })
            .collect();
        members.sort_by(|a, b| a.1.total_cmp(&b.1));
        let mut next = 0;
        for (i, mid) in members {
            let want = if mid == f32::MAX {
                next
            } else {
                mid.round() as usize
            };
            nodes[i].row = want.max(next);
            next = nodes[i].row + 1;
        }
    }
    nodes
}

fn level(list: &SkillList, skill: SkillEnum) -> i16 {
    list.get_skill(skill).map_or(0, |s| s.level)
}

fn met(list: &SkillList, node: &Node) -> bool {
    node.requires
        .iter()
        .all(|&(need, lv)| level(list, need) >= i16::from(lv))
}

impl Skills {
    /// The pages laid out again when the job changed; their icons asked for once
    fn refresh(&mut self, job: u16, textures: &mut Textures) {
        if self.job == Some(job) {
            return;
        }
        let tree = skill_tree::tree(job);
        self.pages = skill_tree::pages(job, &tree)
            .into_iter()
            .map(|page| (page, layout(&tree, page)))
            .collect();
        self.labels = self
            .pages
            .iter()
            .map(|(job, _)| job_class_name(*job))
            .collect();
        for (_, nodes) in &self.pages {
            for n in nodes {
                textures.want(&n.icon);
            }
        }
        self.job = Some(job);
        self.page = self.pages.len().saturating_sub(1);
        self.picked = self.first_own();
        self.offset = 0;
    }

    /// The top line's note: the skill points, and the rows in sight on a tall page
    pub fn summary(&self, skill_points: u32) -> String {
        match self.rows_shown {
            Some((first, last, rows)) => {
                crate::pt_format!("rows {first} to {last} of {rows} · Skill points {skill_points}", "linhas {first} a {last} de {rows} · Pontos de habilidade {skill_points}")
            }
            None => crate::pt_format!("Skill points {skill_points}", "Pontos de habilidade {skill_points}"),
        }
    }

    fn nodes(&self) -> &[Node] {
        self.pages.get(self.page).map_or(&[], |(_, n)| n.as_slice())
    }

    fn first_own(&self) -> usize {
        self.nodes().iter().position(|n| !n.earlier).unwrap_or(0)
    }

    fn can_learn(ctx: &BuildCtx, skill: SkillEnum) -> bool {
        let c = &*ctx.character;
        c.skill_point > 0 && c.skills.get_skill(skill).is_some_and(|s| s.upgradable)
    }

    /// Learnt and used, so it can go on the skill bar
    fn fits_bar(ctx: &BuildCtx, skill: SkillEnum) -> bool {
        ctx.character
            .skills
            .get_skill(skill)
            .is_some_and(skill_fits)
    }

    pub fn footer(&self, ctx: &BuildCtx) -> (String, Vec<Hint>) {
        let mut hints = vec![Hint::MOVE, Hint::TABS];
        if self
            .nodes()
            .get(self.picked)
            .is_some_and(|n| Self::fits_bar(ctx, n.skill))
        {
            hints.push(Hint::TO_BAR);
        }
        let names = ctx.data.skill_name.as_ref();
        let help = match self.nodes().get(self.picked) {
            Some(node) if Self::can_learn(ctx, node.skill) => {
                hints.push(Hint::tap("Enter", crate::pt!("learn", "aprender"), Key::Enter));
                let name = format_skill_display_name(node.skill, names);
                crate::pt_format!("Enter puts a point into {name}.", "Confirmar adiciona um ponto em {name}.")
            }
            Some(node) if node.earlier => {
                crate::pt!("From an earlier job: learnt on that job's page.", "Da classe anterior: aprenda na página daquela classe.").to_string()
            }
            _ => crate::pt!("Lines run from a skill to the ones it opens.", "As linhas indicam quais habilidades serão liberadas.").to_string(),
        };
        hints.push(Hint::BACK);
        (help, hints)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        ctx: &BuildCtx,
        area: Rect,
        keys: &Keys,
        textures: &mut Textures,
        to_bar: &mut Option<ToBar>,
        events: &mut Vec<GameEvent>,
    ) {
        self.refresh(ctx.job_class, textures);
        let clicked = frame::tabs(ui, area.x, area.y, &self.labels, self.page);
        let page = frame::switch_tab(keys, self.page, self.pages.len(), clicked);
        if page != self.page {
            self.page = page;
            self.picked = self.first_own();
            self.offset = 0;
        }
        let before = self.picked;
        self.picked = self.moved(keys);

        let tree = Rect::new(area.x, area.y + TABS_H, area.w, area.h - TABS_H - DETAIL_H);
        let cols = self.nodes().iter().map(|n| n.col + 1).max().unwrap_or(1);
        let pitch = (tree.w / cols as f32).min(MAX_PITCH).floor();
        let node_w = pitch - NODE_GAP;
        let rows = self.nodes().iter().map(|n| n.row + 1).max().unwrap_or(0);
        // rows close up to fit the page, down to a small gap, then it scrolls
        let row_pitch = ((tree.h - NODE_H) / rows.saturating_sub(1).max(1) as f32)
            .clamp(NODE_H + MIN_ROW_GAP, ROW_PITCH)
            .floor();
        let visible = ((tree.h + row_pitch - NODE_H) / row_pitch).floor() as usize;
        if let Some(row) = self.nodes().get(self.picked).map(|n| n.row) {
            let follow = before != self.picked;
            frame::scroll(ui, tree, &mut self.offset, row, rows, visible, follow);
        }
        let place = |n: &Node| {
            (n.row >= self.offset && n.row < self.offset + visible).then(|| {
                Rect::new(
                    tree.x + n.col as f32 * pitch,
                    tree.y + (n.row - self.offset) as f32 * row_pitch,
                    node_w,
                    NODE_H,
                )
            })
        };

        let nodes = self.nodes();
        for (to, n) in nodes.iter().enumerate() {
            for &from in &n.needs {
                let (Some(a), Some(b)) = (place(&nodes[from]), place(n)) else {
                    continue;
                };
                let (color, thick) = if to == self.picked {
                    (style::ACCENT, 2.0)
                } else if from == self.picked {
                    (LINE_OUT, 2.0)
                } else {
                    (LINE_DIM, 1.0)
                };
                link(ui, a, b, color, thick);
            }
        }

        let list = &ctx.character.skills;
        let needed_by_picked: &[usize] = nodes.get(self.picked).map_or(&[], |p| &p.needs);
        let mut picked = None;
        let mut learn = None;
        for (i, n) in nodes.iter().enumerate().take(NODE_IDS as usize) {
            let Some(rect) = place(n) else {
                continue;
            };
            let resp = ui.interact(WidgetId(NODE_BASE_ID + i as u32), rect);
            if resp.clicked() {
                picked = Some(i);
            }
            if resp.double_clicked() {
                learn = Some(n.skill);
            }
            let ring = if i == self.picked {
                style::ACCENT
            } else if needed_by_picked.contains(&i) {
                NODE_RING_NEED
            } else {
                NODE_RING
            };
            let open = n.earlier || met(list, n);
            let alpha = if open { 1.0 } else { LOCKED_ALPHA };
            node(ui, ctx, n, rect, ring, alpha, Self::can_learn(ctx, n.skill));
        }
        let picked = picked.unwrap_or(self.picked);
        if keys.enter {
            learn = nodes.get(picked).map(|n| n.skill);
        }
        if let Some(skill) = learn.filter(|&s| Self::can_learn(ctx, s)) {
            events.push(GameEvent::RequestSkillLevelUp { skill });
        }
        if keys.to_bar
            && let Some(skill) = nodes.get(picked).map(|n| n.skill)
            && Self::fits_bar(ctx, skill)
        {
            *to_bar = Some(ToBar::Skill(skill));
        }
        self.picked = picked;

        self.rows_shown =
            (rows > visible).then(|| (self.offset + 1, (self.offset + visible).min(rows), rows));

        let detail = Rect::new(area.x, tree.y + tree.h, area.w, DETAIL_H);
        self.detail(ui, ctx, detail);
    }

    /// The node the arrow keys move to: up and down in its column, left and right to the nearest row of
    /// the next column that has a skill
    fn moved(&self, keys: &Keys) -> usize {
        let nodes = self.nodes();
        let Some(cur) = nodes.get(self.picked) else {
            return 0;
        };
        let nearest = |pick: &dyn Fn(&Node) -> bool, by: &dyn Fn(&Node) -> usize| {
            nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| pick(n))
                .min_by_key(|(_, n)| by(n))
                .map(|(i, _)| i)
        };
        let target = if keys.up {
            nearest(&|n| n.col == cur.col && n.row < cur.row, &|n| {
                cur.row - n.row
            })
        } else if keys.down {
            nearest(&|n| n.col == cur.col && n.row > cur.row, &|n| {
                n.row - cur.row
            })
        } else if keys.left {
            nearest(&|n| n.col < cur.col, &|n| {
                (cur.col - n.col) * 100 + n.row.abs_diff(cur.row)
            })
        } else if keys.right {
            nearest(&|n| n.col > cur.col, &|n| {
                (n.col - cur.col) * 100 + n.row.abs_diff(cur.row)
            })
        } else {
            None
        };
        target.unwrap_or(self.picked)
    }

    /// Under the tree: the picked skill, what it needs and opens, its description
    fn detail(&mut self, ui: &mut UiFrame, ctx: &BuildCtx, area: Rect) {
        let Some(n) = self
            .pages
            .get(self.page)
            .and_then(|(_, nodes)| nodes.get(self.picked))
        else {
            return;
        };
        let names = ctx.data.skill_name.as_ref();
        let picked = self.picked;
        let opens: Vec<&str> = self
            .nodes()
            .iter()
            .filter(|m| m.needs.contains(&picked))
            .map(|m| format_skill_display_name(m.skill, names))
            .collect();
        let line = style::line(ui);
        let c = &*ctx.character;
        style::rule(ui, area.x, area.y + 6.0, area.w);
        let top = area.y + 18.0;
        let bottom = area.y + area.h;

        // what it is
        let col_w = (area.w * 0.3).floor();
        frame::image(
            ui,
            Rect::new(area.x, top, 36.0, 36.0),
            n.icon.clone(),
            [1.0; 4],
        );
        let tx = area.x + 48.0;
        let tw = col_w - 48.0 - 16.0;
        let name = style::fit(ui, format_skill_display_name(n.skill, names), tw);
        style::text_bold(ui, tx, style::baseline(ui, top), &name, style::TEXT);
        let lv = level(&c.skills, n.skill);
        let state = if n.earlier {
            crate::pt!("Earlier job", "Classe anterior")
        } else if !met(&c.skills, n) {
            crate::pt!("Locked", "Bloqueada")
        } else if lv >= i16::from(n.max) {
            crate::pt!("Mastered", "Dominada")
        } else if lv == 0 {
            crate::pt!("Not learnt", "Não aprendida")
        } else {
            crate::pt!("Learnt", "Aprendida")
        };
        let state = format!("{state} · {lv} / {}", n.max);
        style::text(ui, tx, style::baseline(ui, top + line), &state, style::DIM);
        let mut y = top + 2.0 * line;
        if let Some(cost) = cost(ctx, n.skill, lv) {
            style::text(ui, tx, style::baseline(ui, y), &cost, style::DIM);
            y += line;
        }
        let opens = if opens.is_empty() {
            crate::pt!("Opens nothing on this page", "Não libera nada nesta página").to_string()
        } else {
            crate::pt_format!("Opens {}", "Libera {}", opens.join(", "))
        };
        let rows = ((bottom - y) / line).floor().max(0.0) as usize;
        for row in style::wrap(ui, &opens, col_w - 16.0).iter().take(rows) {
            style::text(ui, area.x, style::baseline(ui, y), row, style::DIM);
            y += line;
        }

        // what it needs
        let x = area.x + col_w;
        let w = (area.w * 0.28).floor();
        let mut y = frame::section(ui, x, top, w - 16.0, crate::pt!("Needs", "Requer"));
        if n.requires.is_empty() {
            style::text(ui, x, style::baseline(ui, y), crate::pt!("Nothing", "Nada"), style::DIM);
        }
        let room = ((bottom - y) / line).floor().max(1.0) as usize;
        let shown = if n.requires.len() > room {
            room - 1
        } else {
            room
        };
        for &(need, need_lv) in n.requires.iter().take(shown) {
            let done = level(&c.skills, need) >= i16::from(need_lv);
            style::dot(ui, x, y + ((line - 6.0) / 2.0).floor(), 6.0, done);
            let label = format!("{} {need_lv}", format_skill_display_name(need, names));
            let label = style::fit(ui, &label, w - 32.0);
            style::text(ui, x + 14.0, style::baseline(ui, y), &label, style::TEXT);
            y += line;
        }
        if n.requires.len() > shown {
            let more = crate::pt_format!("and {} more", "e mais {}", n.requires.len() - shown);
            style::text(ui, x + 14.0, style::baseline(ui, y), &more, style::DIM);
        }

        // what it does
        let x = x + w;
        let w = area.x + area.w - x;
        let y = frame::section(ui, x, top, w, crate::pt!("About", "Sobre"));
        let skill = n.skill;
        let text = Rect::new(x, y, w, bottom - y);
        self.about
            .build(ui, u64::from(skill.id()), text, || about(ctx, skill));
    }
}

/// SP at the level learnt (the first level before then), and the reach once learnt
fn cost(ctx: &BuildCtx, skill: SkillEnum, lv: i16) -> Option<String> {
    let known = ctx.character.skills.get_skill(skill);
    let sp = ctx
        .data
        .skill_use_level
        .as_ref()
        .and_then(|t| t.sp_at_level(skill, lv.max(1)))
        .or(known.map(|s| s.sp_cost))
        .filter(|&sp| sp > 0);
    let range = known.map(|s| s.attack_range).filter(|&r| r > 1);
    match (sp, range) {
        (Some(sp), Some(r)) => Some(format!("SP {sp} · range {r}")),
        (Some(sp), None) => Some(format!("SP {sp}")),
        (None, Some(r)) => Some(crate::pt_format!("Range {r}", "Alcance {r}")),
        (None, None) => None,
    }
}

/// The skill's description without its name line
fn about(ctx: &BuildCtx, skill: SkillEnum) -> String {
    let name = format_skill_display_name(skill, ctx.data.skill_name.as_ref());
    ctx.data
        .skill_description
        .as_ref()
        .and_then(|t| t.get_description(skill))
        .map(|lines| {
            lines
                .iter()
                .map(|l| draw::strip_color_codes(l))
                .skip_while(|l| l.trim().is_empty() || l.trim() == name)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// A node: its icon, name and level, a "+" while a point can go in
fn node(
    ui: &mut UiFrame,
    ctx: &BuildCtx,
    n: &Node,
    r: Rect,
    ring: [f32; 4],
    alpha: f32,
    learnable: bool,
) {
    let fade = |c: [f32; 4]| [c[0], c[1], c[2], c[3] * alpha];
    let earlier = if n.earlier { 0.6 } else { 1.0 };
    style::push_white(ui, draw::rounded_rect(r.x, r.y, r.w, r.h, 6.0, fade(ring)));
    style::push_white(
        ui,
        draw::rounded_rect(
            r.x + 1.0,
            r.y + 1.0,
            r.w - 2.0,
            r.h - 2.0,
            5.0,
            fade(NODE_BG),
        ),
    );
    let pad = ((r.h - ICON) / 2.0).floor();
    let icon = Rect::new(r.x + pad, r.y + pad, ICON, ICON);
    frame::image(ui, icon, n.icon.clone(), [1.0, 1.0, 1.0, alpha * earlier]);
    let tx = r.x + pad + ICON + 8.0;
    let plus_w = if learnable { 18.0 } else { 0.0 };
    let max_w = r.x + r.w - tx - 6.0 - plus_w;
    let name = format_skill_display_name(n.skill, ctx.data.skill_name.as_ref());
    let name = style::fit_small(ui, name, max_w);
    let lv = level(&ctx.character.skills, n.skill);
    let text_color = fade(if n.earlier { style::DIM } else { style::TEXT });
    style::outlined(ui, tx, r.y + 17.0, &name, text_color);
    let lv_color = if lv >= i16::from(n.max) {
        style::GOOD
    } else {
        style::DIM
    };
    style::outlined(
        ui,
        tx,
        r.y + 33.0,
        &format!("{lv} / {}", n.max),
        fade(lv_color),
    );
    if learnable {
        style::outlined(ui, r.x + r.w - 6.0 - 10.0, r.y + 25.0, "+", style::ACCENT);
    }
}

/// A line from `a`'s right edge to `b`'s left, bent once just after `a`
fn link(ui: &mut UiFrame, a: Rect, b: Rect, color: [f32; 4], thick: f32) {
    let y1 = (a.y + a.h / 2.0).floor();
    let y2 = (b.y + b.h / 2.0).floor();
    let x1 = a.x + a.w;
    let xb = x1 + ELBOW;
    let mut seg = |x: f32, y: f32, w: f32, h: f32| {
        style::push_quad(ui, draw::quad_vertices(x, y, w, h, color));
    };
    if y1 == y2 {
        seg(x1, y1, b.x - x1, thick);
        return;
    }
    seg(x1, y1, xb - x1, thick);
    seg(xb, y1.min(y2), thick, (y2 - y1).abs() + thick);
    seg(xb, y2, b.x - xb, thick);
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNIGHT: u16 = 7;

    #[test]
    fn a_skill_sits_right_of_what_it_needs_and_earlier_skills_come_first() {
        let tree = skill_tree::tree(KNIGHT);
        let nodes = layout(&tree, KNIGHT);
        let find = |s| nodes.iter().find(|n| n.skill == s).unwrap();
        let bash = find(SkillEnum::SmBash);
        assert!(bash.earlier);
        assert_eq!(bash.col, 0);
        let pierce = find(SkillEnum::KnPierce);
        let mastery = find(SkillEnum::KnSpearmastery);
        assert_eq!(pierce.col, mastery.col + 1);
        let bowling = find(SkillEnum::KnBowlingbash);
        for (need, _) in bowling.requires {
            assert!(find(*need).col < bowling.col);
        }
        // no two nodes share a cell
        for (i, a) in nodes.iter().enumerate() {
            for b in &nodes[i + 1..] {
                assert!((a.col, a.row) != (b.col, b.row));
            }
        }
    }

    #[test]
    fn every_job_page_lays_out() {
        for job in [0, 1, 7, 4008, 4054, 4252] {
            let tree = skill_tree::tree(job);
            for page in skill_tree::pages(job, &tree) {
                assert!(!layout(&tree, page).is_empty(), "job {job} page {page}");
            }
        }
    }
}
