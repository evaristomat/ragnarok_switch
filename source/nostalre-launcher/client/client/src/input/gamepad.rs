//! Classic controller: left stick walks; right stick rotates/tilts the camera in the world and aims ground
//! skills or moves the UI cursor. ZL/ZR cycle nearby living monsters without attacking. A/B are the
//! mouse buttons, X attacks the closest monster, Y picks up the closest item or else talks to the nearest NPC (in
//! a dialog A goes on and B cancels; in a shop A moves a line across, Start buys). L1, R1 or both held with a face
//! button use a hotbar slot, in one of three sets (Select+L1/R1). The camera is ours (the design canvas,
//! "Camera on controller and touch"): right stick turns/tilts it, Select+Up/Down zooms, R3
//! puts the starting view back. Start is Enter, Select+Start Escape, the D-pad alone the arrow keys. While the
//! HUD's menu is open, A, B, X, Y, L1 and R1 are its keys (`menu_press`).
//!
//! Buttons arrive as key events carrying the Android keycode; sticks, the D-pad hat and the triggers as
//! DeviceEvent::Motion (vendor/winit). Android only for now.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{DeviceId, ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, NativeKey, PhysicalKey};
use winit::window::WindowId;

use crate::App;
use nostalre_game::app_state::AppState;
use nostalre_game::entity::EntityType;
use nostalre_game::entity_collection::TALK_RANGE;
use nostalre_game::hotkey::{PAD_SET_SLOTS, PAD_SETS};
use nostalre_game::cursor::PendingSkillTarget;
use models::enums::skill_enums::SkillEnum;
use models::enums::skill::SkillTargetType;
use nostalre_renderer::camera::CameraControl;

/// Stick values within this are rest (roBrowser joyDeadline).
const DEADZONE: f32 = 0.1;
/// Cursor speed at full tilt, logical pixels per second: four times roBrowser's (joySense 25), for a handheld's
/// screen at 150%
const CURSOR_SPEED: f32 = 1000.0;
/// Cells ahead the walk aims at full tilt; a partial tilt aims shorter.
const WALK_REACH: f32 = 3.0;
/// A walk request is repeated at least this often while the stick is held (the server walks one cell each
/// `speed` ms, this is its fallback when the speed is unknown).
const WALK_REPEAT_FALLBACK: Duration = Duration::from_millis(150);
/// Search radius, in cells, for X (monsters) and Y (floor items).
const REACH_CELLS: i32 = 20;
/// The camera at full trigger: degrees turned or tilted a second; a light press goes slower (squared)
const TURN_DEG_PER_SEC: f32 = 120.0;
const TILT_DEG_PER_SEC: f32 = 30.0;
/// Select+Up/Down held: wheel notches a second
const ZOOM_NOTCHES_PER_SEC: f32 = 6.0;
/// A trigger pressed less than this is at rest
const TRIGGER_REST: f32 = 0.05;
/// The camera readout stays this long after the camera stops moving, seconds
const READOUT_LINGER: f32 = 1.0;

/// Android axis ids (MotionEvent.AXIS_*), as vendor/winit reports them.
mod axis {
    pub const X: u32 = 0;
    pub const Y: u32 = 1;
    pub const Z: u32 = 11;
    pub const RZ: u32 = 14;
    pub const HAT_X: u32 = 15;
    pub const HAT_Y: u32 = 16;
    pub const LTRIGGER: u32 = 17;
    pub const RTRIGGER: u32 = 18;
    pub const GAS: u32 = 22;
    pub const BRAKE: u32 = 23;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Button {
    A,
    B,
    X,
    Y,
    L1,
    R1,
    L2,
    R2,
    R3,
    Start,
    Select,
    Up,
    Down,
    Left,
    Right,
}

impl Button {
    /// Android KeyEvent.KEYCODE_BUTTON_* / KEYCODE_DPAD_*.
    fn from_android(code: u32) -> Option<Self> {
        Some(match code {
            96 => Self::A,
            97 => Self::B,
            99 => Self::X,
            100 => Self::Y,
            102 => Self::L1,
            103 => Self::R1,
            104 => Self::L2,
            105 => Self::R2,
            107 => Self::R3,
            108 => Self::Start,
            109 => Self::Select,
            _ => return None,
        })
    }
}

/// The modifier held with a face button: which hotbar slots the face buttons reach.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Group {
    L1,
    R1,
    L1R1,
}

/// A set's twelve slots, in the order of the HUD: L1's four, L1+R1's, R1's, each Y, X, B, A
fn hotbar_slot(group: Group, face: Button, set: u8) -> Option<usize> {
    let face = match face {
        Button::Y => 0,
        Button::X => 1,
        Button::B => 2,
        Button::A => 3,
        _ => return None,
    };
    let group = match group {
        Group::L1 => 0,
        Group::L1R1 => 1,
        Group::R1 => 2,
    };
    Some(PAD_SET_SLOTS * set as usize + 4 * group + face)
}

/// Octant 0..8 of a direction, for "walk again when the direction turns".
fn octant(x: f32, y: f32) -> i32 {
    ((y.atan2(x) / std::f32::consts::FRAC_PI_4).round() as i32) & 7
}

/// The cell the left stick aims at from `cell`: `stick` is the raw stick (y down), `yaw` the camera's.
fn walk_target(cell: (u16, u16), stick: (f32, f32), yaw: f32) -> (i32, i32) {
    // screen up is +y on the map at yaw 0; the camera's yaw turns the screen over the map
    let (sx, sy) = (stick.0, -stick.1);
    let (sin, cos) = yaw.sin_cos();
    let dx = sx * cos - sy * sin;
    let dy = sx * sin + sy * cos;
    (
        (cell.0 as f32 + dx * WALK_REACH).round() as i32,
        (cell.1 as f32 + dy * WALK_REACH).round() as i32,
    )
}

