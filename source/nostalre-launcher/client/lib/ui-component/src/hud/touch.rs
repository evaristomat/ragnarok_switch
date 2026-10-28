//! The on-screen controls of a touch screen (the design canvas, "Touch HUD" and "Camera on controller and
//! touch"): a move stick bottom left with the camera's turn buttons over it, Attack bottom right with four
//! skills in an arc, three more slots beside them, the hotbar set; zoom and reset beside the minimap. Over the
//! arc, while an NPC is in reach, a Talk button with its name.
//!
//! Several fingers work at once. A finger that lands on a control (and not on a window drawn over it) is the
//! controls' until it lifts: the client never makes it the mouse (`touch_zones`), and only such fingers press
//! the controls, so a finger walking the world does not press what it slides over.
use super::style;
use nostalre_game::hotkey::{HotkeySlotContent, PAD_SET_SLOTS, PAD_SETS};
use nostalre_ui::draw::{self, DrawCall, TextureRef};
use nostalre_ui::frame::{UiFrame, WidgetId};
use nostalre_ui::rect::Rect;

const STICK_ID: WidgetId = WidgetId(7010);
const TURN_LEFT_ID: WidgetId = WidgetId(7011);
const TURN_RIGHT_ID: WidgetId = WidgetId(7012);
const ZOOM_IN_ID: WidgetId = WidgetId(7013);
const ZOOM_OUT_ID: WidgetId = WidgetId(7014);
const RESET_ID: WidgetId = WidgetId(7015);
const ATTACK_ID: WidgetId = WidgetId(7016);
const SET_ID: WidgetId = WidgetId(7017);
const TALK_ID: WidgetId = WidgetId(7018);
/// The slot buttons: the arc's four, then the three beside it, 7020 to 7026
const SLOT_BASE_ID: u32 = 7020;

/// The mouse's id among the fingers: a desktop trying the controls
const MOUSE: u64 = u64::MAX;
/// The most fingers the controls follow at once
const MAX_POINTERS: usize = 10;
// Sized for a thumb on a handheld (a 5.5" 1080p screen at Android's 150%: one unit about 0.1 mm), the stick
// about 20 mm across and in from the corner, where the thumb rests rather than at the screen's edge
const MARGIN: f32 = 24.0;
const STICK_MARGIN: f32 = 48.0;
const STICK_R: f32 = 96.0;
const KNOB_R: f32 = 36.0;
const STICK_BOTTOM: f32 = 52.0;
const TURN_SIZE: f32 = 54.0;
const SMALL_SIZE: f32 = 44.0;
const GAP: f32 = 10.0;
const ATTACK_SIZE: f32 = 108.0;
/// Clear of the wallet, bottom right
const ATTACK_BOTTOM: f32 = 104.0;
/// From the screen's right edge to Attack's
const ATTACK_RIGHT: f32 = MARGIN + 20.0;
const SKILL_SIZE: f32 = 66.0;
const ARC_R: f32 = 126.0;
/// The arc's skills, degrees round from the left of Attack to above it
const ARC_DEG: [f32; 4] = [190.0, 155.0, 120.0, 85.0];
const SET_DEG: f32 = 50.0;
const SET_R: f32 = 112.0;
const SET_SIZE: f32 = 42.0;
const SET_LABELS: [&str; PAD_SETS as usize] = ["1/3", "2/3", "3/3"];
const ITEM_SIZE: f32 = 56.0;
const ICON: f32 = 38.0;
const TALK_W: f32 = 170.0;
const TALK_H: f32 = 46.0;
/// Room left and right of the NPC's name in the Talk button
const TALK_PAD: f32 = 16.0;
const HELD: [f32; 4] = style::faded(style::ACCENT, 0.35);
const STICK_RING: [f32; 4] = style::faded(style::ACCENT, 0.4);
const STICK_BASE: [f32; 4] = style::shade(0.3);
/// The arc's four and the three beside it
pub const SLOT_BUTTONS: usize = 7;

/// The room the stick and the turn buttons over it take at the bottom left: the chat goes above
pub const LEFT_COLUMN_H: f32 = STICK_BOTTOM + 2.0 * STICK_R + GAP + TURN_SIZE + GAP;

