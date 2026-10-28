//! The frame every menu screen shares: the veil over the world, the top line, the commands on the right,
//! the help line and key hints at the bottom, and the pieces screens build with (tabs, rows, keys).
use super::{CMD_BASE_ID, CRUMB_ID, HINT_BASE_ID, ROW_BASE_ID, TAB_BASE_ID};
use crate::hud::style;
use nostalre_ui::context::PhysicalKeyCode as KeyCode;
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{Response, UiFrame, WidgetId};
use nostalre_ui::rect::Rect;
use std::collections::HashSet;

/// The world's shade under the menu: the HUD's dark blue, nearly opaque
const VEIL: [f32; 4] = style::shade(0.84);
const TOP: f32 = 28.0;
const CMD_W: f32 = 300.0;
const CMD_GAP: f32 = 48.0;
const MAX_CMD_ROW: f32 = 34.0;
const SECTION_GAP: f32 = 20.0;
/// The text of a row, from its left: room for the caret
pub(super) const ROW_TEXT: f32 = 24.0;
/// A list row's height, the tabs' strip, an icon in a row, the gap between a screen's columns
pub(super) const ROW_H: f32 = 36.0;
pub(super) const TABS_H: f32 = 44.0;
pub(super) const ICON: f32 = 24.0;
pub(super) const COLUMN_GAP: f32 = 36.0;
/// An item not appraised yet, greyed
pub(super) const NOT_APPRAISED: [f32; 4] = [0.67, 0.67, 0.67, 1.0];
/// A handheld (Android): Escape is its Back button
pub(crate) const HANDHELD: bool = cfg!(any(target_os = "android", target_os = "horizon"));
const HINT_GAP: f32 = 18.0;
const CAP_GAP: f32 = 6.0;
const TAB_GAP: f32 = 26.0;

/// Where the frame's parts go on this screen
pub(super) struct Layout {
    margin: f32,
    rule_y: f32,
    cmd: Rect,
    footer_y: f32,
    /// What a screen may fill: left of the commands, between the top line and the footer
    pub content: Rect,
}

impl Layout {
    pub fn new(ui: &UiFrame) -> Self {
        let (w, h) = (ui.ctx.screen_width, ui.ctx.screen_height);
        let line = style::line(ui);
        let margin = (w * 0.05).clamp(16.0, 64.0).floor();
        let rule_y = TOP + line + 6.0;
        let footer_y = h - TOP - line - 14.0;
        let top = rule_y + SECTION_GAP;
        let cmd = Rect::new(w - margin - CMD_W, top, CMD_W, footer_y - SECTION_GAP - top);
        let content = Rect::new(margin, top, (cmd.x - CMD_GAP - margin).max(0.0), cmd.h);
        Self {
            margin,
            rule_y,
            cmd,
            footer_y,
            content,
        }
    }
}

pub(super) fn veil(ui: &mut UiFrame) {
    let screen = Rect::new(0.0, 0.0, ui.ctx.screen_width, ui.ctx.screen_height);
    style::fill(ui, screen, VEIL);
}

/// "MENU", the screen's name after it, `info` on the right; true when "MENU" was clicked to go back
pub(super) fn top_line(ui: &mut UiFrame, l: &Layout, crumb: Option<&str>, info: &str) -> bool {
    let baseline = style::baseline(ui, TOP);
    let mut back = false;
    match crumb {
        Some(name) => {
            let menu_w = style::heading_width(ui, "Menu");
            let rect = Rect::new(l.margin, TOP, menu_w, style::line(ui));
            let resp = ui.interact(CRUMB_ID, rect);
            let color = if resp.hovered() {
                style::TEXT
            } else {
                style::DIM
            };
            style::heading_in(ui, l.margin, baseline, "Menu", color);
            let sep_x = l.margin + menu_w + 10.0;
            style::text(ui, sep_x, baseline, "»", style::DIM);
            style::heading(ui, sep_x + style::measure(ui, "»") + 10.0, baseline, name);
            back = resp.clicked();
        }
        None => style::heading(ui, l.margin, baseline, "Menu"),
    }
    let right = ui.ctx.screen_width - l.margin;
    style::text_right(ui, right, baseline, info, style::DIM);
    style::rule(ui, l.margin, l.rule_y, right - l.margin);
    back
}