fn dead(v: f32) -> f32 {
    if v.abs() <= DEADZONE { 0.0 } else { v }
}

#[derive(Default)]
pub(crate) struct Gamepad {
    held: HashSet<Button>,
    /// Pressed since the last frame, in order.
    fresh: Vec<Button>,
    released: Vec<Button>,
    axes: [f32; 24],
    /// Hotbar set (Select+L1/R1): 0 to `PAD_SETS - 1`.
    set: u8,
    /// Seconds the camera readout still shows
    readout: f32,
    /// R3 was pressed: the starting view, next frame
    reset: bool,
    /// Last walk request: when, and the octant it went.
    walk: Option<(Instant, i32)>,
    last_frame: Option<Instant>,
    /// The pad drives the cursor from here (logical px), seeded from the mouse position.
    cursor: Option<(f32, f32)>,
    aiming: bool,
    queued_pad_skill: Option<(SkillEnum, i16)>,
    /// Pressed while the menu was open: the menu's keys, so their release is not a click.
    menu_presses: HashSet<Button>,
    menu_direction: Option<Button>,
    menu_next: Option<Instant>,
}

impl Gamepad {
    /// Deterministic initial delay and repeat, shared by left stick and D-pad.
    fn menu_step(&mut self, now: Instant, active: bool) -> Option<Button> {
        let (x, y) = self.stick(axis::X, axis::Y);
        let direction = active
            .then(|| {
                [Button::Up, Button::Down, Button::Left, Button::Right]
                    .into_iter()
                    .find(|b| self.held.contains(b))
                    .or_else(|| {
                        if x.abs().max(y.abs()) < 0.55 {
                            None
                        } else if x.abs() > y.abs() {
                            Some(if x < 0.0 { Button::Left } else { Button::Right })
                        } else {
                            Some(if y < 0.0 { Button::Up } else { Button::Down })
                        }
                    })
            })
            .flatten();
        let Some(direction) = direction else {
            self.menu_direction = None;
            self.menu_next = None;
            return None;
        };
        if self.menu_direction != Some(direction) {
            self.menu_direction = Some(direction);
            self.menu_next = Some(now + Duration::from_millis(300));
            Some(direction)
        } else if self.menu_next.is_some_and(|next| now >= next) {
            self.menu_next = Some(now + Duration::from_millis(110));
            Some(direction)
        } else {
            None
        }
    }

    /// Automated tests feed the same normalized buttons as the Horizon backend.
    #[cfg(test)]
    pub(crate) fn validation_button(&mut self, button: Button, down: bool) {
        self.set_held(button, down);
    }

    /// A controller button: noted for the next frame. False for anything else (keyboards keep their keys).
    pub(crate) fn key(&mut self, event: &KeyEvent) -> bool {
        let button = match (&event.logical_key, event.physical_key) {
            (Key::Unidentified(NativeKey::Android(code)), _) => Button::from_android(*code),
            (_, PhysicalKey::Code(code)) => self.arrow(code, event.state),
            _ => None,
        };
        let Some(button) = button else {
            return false;
        };
        match event.state {
            ElementState::Pressed if !event.repeat => self.press(button),
            ElementState::Pressed => {}
            ElementState::Released => self.release(button),
        }
        true
    }

    /// Controllers that send their D-pad as arrow keys: the pad's own with Select held (zoom), plain
    /// arrow keys otherwise. A release is the pad's while its button is held, also when Select was let go first.
    fn arrow(&self, code: KeyCode, state: ElementState) -> Option<Button> {
        let button = match code {
            KeyCode::ArrowUp => Button::Up,
            KeyCode::ArrowDown => Button::Down,
            KeyCode::ArrowLeft => Button::Left,
            KeyCode::ArrowRight => Button::Right,
            _ => return None,
        };
        let held = |b| self.held.contains(&b);
        (held(Button::Select) || (state == ElementState::Released && held(button)))
            .then_some(button)
    }

    pub(crate) fn axis(&mut self, id: u32, value: f64) {
        let Some(slot) = self.axes.get_mut(id as usize) else {
            return;
        };
        *slot = value as f32;
        match id {
            axis::HAT_X => {
                self.set_held(Button::Left, value < -0.5);
                self.set_held(Button::Right, value > 0.5);
            }
            axis::HAT_Y => {
                self.set_held(Button::Up, value < -0.5);
                self.set_held(Button::Down, value > 0.5);
            }
            // triggers that only report an axis still work as L2/R2
            axis::LTRIGGER | axis::BRAKE => {
                let v = self.axes[axis::LTRIGGER as usize].max(self.axes[axis::BRAKE as usize]);
                self.set_held(Button::L2, v > 0.5);
            }
            axis::RTRIGGER | axis::GAS => {
                let v = self.axes[axis::RTRIGGER as usize].max(self.axes[axis::GAS as usize]);
                self.set_held(Button::R2, v > 0.5);
            }
            _ => {}
        }
    }

    fn press(&mut self, button: Button) {
        if self.held.insert(button) {
            self.fresh.push(button);
        }
    }

    fn release(&mut self, button: Button) {
        if self.held.remove(&button) {
            self.released.push(button);
        }
    }

    fn set_held(&mut self, button: Button, down: bool) {
        if down {
            self.press(button)
        } else {
            self.release(button)
        }
    }

