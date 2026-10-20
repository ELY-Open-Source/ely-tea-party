//! Ely's window on macOS: a borderless panel, the main window's child at its level, so it moves with it and keeps its layer and Space.

use anyhow::bail;
use gpui::Window;
use objc::{
    class, msg_send,
    runtime::{NO, Object, YES},
    sel, sel_impl,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::{Edge, edge, left};

/// AppKit's `NSWindowAbove`.
const ABOVE: isize = 1;
/// AppKit's `NSWindowStyleMaskNonactivatingPanel` alone: a panel without the titled frame gpui's popup carries.
const BORDERLESS_PANEL: usize = 1 << 7;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    width: f64,
    height: f64,
}

/// Her NSWindow and the main one's.
pub struct Glue {
    pet: *mut Object,
    main: *mut Object,
}

impl Glue {
    /// Makes her window a borderless panel, the main window's child, at its level and in its Spaces, with no shadow and never key, so a press leaves the keys with the terminal.
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        let (pet, main) = (native(pet)?, native(main)?);
        unsafe {
            // A titled window gets a frame macOS draws and keeps under the menu bar.
            let titled: usize = msg_send![pet, styleMask];
            let _: () = msg_send![pet, setStyleMask: BORDERLESS_PANEL];
            let mask: usize = msg_send![pet, styleMask];
            anyhow::ensure!(
                mask == BORDERLESS_PANEL,
                "pet: her panel keeps style mask {mask:#x}"
            );
            log::info!(
                "pet: her panel drops its titled frame, style mask {titled:#x} to {mask:#x}"
            );
            let level: isize = msg_send![main, level];
            let _: () = msg_send![pet, setLevel: level];
            let spaces: usize = msg_send![main, collectionBehavior];
            let _: () = msg_send![pet, setCollectionBehavior: spaces];
            let _: () = msg_send![pet, setHasShadow: NO];
            let _: () = msg_send![pet, setBecomesKeyOnlyIfNeeded: YES];
            let _: () = msg_send![main, addChildWindow: pet ordered: ABOVE];
        }
        Ok(Self { pet, main })
    }

    /// Stands her on the main window's top edge while her box fits under the top of its screen's visible frame, else on its bottom edge, `at` in as far as its width allows, and gives the edge.
    pub fn place(&self, _: &Window, at: f32) -> anyhow::Result<Edge> {
        unsafe {
            let frame: Rect = msg_send![self.main, frame];
            let screen: *mut Object = msg_send![self.main, screen];
            anyhow::ensure!(!screen.is_null(), "pet: the main window is on no screen");
            let visible: Rect = msg_send![screen, visibleFrame];
            let top = frame.origin.y + frame.height;
            let edge = edge((visible.origin.y + visible.height - top) as f32);
            let at = Point {
                x: frame.origin.x + f64::from(left(gpui::px(frame.width as f32), at)),
                y: match edge {
                    Edge::Top => top,
                    Edge::Bottom => frame.origin.y,
                },
            };
            let _: () = msg_send![self.pet, setFrameOrigin: at];
            Ok(edge)
        }
    }

    /// The pointer's x on the screen, in points.
    pub fn pointer(&self, _: f32) -> anyhow::Result<f32> {
        let at: Point = unsafe { msg_send![class!(NSEvent), mouseLocation] };
        Ok(at.x as f32)
    }
}

/// The NSWindow gpui made for `window`.
fn native(window: &Window) -> anyhow::Result<*mut Object> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("pet: no native handle: {error:?}"))?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        bail!("pet: not an AppKit window");
    };
    let view = appkit.ns_view.as_ptr() as *mut Object;
    let native: *mut Object = unsafe { msg_send![view, window] };
    anyhow::ensure!(!native.is_null(), "pet: the view is in no window");
    Ok(native)
}