/// The command column: `labels` (a name and its key), a gap before `first_system`; `picked` highlighted,
/// bold while its screen is open. Returns the one clicked
pub(super) fn commands(
    ui: &mut UiFrame,
    l: &Layout,
    labels: &[(&str, Option<&str>)],
    first_system: usize,
    picked: usize,
    screen_open: bool,
) -> Option<usize> {
    let line = style::line(ui);
    let gap = SECTION_GAP;
    let row_h = ((l.cmd.h - gap) / labels.len() as f32)
        .min(MAX_CMD_ROW)
        .floor();
    let mut y = l.cmd.y;
    let mut clicked = None;
    for (i, (label, key)) in labels.iter().enumerate() {
        if i == first_system {
            style::rule(ui, l.cmd.x, y + (gap * 0.5).floor(), l.cmd.w);
            y += gap;
        }
        let rect = Rect::new(l.cmd.x, y, l.cmd.w, row_h);
        let resp = ui.interact(WidgetId(CMD_BASE_ID + i as u32), rect);
        if i == picked {
            style::highlight(ui, rect);
        }
        let baseline = style::baseline(ui, y + ((row_h - line) / 2.0).floor());
        let x = rect.x + ROW_TEXT;
        if i == picked && screen_open {
            style::text_bold(ui, x, baseline, label, style::TEXT);
        } else {
            let color = if i == picked || resp.hovered() {
                style::TEXT
            } else {
                style::DIM
            };
            style::text(ui, x, baseline, label, color);
        }
        if let Some(key) = key {
            let w = ui.atlas.measure_text(key);
            style::outlined(ui, rect.x + rect.w - 8.0 - w, baseline, key, style::DIM);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        y += row_h;
    }
    clicked
}

/// The keys a screen reads, or the same press from a hint tapped on a touch screen
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Keys {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub enter: bool,
    pub back: bool,
    /// Delete or Backspace: take off, drop, the second action
    pub remove: bool,
    /// S: onto the skill bar
    pub to_bar: bool,
    pub next_tab: bool,
    pub prev_tab: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Key {
    Enter,
    Back,
    Remove,
    ToBar,
}

impl Keys {
    /// The menu owns the keyboard while it is open, so it reads the keys as they came
    pub fn read(ui: &UiFrame) -> Self {
        let c = &ui.ctx;
        let pressed = |code| c.pressed_codes.contains(&code);
        Self {
            up: c.key_up,
            down: c.key_down,
            left: c.key_left,
            right: c.key_right,
            enter: c.key_enter,
            back: false,
            remove: c.key_delete || c.key_backspace,
            // not Alt+S, the skill tree's window key
            to_bar: pressed(KeyCode::KeyS) && !c.alt_pressed && !c.ctrl_pressed,
            next_tab: pressed(KeyCode::PageDown) || (c.key_tab && !c.shift_pressed),
            prev_tab: pressed(KeyCode::PageUp) || (c.key_tab && c.shift_pressed),
        }
    }

    pub fn press(&mut self, key: Key) {
        match key {
            Key::Enter => self.enter = true,
            Key::Back => self.back = true,
            Key::Remove => self.remove = true,
            Key::ToBar => self.to_bar = true,
        }
    }
}

/// A key and what it does, at the bottom right; one with a `key` can be tapped
#[derive(Debug, Clone, Copy)]
pub(super) struct Hint {
    pub cap: &'static str,
    /// The controller's name for a hint that is not a key: the D-pad, the shoulders
    pad: &'static str,
    pub label: &'static str,
    pub key: Option<Key>,
}

impl Hint {
    pub const MOVE: Hint = Hint::shown(crate::pt!("Arrows", "Setas"), "D-pad", crate::pt!("choose", "escolher"));
    pub const TABS: Hint = Hint::shown("PgUp PgDn", "L1 R1", crate::pt!("tab", "aba"));
    pub const MOVE_CHANGE: Hint = Hint::shown(crate::pt!("Arrows", "Setas"), "D-pad", crate::pt!("choose, change", "escolher, alterar"));
    pub const PAGES: Hint = Hint::shown("PgUp PgDn", "L1 R1", crate::pt!("page", "página"));
    pub const OPEN: Hint = Hint::tap("Enter", crate::pt!("open", "abrir"), Key::Enter);
    pub const CLOSE: Hint = Hint::tap("Esc", crate::pt!("close", "fechar"), Key::Back);
    pub const BACK: Hint = Hint::tap("Esc", crate::pt!("back", "voltar"), Key::Back);
    pub const ENTER_YES: Hint = Hint::tap("Enter", crate::pt!("yes", "sim"), Key::Enter);
    pub const ESC_NO: Hint = Hint::tap("Esc", "no", Key::Back);
    pub const TO_BAR: Hint = Hint::tap("S", crate::pt!("to bar", "para a barra"), Key::ToBar);
    pub const CANCEL: Hint = Hint::tap("Esc", crate::pt!("cancel", "cancelar"), Key::Back);

    const fn shown(cap: &'static str, pad: &'static str, label: &'static str) -> Self {
        Self {
            cap,
            pad,
            label,
            key: None,
        }
    }

    pub const fn tap(cap: &'static str, label: &'static str, key: Key) -> Self {
        Self {
            cap,
            pad: "",
            label,
            key: Some(key),
        }
    }

    /// The cap as the player's hands know it: the controller's button after a pad press (`gamepad.rs`
    /// `menu_press`), Android's Back for Escape
    fn cap(&self, ui: &UiFrame) -> &'static str {
        match (ui.ctx.pad_labels, self.key) {
            (true, Some(Key::Enter)) => "A",
            (true, Some(Key::Back)) => "B",
            (true, Some(Key::Remove)) => "Y",
            (true, Some(Key::ToBar)) => "X",
            (true, None) => self.pad,
            (false, Some(Key::Back)) => back_key(),
            _ => self.cap,
        }
    }
}

/// What the device calls Escape
fn back_key() -> &'static str {
    if HANDHELD { crate::pt!("Back", "Voltar") } else { "Esc" }
}