    /// Select held this frame: still down, or let go since the last one (a quick Select+D-pad press can start
    /// and end between two frames).
    fn select_down(&self) -> bool {
        self.held.contains(&Button::Select) || self.released.contains(&Button::Select)
    }

    fn stick(&self, x: u32, y: u32) -> (f32, f32) {
        (dead(self.axes[x as usize]), dead(self.axes[y as usize]))
    }

    /// Shoulder chords work even when both buttons arrive in the same frame.
    fn group(&self) -> Option<Group> {
        let held = |b: Button| self.held.contains(&b);
        match (held(Button::L1), held(Button::R1)) {
            (true, true) => Some(Group::L1R1),
            (true, false) => Some(Group::L1),
            (false, true) => Some(Group::R1),
            (false, false) => None,
        }
    }

    /// How far a trigger is pressed, 0 to 1: its axis, or all the way for one that only sends a button
    fn trigger(&self, button: Button) -> f32 {
        let (a, b) = match button {
            Button::L2 => (axis::LTRIGGER, axis::BRAKE),
            _ => (axis::RTRIGGER, axis::GAS),
        };
        let analog = self.axes[a as usize].max(self.axes[b as usize]);
        if analog > TRIGGER_REST {
            analog.min(1.0)
        } else if self.held.contains(&button) {
            1.0
        } else {
            0.0
        }
    }

    /// R2 minus L2, a light press counting less (squared): -1 to 1
    fn trigger_turn(&self) -> f32 {
        let curve = |v: f32| v * v;
        curve(self.trigger(Button::R2)) - curve(self.trigger(Button::L2))
    }
}

impl App {
    /// Once per frame, before the UI is built: what the pad did since the last one.
    pub(crate) fn gamepad_frame(&mut self, event_loop: &ActiveEventLoop, window: WindowId) {
        self.prune_controller_target();
        self.resolve_controller_skill();
        let now = Instant::now();
        let dt = self
            .gamepad
            .last_frame
            .map_or(0.0, |t| now.duration_since(t).as_secs_f32().min(0.1));
        self.gamepad.last_frame = Some(now);

        self.gamepad_cursor(event_loop, window, dt);
        self.gamepad_walk(now);
        self.gamepad_camera(dt);
        if let Some(button) = self.gamepad.menu_step(now, self.windows.menu.is_open() || self.windows.quest_window.is_open() || self.windows.quest_detail_window.is_open()) {
            self.with_ui(|ui| {
                ui.pad_labels = true;
                match button {
                    Button::Up => ui.key_up = true,
                    Button::Down => ui.key_down = true,
                    Button::Left => ui.key_left = true,
                    Button::Right => ui.key_right = true,
                    _ => {}
                }
            });
        }
        self.touch_actions();

        let select = self.gamepad.select_down();
        let fresh = std::mem::take(&mut self.gamepad.fresh);
        let released = std::mem::take(&mut self.gamepad.released);
        let group = self.gamepad.group();
        for &button in &fresh {
            self.gamepad_press(event_loop, window, button, select, group);
        }
        for button in released {
            if self.gamepad.menu_presses.remove(&button) {
                continue;
            }
            match button {
                Button::A => self.gamepad_click(
                    event_loop,
                    window,
                    MouseButton::Left,
                    ElementState::Released,
                ),
                Button::B => self.gamepad_click(
                    event_loop,
                    window,
                    MouseButton::Right,
                    ElementState::Released,
                ),
                _ => {}
            }
        }
    }