/// A hotbar slot as an on-screen button shows it
#[derive(Debug, Clone, PartialEq)]
pub struct SlotFace {
    pub icon: String,
    /// An item's count, a skill's level
    pub count: Option<String>,
    /// What it was made from: it is made again only when these change
    pub content: HotkeySlotContent,
    pub amount: i32,
}

/// The slots the controls show, set by the client while they are on screen
#[derive(Debug, Clone, Default)]
pub struct TouchSlots {
    /// The hotbar set: the buttons show its first seven slots
    pub set: u8,
    pub faces: [Option<SlotFace>; SLOT_BUTTONS],
    /// The NPC a tap on Talk talks to, by name ("" when it has none yet); no Talk button without one
    pub talk: Option<String>,
}

impl TouchSlots {
    /// The hotbar slot (0 to 35) of the `i`th button
    pub fn slot(&self, i: usize) -> usize {
        PAD_SET_SLOTS * self.set as usize + i
    }
}

/// What the on-screen controls ask this frame
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TouchInput {
    /// The move stick, -1 to 1 each way, y down
    pub stick: (f32, f32),
    /// -1 left, 1 right, while held
    pub turn: f32,
    /// -1 in, 1 out, while held
    pub zoom: f32,
    pub reset: bool,
    pub attack: bool,
    /// The `i`th slot button tapped (see [`TouchSlots::slot`])
    pub slot: Option<usize>,
    pub next_set: bool,
    pub talk: bool,
}

/// A finger the controls own, or the mouse while its button is down
#[derive(Clone, Copy, Default)]
struct Pointer {
    id: u64,
    x: f32,
    y: f32,
    landed: bool,
}

/// Where each control goes on this screen
struct Layout {
    stick: (f32, f32),
    turn_left: Rect,
    turn_right: Rect,
    zoom_in: Rect,
    zoom_out: Rect,
    reset: Rect,
    attack: Rect,
    set: Rect,
    slots: [Rect; SLOT_BUTTONS],
    /// While an NPC is in reach
    talk: Option<Rect>,
}

impl Layout {
    /// `minimap`: where the HUD drew it; `dock`: the skill rows' rectangle, which the slot row keeps clear of;
    /// `talk`: an NPC is in reach
    fn new(w: f32, h: f32, minimap: Rect, dock: Rect, talk: bool) -> Self {
        let stick = (STICK_MARGIN + STICK_R, h - STICK_BOTTOM - STICK_R);
        let turn_top = stick.1 - STICK_R - GAP - TURN_SIZE;
        let column = minimap.x - GAP - SMALL_SIZE;
        let small = |i: f32| {
            let y = minimap.y + i * (SMALL_SIZE + GAP);
            Rect::new(column, y, SMALL_SIZE, SMALL_SIZE)
        };
        let (ax, ay) = (
            w - ATTACK_RIGHT - ATTACK_SIZE / 2.0,
            h - ATTACK_BOTTOM - ATTACK_SIZE / 2.0,
        );
        let round = |deg: f32, r: f32, size: f32| {
            let (sin, cos) = deg.to_radians().sin_cos();
            centred(ax + r * cos, ay - r * sin, size)
        };
        let mut slots = [Rect::new(0.0, 0.0, 0.0, 0.0); SLOT_BUTTONS];
        for (slot, deg) in slots.iter_mut().zip(ARC_DEG) {
            *slot = round(deg, ARC_R, SKILL_SIZE);
        }
        // the three beside the arc, right to left; above the skill rows when they would meet them
        let first = slots[0].x - GAP;
        let row_y = ay + ATTACK_SIZE / 2.0 - ITEM_SIZE;
        let leftmost = first - 3.0 * (ITEM_SIZE + GAP) + GAP;
        let row_y = if leftmost < dock.x + dock.w + GAP && row_y + ITEM_SIZE > dock.y {
            dock.y - GAP - ITEM_SIZE
        } else {
            row_y
        };
        for (i, slot) in slots[4..].iter_mut().enumerate() {
            let x = first - (i as f32 + 1.0) * (ITEM_SIZE + GAP) + GAP;
            *slot = Rect::new(x, row_y, ITEM_SIZE, ITEM_SIZE);
        }
        // over the arc's top skill, at Attack's right edge
        let arc_top = ay - ARC_R * ARC_DEG[3].to_radians().sin() - SKILL_SIZE / 2.0;
        let talk = talk.then(|| {
            Rect::new(
                w - ATTACK_RIGHT - TALK_W,
                (arc_top - GAP - TALK_H).floor(),
                TALK_W,
                TALK_H,
            )
        });
        Self {
            talk,
            stick,
            turn_left: Rect::new(STICK_MARGIN, turn_top, TURN_SIZE, TURN_SIZE),
            turn_right: Rect::new(
                STICK_MARGIN + 2.0 * STICK_R - TURN_SIZE,
                turn_top,
                TURN_SIZE,
                TURN_SIZE,
            ),
            zoom_in: small(0.0),
            zoom_out: small(1.0),
            reset: small(2.0),
            attack: centred(ax, ay, ATTACK_SIZE),
            set: round(SET_DEG, SET_R, SET_SIZE),
            slots,
        }
    }

