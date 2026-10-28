//! The frameless HUD's look (`docs/nostalre-hud.md`, "Look"): shadowed text, thin rounded bars, dark slots.
use nostalre_renderer::font_atlas::{hud_bold_char, hud_char};
use nostalre_renderer::ui_renderer::UiVertex;
use nostalre_ui::draw::{self, DrawCall, TextOutline, TextureRef};
use nostalre_ui::frame::UiFrame;
use nostalre_ui::rect::Rect;

const fn rgba(r: u8, g: u8, b: u8, a: f32) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a]
}

/// `color` at `alpha`
pub(crate) const fn faded(color: [f32; 4], alpha: f32) -> [f32; 4] {
    [color[0], color[1], color[2], alpha]
}

/// The HUD's dark blue at `alpha`: bar tracks, the chat's scrim, the menu's veil
pub(crate) const fn shade(alpha: f32) -> [f32; 4] {
    rgba(8, 12, 28, alpha)
}

pub const TEXT: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
pub const DIM: [f32; 4] = rgba(0xd3, 0xdb, 0xec, 1.0);
pub const ACCENT: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 1.0);
pub const WARN: [f32; 4] = rgba(0xff, 0xb0, 0x8a, 1.0);
pub const GOOD: [f32; 4] = rgba(0x8f, 0xe3, 0x9a, 1.0);
/// Text that is done with: a finished objective
pub const DIM_MORE: [f32; 4] = rgba(0xd3, 0xdb, 0xec, 0.6);
const RULE: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 0.35);
const OPEN_DOT: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 0.55);
const SHADOW: [f32; 4] = [0.0, 0.0, 0.0, 0.75];
const TRACK: [f32; 4] = shade(0.6);
const LIGHT_TRACK: [f32; 4] = rgba(0x3a, 0x46, 0x6e, 0.7);
const SLOT_BG: [f32; 4] = rgba(10, 16, 38, 0.55);
const SLOT_BORDER: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 0.22);
const SLOT_BORDER_HOVER: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 0.8);
const UNDERLINE: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 0.35);
const SCRIM: [f32; 4] = shade(0.55);
const BADGE_RING: [f32; 4] = rgba(0xd2, 0xea, 0xfc, 0.6);
const HEADING_SPACING: f32 = 2.0;
const BAR_RADIUS_MAX: f32 = 6.0;
const BAR_OUTLINE: [f32; 4] = [0.0, 0.0, 0.0, 0.35];
const GLOSS: [f32; 4] = [1.0, 1.0, 1.0, 0.35];
const TRAIL: [f32; 4] = rgba(0xff, 0xe0, 0xd8, 0.7);
const PILL_PAD: f32 = 6.0;
const SLOT_TOP: [f32; 4] = rgba(46, 58, 96, 0.6);
const SLOT_GLOSS: [f32; 4] = [1.0, 1.0, 1.0, 0.12];
const SLOT_RADIUS: f32 = 5.0;
const PICKED: [f32; 4] = rgba(130, 148, 200, 0.75);
/// The picked row's blue, opaque: the lines from a picked skill, the skills it needs
pub(crate) const PICKED_LINE: [f32; 4] = rgba(130, 148, 200, 1.0);
const KEY_RING: [f32; 4] = rgba(0x9a, 0xa6, 0xc8, 1.0);
/// A switch turned on: the accent, deeper at the bottom
pub(crate) const SWITCH_ON: Fill = Fill(rgba(0xee, 0xf7, 0xff, 1.0), rgba(0x9f, 0xc6, 0xe6, 1.0));

/// A bar's fill, top to bottom
#[derive(Clone, Copy)]
pub struct Fill(pub [f32; 4], pub [f32; 4]);