    fn gamepad_press(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: WindowId,
        button: Button,
        select: bool,
        group: Option<Group>,
    ) {
        let in_game = self.game.session.app_state == AppState::InGame;
        let playing = in_game && !self.windows.menu.is_open();
        self.with_ui(|ui| ui.pad_labels = true);
        if select {
            match button {
                Button::Start => self.with_ui(|ui| ui.key_escape = true),
                Button::L1 if playing => {
                    self.gamepad.set = (self.gamepad.set + PAD_SETS - 1) % PAD_SETS;
                }
                Button::R1 if playing => self.gamepad.set = (self.gamepad.set + 1) % PAD_SETS,
                _ => {}
            }
            return;
        }
        if in_game && self.windows.menu.is_open() && self.menu_press(button) {
            self.gamepad.menu_presses.insert(button);
            return;
        }
        if in_game && group.is_none() && button == Button::B && self.windows.item_info_window.is_open() {
            self.windows.item_info_window.close();
            self.gamepad.menu_presses.insert(button);
            return;
        }
        if in_game && group.is_none() && button == Button::B && self.windows.vending_shop_window.is_open() {
            self.windows.vending_shop_window.close();
            self.gamepad.menu_presses.insert(button);
            return;
        }
        if in_game
            && group.is_none()
            && self.windows.npc_shop.shop.is_open()
            && self.shop_press(button)
        {
            self.gamepad.menu_presses.insert(button);
            return;
        }
        if in_game
            && group.is_none()
            && self.windows.npc_dialog.dialog.is_open()
            && self.dialog_press(button)
        {
            self.gamepad.menu_presses.insert(button);
            return;
        }
        if in_game && group.is_none()
            && (self.windows.quest_window.is_open() || self.windows.quest_detail_window.is_open())
        {
            let handled = match button {
                Button::A => { self.with_ui(|ui| ui.key_enter = true); true },
                Button::B => { self.with_ui(|ui| ui.key_escape = true); true },
                Button::Up => { self.with_ui(|ui| ui.key_up = true); true },
                Button::Down => { self.with_ui(|ui| ui.key_down = true); true },
                _ => false,
            };
            if handled { self.gamepad.menu_presses.insert(button); return; }
        }
        if in_game && button == Button::Start {
            if self.windows.menu.is_open() {
                self.windows.menu.close();
            } else {
                self.with_ui(|ui| ui.key_escape = true);
            }
            self.gamepad.menu_presses.insert(button);
            return;
        }
        // Direction presses in the menu are generated once by menu_step (including repeats).
        if self.windows.menu.is_open()
            && matches!(
                button,
                Button::Up | Button::Down | Button::Left | Button::Right
            )
        {
            return;
        }
        match button {
            Button::Start => self.with_ui(|ui| ui.key_enter = true),
            Button::Up => self.with_ui(|ui| ui.key_up = true),
            Button::Down => self.with_ui(|ui| ui.key_down = true),
            Button::Left => self.with_ui(|ui| ui.key_left = true),
            Button::Right => self.with_ui(|ui| ui.key_right = true),
            Button::A | Button::B | Button::X | Button::Y if group.is_some() && self.controller_world_input() => {
                if let Some(slot) = group.and_then(|g| hotbar_slot(g, button, self.gamepad.set)) {
                    self.use_hotbar_slot(slot);
                }
            }
            Button::A => {
                self.gamepad_click(event_loop, window, MouseButton::Left, ElementState::Pressed)
            }
            Button::B if self.controller_world_input() && self.game.pending_casts.pending_skill_target.is_none()
                && self.game.combat.selected_target_id.is_some() => {
                self.stop_attacking();
                self.game.combat.selected_target_id = None;
                self.gamepad.menu_presses.insert(button);
            }
            Button::B => self.gamepad_click(
                event_loop,
                window,
                MouseButton::Right,
                ElementState::Pressed,
            ),
            Button::L2 if playing && self.controller_world_input() => self.cycle_controller_target(-1),
            Button::R2 if playing && self.controller_world_input() => self.cycle_controller_target(1),
            Button::X if playing && self.controller_world_input() => self.attack_closest_monster(),
            Button::Y if in_game => {
                if !self.pick_up_closest_item() {
                    self.talk_to_nearest_npc();
                }
            }
            Button::R3 if in_game => self.gamepad.reset = true,
            _ => {}
        }
    }

    /// The menu's keys (`docs/nostalre-menu.md`): A opens, B goes back, L1 and R1 turn the tabs, Y takes
    /// off or drops, X puts on the skill bar; false for a button the menu leaves alone
    fn menu_press(&mut self, button: Button) -> bool {
        match button {
            Button::A => self.with_ui(|ui| ui.key_enter = true),
            Button::B => self.with_ui(|ui| ui.key_escape = true),
            Button::Y => self.with_ui(|ui| ui.key_delete = true),
            Button::X => self.with_ui(|ui| ui.pressed_codes.push(KeyCode::KeyS)),
            Button::L2 => self.with_ui(|ui| ui.pressed_codes.push(KeyCode::Home)),
            Button::R2 => self.with_ui(|ui| ui.pressed_codes.push(KeyCode::End)),
            Button::L1 => self.with_ui(|ui| ui.pressed_codes.push(KeyCode::PageUp)),
            Button::R1 => self.with_ui(|ui| ui.pressed_codes.push(KeyCode::PageDown)),
            _ => return false,
        }
        true
    }

    /// An NPC's dialog: A goes on (next, close, the chosen line), B cancels, the D-pad picks a line (arrow
    /// keys); X and Y wait, so a press meant for the dialog attacks or talks to no one
    fn dialog_press(&mut self, button: Button) -> bool {
        match button {
            Button::A => self.with_ui(|ui| ui.key_enter = true),
            Button::B => self.with_ui(|ui| ui.key_escape = true),
            Button::X | Button::Y => {}
            _ => return false,
        }
        true
    }

    /// A shop (buy, sell, a market, a barter): the D-pad (arrow keys) walks the list, the basket and, down off
    /// the basket, its Buy and Cancel; A moves the picked line across, presses the button or answers the amount
    /// (where Left/Right step it by ten), B closes the amount or the shop, Start buys or sells; X and Y wait
    fn shop_press(&mut self, button: Button) -> bool {
        match button {
            Button::A => self.with_ui(|ui| ui.key_enter = true),
            Button::B => self.with_ui(|ui| ui.key_escape = true),
            Button::Start => self.with_ui(|ui| ui.pressed_codes.push(KeyCode::Space)),
            Button::X | Button::Y => {}
            _ => return false,
        }
        true
    }

    fn with_ui(&mut self, set: impl FnOnce(&mut nostalre_ui::context::UiContext)) {
        if let Some(ui) = &mut self.ui_context {
            set(ui);
        }
    }