/// The help line and the hints under the last rule; returns the key of a hint tapped
pub(super) fn footer(ui: &mut UiFrame, l: &Layout, help: &str, hints: &[Hint]) -> Option<Key> {
    let right = ui.ctx.screen_width - l.margin;
    style::rule(ui, l.margin, l.footer_y, right - l.margin);
    let top = l.footer_y + 14.0;
    let baseline = style::baseline(ui, top);
    let mut x = right;
    let mut tapped = None;
    for (i, hint) in hints.iter().enumerate().rev() {
        let label_w = style::measure(ui, hint.label);
        let cap = hint.cap(ui);
        let cap_w = style::key_cap_width(ui, cap);
        let w = cap_w + CAP_GAP + label_w;
        x -= w;
        style::key_cap(ui, x, top, cap);
        style::text(ui, x + cap_w + CAP_GAP, baseline, hint.label, style::DIM);
        if let Some(key) = hint.key {
            let rect = Rect::new(x, top, w, style::line(ui));
            if ui
                .interact(WidgetId(HINT_BASE_ID + i as u32), rect)
                .clicked()
            {
                tapped = Some(key);
            }
        }
        x -= HINT_GAP;
    }
    let help = style::fit(ui, help, (x - l.margin - HINT_GAP).max(0.0));
    style::text(ui, l.margin, baseline, &help, style::TEXT);
    tapped
}

/// A row of tabs at `top`, `active` underlined; returns the one clicked
pub(super) fn tabs(
    ui: &mut UiFrame,
    x: f32,
    top: f32,
    labels: &[&str],
    active: usize,
) -> Option<usize> {
    let line = style::line(ui);
    let baseline = style::baseline(ui, top);
    let mut tx = x;
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let w = style::measure_bold(ui, label);
        let rect = Rect::new(tx, top, w, line + 6.0);
        let resp = ui.interact(WidgetId(TAB_BASE_ID + i as u32), rect);
        if i == active {
            style::text_bold(ui, tx, baseline, label, style::TEXT);
            style::underline_accent(ui, Rect::new(tx, top, w, line + 5.0));
        } else {
            let color = if resp.hovered() {
                style::TEXT
            } else {
                style::DIM
            };
            style::text(ui, tx, baseline, label, color);
        }
        if resp.clicked() {
            clicked = Some(i);
        }
        tx += w + TAB_GAP;
    }
    clicked
}

/// The tab after or before `active` for the tab keys, else the one clicked
pub(super) fn switch_tab(
    keys: &Keys,
    active: usize,
    count: usize,
    clicked: Option<usize>,
) -> usize {
    if count == 0 {
        return 0;
    }
    if let Some(i) = clicked {
        i
    } else if keys.next_tab {
        (active + 1) % count
    } else if keys.prev_tab {
        (active + count - 1) % count
    } else {
        active
    }
}

/// A list row, highlighted when picked
pub(super) fn row(ui: &mut UiFrame, id: WidgetId, rect: Rect, picked: bool) -> Response {
    let resp = ui.interact(id, rect);
    if picked {
        style::highlight(ui, rect);
    }
    resp
}

/// The baseline of one line of text in the middle of `rect`
pub(super) fn row_baseline(ui: &UiFrame, rect: Rect) -> f32 {
    style::baseline(ui, rect.y + ((rect.h - style::line(ui)) / 2.0).floor())
}