pub const HP: Fill = Fill(rgba(0x6e, 0xe8, 0x77, 1.0), rgba(0x1f, 0x9c, 0x2d, 1.0));
pub const SP: Fill = Fill(rgba(0x7d, 0xb0, 0xff, 1.0), rgba(0x1f, 0x57, 0xd1, 1.0));
pub const AP: Fill = Fill(rgba(0xff, 0xd6, 0x8a, 1.0), rgba(0xe0, 0x9a, 0x2c, 1.0));
pub const EXP: Fill = Fill(rgba(0xff, 0xe3, 0x83, 1.0), rgba(0xd2, 0x9a, 0x12, 1.0));
pub const JOB_EXP: Fill = Fill(rgba(0xb4, 0xe3, 0xff, 1.0), rgba(0x37, 0x96, 0xd6, 1.0));
pub const ENEMY_HP: Fill = Fill(rgba(0xff, 0x84, 0x84, 1.0), rgba(0xc3, 0x21, 0x2b, 1.0));
/// A pet's fullness
pub const HUNGER: Fill = Fill(rgba(0xff, 0xc4, 0x7a, 1.0), rgba(0xd9, 0x7a, 0x1c, 1.0));
pub const CAST: Fill = Fill(rgba(0xff, 0xf2, 0xb0, 1.0), rgba(0xe3, 0xb2, 0x3c, 1.0));

pub(crate) fn push_white(ui: &mut UiFrame, (vertices, indices): Mesh) {
    ui.draw_calls.push(DrawCall {
        vertices: vertices.into(),
        indices: indices.into(),
        texture: TextureRef::White,
    });
}

/// A quad of one colour, or shaded as `quad` was built; kept inline, unlike a [`Mesh`]
pub(crate) fn push_quad(ui: &mut UiFrame, (vertices, indices): ([UiVertex; 4], &'static [u32])) {
    ui.draw_calls.push(DrawCall {
        vertices: vertices.into(),
        indices: indices.into(),
        texture: TextureRef::White,
    });
}

/// A rect of one colour
pub(crate) fn fill(ui: &mut UiFrame, r: Rect, color: [f32; 4]) {
    push_quad(ui, draw::quad_vertices(r.x, r.y, r.w, r.h, color));
}

/// The HUD's text size (`HUD_SCALE`): its glyphs for `content`
fn hud(content: &str) -> String {
    content.chars().map(hud_char).collect()
}

/// The bold glyphs where the atlas has them (ASCII), at the HUD's size
fn bold(content: &str) -> String {
    content.chars().map(hud_bold_char).collect()
}

/// Text at the atlas's own size with the HUD's outline: the chat's lines, which wrap at that size
pub fn outlined(ui: &mut UiFrame, x: f32, baseline: f32, content: &str, color: [f32; 4]) {
    ui.text_with_outline(x, baseline, content, color, &TextOutline::cross(SHADOW));
}

/// Text at a baseline, at the HUD's size, with the soft outline that keeps it readable on any ground
pub fn text(ui: &mut UiFrame, x: f32, baseline: f32, content: &str, color: [f32; 4]) {
    outlined(ui, x, baseline, &hud(content), color);
}

pub fn text_bold(ui: &mut UiFrame, x: f32, baseline: f32, content: &str, color: [f32; 4]) {
    outlined(ui, x, baseline, &bold(content), color);
}

pub fn measure(ui: &UiFrame, content: &str) -> f32 {
    ui.atlas.measure_text(&hud(content))
}

pub fn measure_bold(ui: &UiFrame, content: &str) -> f32 {
    ui.atlas.measure_text(&bold(content))
}

/// The HUD's line height
pub fn line(ui: &UiFrame) -> f32 {
    ui.atlas.hud_line_height
}

/// `content` cut with "..." to fit `max_w` at the HUD's size
pub fn fit(ui: &UiFrame, content: &str, max_w: f32) -> String {
    fit_by(ui, content, max_w, hud_char)
}

/// [`fit`] at the atlas's own size, as small text (the skill tree's nodes) is drawn
pub fn fit_small(ui: &UiFrame, content: &str, max_w: f32) -> String {
    fit_by(ui, content, max_w, |c| c)
}

fn fit_by(ui: &UiFrame, content: &str, max_w: f32, glyph: fn(char) -> char) -> String {
    let advance = |c: char| ui.atlas.glyph(glyph(c)).advance;
    if content.chars().map(advance).sum::<f32>() <= max_w {
        return content.to_string();
    }
    let dots_w = advance('.') * 3.0;
    let mut out = String::new();
    let mut w = 0.0;
    for ch in content.chars() {
        let cw = advance(ch);
        if w + cw + dots_w > max_w {
            break;
        }
        out.push(ch);
        w += cw;
    }
    out.push_str("...");
    out
}

/// A heading: capitals spaced apart, in the accent colour. One draw call for the outline, one for the
/// letters, however long it is
pub fn heading(ui: &mut UiFrame, x: f32, baseline: f32, content: &str) {
    heading_in(ui, x, baseline, content, ACCENT);
}

/// [`heading`] in another colour
pub fn heading_in(ui: &mut UiFrame, x: f32, baseline: f32, content: &str, color: [f32; 4]) {
    let step = draw::outline_step(ui.atlas, 1.0);
    let mut outline: Mesh = (Vec::new(), Vec::new());
    let mut letters: Mesh = (Vec::new(), Vec::new());
    let mut cx = x;
    let mut buf = [0u8; 4];
    for ch in hud(&content.to_uppercase()).chars() {
        let glyph = ch.encode_utf8(&mut buf);
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            let mesh = draw::text_vertices(
                glyph,
                cx + dx * step,
                baseline + dy * step,
                SHADOW,
                ui.atlas,
            );
            append(&mut outline, mesh);
        }
        append(
            &mut letters,
            draw::text_vertices(glyph, cx, baseline, color, ui.atlas),
        );
        cx += ui.atlas.measure_text(glyph) + HEADING_SPACING;
    }
    for mesh in [outline, letters] {
        if !mesh.0.is_empty() {
            ui.draw_calls.push(DrawCall {
                vertices: mesh.0.into(),
                indices: mesh.1.into(),
                texture: TextureRef::FontAtlas,
            });
        }
    }
}

