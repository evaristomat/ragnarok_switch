//! Single-window libnx event loop for the nostalRE client.
use std::cell::Cell;
use std::marker::PhantomData;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
#[path = "frame_pacer.rs"] mod frame_pacer;
use frame_pacer::FramePacer;

use crate::application::ApplicationHandler;
use crate::dpi::PhysicalPosition;
use crate::error::{EventLoopError, OsError};
use crate::event::{DeviceEvent, DeviceId, ElementState, KeyEvent, StartCause, Touch, TouchPhase, WindowEvent};
use crate::keyboard::{Key, KeyLocation, NativeKey, NativeKeyCode, PhysicalKey};
use crate::window::{Window, WindowAttributes, WindowId, REDRAW};

unsafe extern "C" {
    fn appletMainLoop() -> bool;
    fn appletSetFocusHandlingMode(mode: u32) -> u32;
    fn appletGetFocusState() -> u32;
    fn nostalre_hid_init();
    fn nostalre_hid_poll(buttons: *mut u64, sticks: *mut i32);
    fn nostalre_touch_poll(points: *mut u32) -> u32;
    fn nostalre_keyboard(text: *mut std::ffi::c_char, capacity: usize) -> u32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AsyncRequestSerial(pub u64);
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControlFlow { Poll, #[default] Wait, WaitUntil(Instant) }
pub struct ActiveEventLoop { exit: Cell<bool>, flow: Cell<ControlFlow> }
pub struct EventLoop<T: 'static> { active: ActiveEventLoop, marker: PhantomData<T> }

impl ActiveEventLoop {
    pub fn create_window(&self, _: WindowAttributes) -> Result<Window, OsError> { Window::new() }
    pub fn exit(&self) { self.exit.set(true); }
    pub fn exiting(&self) -> bool { self.exit.get() }
    pub fn set_control_flow(&self, flow: ControlFlow) { self.flow.set(flow); }
    pub fn control_flow(&self) -> ControlFlow { self.flow.get() }
}

fn controller_key(code: u32, pressed: bool) -> WindowEvent {
    WindowEvent::KeyboardInput {
        device_id: DeviceId::dummy(), is_synthetic: false,
        event: KeyEvent {
            physical_key: PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
            logical_key: Key::Unidentified(NativeKey::Android(code)), text: None,
            location: KeyLocation::Standard,
            state: if pressed { ElementState::Pressed } else { ElementState::Released },
            repeat: false, platform_specific: Default::default(),
        },
    }
}

impl EventLoop<()> {
    pub fn new() -> Result<Self, EventLoopError> {
        bevy_horizon::init_switch_logging("nostalre");
        bevy_horizon::set_thread_core_policy(bevy_horizon::ThreadCorePolicy::AnyCore);
        bevy_horizon::lifecycle::init_horizon_services();
        // libnx automatically suspends the entire app on HOME/sleep. Bevy's manual
        // suspend policy is unnecessary for this standalone event loop.
        unsafe { appletSetFocusHandlingMode(0); nostalre_hid_init(); }
        Ok(Self { active: ActiveEventLoop { exit: Cell::new(false), flow: Cell::new(ControlFlow::Poll) }, marker: PhantomData })
    }
}

impl<T: 'static> EventLoop<T> {
    pub fn run_app<A: ApplicationHandler<T>>(self, app: &mut A) -> Result<(), EventLoopError> {
        let event_loop = &self.active;
        let id = WindowId(1);
        let device = DeviceId::dummy();
        let mut last_buttons = 0u64;
        let mut last_sticks = [0i32; 4];
        let mut touches = [(0u32, (0u32, 0u32)); 16];
        let mut touch_count = 0usize;
        let mut focused = true;
        let buttons = [(0,96),(1,97),(2,99),(3,100),(5,107),(6,102),(7,103),
            (8,104),(9,105),(10,108),(11,109)];
        app.new_events(event_loop, StartCause::Init);
        app.resumed(event_loop);
        let mut pacer = FramePacer::new(Instant::now());
        let mut last_log_sync = Instant::now();
        while !event_loop.exiting() && unsafe { appletMainLoop() } {
            let new_focus = unsafe { appletGetFocusState() } == 1;
            if focused != new_focus {
                focused = new_focus;
                app.window_event(event_loop, id, WindowEvent::Focused(focused));
            }
            if !focused { std::thread::sleep(Duration::from_millis(20)); continue; }
            let mut held = 0u64;
            let mut sticks = [0i32; 4];
            unsafe { nostalre_hid_poll(&mut held, sticks.as_mut_ptr()); }
            for &(bit, code) in &buttons {
                if (held ^ last_buttons) & (1 << bit) != 0 {
                    app.window_event(event_loop, id, controller_key(code, held & (1 << bit) != 0));
                }
            }
            // Existing controller logic uses Android's axis numbering. Reuse it,
            // reversing Y because libnx's positive Y points up.
            for (i, axis) in [0,1,11,14].into_iter().enumerate() {
                if sticks[i] != last_sticks[i] {
                    let sign = if i % 2 == 1 { -1.0 } else { 1.0 };
                    app.device_event(event_loop, device, DeviceEvent::Motion {
                        axis, value: sign * f64::from(sticks[i]) / 32767.0,
                    });
                }
            }
            if (held ^ last_buttons) & (0xF << 12) != 0 {
                let x = i32::from(held & (1 << 14) != 0) - i32::from(held & (1 << 12) != 0);
                let y = i32::from(held & (1 << 15) != 0) - i32::from(held & (1 << 13) != 0);
                app.device_event(event_loop, device, DeviceEvent::Motion { axis: 15, value: f64::from(x) });
                app.device_event(event_loop, device, DeviceEvent::Motion { axis: 16, value: f64::from(y) });
            }
            // Press the left stick to enter text into the currently selected field.
            if held & !last_buttons & (1 << 4) != 0 {
                let mut buffer = [0 as std::ffi::c_char; 256];
                if unsafe { nostalre_keyboard(buffer.as_mut_ptr(), buffer.len()) } == 0 {
                    let text = unsafe { std::ffi::CStr::from_ptr(buffer.as_ptr()) }.to_string_lossy();
                    for c in text.chars() {
                        let mut key = match controller_key(0, true) {
                            WindowEvent::KeyboardInput { event, .. } => event,
                            _ => unreachable!(),
                        };
                        let text: crate::keyboard::SmolStr = c.to_string().into();
                        key.logical_key = Key::Character(text.clone()); key.text = Some(text);
                        app.window_event(event_loop, id, WindowEvent::KeyboardInput {
                            device_id: device, event: key, is_synthetic: false,
                        });
                    }
                }
            }
            let mut raw = [0u32; 48];
            let count = unsafe { nostalre_touch_poll(raw.as_mut_ptr()) }.min(16) as usize;
            let mut current = [(0u32, (0u32, 0u32)); 16];
            for (index, point) in raw[..count * 3].chunks_exact(3).enumerate() {
                let finger = point[0]; let position = (point[1], point[2]);
                let phase = if touches[..touch_count].iter().any(|&(id, _)| id == finger) { TouchPhase::Moved } else { TouchPhase::Started };
                app.window_event(event_loop, id, WindowEvent::Touch(Touch {
                    device_id: device, phase, location: PhysicalPosition::new(position.0 as f64, position.1 as f64),
                    force: None, id: u64::from(finger),
                }));
                current[index] = (finger, position);
            }
            for &(finger, position) in &touches[..touch_count] {
                if !current[..count].iter().any(|&(id, _)| id == finger) {
                    app.window_event(event_loop, id, WindowEvent::Touch(Touch {
                        device_id: device, phase: TouchPhase::Ended,
                        location: PhysicalPosition::new(position.0 as f64, position.1 as f64),
                        force: None, id: u64::from(finger),
                    }));
                }
            }
            touches = current; touch_count = count; last_buttons = held; last_sticks = sticks;
            app.about_to_wait(event_loop);
            if event_loop.exiting() { break; }
            if REDRAW.swap(false, Ordering::Relaxed) {
                app.window_event(event_loop, id, WindowEvent::RedrawRequested);
            }
            if last_log_sync.elapsed() >= Duration::from_secs(1) {
                bevy_horizon::request_switch_log_sync();
                last_log_sync = Instant::now();
            }
            match event_loop.control_flow() {
                ControlFlow::WaitUntil(deadline) => {
                    if let Some(delay) = deadline.checked_duration_since(Instant::now()) {
                        std::thread::sleep(delay.min(Duration::from_millis(20)));
                    }
                },
                ControlFlow::Wait => std::thread::sleep(Duration::from_millis(16)),
                ControlFlow::Poll => if let Some(delay) = pacer.delay_after_frame(Instant::now()) {
                    std::thread::sleep(delay);
                },
            }
        }
        app.exiting(event_loop);
        Ok(())
    }
}