    fn stick_base(&self) -> Rect {
        centred(self.stick.0, self.stick.1, 2.0 * STICK_R)
    }

    fn zones(&self) -> impl Iterator<Item = Rect> + '_ {
        let fixed = [
            self.stick_base(),
            self.turn_left,
            self.turn_right,
            self.zoom_in,
            self.zoom_out,
            self.reset,
            self.attack,
            self.set,
        ];
        fixed.into_iter().chain(self.slots).chain(self.talk)
    }
}

/// The controls' state between frames: the fingers they own, which holds the stick, where they were
#[derive(Debug, Default)]
pub struct TouchControls {
    /// Fingers that landed on a control, until they lift
    owned: Vec<u64>,
    stick: Option<u64>,
    /// Where each control is: a finger landing in one is not the mouse. Kept while a controller hides the
    /// controls, so the first finger back lands on them
    zones: Vec<Rect>,
}

impl TouchControls {
    pub fn zones(&self) -> &[Rect] {
        &self.zones
    }

    /// No controls on this screen (the menu, the classic HUD, out of the game): no finger is theirs
    pub fn hide(&mut self) {
        self.owned.clear();
        self.stick = None;
        self.zones.clear();
    }

    /// A controller is in use: the controls are not drawn, but their places stay known
    pub fn rest(&mut self, ui: &UiFrame, minimap: Rect, dock: Rect) {
        let layout = Layout::new(
            ui.ctx.screen_width,
            ui.ctx.screen_height,
            minimap,
            dock,
            false,
        );
        self.owned.clear();
        self.stick = None;
        self.zones.clear();
        self.zones.extend(layout.zones());
    }

