//! Ely's window on Windows: a bare popup owned by the main window, so it stays above it and hides with it, sized and moved as it moves.

use std::{
    cell::Cell,
    sync::atomic::{AtomicIsize, Ordering},
};

use anyhow::bail;
use gpui::{Bounds, Pixels, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromRect},
    UI::{
        HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
        WindowsAndMessaging::{
            CallWindowProcW, GWL_STYLE, GWLP_HWNDPARENT, GWLP_WNDPROC, GetWindowInfo,
            GetWindowLongPtrW, HWND_NOTOPMOST, MA_NOACTIVATE, PostMessageW, SWP_FRAMECHANGED,
            SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos,
            USER_DEFAULT_SCREEN_DPI, WINDOWINFO, WM_APP, WM_DPICHANGED, WM_MOUSEACTIVATE, WNDPROC,
            WS_CAPTION, WS_POPUP,
        },
    },
};

use super::{HEAD, ROOM, WIDTH, left, pane};

#[cfg(test)]
mod tests;

/// gpui's window procedure, which hers passes every message on to but a press's activation and her places.
static GPUI: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    /// The bottom left corner her last place gave her, on the thread her window lives on.
    static CORNER: Cell<Option<(i32, i32)>> = const { Cell::new(None) };
}

/// Her place, posted to her window: Windows sends a new size at once, which gpui hears only outside its own updates.
const PLACE: u32 = WM_APP;
/// Her corner at a new DPI, posted so it follows the whole move that brought the change.
const KEEP: u32 = WM_APP + 1;
const _: () = assert!(
    size_of::<WPARAM>() == 8,
    "a place packs two i32 into a word"
);

/// Her window and the main one.
pub struct Glue {
    pet: HWND,
    main: HWND,
}

impl Glue {
    pub fn new(pet: &Window, main: &Window) -> anyhow::Result<Self> {
        Self::hold(native(pet)?, native(main)?)
    }

    /// Gives her window the main window as owner, a bare popup's frame, none of the topmost place gpui gives a popup, and keeps a press from activating it, so the keys stay with the terminal.
    fn hold(pet: HWND, main: HWND) -> anyhow::Result<Self> {
        unsafe {
            SetWindowLongPtrW(pet, GWLP_HWNDPARENT, main as isize);
            // gpui's popup is overlapped, which Windows gives a caption and a frame.
            let style = (GetWindowLongPtrW(pet, GWL_STYLE) as u32 & !WS_CAPTION) | WS_POPUP;
            SetWindowLongPtrW(pet, GWL_STYLE, style as isize);
            let gpui = SetWindowLongPtrW(pet, GWLP_WNDPROC, inactive as *const () as isize);
            anyhow::ensure!(gpui != 0, "pet: her window's procedure does not read");
            GPUI.store(gpui, Ordering::Relaxed);
            let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED;
            anyhow::ensure!(
                SetWindowPos(pet, HWND_NOTOPMOST, 0, 0, 0, 0, flags) != 0,
                "pet: her window keeps the topmost place or its frame"
            );
        }
        Ok(Self { pet, main })
    }

    /// Stands her on the main window's top edge, as far in as its width allows.
    pub fn place(&self, main: &Window) -> anyhow::Result<()> {
        self.put(main.scale_factor(), pane(main))
    }

    /// Posts her bottom left corner on `pane` at `scale`, in the main window's points; her window sizes her at the DPI it has when the place lands, as gpui did not when it opened her off the display.
    fn put(&self, scale: f32, pane: Bounds<Pixels>) -> anyhow::Result<()> {
        let mut main = WINDOWINFO {
            cbSize: size_of::<WINDOWINFO>() as u32,
            ..Default::default()
        };
        let read = unsafe { GetWindowInfo(self.main, &mut main) };
        anyhow::ensure!(read != 0, "pet: the main window's place does not read");
        let corner = stand(main.rcClient, scale, pane);
        let posted = unsafe { PostMessageW(self.pet, PLACE, pack(corner), 0) };
        anyhow::ensure!(posted != 0, "pet: her place does not post");
        log::debug!("pet: Ely is to stand on {corner:?}");
        Ok(())
    }
}

/// Her bottom left corner on `pane` at `scale` over the main window's `client` rect, in pixels.
fn stand(client: RECT, scale: f32, pane: Bounds<Pixels>) -> (i32, i32) {
    let (x, y) = (f32::from(pane.origin.x), f32::from(pane.origin.y));
    (
        client.left + ((x + left(pane.size.width)) * scale).round() as i32,
        client.top + (y * scale).round() as i32,
    )
}

