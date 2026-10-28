use crate::key_layout::KeyLabels;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, Touch, TouchPhase, WindowEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

pub use winit::keyboard::KeyCode as PhysicalKeyCode;

pub const DOUBLE_CLICK_THRESHOLD_MS: u128 = 400;
pub const DOUBLE_CLICK_DISTANCE: f32 = 5.0;

/// A finger on a touch screen, in UI units: the on-screen controls read these, several at once
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Finger {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    /// Landed since the last frame
    pub landed: bool,
    /// Lifted since the last frame: still seen this frame, so a quick tap is not lost
    pub lifted: bool,
}

pub struct UiContext {
    pub screen_width: f32,
    pub screen_height: f32,
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_clicked: bool,
    /// A left press happened this frame, whoever acts on it. Never consumed,
    /// unlike [`Self::mouse_clicked`], so a press the world took still dismisses
    /// an open popup.
    pub mouse_pressed: bool,
    pub mouse_double_clicked: bool,
    pub mouse_down: bool,
    pub mouse_right_clicked: bool,
    pub typed_chars: Vec<char>,
    pub pressed_codes: Vec<KeyCode>,
    pub key_labels: KeyLabels,
    last_click_time: std::time::Instant,
    last_click_pos: (f32, f32),
    pub key_backspace: bool,
    pub key_enter: bool,
    pub key_tab: bool,
    pub key_escape: bool,
    pub key_left: bool,
    pub key_right: bool,
    pub key_delete: bool,
    pub key_up: bool,
    pub key_down: bool,
    pub key_f1: bool,
    pub key_f2: bool,
    pub key_f3: bool,
    pub key_f4: bool,
    pub key_f5: bool,
    pub key_f6: bool,
    pub key_f7: bool,
    pub key_f8: bool,
    pub key_f9: bool,
    pub key_f10: bool,
    pub key_f12: bool,
    pub ctrl_pressed: bool,
    pub shift_pressed: bool,
    pub alt_pressed: bool,
    pub scroll_delta: f32,
    pub dpi_scale: f32,
    pub now_ms: u64,
    /// When set, `UiFrame::window_account` locks account-flow windows to a fixed
    /// centered position instead of the draggable default. The game turns this on;
    /// the component dev tool leaves it off so those windows stay draggable.
    pub lock_account_windows: bool,
    /// The last key came from a controller: hints name its buttons (A, B, L1...) instead of the keyboard's
    pub pad_labels: bool,
    /// The fingers on the screen
    pub fingers: Vec<Finger>,
}

