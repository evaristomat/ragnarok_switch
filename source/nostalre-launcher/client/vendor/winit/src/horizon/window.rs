use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use crate::dpi::{LogicalSize, PhysicalSize};
use crate::raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle,
    HorizonWindowHandle, RawWindowHandle, WindowHandle};

pub(crate) static REDRAW: AtomicBool = AtomicBool::new(true);
pub(crate) static WIDTH: AtomicU32 = AtomicU32::new(1280);
pub(crate) static HEIGHT: AtomicU32 = AtomicU32::new(720);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) u64);
impl WindowId { pub const fn dummy() -> Self { Self(1) } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivationToken(pub String);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme { Light, Dark }
#[derive(Clone, Debug)]
pub enum Fullscreen { Borderless(Option<()>) }
#[derive(Clone, Debug, Default)]
pub struct WindowAttributes { pub title: String }
impl WindowAttributes {
    pub fn with_title(mut self, title: impl Into<String>) -> Self { self.title = title.into(); self }
    pub fn with_inner_size<T>(self, _size: LogicalSize<T>) -> Self { self }
}
pub struct Window { native: NonNull<std::ffi::c_void> }
unsafe impl Send for Window {}
unsafe impl Sync for Window {}
impl Window {
    pub(crate) fn new() -> Result<Self, crate::error::OsError> {
        unsafe extern "C" { fn nwindowGetDefault() -> *mut std::ffi::c_void; }
        NonNull::new(unsafe { nwindowGetDefault() }).map(|native| Self { native })
            .ok_or_else(|| crate::error::OsError::new(line!(), file!(),
                std::io::Error::other("libnx did not provide a window")))
    }
    pub fn id(&self) -> WindowId { WindowId(1) }
    pub fn scale_factor(&self) -> f64 { 1.0 }
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        PhysicalSize::new(WIDTH.load(Ordering::Relaxed), HEIGHT.load(Ordering::Relaxed))
    }
    pub fn request_redraw(&self) { REDRAW.store(true, Ordering::Relaxed); }
    pub fn set_fullscreen(&self, _: Option<Fullscreen>) {}
    pub fn set_cursor_visible(&self, _: bool) {}
}
impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Horizon(HorizonWindowHandle::new(self.native))) })
    }
}
impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> { Ok(DisplayHandle::horizon()) }
}