/// Her box at `dpi` on her bottom left `corner`.
fn boxed((left, bottom): (i32, i32), dpi: u32) -> RECT {
    let (width, height) = sized(dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32);
    RECT {
        left,
        top: bottom - height,
        right: left + width,
        bottom,
    }
}

/// Her box's size in pixels at `own` scale.
fn sized(own: f32) -> (i32, i32) {
    (
        (WIDTH * own).round() as i32,
        ((HEAD + ROOM) * own).round() as i32,
    )
}

/// Her window's procedure: a press does not activate her window, she takes her places, and all else is gpui's.
unsafe extern "system" fn inactive(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let gpui: WNDPROC = unsafe { std::mem::transmute(GPUI.load(Ordering::Relaxed)) };
    match message {
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        PLACE => {
            let corner = unpack(wparam);
            CORNER.set(Some(corner));
            match unsafe { GetDpiForWindow(window) } {
                0 => log::error!("pet: her window's DPI does not read at her place"),
                dpi => move_to(window, boxed(corner, dpi)),
            }
            0
        }
        WM_DPICHANGED => {
            // gpui takes the rect Windows suggests; her corner follows once the move is done.
            let answer = unsafe { CallWindowProcW(gpui, window, message, wparam, lparam) };
            if unsafe { PostMessageW(window, KEEP, wparam & 0xFFFF, 0) } == 0 {
                log::error!("pet: her corner does not post at a new DPI");
            }
            answer
        }
        KEEP => {
            let (dpi, current) = (wparam as u32, unsafe { GetDpiForWindow(window) });
            if current == 0 {
                log::error!("pet: her window's DPI does not read at a new DPI");
                return 0;
            }
            let Some(corner) = CORNER.get() else {
                log::debug!("pet: at {dpi} DPI Ely has no place to keep yet");
                return 0;
            };
            match keeping(corner, dpi, current, display_dpi) {
                Ok(Some(kept)) => {
                    move_to(window, kept);
                    log::debug!("pet: at {dpi} DPI Ely keeps her corner at {corner:?}");
                }
                Ok(None) => log::debug!(
                    "pet: a keep at {dpi} DPI, hers {current}, leaves Ely where Windows put her"
                ),
                Err(error) => log::error!("{error:#}"),
            }
            0
        }
        _ => unsafe { CallWindowProcW(gpui, window, message, wparam, lparam) },
    }
}

/// Moves her window to `rect`.
fn move_to(window: HWND, rect: RECT) {
    let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
    let flags = SWP_NOZORDER | SWP_NOACTIVATE;
    let null = std::ptr::null_mut();
    if unsafe { SetWindowPos(window, null, rect.left, rect.top, width, height, flags) } == 0 {
        log::error!(
            "pet: her window does not move to {},{}",
            rect.left,
            rect.top
        );
    }
}

/// Her box at `dpi` on the bottom left `corner` her place gave her, while `dpi` is still hers, `current`, and the box lies mostly on a display of that DPI, as `dpi_of` reads it, so the move changes no DPI and posts no other keep; else none, and Windows' rect stands.
fn keeping(
    corner: (i32, i32),
    dpi: u32,
    current: u32,
    dpi_of: impl Fn(&RECT) -> anyhow::Result<u32>,
) -> anyhow::Result<Option<RECT>> {
    if dpi != current {
        return Ok(None);
    }
    let kept = boxed(corner, dpi);
    Ok((dpi_of(&kept)? == dpi).then_some(kept))
}

/// The DPI of the display under most of `rect`, which Windows gives a window there.
fn display_dpi(rect: &RECT) -> anyhow::Result<u32> {
    let monitor = unsafe { MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST) };
    let (mut x, mut y) = (0, 0);
    let read = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) };
    anyhow::ensure!(
        read == 0,
        "pet: the display under her box gives no DPI: {read:#x}"
    );
    Ok(x)
}

fn pack((low, high): (i32, i32)) -> WPARAM {
    low as u32 as WPARAM | (high as u32 as WPARAM) << 32
}

fn unpack(word: WPARAM) -> (i32, i32) {
    (word as u32 as i32, (word >> 32) as u32 as i32)
}

/// The HWND gpui made for `window`.
fn native(window: &Window) -> anyhow::Result<HWND> {
    let handle = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("pet: no native handle: {error:?}"))?;
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        bail!("pet: not a Win32 window");
    };
    Ok(win32.hwnd.get() as HWND)
}