    /// The right stick rotates and tilts; touch turn buttons turn it (tilt with Select),
    /// Select+Up/Down or the touch zoom buttons zoom. Not while the menu is open. The HUD's readout shows
    /// while it moves and a moment after
    fn gamepad_camera(&mut self, dt: f32) {
        self.gamepad.readout = (self.gamepad.readout - dt).max(0.0);
        let playing = self.controller_world_input() && self.game.pending_casts.pending_skill_target.is_none();
        let touch = self.windows.hud.touch_input();
        let pad = &self.gamepad;
        let select = pad.select_down();
        let (right_x, right_y) = pad.stick(axis::Z, axis::RZ);
        let turn = (right_x + if select { 0.0 } else { touch.turn }).clamp(-1.0, 1.0);
        let tilt = (-right_y + if select { touch.turn } else { 0.0 }).clamp(-1.0, 1.0);
        let zoom = match (
            pad.held.contains(&Button::Up),
            pad.held.contains(&Button::Down),
        ) {
            (true, false) if select => 1.0,
            (false, true) if select => -1.0,
            _ => touch.zoom,
        };
        let reset = std::mem::take(&mut self.gamepad.reset) || touch.reset;
        let moving = playing && (turn != 0.0 || tilt != 0.0 || zoom != 0.0 || reset);
        if moving {
            self.gamepad.readout = READOUT_LINGER;
        }
        let control = self.camera_control();
        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let camera = &mut renderer.camera;
        self.windows.hud.input.camera = (playing && self.gamepad.readout > 0.0).then(|| {
            nostalre_ui_component::hud::CameraReadout {
                turn_deg: camera.yaw.to_degrees().rem_euclid(360.0),
                tilt_deg: camera.pitch.to_degrees(),
                distance: camera.distance,
            }
        });
        if !moving {
            return;
        }
        if reset {
            camera.reset_view(control);
        }
        // each only when asked: a call by 0 still pulls the view into its band (a Ctrl-lifted zoom)
        if turn != 0.0 {
            camera.rotate_by(turn * TURN_DEG_PER_SEC * dt, control);
        }
        if tilt != 0.0 {
            camera.tilt_by(tilt * TILT_DEG_PER_SEC * dt, CameraControl { ctrl: true, ..control });
        }
        if zoom != 0.0 {
            // no Ctrl here: it would lift the zoom's limits
            camera.apply_wheel(zoom * ZOOM_NOTCHES_PER_SEC * dt, control);
        }
    }

    /// The on-screen controls' taps: Attack as X, a slot button as its hotbar slot, the set button as
    /// Select+R1. The stick walks in `gamepad_walk`, the camera buttons work in `gamepad_camera`
    fn touch_actions(&mut self) {
        self.windows.hud.input.touch_slots.set = self.gamepad.set;
        self.windows.hud.input.controller_group = self.gamepad.group().map(|g| match g {
            Group::L1 => 0,
            Group::L1R1 => 1,
            Group::R1 => 2,
        });
        if self.game.session.app_state != AppState::InGame {
            // out of the game no control is drawn, and none keeps a finger or a press
            self.windows.hud.hidden();
            return;
        }
        if self.windows.menu.is_open() {
            return;
        }
        // Talk shows the NPC it would talk to; none while a dialog or a shop is up
        let talk = (!self.windows.npc_dialog.dialog.is_open()
            && !self.windows.npc_shop.shop.is_open())
        .then(|| self.talkable_npc())
        .flatten()
        .map(|(_, name)| name.unwrap_or_default().to_string());
        self.windows.hud.input.touch_slots.talk = talk;
        let touch = self.windows.hud.touch_input();
        if touch.attack {
            self.attack_closest_monster();
        }
        if touch.talk {
            self.talk_to_nearest_npc();
        }
        if let Some(i) = touch.slot {
            let slot = self.windows.hud.input.touch_slots.slot(i);
            self.use_hotbar_slot(slot);
        }
        if touch.next_set {
            self.gamepad.set = (self.gamepad.set + 1) % PAD_SETS;
        }
    }

    /// Right stick: the cursor, as if the mouse moved (the UI and the world both see it).
    fn gamepad_cursor(&mut self, event_loop: &ActiveEventLoop, window: WindowId, dt: f32) {
        let aiming = self.controller_world_input() && self.game.pending_casts.pending_skill_target.is_some();
        let new_aim = aiming && !self.gamepad.aiming;
        self.gamepad.aiming = aiming;
        if self.controller_world_input() && !aiming {
            self.gamepad.cursor = None;
            return;
        }
        if self.windows.menu.is_open() {
            self.gamepad.cursor = None;
            return;
        }
        let (x, y) = self.gamepad.stick(axis::Z, axis::RZ);
        if x == 0.0 && y == 0.0 && !new_aim {
            self.gamepad.cursor = None;
            return;
        }
        let dpi = self.renderer.as_ref().map_or(1.0, |r| r.dpi_scale);
        let (w, h) = self
            .window
            .as_ref()
            .map(|w| w.inner_size())
            .map_or((1.0, 1.0), |s| {
                (s.width as f32 / dpi, s.height as f32 / dpi)
            });
        let (cx, cy) = if new_aim { (w * 0.5, h * 0.5) } else { self.gamepad.cursor.unwrap_or((
            self.input.mouse_position.0 as f32,
            self.input.mouse_position.1 as f32,
        )) };
        // speed by the tilt squared: a light push stays fine enough for a small button, a full one crosses
        // the screen in about a second
        let gain = CURSOR_SPEED * (x * x + y * y).sqrt().min(1.0) * dt;
        let next = (
            (cx + x * gain).clamp(0.0, w - 1.0),
            (cy + y * gain).clamp(0.0, h - 1.0),
        );
        self.gamepad.cursor = Some(next);
        let position =
            winit::dpi::PhysicalPosition::new((next.0 * dpi) as f64, (next.1 * dpi) as f64);
        self.window_event(
            event_loop,
            window,
            WindowEvent::CursorMoved {
                device_id: pad(),
                position,
            },
        );
    }

    fn gamepad_click(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: WindowId,
        button: MouseButton,
        state: ElementState,
    ) {
        let event = WindowEvent::MouseInput {
            device_id: pad(),
            state,
            button,
        };
        self.window_event(event_loop, window, event);
    }