/// A list of names from `area`'s top, `w` wide, the arrows moving `picked`; the one Enter or a tap opens
pub(super) fn list(
    ui: &mut UiFrame,
    area: Rect,
    w: f32,
    names: &[&str],
    picked: &mut usize,
    keys: &Keys,
) -> Option<usize> {
    *picked = step(keys, *picked, names.len());
    let mut open = keys.enter.then_some(*picked);
    for (i, name) in names.iter().enumerate() {
        let rect = Rect::new(area.x, area.y + i as f32 * ROW_H, w.min(area.w), ROW_H);
        let resp = row(ui, WidgetId(ROW_BASE_ID + i as u32), rect, i == *picked);
        let baseline = row_baseline(ui, rect);
        style::text(ui, rect.x + ROW_TEXT, baseline, name, style::TEXT);
        if resp.clicked() {
            *picked = i;
            open = Some(i);
        }
    }
    open
}

/// A texture drawn over `r`, tinted
pub(super) fn image(ui: &mut UiFrame, r: Rect, path: String, tint: [f32; 4]) {
    let (v, i) = draw::quad_vertices(r.x, r.y, r.w, r.h, tint);
    ui.draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: i.into(),
        texture: TextureRef::Named(path.into()),
    });
}

/// The wheel over `area`: 1 a row down, -1 up, 0 for none
fn wheel_step(ui: &mut UiFrame, area: Rect) -> isize {
    let wheel = ui.take_scroll(area);
    if wheel > 0.0 {
        -1
    } else if wheel < 0.0 {
        1
    } else {
        0
    }
}

/// A block of wrapped text the wheel scrolls; wrapped once per text (`key`) and width, not every frame
#[derive(Default)]
pub(super) struct ScrolledText {
    key: Option<(u64, u32)>,
    rows: Vec<String>,
    offset: usize,
}

impl ScrolledText {
    /// Draws the rows of `text` that fit `area`, the last one cut with "..." while more follow; `text` is only
    /// built when `key` or the width changes
    pub fn build(&mut self, ui: &mut UiFrame, key: u64, area: Rect, text: impl FnOnce() -> String) {
        let width = area.w.round() as u32;
        if self.key != Some((key, width)) {
            self.key = Some((key, width));
            self.rows = style::wrap(ui, &text(), area.w);
            self.offset = 0;
        }
        let line = style::line(ui);
        let visible = (area.h / line).floor().max(0.0) as usize;
        let max = self.rows.len().saturating_sub(visible);
        let moved = self.offset as isize + wheel_step(ui, area);
        self.offset = (moved.max(0) as usize).min(max);
        let more = self.offset < max;
        for (i, row) in self.rows.iter().skip(self.offset).take(visible).enumerate() {
            let baseline = style::baseline(ui, area.y + i as f32 * line);
            if more && i + 1 == visible {
                let cut = format!("{}...", row.trim_end());
                style::text(ui, area.x, baseline, &cut, style::DIM);
            } else {
                style::text(ui, area.x, baseline, row, style::DIM);
            }
        }
    }
}

/// The first row a list of `len` shows, `visible` at a time: the wheel over `area` moves it, and after a key
/// moved `picked` (`follow`) it keeps that row in sight
pub(super) fn scroll(
    ui: &mut UiFrame,
    area: Rect,
    offset: &mut usize,
    picked: usize,
    len: usize,
    visible: usize,
    follow: bool,
) {
    let max = len.saturating_sub(visible);
    *offset = (*offset as isize + wheel_step(ui, area)).max(0) as usize;
    if follow && picked < *offset {
        *offset = picked;
    } else if follow && visible > 0 && picked >= *offset + visible {
        *offset = picked + 1 - visible;
    }
    *offset = (*offset).min(max);
}

/// The picked index after the up and down keys, wrapping
pub(super) fn step(keys: &Keys, picked: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    let picked = picked.min(len - 1);
    if keys.up {
        picked.checked_sub(1).unwrap_or(len - 1)
    } else if keys.down {
        (picked + 1) % len
    } else {
        picked
    }
}

/// Art a screen draws that may not be loaded yet (an item's picture): asked for once, loaded by the client
#[derive(Default)]
pub(super) struct Textures {
    asked: HashSet<String>,
    wanted: Vec<String>,
}

impl Textures {
    pub fn want(&mut self, path: &str) {
        if !self.asked.contains(path) {
            self.asked.insert(path.to_string());
            self.wanted.push(path.to_string());
        }
    }

    pub fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.wanted)
    }
}

/// A section heading with its rule, `w` wide; returns the y under it
pub(super) fn section(ui: &mut UiFrame, x: f32, top: f32, w: f32, heading: &str) -> f32 {
    style::heading_rule(ui, x, top, w, heading);
    top + style::line(ui) + 8.0
}