/// A heading with a thin rule running from it to `w`, as the menu's sections have
pub fn heading_rule(ui: &mut UiFrame, x: f32, top: f32, w: f32, content: &str) {
    heading(ui, x, baseline(ui, top), content);
    let start = x + heading_width(ui, content) + 10.0;
    let rule_y = (top + line(ui) * 0.5).floor();
    if start < x + w {
        push_quad(
            ui,
            draw::quad_vertices(start, rule_y, x + w - start, 1.0, RULE),
        );
    }
}

/// A rule that fades out to the right, as the menu's lines are
pub fn rule(ui: &mut UiFrame, x: f32, y: f32, w: f32) {
    let clear = [RULE[0], RULE[1], RULE[2], 0.0];
    push_quad(ui, draw::quad_vertices_hgrad(x, y, w, 1.0, RULE, clear));
}

/// The menu's picked row: a band that fades out to the right, a caret in front
pub fn highlight(ui: &mut UiFrame, r: Rect) {
    let clear = [PICKED[0], PICKED[1], PICKED[2], 0.0];
    push_quad(
        ui,
        draw::quad_vertices_hgrad(r.x, r.y, r.w, r.h, PICKED, clear),
    );
    let size = (r.h * 0.32).floor();
    let y = r.y + ((r.h - size) / 2.0).floor();
    caret(ui, r.x + 6.0, y, size, ACCENT);
}

/// A small triangle pointing right
pub fn caret(ui: &mut UiFrame, x: f32, y: f32, size: f32, color: [f32; 4]) {
    triangle(ui, x, y, size, color, false);
}

/// [`caret`] pointing left: a setting's earlier choice
pub fn caret_left(ui: &mut UiFrame, x: f32, y: f32, size: f32, color: [f32; 4]) {
    triangle(ui, x, y, size, color, true);
}

fn triangle(ui: &mut UiFrame, x: f32, y: f32, size: f32, color: [f32; 4], left: bool) {
    let w = (size * 0.8).round();
    let (back, tip) = if left { (x + w, x) } else { (x, x + w) };
    let corner = |position: [f32; 2]| UiVertex {
        position,
        tex_coord: [0.0, 0.0],
        color,
    };
    push_white(
        ui,
        (
            vec![
                corner([back, y]),
                corner([tip, y + size * 0.5]),
                corner([back, y + size]),
            ],
            vec![0, 1, 2],
        ),
    );
}