    /// Left stick: walk toward where it points, relative to the camera. A request goes out when the direction
    /// turns, else once per cell walked.
    fn gamepad_walk(&mut self, now: Instant) {
        let (mut x, mut y) = self.gamepad.stick(axis::X, axis::Y);
        if x == 0.0 && y == 0.0 {
            (x, y) = self.windows.hud.touch_input().stick;
            (x, y) = (dead(x), dead(y));
        }
        if (x == 0.0 && y == 0.0)
            || !self.controller_world_input()
        {
            self.gamepad.walk = None;
            return;
        }
        let Some(player) = self.game.world.entities.player() else {
            return;
        };
        let cell = player.movement.cell_position();
        let repeat = match player.speed {
            0 => WALK_REPEAT_FALLBACK,
            ms => Duration::from_millis(ms as u64),
        };
        let yaw = self.renderer.as_ref().map_or(0.0, |r| r.camera.yaw);
        let (tx, ty) = walk_target(cell, (x, y), yaw);
        let turn = octant(x, -y);
        let due = self
            .gamepad
            .walk
            .is_none_or(|(at, last)| last != turn || now.duration_since(at) >= repeat);
        if !due {
            return;
        }
        self.gamepad.walk = Some((now, turn));
        self.stop_attacking();
        self.walk_to(tx, ty);
    }

    /// A walk request toward `(x, y)`, cut to where the path from the player can go.
    fn walk_to(&mut self, x: i32, y: i32) {
        let Some(gat) = &self.game.session.gat else {
            return;
        };
        let Some((sx, sy)) = self
            .game
            .world
            .entities
            .player()
            .map(|e| e.movement.cell_position())
        else {
            return;
        };
        if let Some(step) = nostalre_game::path::try_move_to(gat, sx, sy, x, y) {
            self.channel
                .send_packet(nostalre_network::build_request_move_packet(
                    step.dest_x,
                    step.dest_y,
                    self.active_packetver,
                ));
        }
    }

    fn controller_world_input(&self) -> bool {
        self.game.session.app_state == AppState::InGame
            && !self.game.session.player_dead
            && !self.windows.menu.is_open()
            && !self.windows.system_menu.open
            && !self.windows.item_info_window.is_open()
            && !self.windows.vending_shop_window.is_open()
            && !self.windows.vending_setup_window.is_open()
            && !self.windows.npc_dialog.dialog.is_open()
            && !self.windows.npc_shop.shop.is_open()
            && !self.windows.quest_window.is_open()
            && !self.windows.quest_detail_window.is_open()
            && !self.windows.chat_window.active
            && self.windows.confirm_dialog.state.is_none()
    }

    fn controller_targets(&self) -> Vec<u32> {
        let Some(player) = self.game.world.entities.player() else { return vec![]; };
        let cell = player.movement.cell_position();
        let mut candidates: Vec<_> = self.game.world.entities.iter()
            .filter(|e| e.entity_type == EntityType::Monster && !e.is_pet && e.is_alive() && !e.is_fading())
            .map(|e| (cell_distance(cell, e.movement.cell_position()), e.id))
            .filter(|(distance, _)| *distance <= REACH_CELLS)
            .collect();
        candidates.sort_unstable();
        candidates.into_iter().map(|(_, id)| id).collect()
    }

    fn prune_controller_target(&mut self) {
        if self.game.session.app_state != AppState::InGame || self.game.session.player_dead {
            self.game.combat.selected_target_id = None;
            self.gamepad.queued_pad_skill = None;
            return;
        }
        if self.game.combat.selected_target_id.is_some_and(|id| !self.controller_targets().contains(&id)) {
            self.game.combat.selected_target_id = None;
        }
    }

    fn cycle_controller_target(&mut self, direction: i32) {
        let candidates = self.controller_targets();
        let current = self.game.combat.selected_target_id.or(self.game.combat.attack_target_id);
        let next = cycle_target(&candidates, current, direction);
        let attacking = self.game.combat.attack_target_id.is_some();
        self.game.combat.selected_target_id = next;
        if attacking && next != current {
            self.stop_attacking();
            if let Some(id) = next { self.initiate_attack(id); }
        }
    }

    fn resolve_controller_skill(&mut self) {
        let Some((requested, requested_level)) = self.gamepad.queued_pad_skill.take() else { return; };
        if !self.controller_world_input() { return; }
        let Some(PendingSkillTarget::Entity { skill, level }) = self.game.pending_casts.pending_skill_target else { return; };
        if skill != requested || level != requested_level || self.skill_on_cooldown(skill)
            || !matches!(self.game.resolve_cast_skill(skill), Some((SkillTargetType::Target, _))) { return; }
        let candidates = self.controller_targets();
        let chosen = self.game.combat.selected_target_id
            .or(self.game.combat.attack_target_id).filter(|id| candidates.contains(id))
            .or_else(|| candidates.first().copied());
        let Some(id) = chosen else { return; };
        self.game.combat.selected_target_id = Some(id);
        self.game.pending_casts.pending_skill_target = None;
        self.stop_attacking();
        if self.cast_skill_on_unit(skill, level, id) {
            self.game.pending_casts.pending_skill = None;
            self.game.pending_casts.pending_skill_level = None;
        } else if self.game.combat.attack_target_id.is_none() {
            // An unreachable mob must leave a cancellable selector, not a stuck approach.
            self.game.pending_casts.pending_skill_target = Some(PendingSkillTarget::Entity {skill,level});
        }
    }