    pub fn build(
        &mut self,
        ui: &mut UiFrame,
        minimap: Rect,
        dock: Rect,
        slots: &TouchSlots,
    ) -> TouchInput {
        let (w, h) = (ui.ctx.screen_width, ui.ctx.screen_height);
        let layout = Layout::new(w, h, minimap, dock, slots.talk.is_some());
        self.zones.clear();
        self.zones.extend(layout.zones());
        let (ptrs, count) = self.pointers(ui);
        let ptrs = &ptrs[..count];
        let mut out = TouchInput::default();

        // shapes first, then the icons, then the text: few texture changes, few draw calls
        out.stick = self.stick(ui, layout.stick, ptrs);
        let press = |ui: &mut UiFrame, id, r| self.press(ui, id, r, ptrs);
        let turn_left = press(ui, TURN_LEFT_ID, layout.turn_left);
        let turn_right = press(ui, TURN_RIGHT_ID, layout.turn_right);
        let zoom_in = press(ui, ZOOM_IN_ID, layout.zoom_in);
        let zoom_out = press(ui, ZOOM_OUT_ID, layout.zoom_out);
        let reset = press(ui, RESET_ID, layout.reset);
        let attack = press(ui, ATTACK_ID, layout.attack);
        let set = press(ui, SET_ID, layout.set);
        let talk = layout.talk.map(|r| (r, press(ui, TALK_ID, r)));
        let mut slot_presses = [Press::default(); SLOT_BUTTONS];
        for (i, p) in slot_presses.iter_mut().enumerate() {
            *p = press(ui, WidgetId(SLOT_BASE_ID + i as u32), layout.slots[i]);
        }

        for (r, p) in [
            (layout.turn_left, turn_left),
            (layout.turn_right, turn_right),
            (layout.zoom_in, zoom_in),
            (layout.zoom_out, zoom_out),
            (layout.reset, reset),
            (layout.attack, attack),
            (layout.set, set),
        ] {
            round_button(ui, r, p.held);
        }
        if let Some((r, p)) = talk {
            style::pill_button(ui, r);
            if p.held {
                style::push_white(
                    ui,
                    draw::rounded_rect(
                        r.x + 1.0,
                        r.y + 1.0,
                        r.w - 2.0,
                        r.h - 2.0,
                        r.h * 0.5 - 1.0,
                        HELD,
                    ),
                );
            }
        }
        for (i, (r, p)) in layout.slots.iter().zip(&slot_presses).enumerate() {
            if i < ARC_DEG.len() {
                round_button(ui, *r, p.held);
            } else {
                style::slot(ui, *r, p.held);
            }
        }
        Icon::Left.draw(ui, layout.turn_left);
        Icon::Right.draw(ui, layout.turn_right);
        Icon::Plus.draw(ui, layout.zoom_in);
        Icon::Minus.draw(ui, layout.zoom_out);
        Icon::Reset.draw(ui, layout.reset);
        for (r, face) in layout.slots.iter().zip(&slots.faces) {
            if let Some(face) = face {
                slot_icon(ui, *r, &face.icon);
            }
        }
        label(ui, layout.attack, crate::pt!("Attack", "Atacar"));
        if let (Some((r, _)), Some(name)) = (talk, &slots.talk) {
            let text = if name.is_empty() {
                crate::pt!("Talk", "Conversar").to_string()
            } else {
                format!("Talk · {name}")
            };
            label(ui, r, &fit(ui, &text, r.w - 2.0 * TALK_PAD));
        }
        label(
            ui,
            layout.set,
            SET_LABELS[slots.set as usize % SET_LABELS.len()],
        );
        for (i, (r, face)) in layout.slots.iter().zip(&slots.faces).enumerate() {
            if let Some(count) = face.as_ref().and_then(|f| f.count.as_deref()) {
                let inset = if i < ARC_DEG.len() { 10.0 } else { 4.0 };
                let baseline = r.y + r.h - 4.0;
                style::text_right(ui, r.x + r.w - inset, baseline, count, style::TEXT);
            }
        }

        out.turn = f32::from(turn_right.held) - f32::from(turn_left.held);
        out.zoom = f32::from(zoom_out.held) - f32::from(zoom_in.held);
        out.reset = reset.tapped;
        out.attack = attack.tapped;
        out.next_set = set.tapped;
        out.slot = slot_presses.iter().position(|p| p.tapped);
        out.talk = talk.is_some_and(|(_, p)| p.tapped);
        out
    }

    /// The fingers the controls own: kept while down, taken when they land on a control and not on a window.
    /// On a desktop the mouse while its button is down
    fn pointers(&mut self, ui: &UiFrame) -> ([Pointer; MAX_POINTERS], usize) {
        let fingers = &ui.ctx.fingers;
        self.owned.retain(|id| fingers.iter().any(|f| f.id == *id));
        for f in fingers.iter().filter(|f| f.landed) {
            let on_control = self.zones.iter().any(|r| r.contains(f.x, f.y));
            if on_control && !ui.window_over(f.x, f.y) && !self.owned.contains(&f.id) {
                self.owned.push(f.id);
            }
        }
        let mut out = [Pointer::default(); MAX_POINTERS];
        let mut count = 0;
        let owned = fingers.iter().filter(|f| self.owned.contains(&f.id));
        for (slot, f) in out.iter_mut().zip(owned) {
            *slot = Pointer {
                id: f.id,
                x: f.x,
                y: f.y,
                landed: f.landed,
            };
            count += 1;
        }
        let c = &ui.ctx;
        let desktop = !crate::menu::HANDHELD && fingers.is_empty();
        if desktop && (c.mouse_down || c.mouse_pressed) && !ui.window_over(c.mouse_x, c.mouse_y) {
            out[0] = Pointer {
                id: MOUSE,
                x: c.mouse_x,
                y: c.mouse_y,
                landed: c.mouse_pressed,
            };
            count = 1;
        }
        (out, count)
    }