pub fn switch_width(h: f32) -> f32 {
    (h * 1.9).round()
}

/// An on/off switch `h` high: a knob in a rounded track, the track lit when on; returns its width
pub fn switch(ui: &mut UiFrame, x: f32, top: f32, h: f32, on: bool) -> f32 {
    let w = switch_width(h);
    let r = (h * 0.5).floor();
    let inset = 3.0;
    let knob = h - 2.0 * inset;
    if on {
        push_white(
            ui,
            draw::rounded_rect_vgrad(x, top, w, h, r, SWITCH_ON.0, SWITCH_ON.1),
        );
    } else {
        push_white(ui, draw::rounded_rect(x, top, w, h, r, KEY_RING));
        push_white(
            ui,
            draw::rounded_rect(x + 1.0, top + 1.0, w - 2.0, h - 2.0, r - 1.0, SLOT_BG),
        );
    }
    let (kx, color) = if on {
        (x + w - inset - knob, shade(1.0))
    } else {
        (x + inset, DIM)
    };
    push_white(
        ui,
        draw::rounded_rect(kx, top + inset, knob, knob, knob * 0.5, color),
    );
    w
}

/// A key's label in a rounded frame, as the menu's hints show them; returns its width
pub fn key_cap(ui: &mut UiFrame, x: f32, top: f32, label: &str) -> f32 {
    let h = line(ui) - 2.0;
    let w = key_cap_width(ui, label);
    let r = (h * 0.5).floor();
    push_white(ui, draw::rounded_rect(x, top + 1.0, w, h, r, KEY_RING));
    push_white(
        ui,
        draw::rounded_rect(x + 1.0, top + 2.0, w - 2.0, h - 2.0, r - 1.0, SLOT_BG),
    );
    let tx = x + ((w - measure_bold(ui, label)) / 2.0).floor();
    text_bold(ui, tx, baseline(ui, top), label, TEXT);
    w
}

pub fn key_cap_width(ui: &UiFrame, label: &str) -> f32 {
    (measure_bold(ui, label) + 2.0 * PILL_PAD).max(line(ui) - 2.0)
}

/// `content` broken into lines that fit `max_w` at the HUD's size
pub fn wrap(ui: &UiFrame, content: &str, max_w: f32) -> Vec<String> {
    draw::word_wrap(content, max_w, |t| measure(ui, t), false)
}

/// A round marker: open, or done in green
pub fn dot(ui: &mut UiFrame, x: f32, y: f32, size: f32, done: bool) {
    let color = if done { GOOD } else { OPEN_DOT };
    push_white(ui, draw::rounded_rect(x, y, size, size, size * 0.5, color));
}

pub fn heading_width(ui: &UiFrame, content: &str) -> f32 {
    let caps = content.to_uppercase();
    let gaps = caps.chars().count().saturating_sub(1) as f32;
    measure(ui, &caps) + gaps * HEADING_SPACING
}

pub(crate) type Mesh = (Vec<UiVertex>, Vec<u32>);

fn append(into: &mut Mesh, (vertices, indices): Mesh) {
    let base = into.0.len() as u32;
    into.0.extend(vertices);
    into.1.extend(indices.into_iter().map(|i| i + base));
}

/// A round badge with a short label: a job, a companion kind
pub fn badge(ui: &mut UiFrame, x: f32, y: f32, size: f32, label: &str) {
    let r = size * 0.5;
    push_white(ui, draw::rounded_rect(x, y, size, size, r, BADGE_RING));
    push_white(
        ui,
        draw::rounded_rect(x + 1.0, y + 1.0, size - 2.0, size - 2.0, r - 1.0, SLOT_BG),
    );
    let w = measure(ui, label);
    let baseline = y + ((size - line(ui)) / 2.0).floor() + ui.atlas.hud_ascent;
    text(ui, (x + (size - w) / 2.0).floor(), baseline, label, ACCENT);
}

/// A badge's ring and fill, stretched to a pill: a button
pub fn pill_button(ui: &mut UiFrame, r: Rect) {
    let radius = r.h * 0.5;
    push_white(
        ui,
        draw::rounded_rect(r.x, r.y, r.w, r.h, radius, BADGE_RING),
    );
    push_white(
        ui,
        draw::rounded_rect(
            r.x + 1.0,
            r.y + 1.0,
            r.w - 2.0,
            r.h - 2.0,
            radius - 1.0,
            SLOT_BG,
        ),
    );
}