    fn attack_closest_monster(&mut self) {
        if let Some(id) = self.game.combat.selected_target_id.or(self.game.combat.attack_target_id)
            .filter(|id| self.controller_targets().contains(id)) {
            self.game.combat.selected_target_id = Some(id);
            self.initiate_attack(id);
            return;
        }
        let Some(player) = self.game.world.entities.player() else {
            return;
        };
        let (px, py) = player.movement.cell_position();
        let closest = self
            .game
            .world
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Monster && !e.is_pet && e.is_alive())
            .map(|e| (e.id, cell_distance((px, py), e.movement.cell_position())))
            .filter(|&(_, d)| d <= REACH_CELLS)
            .min_by_key(|&(_, d)| d)
            .map(|(id, _)| id);
        if let Some(id) = closest {
            self.game.combat.selected_target_id = Some(id);
            self.initiate_attack(id);
        }
    }

    /// False when no item lies within reach
    fn pick_up_closest_item(&mut self) -> bool {
        let Some(player) = self.game.world.entities.player() else {
            return false;
        };
        let (px, py) = player.movement.cell_position();
        let closest = self
            .game
            .world
            .floor_items
            .values()
            .map(|item| {
                (
                    item.id,
                    cell_distance((px, py), (item.x as u16, item.y as u16)),
                )
            })
            .filter(|&(_, d)| d <= REACH_CELLS)
            .min_by_key(|&(_, d)| d)
            .map(|(id, _)| id);
        let Some(id) = closest else {
            return false;
        };
        self.game.combat.attack_target_id = None;
        self.game.pending_casts.pending_pickup_item_id = Some(id);
        self.check_pending_pickup();
        true
    }

    /// The NPC the player can talk to now (`nearest_talkable_npc`): its id and name
    pub(crate) fn talkable_npc(&self) -> Option<(u32, Option<&str>)> {
        let entities = &self.game.world.entities;
        let from = entities.player()?.movement.cell_position();
        entities
            .nearest_talkable_npc(from, TALK_RANGE)
            .map(|npc| (npc.id, npc.name.as_deref()))
    }

    /// Talks to the nearest NPC in reach, as a click on it does; false when none is
    pub(crate) fn talk_to_nearest_npc(&mut self) -> bool {
        let Some((id, _)) = self.talkable_npc() else {
            return false;
        };
        self.stop_attacking();
        self.channel
            .send_packet(nostalre_network::sender::build_contact_npc_packet(
                id,
                self.active_packetver,
            ));
        true
    }

    fn use_hotbar_slot(&mut self, slot: usize) {
        let mut events = Vec::new();
        self.windows.hotkey_bar.execute_slot(
            slot,
            &self.game.character,
            self.game.data_table.skill_use_level.as_ref(),
            &mut events,
        );
        self.gamepad.queued_pad_skill = events.iter().find_map(|event| match event {
            nostalre_game::event::GameEvent::RequestUseSkill { skill, level } => Some((*skill, *level)),
            _ => None,
        });
        self.pending_events.extend(events);
    }
}

/// Both triggers initially select the nearest; subsequent presses wrap in opposite directions.
fn cycle_target(candidates: &[u32], current: Option<u32>, direction: i32) -> Option<u32> {
    if candidates.is_empty() { return None; }
    let Some(index) = current.and_then(|id| candidates.iter().position(|&candidate| candidate == id)) else {
        return candidates.first().copied();
    };
    Some(candidates[(index as i32 + direction).rem_euclid(candidates.len() as i32) as usize])
}

/// Chebyshev distance in cells, how RO measures range.
fn cell_distance(a: (u16, u16), b: (u16, u16)) -> i32 {
    (a.0 as i32 - b.0 as i32)
        .abs()
        .max((a.1 as i32 - b.1 as i32).abs())
}