    /// The stick: a finger that lands on it holds it until lifted, wherever it goes; returns where it points
    fn stick(&mut self, ui: &mut UiFrame, (cx, cy): (f32, f32), ptrs: &[Pointer]) -> (f32, f32) {
        let base = centred(cx, cy, 2.0 * STICK_R);
        ui.interact(STICK_ID, base);
        let held = self
            .stick
            .and_then(|id| ptrs.iter().find(|p| p.id == id))
            .or_else(|| ptrs.iter().find(|p| p.landed && base.contains(p.x, p.y)));
        self.stick = held.map(|p| p.id);
        let v = held.map_or((0.0, 0.0), |p| {
            unit((p.x - cx) / STICK_R, (p.y - cy) / STICK_R)
        });
        style::push_white(ui, circle(base, STICK_RING));
        style::push_white(ui, circle(inset(base), STICK_BASE));
        let travel = STICK_R - KNOB_R;
        let knob = centred(cx + v.0 * travel, cy + v.1 * travel, 2.0 * KNOB_R);
        round_button(ui, knob, held.is_some());
        v
    }

    /// Where the owned fingers are on `r`: one held on it, one landed on it this frame. Not the stick's
    fn press(&self, ui: &mut UiFrame, id: WidgetId, r: Rect, ptrs: &[Pointer]) -> Press {
        ui.interact(id, r);
        let mut press = Press::default();
        for p in ptrs
            .iter()
            .filter(|p| Some(p.id) != self.stick && r.contains(p.x, p.y))
        {
            press.held = true;
            press.tapped |= p.landed;
        }
        press
    }
}

#[derive(Clone, Copy, Default)]
struct Press {
    held: bool,
    tapped: bool,
}

/// (x, y) cut to length 1 at most
fn unit(x: f32, y: f32) -> (f32, f32) {
    let len = (x * x + y * y).sqrt();
    if len > 1.0 {
        (x / len, y / len)
    } else {
        (x, y)
    }
}

fn centred(cx: f32, cy: f32, size: f32) -> Rect {
    let corner = |c: f32| (c - size / 2.0).floor();
    Rect::new(corner(cx), corner(cy), size, size)
}

fn inset(r: Rect) -> Rect {
    Rect::new(r.x + 1.0, r.y + 1.0, r.w - 2.0, r.h - 2.0)
}

fn circle(r: Rect, color: [f32; 4]) -> style::Mesh {
    draw::rounded_rect(r.x, r.y, r.w, r.h, r.w * 0.5, color)
}

/// A round button, lit while held
fn round_button(ui: &mut UiFrame, r: Rect, held: bool) {
    style::badge(ui, r.x, r.y, r.w, "");
    if held {
        style::push_white(ui, circle(inset(r), HELD));
    }
}

fn slot_icon(ui: &mut UiFrame, r: Rect, icon: &str) {
    let (x, y) = (
        (r.x + (r.w - ICON) / 2.0).floor(),
        (r.y + (r.h - ICON) / 2.0).floor(),
    );
    let (v, i) = draw::quad_vertices(x, y, ICON, ICON, [1.0; 4]);
    ui.draw_calls.push(DrawCall {
        vertices: v.into(),
        indices: i.into(),
        texture: TextureRef::Named(icon.to_string().into()),
    });
}

/// `text` cut to `max` wide in bold, an ellipsis where it was cut
fn fit(ui: &UiFrame, text: &str, max: f32) -> String {
    if style::measure_bold(ui, text) <= max {
        return text.to_string();
    }
    let mut cut: String = text.to_string();
    while !cut.is_empty() && style::measure_bold(ui, &format!("{cut}…")) > max {
        cut.pop();
    }
    format!("{}…", cut.trim_end())
}

/// Bold text in the middle of `r`
fn label(ui: &mut UiFrame, r: Rect, text: &str) {
    let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
    let baseline = style::baseline(ui, cy - style::line(ui) / 2.0);
    let x = cx - style::measure_bold(ui, text) / 2.0;
    style::text_bold(ui, x.floor(), baseline, text, style::TEXT);
}