impl UiContext {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            screen_width,
            screen_height,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_clicked: false,
            mouse_pressed: false,
            mouse_double_clicked: false,
            mouse_down: false,
            mouse_right_clicked: false,
            typed_chars: Vec::new(),
            pressed_codes: Vec::new(),
            key_labels: KeyLabels::resolve(),
            last_click_time: std::time::Instant::now(),
            last_click_pos: (0.0, 0.0),
            now_ms: 0,
            key_backspace: false,
            key_enter: false,
            key_tab: false,
            key_escape: false,
            key_left: false,
            key_right: false,
            key_delete: false,
            key_up: false,
            key_down: false,
            key_f1: false,
            key_f2: false,
            key_f3: false,
            key_f4: false,
            key_f5: false,
            key_f6: false,
            key_f7: false,
            key_f8: false,
            key_f9: false,
            key_f10: false,
            key_f12: false,
            ctrl_pressed: false,
            shift_pressed: false,
            alt_pressed: false,
            scroll_delta: 0.0,
            dpi_scale: 1.0,
            lock_account_windows: false,
            pad_labels: false,
            fingers: Vec::new(),
        }
    }

    /// A finger landed, moved or lifted; returns where, in UI units
    pub fn handle_touch(&mut self, touch: &Touch) -> (f32, f32) {
        let (x, y) = (
            touch.location.x as f32 / self.dpi_scale,
            touch.location.y as f32 / self.dpi_scale,
        );
        let landed = Finger {
            id: touch.id,
            x,
            y,
            landed: true,
            lifted: false,
        };
        let known = self.fingers.iter_mut().find(|f| f.id == touch.id);
        match (touch.phase, known) {
            (TouchPhase::Started, None) => self.fingers.push(landed),
            // the id came back within the frame: a new finger, not the lifted one
            (TouchPhase::Started, Some(f)) => *f = landed,
            (TouchPhase::Moved, Some(f)) => (f.x, f.y) = (x, y),
            (TouchPhase::Ended | TouchPhase::Cancelled, Some(f)) => {
                (f.x, f.y) = (x, y);
                f.lifted = true;
            }
            _ => {}
        }
        (x, y)
    }

    /// The window lost focus or the app was put away: no finger stays down
    pub fn clear_fingers(&mut self) {
        self.fingers.clear();
    }

    /// A key press was taken by something outside the UI (a key being rebound): the rest of the frame sees no
    /// key, no character
    pub fn forget_key_press(&mut self) {
        self.typed_chars.clear();
        self.pressed_codes.clear();
        self.key_backspace = false;
        self.key_enter = false;
        self.key_tab = false;
        self.key_escape = false;
        self.key_left = false;
        self.key_right = false;
        self.key_delete = false;
        self.key_up = false;
        self.key_down = false;
    }

    pub fn begin_frame(&mut self) {
        self.fingers.retain(|f| !f.lifted);
        for finger in &mut self.fingers {
            finger.landed = false;
        }
        self.mouse_clicked = false;
        self.mouse_pressed = false;
        self.mouse_double_clicked = false;
        self.mouse_right_clicked = false;
        self.typed_chars.clear();
        self.pressed_codes.clear();
        self.key_backspace = false;
        self.key_enter = false;
        self.key_tab = false;
        self.key_escape = false;
        self.key_left = false;
        self.key_right = false;
        self.key_delete = false;
        self.key_up = false;
        self.key_down = false;
        self.key_f1 = false;
        self.key_f2 = false;
        self.key_f3 = false;
        self.key_f4 = false;
        self.key_f5 = false;
        self.key_f6 = false;
        self.key_f7 = false;
        self.key_f8 = false;
        self.key_f9 = false;
        self.key_f10 = false;
        self.key_f12 = false;
        self.scroll_delta = 0.0;
    }

    fn is_paste_chord(&self, event: &winit::event::KeyEvent) -> bool {
        let ctrl_v =
            self.ctrl_pressed && matches!(event.physical_key, PhysicalKey::Code(KeyCode::KeyV));
        let shift_insert =
            self.shift_pressed && matches!(event.physical_key, PhysicalKey::Code(KeyCode::Insert));
        ctrl_v || shift_insert
    }

    #[cfg(any(target_os = "android", target_os = "horizon"))]
    fn paste_from_clipboard(&mut self) {}

    #[cfg(not(any(target_os = "android", target_os = "horizon")))]
    fn paste_from_clipboard(&mut self) {
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            self.inject_pasted_text(&text);
        }
    }

    pub fn inject_pasted_text(&mut self, text: &str) {
        for ch in text.chars() {
            if !ch.is_control() {
                self.typed_chars.push(ch);
            }
        }
    }

    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32 / self.dpi_scale;
                self.mouse_y = position.y as f32 / self.dpi_scale;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if *button == MouseButton::Left {
                    match state {
                        ElementState::Pressed => {
                            let now = std::time::Instant::now();
                            let elapsed = now.duration_since(self.last_click_time).as_millis();
                            let dx = self.mouse_x - self.last_click_pos.0;
                            let dy = self.mouse_y - self.last_click_pos.1;
                            let dist = (dx * dx + dy * dy).sqrt();

                            self.mouse_clicked = true;
                            self.mouse_pressed = true;
                            self.mouse_down = true;

                            if elapsed < DOUBLE_CLICK_THRESHOLD_MS && dist < DOUBLE_CLICK_DISTANCE {
                                self.mouse_double_clicked = true;
                                // Reset so a third click doesn't count as another double
                                self.last_click_time = now - std::time::Duration::from_secs(1);
                            } else {
                                self.last_click_time = now;
                            }
                            self.last_click_pos = (self.mouse_x, self.mouse_y);
                        }
                        ElementState::Released => {
                            self.mouse_down = false;
                        }
                    }
                }
                if *button == MouseButton::Right && *state == ElementState::Pressed {
                    self.mouse_right_clicked = true;
                }
            }
            // a modifier let go while another window has focus never reports its release
            WindowEvent::Focused(false) => {
                self.fingers.clear();
                self.ctrl_pressed = false;
                self.shift_pressed = false;
                self.alt_pressed = false;
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.ctrl_pressed = modifiers.state().control_key();
                self.shift_pressed = modifiers.state().shift_key();
                self.alt_pressed = modifiers.state().alt_key();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    if self.is_paste_chord(event) {
                        self.paste_from_clipboard();
                        return;
                    }
                    // Function keys are matched on the physical scancode, not the
                    // logical key: some keyboards/layouts deliver an Fn-remapped
                    // logical key for the upper F-row, which a NamedKey match
                    // silently misses.
                    if let PhysicalKey::Code(code) = event.physical_key {
                        self.pressed_codes.push(code);
                        match code {
                            KeyCode::F1 => self.key_f1 = true,
                            KeyCode::F2 => self.key_f2 = true,
                            KeyCode::F3 => self.key_f3 = true,
                            KeyCode::F4 => self.key_f4 = true,
                            KeyCode::F5 => self.key_f5 = true,
                            KeyCode::F6 => self.key_f6 = true,
                            KeyCode::F7 => self.key_f7 = true,
                            KeyCode::F8 => self.key_f8 = true,
                            KeyCode::F9 => self.key_f9 = true,
                            KeyCode::F10 => self.key_f10 = true,
                            KeyCode::F12 => self.key_f12 = true,
                            _ => {}
                        }
                    }
                    match &event.logical_key {
                        Key::Named(NamedKey::Backspace) => self.key_backspace = true,
                        Key::Named(NamedKey::Enter) => self.key_enter = true,
                        Key::Named(NamedKey::Tab) => self.key_tab = true,
                        Key::Named(NamedKey::Escape) => self.key_escape = true,
                        Key::Named(NamedKey::ArrowLeft) => self.key_left = true,
                        Key::Named(NamedKey::ArrowRight) => self.key_right = true,
                        Key::Named(NamedKey::Delete) => self.key_delete = true,
                        Key::Named(NamedKey::ArrowUp) => self.key_up = true,
                        Key::Named(NamedKey::ArrowDown) => self.key_down = true,
                        Key::Named(NamedKey::Space) => self.typed_chars.push(' '),
                        // Android maps Enter to a newline character, not the named key
                        Key::Character(character) if matches!(character.as_str(), "\n" | "\r") => {
                            self.key_enter = true
                        }
                        Key::Character(character) => {
                            // Android's winit leaves `text` empty: the key's character is what was typed
                            if let Some(text) = event.text.as_ref().or(Some(character)) {
                                for ch in text.chars() {
                                    if !ch.is_control() {
                                        self.typed_chars.push(ch);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll_delta += match delta {
                    MouseScrollDelta::LineDelta(_, y) => *y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 40.0,
                };
            }
            WindowEvent::Resized(size) => {
                self.screen_width = size.width as f32 / self.dpi_scale;
                self.screen_height = size.height as f32 / self.dpi_scale;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text_input::TextInput;

    fn touch(id: u64, phase: TouchPhase, x: f64) -> Touch {
        Touch {
            device_id: winit::event::DeviceId::dummy(),
            phase,
            location: winit::dpi::PhysicalPosition::new(x, 10.0),
            force: None,
            id,
        }
    }

    #[test]
    fn a_tap_within_one_frame_is_seen_that_frame() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.dpi_scale = 2.0;
        assert_eq!(
            ctx.handle_touch(&touch(1, TouchPhase::Started, 40.0)),
            (20.0, 5.0)
        );
        ctx.handle_touch(&touch(1, TouchPhase::Ended, 40.0));
        assert!(matches!(ctx.fingers.as_slice(), [f] if f.landed && f.lifted));
        ctx.begin_frame();
        assert!(ctx.fingers.is_empty());
    }

    #[test]
    fn a_finger_stays_down_until_lifted_and_landed_once() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.handle_touch(&touch(1, TouchPhase::Started, 4.0));
        ctx.begin_frame();
        ctx.handle_touch(&touch(1, TouchPhase::Moved, 9.0));
        assert!(matches!(ctx.fingers.as_slice(), [f] if !f.landed && f.x == 9.0));
        ctx.handle_event(&WindowEvent::Focused(false));
        assert!(ctx.fingers.is_empty());
    }

    #[test]
    fn an_id_back_within_the_frame_is_a_new_finger() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.handle_touch(&touch(1, TouchPhase::Started, 4.0));
        ctx.handle_touch(&touch(1, TouchPhase::Ended, 4.0));
        ctx.handle_touch(&touch(1, TouchPhase::Started, 7.0));
        ctx.begin_frame();
        assert!(matches!(ctx.fingers.as_slice(), [f] if f.x == 7.0 && !f.lifted));
    }

    #[test]
    fn a_forgotten_press_reaches_nothing() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.key_delete = true;
        ctx.key_tab = true;
        ctx.pressed_codes.push(KeyCode::PageDown);
        ctx.typed_chars.push('a');
        ctx.forget_key_press();
        assert!(!ctx.key_delete && !ctx.key_tab);
        assert!(ctx.pressed_codes.is_empty() && ctx.typed_chars.is_empty());
    }

    #[test]
    fn losing_focus_lets_go_of_the_modifiers() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.ctrl_pressed = true;
        ctx.alt_pressed = true;
        ctx.handle_event(&WindowEvent::Focused(false));
        assert!(!ctx.ctrl_pressed && !ctx.alt_pressed && !ctx.shift_pressed);
    }

    #[test]
    fn pasted_text_fills_focused_input_and_drops_newlines() {
        let mut ctx = UiContext::new(800.0, 600.0);
        ctx.inject_pasted_text("hello\nworld");

        let mut input = TextInput::new(100, false);
        input.process_keys(&ctx);

        assert_eq!(input.text, "helloworld");
        assert_eq!(input.cursor_pos, 10);
    }
}