/// The device the pad's mouse events claim to come from.
fn pad() -> DeviceId {
    DeviceId::dummy()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_sets_of_twelve_cover_the_four_rows() {
        assert_eq!(hotbar_slot(Group::L1, Button::Y, 0), Some(0));
        assert_eq!(hotbar_slot(Group::L1R1, Button::Y, 0), Some(4));
        assert_eq!(hotbar_slot(Group::R1, Button::A, 0), Some(11));
        assert_eq!(hotbar_slot(Group::L1, Button::X, 1), Some(13));
        assert_eq!(hotbar_slot(Group::R1, Button::A, 2), Some(35));
        assert_eq!(hotbar_slot(Group::L1, Button::Start, 0), None);
    }

    #[test]
    fn every_pad_combination_reaches_exactly_one_of_all_36_slots() {
        let mut slots = std::collections::BTreeSet::new();
        for set in 0..PAD_SETS {
            for group in [Group::L1, Group::L1R1, Group::R1] {
                for face in [Button::Y, Button::X, Button::B, Button::A] {
                    assert!(slots.insert(hotbar_slot(group, face, set).unwrap()));
                }
            }
        }
        assert_eq!(
            slots.into_iter().collect::<Vec<_>>(),
            (0..36).collect::<Vec<_>>()
        );
    }

    #[test]
    fn repeated_button_down_queues_one_action_until_released() {
        let mut pad = Gamepad::default();
        for _ in 0..120 {
            pad.set_held(Button::X, true);
        }
        assert_eq!(pad.fresh, [Button::X]);
        pad.fresh.clear();
        pad.set_held(Button::X, false);
        pad.set_held(Button::X, true);
        assert_eq!(pad.fresh, [Button::X]);
        assert_eq!(pad.released, [Button::X]);
    }

    #[test]
    fn triggers_are_no_hotbar_group() {
        let mut pad = Gamepad::default();
        pad.press(Button::L2);
        pad.fresh.clear();
        assert_eq!(pad.group(), None);
    }

    #[test]
    fn a_trigger_turns_by_how_far_it_is_pressed() {
        let mut pad = Gamepad::default();
        assert_eq!(pad.trigger_turn(), 0.0);
        pad.axis(axis::RTRIGGER, 0.5);
        assert!((pad.trigger_turn() - 0.25).abs() < 1e-6);
        pad.axis(axis::LTRIGGER, 1.0);
        assert!((pad.trigger_turn() + 0.75).abs() < 1e-6);
        // a trigger that only sends a button is all the way in
        let mut digital = Gamepad::default();
        digital.press(Button::L2);
        assert_eq!(digital.trigger_turn(), -1.0);
    }

    #[test]
    fn a_shoulder_pressed_with_the_face_button_is_a_modifier() {
        let mut pad = Gamepad::default();
        pad.press(Button::L1);
        assert_eq!(pad.group(), Some(Group::L1));
        pad.fresh.clear();
        assert_eq!(pad.group(), Some(Group::L1));
        pad.press(Button::R1);
        pad.fresh.clear();
        assert_eq!(pad.group(), Some(Group::L1R1));
    }

    #[test]
    fn target_cycle_starts_nearby_wraps_and_handles_disappearing_target() {
        let candidates = [20, 7, 99];
        assert_eq!(cycle_target(&[], Some(7), 1), None);
        assert_eq!(cycle_target(&candidates, None, 1), Some(20));
        assert_eq!(cycle_target(&candidates, None, -1), Some(20));
        assert_eq!(cycle_target(&candidates, Some(20), 1), Some(7));
        assert_eq!(cycle_target(&candidates, Some(20), -1), Some(99));
        assert_eq!(cycle_target(&candidates, Some(99), 1), Some(20));
        assert_eq!(cycle_target(&candidates, Some(500), -1), Some(20));
        assert_eq!(cycle_target(&[20], Some(20), -1), Some(20));
    }

    #[test]
    fn an_arrow_released_after_select_still_releases() {
        let mut pad = Gamepad::default();
        assert_eq!(pad.arrow(KeyCode::ArrowUp, ElementState::Pressed), None);
        pad.press(Button::Select);
        let up = pad.arrow(KeyCode::ArrowUp, ElementState::Pressed);
        assert_eq!(up, Some(Button::Up));
        pad.press(Button::Up);
        pad.release(Button::Select);
        assert_eq!(
            pad.arrow(KeyCode::ArrowUp, ElementState::Released),
            Some(Button::Up)
        );
        pad.release(Button::Up);
        assert_eq!(pad.arrow(KeyCode::ArrowUp, ElementState::Released), None);
    }

    #[test]
    fn select_let_go_within_the_frame_still_counts() {
        let mut pad = Gamepad::default();
        assert!(!pad.select_down());
        pad.press(Button::Select);
        pad.press(Button::Down);
        pad.release(Button::Select);
        pad.release(Button::Down);
        assert!(pad.select_down());
    }

    #[test]
    fn the_hat_is_the_dpad() {
        let mut pad = Gamepad::default();
        pad.axis(axis::HAT_X, -1.0);
        assert_eq!(pad.fresh, [Button::Left]);
        pad.axis(axis::HAT_X, 0.0);
        assert_eq!(pad.released, [Button::Left]);
    }

    #[test]
    fn analog_triggers_are_l2_r2() {
        let mut pad = Gamepad::default();
        pad.axis(axis::GAS, 0.9);
        assert!(pad.held.contains(&Button::R2));
        pad.axis(axis::GAS, 0.1);
        assert!(!pad.held.contains(&Button::R2));
    }

    #[test]
    fn walk_aims_up_the_screen_at_full_tilt() {
        assert_eq!(walk_target((100, 100), (0.0, -1.0), 0.0), (100, 103));
        assert_eq!(walk_target((100, 100), (1.0, 0.0), 0.0), (103, 100));
        assert_eq!(walk_target((100, 100), (0.0, -0.5), 0.0), (100, 102));
    }

    #[test]
    fn deadzone_is_rest() {
        assert_eq!(dead(0.05), 0.0);
        assert_eq!(dead(-0.3), -0.3);
    }
}

#[cfg(test)]
mod menu_navigation_tests {
    use super::*;

    #[test]
    fn stick_navigation_repeats_after_delay_and_resets_on_close() {
        let now = Instant::now();
        let mut pad = Gamepad::default();
        pad.axis(axis::X, 0.8);
        assert_eq!(pad.menu_step(now, true), Some(Button::Right));
        assert_eq!(pad.menu_step(now + Duration::from_millis(50), true), None);
        assert_eq!(
            pad.menu_step(now + Duration::from_millis(300), true),
            Some(Button::Right)
        );
        assert_eq!(pad.menu_step(now + Duration::from_millis(330), true), None);
        assert_eq!(pad.menu_step(now, false), None);
        assert_eq!(pad.menu_step(now, true), Some(Button::Right));
        pad.axis(axis::X, 0.4);
        assert_eq!(pad.menu_step(now, true), None);
    }

    #[test]
    fn dpad_direction_takes_precedence_over_stick_and_changes_immediately() {
        let now = Instant::now();
        let mut pad = Gamepad::default();
        pad.axis(axis::X, 1.0);
        pad.set_held(Button::Down, true);
        assert_eq!(pad.menu_step(now, true), Some(Button::Down));
        pad.set_held(Button::Down, false);
        assert_eq!(pad.menu_step(now, true), Some(Button::Right));
    }
}