#[derive(Clone, Copy)]
enum Icon {
    Left,
    Right,
    Plus,
    Minus,
    Reset,
}

impl Icon {
    fn draw(self, ui: &mut UiFrame, r: Rect) {
        let size = (r.w * 0.34).floor();
        let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        let bar = |ui: &mut UiFrame, x: f32, y: f32, bw: f32, bh: f32| {
            style::fill(ui, Rect::new(x.floor(), y.floor(), bw, bh), style::TEXT);
        };
        let top = cy - size / 2.0;
        match self {
            Icon::Left => style::caret_left(ui, cx - size * 0.45, top, size, style::TEXT),
            Icon::Right => style::caret(ui, cx - size * 0.35, top, size, style::TEXT),
            Icon::Plus => {
                bar(ui, cx - size / 2.0, cy - 1.0, size, 2.0);
                bar(ui, cx - 1.0, top, 2.0, size);
            }
            Icon::Minus => bar(ui, cx - size / 2.0, cy - 1.0, size, 2.0),
            Icon::Reset => style::dot(ui, cx - size / 4.0, cy - size / 4.0, size / 2.0, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        let minimap = Rect::new(1300.0, 14.0, 190.0, 190.0);
        let dock = Rect::new(500.0, 700.0, 534.0, 50.0);
        Layout::new(1536.0, 864.0, minimap, dock, true)
    }

    #[test]
    fn a_set_shows_its_first_seven_slots() {
        let slots = TouchSlots {
            set: 2,
            ..TouchSlots::default()
        };
        assert_eq!(slots.slot(0), 24);
        assert_eq!(slots.slot(6), 30);
        assert_eq!(SET_LABELS.len(), PAD_SETS as usize);
    }

    #[test]
    fn the_stick_reads_at_most_full_tilt() {
        assert_eq!(unit(0.5, 0.0), (0.5, 0.0));
        let (x, y) = unit(3.0, 4.0);
        assert!((x - 0.6).abs() < 1e-6 && (y - 0.8).abs() < 1e-6);
    }

    #[test]
    fn no_two_controls_overlap() {
        no_overlap(layout());
    }

    #[test]
    fn no_two_controls_overlap_on_a_handheld() {
        // a 1080p handheld at Android's 150%, its minimap and skill rows where the HUD draws them there
        let minimap = Rect::new(1077.0, 40.0, 187.0, 187.0);
        let dock = Rect::new(390.0, 583.0, 500.0, 137.0);
        no_overlap(Layout::new(1280.0, 720.0, minimap, dock, true));
    }

    fn no_overlap(layout: Layout) {
        // the controls are round (the slot row's squares fit in their circle as well): centres far enough apart
        let talk = layout.talk.expect("an NPC in reach");
        let round = Layout {
            talk: None,
            ..layout
        };
        let zones: Vec<Rect> = round.zones().collect();
        // the Talk pill is not: its box clear of every other's
        for r in &zones {
            let apart = talk.x >= r.x + r.w + GAP
                || r.x >= talk.x + talk.w + GAP
                || talk.y >= r.y + r.h + GAP
                || r.y >= talk.y + talk.h + GAP;
            assert!(apart, "{talk:?} meets {r:?}");
        }
        let centre = |r: &Rect| (r.x + r.w / 2.0, r.y + r.h / 2.0);
        for (i, a) in zones.iter().enumerate() {
            for b in &zones[i + 1..] {
                let ((ax, ay), (bx, by)) = (centre(a), centre(b));
                let apart = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
                assert!(apart >= (a.w + b.w) / 2.0, "{a:?} meets {b:?}");
            }
        }
    }

    #[test]
    fn the_slot_row_keeps_off_the_skill_rows() {
        let minimap = Rect::new(800.0, 14.0, 150.0, 150.0);
        let dock = Rect::new(280.0, 400.0, 534.0, 150.0);
        let l = Layout::new(1100.0, 600.0, minimap, dock, true);
        for r in &l.slots[4..] {
            assert!(r.y + r.h <= dock.y, "{r:?}");
        }
    }
}