pub fn text_right(ui: &mut UiFrame, right: f32, baseline: f32, content: &str, color: [f32; 4]) {
    let w = measure(ui, content);
    text(ui, right - w, baseline, content, color);
}

pub fn text_centered(ui: &mut UiFrame, centre: f32, baseline: f32, content: &str, color: [f32; 4]) {
    let w = measure(ui, content);
    text(ui, (centre - w * 0.5).floor(), baseline, content, color);
}

/// Baseline of a line whose box starts at `top`
pub fn baseline(ui: &UiFrame, top: f32) -> f32 {
    top + ui.atlas.hud_ascent
}

/// A thin rounded bar, `ratio` of it filled
pub fn bar(ui: &mut UiFrame, r: Rect, ratio: f32, fill: Fill) {
    bar_on(ui, r, ratio, fill, TRACK);
}

/// [`bar`] on a lighter track, for a bar that is mostly empty (EXP)
pub fn bar_light(ui: &mut UiFrame, r: Rect, ratio: f32, fill: Fill) {
    bar_on(ui, r, ratio, fill, LIGHT_TRACK);
}

fn bar_on(ui: &mut UiFrame, r: Rect, ratio: f32, fill: Fill, track: [f32; 4]) {
    bar_with_trail(ui, r, ratio, None, fill, track);
}

/// [`bar`] with what was just lost still showing, pale, behind the fill (see [`Trail`])
pub fn bar_trailed(ui: &mut UiFrame, r: Rect, ratio: f32, trail: f32, fill: Fill) {
    bar_with_trail(ui, r, ratio, Some(trail), fill, TRACK);
}

fn bar_with_trail(
    ui: &mut UiFrame,
    r: Rect,
    ratio: f32,
    trail: Option<f32>,
    fill: Fill,
    track: [f32; 4],
) {
    let radius = (r.h * 0.5).min(BAR_RADIUS_MAX);
    push_white(
        ui,
        draw::rounded_rect(
            r.x - 1.0,
            r.y - 1.0,
            r.w + 2.0,
            r.h + 2.0,
            radius + 1.0,
            BAR_OUTLINE,
        ),
    );
    push_white(ui, draw::rounded_rect(r.x, r.y, r.w, r.h, radius, track));
    if let Some(trail) = trail.filter(|&t| t > ratio) {
        let w = (r.w * trail.clamp(0.0, 1.0)).round();
        push_white(ui, draw::rounded_rect(r.x, r.y, w, r.h, radius, TRAIL));
    }
    let w = (r.w * ratio.clamp(0.0, 1.0)).round();
    if w > 0.0 {
        push_white(
            ui,
            draw::rounded_rect_vgrad(r.x, r.y, w, r.h, radius, fill.0, fill.1),
        );
        if r.h >= 6.0 && w > 2.0 * radius {
            push_quad(
                ui,
                draw::quad_vertices(r.x + radius, r.y + 1.0, w - 2.0 * radius, 1.0, GLOSS),
            );
        }
    }
}

/// A short label in a pill tinted with its bar's colour: HP, SP
pub fn pill(ui: &mut UiFrame, x: f32, top: f32, content: &str, fill: Fill) {
    let h = line(ui) - 2.0;
    let w = measure_bold(ui, content) + 2.0 * PILL_PAD;
    let tint = [fill.1[0], fill.1[1], fill.1[2], 0.45];
    push_white(ui, draw::rounded_rect(x, top + 1.0, w, h, h * 0.5, tint));
    text_bold(ui, x + PILL_PAD, baseline(ui, top), content, fill.0);
}

pub fn pill_width(ui: &UiFrame, content: &str) -> f32 {
    measure_bold(ui, content) + 2.0 * PILL_PAD
}

