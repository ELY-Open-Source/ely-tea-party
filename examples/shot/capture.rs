use std::path::Path;

use anyhow::{Context as _, bail};
use objc::{
    class, msg_send,
    runtime::{BOOL, Object, YES},
    sel, sel_impl,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// A gpui window's number, as CoreGraphics knows it.
pub fn number(window: &gpui::Window) -> anyhow::Result<u32> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("no native handle: {error:?}"))?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        bail!("not an AppKit window");
    };
    let view = appkit.ns_view.as_ptr() as *mut Object;
    let native: *mut Object = unsafe { msg_send![view, window] };
    let number: i64 = unsafe { msg_send![native, windowNumber] };
    u32::try_from(number).context("window number out of range")
}

/// The window and each of its child windows: number and frame, in AppKit's screen points.
pub fn frames(number: u32) -> anyhow::Result<Vec<(u32, [f64; 4])>> {
    use core_graphics::geometry::CGRect;
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
        anyhow::ensure!(!window.is_null(), "no window {number}");
        let children: *mut Object = msg_send![window, childWindows];
        let count: u64 = if children.is_null() {
            0
        } else {
            msg_send![children, count]
        };
        let children = (0..count).map(|ix| msg_send![children, objectAtIndex: ix]);
        std::iter::once(window)
            .chain(children)
            .map(|window: *mut Object| {
                let number: i64 = msg_send![window, windowNumber];
                let frame: CGRect = msg_send![window, frame];
                let corners = [
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                ];
                Ok((
                    u32::try_from(number).context("window number out of range")?,
                    corners,
                ))
            })
            .collect()
    }
}

/// Shows the window as key, the app left inactive.
pub fn front(number: u32) -> anyhow::Result<()> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
        anyhow::ensure!(!window.is_null(), "no window {number}");
        let _: () = msg_send![window, makeKeyWindow];
        let _: () = msg_send![window, orderFrontRegardless];
    }
    Ok(())
}

/// Each view's AppKit class; an effect view adds its blur.
pub fn views(number: u32) -> anyhow::Result<Vec<String>> {
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let window: *mut Object = msg_send![app, windowWithWindowNumber: i64::from(number)];
        anyhow::ensure!(!window.is_null(), "no window {number}");
        let content: *mut Object = msg_send![window, contentView];
        let views: *mut Object = msg_send![content, subviews];
        let count: u64 = msg_send![views, count];
        let named = (0..count).map(|ix| {
            let view: *mut Object = msg_send![views, objectAtIndex: ix];
            let name = text(msg_send![view, className]);
            let effect: BOOL = msg_send![view, isKindOfClass: class!(NSVisualEffectView)];
            if effect == YES {
                let radius = blur(msg_send![view, layer]).unwrap_or_else(|| "none".into());
                format!("{name}, blur {radius}")
            } else {
                name
            }
        });
        Ok(named.collect())
    }
}

/// The first gaussian blur's radius at or under `layer`.
unsafe fn blur(layer: *mut Object) -> Option<String> {
    if layer.is_null() {
        return None;
    }
    unsafe {
        let filters: *mut Object = msg_send![layer, filters];
        let count: u64 = if filters.is_null() {
            0
        } else {
            msg_send![filters, count]
        };
        for ix in 0..count {
            let filter: *mut Object = msg_send![filters, objectAtIndex: ix];
            if text(msg_send![filter, name]) == "gaussianBlur" {
                let key: *mut Object =
                    msg_send![class!(NSString), stringWithUTF8String: c"inputRadius".as_ptr()];
                let radius: *mut Object = msg_send![filter, valueForKey: key];
                return Some(text(msg_send![radius, description]));
            }
        }
        let sublayers: *mut Object = msg_send![layer, sublayers];
        let count: u64 = if sublayers.is_null() {
            0
        } else {
            msg_send![sublayers, count]
        };
        (0..count).find_map(|ix| blur(msg_send![sublayers, objectAtIndex: ix]))
    }
}

/// An NSString's text, `nil` for none.
fn text(string: *mut Object) -> String {
    if string.is_null() {
        return "nil".into();
    }
    let bytes: *const std::ffi::c_char = unsafe { msg_send![string, UTF8String] };
    unsafe { std::ffi::CStr::from_ptr(bytes) }
        .to_string_lossy()
        .into_owned()
}

/// Each menu item with its key and modifier mask.
pub fn menus() -> Vec<String> {
    let mut lines = Vec::new();
    unsafe {
        let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
        let bar: *mut Object = msg_send![app, mainMenu];
        let count: i64 = msg_send![bar, numberOfItems];
        for at in 0..count {
            let top: *mut Object = msg_send![bar, itemAtIndex: at];
            let menu: *mut Object = msg_send![top, submenu];
            let name = text(msg_send![menu, title]);
            let items: i64 = msg_send![menu, numberOfItems];
            for row in 0..items {
                let item: *mut Object = msg_send![menu, itemAtIndex: row];
                let key = text(msg_send![item, keyEquivalent]);
                let mask: u64 = msg_send![item, keyEquivalentModifierMask];
                let title = text(msg_send![item, title]);
                lines.push(format!("{name} > {title} [{key:?} {mask:#x}]"));
            }
        }
    }
    lines
}

/// Writes window `number` as a PNG.
pub fn snapshot(number: u32, path: &Path) -> anyhow::Result<()> {
    use core_graphics::{
        access::ScreenCaptureAccess,
        geometry::{CGPoint, CGRect, CGSize},
        window,
    };
    if !ScreenCaptureAccess.preflight() {
        log::warn!(
            "shot: the app shot runs under lacks Screen Recording, so the picture may hold the window's ground alone"
        );
    }
    let image = window::create_image(
        CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0)),
        window::kCGWindowListOptionIncludingWindow,
        number,
        window::kCGWindowImageBoundsIgnoreFraming | window::kCGWindowImageBestResolution,
    )
    .context("window capture returned nothing")?;
    if image.bits_per_pixel() != 32 {
        bail!("{} bits per pixel", image.bits_per_pixel());
    }
    let (width, height, stride) = (image.width(), image.height(), image.bytes_per_row());
    let data = image.data();
    let bytes = data.bytes();
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let at = y * stride + x * 4;
            rgba.extend_from_slice(&[bytes[at + 2], bytes[at + 1], bytes[at], bytes[at + 3]]);
        }
    }
    image::save_buffer(
        path,
        &rgba,
        width as u32,
        height as u32,
        image::ColorType::Rgba8,
    )
    .with_context(|| format!("cannot write {}", path.display()))
}
