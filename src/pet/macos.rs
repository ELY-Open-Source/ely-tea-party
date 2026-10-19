//! Ely's window on macOS: a child of the main window at its level, so it moves with it and keeps its layer and Space.

use anyhow::bail;
use gpui::Window;
use objc::{
    msg_send,
    runtime::{NO, Object, YES},
    sel, sel_impl,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use super::left;

/// AppKit's `NSWindowAbove`.
const ABOVE: isize = 1;

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
    /// Makes her window the main window's child, at its level and in its Spaces, with no shadow and never key, so a press leaves the keys with the terminal.
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        let (pet, main) = (native(pet)?, native(main)?);
        unsafe {
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

    /// Stands her on the main window's top edge, as far in as its width allows.
    pub fn place(&self, _: &Window) -> anyhow::Result<()> {
        unsafe {
            let frame: Rect = msg_send![self.main, frame];
            let at = Point {
                x: frame.origin.x + f64::from(left(gpui::px(frame.width as f32))),
                y: frame.origin.y + frame.height,
            };
            let _: () = msg_send![self.pet, setFrameOrigin: at];
        }
        Ok(())
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