/// A skill or item slot: a dark, see-through button lit a little from above, a faint border that
/// lights up on hover
pub fn slot(ui: &mut UiFrame, r: Rect, hovered: bool) {
    let border = if hovered {
        SLOT_BORDER_HOVER
    } else {
        SLOT_BORDER
    };
    push_white(
        ui,
        draw::rounded_rect(r.x, r.y, r.w, r.h, SLOT_RADIUS, border),
    );
    push_white(
        ui,
        draw::rounded_rect_vgrad(
            r.x + 1.0,
            r.y + 1.0,
            r.w - 2.0,
            r.h - 2.0,
            SLOT_RADIUS - 1.0,
            SLOT_TOP,
            SLOT_BG,
        ),
    );
    push_quad(
        ui,
        draw::quad_vertices(
            r.x + SLOT_RADIUS,
            r.y + 1.0,
            r.w - 2.0 * SLOT_RADIUS,
            1.0,
            SLOT_GLOSS,
        ),
    );
}

/// The pale part of a bar that shows what was just lost: it holds a moment, then drains to the value
#[derive(Debug, Default, Clone, Copy)]
pub struct Trail {
    shown: f32,
    last: f32,
    hold_until: f32,
    at: f32,
    key: u32,
}

impl Trail {
    const HOLD_SECS: f32 = 0.5;
    const DRAIN_PER_SEC: f32 = 0.8;

    /// The trail for `value` at `now` (seconds); a new `key` (another unit) starts it afresh
    pub fn update(&mut self, key: u32, value: f32, now: f32) -> f32 {
        if key != self.key {
            *self = Self {
                shown: value,
                last: value,
                hold_until: now,
                at: now,
                key,
            };
        }
        let dt = (now - self.at).max(0.0);
        self.at = now;
        if value < self.last {
            self.hold_until = now + Self::HOLD_SECS;
        }
        self.last = value;
        if value >= self.shown {
            self.shown = value;
        } else if now >= self.hold_until {
            self.shown = (self.shown - Self::DRAIN_PER_SEC * dt).max(value);
        }
        self.shown
    }
}

/// A field's underline: the frameless chat's input
pub fn underline(ui: &mut UiFrame, r: Rect) {
    push_quad(
        ui,
        draw::quad_vertices(r.x, r.y + r.h - 1.0, r.w, 1.0, UNDERLINE),
    );
}

/// An accent line under a picked tab
pub fn underline_accent(ui: &mut UiFrame, r: Rect) {
    push_quad(
        ui,
        draw::quad_vertices(r.x, r.y + r.h - 2.0, r.w, 2.0, ACCENT),
    );
}

/// The soft dark behind the chat: from its bottom left corner, fading out
pub fn corner_scrim(ui: &mut UiFrame, r: Rect) {
    let (cx, cy) = (r.x, r.y + r.h);
    push_white(
        ui,
        draw::radial_gradient(
            cx,
            cy,
            r.w * 1.15,
            r.h * 1.35,
            SCRIM,
            [SCRIM[0], SCRIM[1], SCRIM[2], 0.0],
        ),
    );
}

/// HP as a percentage label: rounded up, so a unit still alive never reads 0%
pub fn percent(ratio: f32) -> String {
    format!("{:.0}%", (ratio * 100.0).ceil())
}

/// A whole-number ratio, 0 when there is no maximum
pub fn ratio(cur: u32, max: u32) -> f32 {
    if max == 0 {
        0.0
    } else {
        cur as f32 / max as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_trail_holds_what_was_lost_then_drains_and_a_new_unit_starts_afresh() {
        let mut trail = Trail::default();
        assert_eq!(trail.update(1, 1.0, 0.0), 1.0);
        // hit to half: the trail stays at full while it holds
        assert_eq!(trail.update(1, 0.5, 0.1), 1.0);
        assert_eq!(trail.update(1, 0.5, 0.5), 1.0);
        // then drains, never under the value
        assert!((trail.update(1, 0.5, 0.75) - 0.8).abs() < 1e-4);
        assert_eq!(trail.update(1, 0.5, 2.0), 0.5);
        // a heal moves it up with the value
        assert_eq!(trail.update(1, 0.9, 2.1), 0.9);
        // another unit: no trail from the last one
        assert_eq!(trail.update(2, 0.3, 2.2), 0.3);
    }
}
