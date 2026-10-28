//! Touch screens (Android): one finger drives the mouse the client is built around. Down presses the left
//! button where the finger lands, moving drags the cursor, lifting releases. Other fingers, and a finger that
//! lands on an on-screen control (the HUD's stick and buttons read the fingers themselves), are not the mouse.
//! The press waits one frame after the cursor jumps there: the UI must see the finger over a button (the
//! HUD's menu line) before the press, or the world takes the tap as a walk. What the finger does meanwhile
//! waits behind the press, in order, so the press lands where the finger did.
//! Android's Back key stands in for Escape.

use winit::event::{ElementState, MouseButton, Touch, TouchPhase, WindowEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

/// Android's Back (button or gesture) as Escape: it closes windows and opens the game menu, like the original
/// client's Escape. Other events pass through.
pub(crate) fn back_as_escape(event: WindowEvent) -> WindowEvent {
    match event {
        WindowEvent::KeyboardInput {
            device_id,
            mut event,
            is_synthetic,
        } if event.logical_key == Key::Named(NamedKey::BrowserBack) => {
            event.logical_key = Key::Named(NamedKey::Escape);
            event.physical_key = PhysicalKey::Code(KeyCode::Escape);
            WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            }
        }
        other => other,
    }
}

#[derive(Default)]
pub(crate) struct TouchMouse {
    finger: Option<u64>,
    /// The press (and a release right behind it) of a finger that just landed, held for a frame
    waiting: Vec<WindowEvent>,
    /// What waited a frame: sent at the next frame's start
    ready: Vec<WindowEvent>,
}

impl TouchMouse {
    /// `on_control`: the finger landed on an on-screen control
    pub(crate) fn translate(&mut self, touch: Touch, on_control: bool) -> Vec<WindowEvent> {
        let Touch {
            device_id,
            phase,
            location: position,
            id,
            ..
        } = touch;
        let moved = WindowEvent::CursorMoved {
            device_id,
            position,
        };
        let button = |state| WindowEvent::MouseInput {
            device_id,
            state,
            button: MouseButton::Left,
        };
        match phase {
            TouchPhase::Started if self.finger.is_none() && !on_control => {
                self.finger = Some(id);
                self.waiting.push(button(ElementState::Pressed));
                vec![moved]
            }
            TouchPhase::Moved if self.finger == Some(id) => self.behind_press(vec![moved]),
            TouchPhase::Ended | TouchPhase::Cancelled if self.finger == Some(id) => {
                self.finger = None;
                self.behind_press(vec![moved, button(ElementState::Released)])
            }
            _ => Vec::new(),
        }
    }

    /// `events` now, or queued behind a press still waiting
    fn behind_press(&mut self, events: Vec<WindowEvent>) -> Vec<WindowEvent> {
        if self.waiting.is_empty() && self.ready.is_empty() {
            events
        } else {
            self.waiting.extend(events);
            Vec::new()
        }
    }

    /// At a frame's start: the presses that have waited a frame
    pub(crate) fn frame(&mut self) -> Vec<WindowEvent> {
        let due = std::mem::take(&mut self.ready);
        self.ready = std::mem::take(&mut self.waiting);
        due
    }
}

#[cfg(test)]
mod tests {
    use super::TouchMouse;
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, ElementState, Force, Touch, TouchPhase, WindowEvent};

    fn touch(id: u64, phase: TouchPhase, x: f64) -> Touch {
        let device_id = DeviceId::dummy();
        Touch {
            device_id,
            phase,
            location: PhysicalPosition::new(x, 10.0),
            force: None::<Force>,
            id,
        }
    }

    fn summary(events: &[WindowEvent]) -> Vec<String> {
        events
            .iter()
            .map(|e| match e {
                WindowEvent::CursorMoved { position, .. } => format!("move {}", position.x),
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    ..
                } => "press".into(),
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    ..
                } => "release".into(),
                other => format!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn one_finger_is_the_left_button_pressed_a_frame_after_it_lands() {
        let mut mouse = TouchMouse::default();
        assert_eq!(
            summary(&mouse.translate(touch(1, TouchPhase::Started, 5.0), false)),
            ["move 5"]
        );
        // the frame that sees the cursor there, then the press
        assert!(mouse.frame().is_empty());
        assert_eq!(summary(&mouse.frame()), ["press"]);
        assert_eq!(
            summary(&mouse.translate(touch(1, TouchPhase::Moved, 7.0), false)),
            ["move 7"]
        );
        assert_eq!(
            summary(&mouse.translate(touch(1, TouchPhase::Ended, 8.0), false)),
            ["move 8", "release"]
        );
    }

    #[test]
    fn a_quick_tap_releases_after_its_press() {
        let mut mouse = TouchMouse::default();
        mouse.translate(touch(1, TouchPhase::Started, 5.0), false);
        assert!(
            mouse
                .translate(touch(1, TouchPhase::Ended, 5.0), false)
                .is_empty()
        );
        assert!(mouse.frame().is_empty());
        assert_eq!(summary(&mouse.frame()), ["press", "move 5", "release"]);
    }

    #[test]
    fn a_finger_moving_before_its_press_presses_where_it_landed() {
        let mut mouse = TouchMouse::default();
        assert_eq!(
            summary(&mouse.translate(touch(1, TouchPhase::Started, 5.0), false)),
            ["move 5"]
        );
        assert!(
            mouse
                .translate(touch(1, TouchPhase::Moved, 9.0), false)
                .is_empty()
        );
        assert!(mouse.frame().is_empty());
        mouse.translate(touch(1, TouchPhase::Moved, 12.0), false);
        // the press, then the moves in order
        assert_eq!(summary(&mouse.frame()), ["press", "move 9"]);
        assert_eq!(summary(&mouse.frame()), ["move 12"]);
        assert_eq!(
            summary(&mouse.translate(touch(1, TouchPhase::Moved, 14.0), false)),
            ["move 14"]
        );
    }

    #[test]
    fn a_finger_on_a_control_is_not_the_mouse() {
        let mut mouse = TouchMouse::default();
        assert!(
            mouse
                .translate(touch(1, TouchPhase::Started, 5.0), true)
                .is_empty()
        );
        assert!(
            mouse
                .translate(touch(1, TouchPhase::Moved, 9.0), false)
                .is_empty()
        );
        // a finger elsewhere meanwhile still is
        assert!(
            !mouse
                .translate(touch(2, TouchPhase::Started, 50.0), false)
                .is_empty()
        );
        assert!(
            mouse
                .translate(touch(1, TouchPhase::Ended, 9.0), false)
                .is_empty()
        );
    }

    #[test]
    fn a_second_finger_is_ignored() {
        let mut mouse = TouchMouse::default();
        mouse.translate(touch(1, TouchPhase::Started, 5.0), false);
        mouse.frame();
        mouse.frame();
        assert!(
            mouse
                .translate(touch(2, TouchPhase::Started, 50.0), false)
                .is_empty()
        );
        assert!(
            mouse
                .translate(touch(2, TouchPhase::Ended, 50.0), false)
                .is_empty()
        );
        assert_eq!(
            summary(&mouse.translate(touch(1, TouchPhase::Ended, 6.0), false)),
            ["move 6", "release"]
        );
    }
}
