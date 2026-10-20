use std::collections::VecDeque;

use gpui::{point, px, size};
use windows_sys::{
    Win32::UI::WindowsAndMessaging::{
        CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, MSG,
        PM_REMOVE, PeekMessageW, SendMessageW, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_OVERLAPPEDWINDOW,
    },
    w,
};

use super::*;

/// A hidden window of `style`, at the size Windows picks, as gpui asks for its windows.
fn window(style: u32, extended: u32) -> HWND {
    let window = unsafe {
        CreateWindowExW(
            extended,
            w!("STATIC"),
            w!(""),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    assert!(!window.is_null(), "a test window");
    window
}

/// Hands her window the messages posted to it, as the message loop does.
fn deliver(window: HWND) {
    let mut message = MSG::default();
    while unsafe { PeekMessageW(&mut message, window, 0, 0, PM_REMOVE) } != 0 {
        unsafe { DispatchMessageW(&message) };
    }
}

/// Moves `window` to `(x, y)`, 800 by 600.
fn move_main(window: HWND, (x, y): (i32, i32)) {
    let flags = SWP_NOZORDER | SWP_NOACTIVATE;
    let moved = unsafe { SetWindowPos(window, std::ptr::null_mut(), x, y, 800, 600, flags) };
    assert_ne!(moved, 0, "the main window moves");
}

/// The work area of the display under most of `window`.
fn work_area(window: HWND) -> RECT {
    let mut display = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let monitor = unsafe { MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) };
    assert_ne!(
        unsafe { GetMonitorInfoW(monitor, &mut display) },
        0,
        "its display"
    );
    display.rcWork
}

fn info(window: HWND) -> WINDOWINFO {
    let mut info = WINDOWINFO {
        cbSize: size_of::<WINDOWINFO>() as u32,
        ..Default::default()
    };
    assert_ne!(unsafe { GetWindowInfo(window, &mut info) }, 0, "its info");
    info
}

/// gpui's procedure as far as these tests need it: at a new DPI it takes the rect Windows suggests.
unsafe extern "system" fn standin(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_DPICHANGED {
        let rect = unsafe { *(lparam as *const RECT) };
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        let flags = SWP_NOZORDER | SWP_NOACTIVATE;
        let null = std::ptr::null_mut();
        unsafe { SetWindowPos(window, null, rect.left, rect.top, width, height, flags) };
        return 0;
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

#[test]
fn her_bottom_stands_on_either_edge_of_the_pane_whatever_the_two_windows_dpi() {
    let client = RECT {
        left: 100,
        top: 20,
        right: 1300,
        bottom: 900,
    };
    let pane = Bounds::new(point(px(10.), px(8.)), size(px(800.), px(600.)));
    for (scale, dpi) in [(1.0, 96), (1.0, 144), (1.5, 96), (2.0, 120)] {
        for (edge, y) in [(Edge::Top, 8.0), (Edge::Bottom, 608.0)] {
            let her = boxed(stand(client, scale, pane, edge, 140.0), dpi);
            let own = dpi as f32 / 96.0;
            let own_box = (
                (WIDTH * own).round() as i32,
                ((HEAD + ROOM) * own).round() as i32,
            );
            let corner = (
                100 + ((10.0 + left(px(800.), 140.0)) * scale).round() as i32,
                20 + (y * scale).round() as i32,
            );
            assert_eq!(
                (
                    (her.right - her.left, her.bottom - her.top),
                    (her.left, her.bottom)
                ),
                (own_box, corner),
                "the main window at {scale}, hers at {dpi} DPI, on the {edge:?} edge"
            );
        }
    }
}

/// Two displays split at 0, `first` DPI before and `second` after, across or down, giving a box the DPI of the one under most of it, as Windows does.
fn displays(down: bool, first: u32, second: u32) -> impl Fn(&RECT) -> anyhow::Result<u32> {
    move |rect: &RECT| {
        let (from, to) = if down {
            (rect.top, rect.bottom)
        } else {
            (rect.left, rect.right)
        };
        Ok(if to.min(0) - from > to - from.max(0) {
            first
        } else {
            second
        })
    }
}

/// What reaches her window: a place, her bottom left corner, or a keep at a DPI.
enum Sent {
    Place((i32, i32)),
    Keep(u32),
}

/// Her window, its rect and DPI, once the queue `places` start drains on `dpi_of`'s displays: a move to a display of another DPI changes hers, gpui takes Windows' rect, scaled about its middle, and a keep joins the queue, as her procedure posts one; and how many changes it took.
fn drain(
    mut window: (RECT, u32),
    places: &[(i32, i32)],
    dpi_of: &impl Fn(&RECT) -> anyhow::Result<u32>,
) -> ((RECT, u32), usize) {
    let mut queue: VecDeque<Sent> = places.iter().map(|&at| Sent::Place(at)).collect();
    let (mut corner, mut changes) = (None, 0);
    while let Some(sent) = queue.pop_front() {
        let target = match sent {
            Sent::Place(at) => {
                corner = Some(at);
                boxed(at, window.1)
            }
            Sent::Keep(dpi) => {
                let corner = corner.expect("a place first");
                match keeping(corner, dpi, window.1, dpi_of).expect("a display") {
                    Some(kept) => kept,
                    None => continue,
                }
            }
        };
        let now = dpi_of(&target).expect("a display");
        if now == window.1 {
            window = (target, now);
            continue;
        }
        changes += 1;
        assert!(changes < 8, "her DPI goes back and forth");
        let grow = |from: i32, to: i32| {
            let length = ((to - from) as f64 * now as f64 / window.1 as f64).round() as i32;
            let start = (from + to - length).div_euclid(2);
            (start, start + length)
        };
        let ((left, right), (top, bottom)) = (
            grow(target.left, target.right),
            grow(target.top, target.bottom),
        );
        window = (
            RECT {
                left,
                top,
                right,
                bottom,
            },
            now,
        );
        queue.push_back(Sent::Keep(now));
    }
    (window, changes)
}

fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
    RECT {
        left,
        top,
        right,
        bottom,
    }
}

#[test]
fn placing_her_across_two_displays_of_two_dpis_settles() {
    // Side by side, 144 DPI then 96: her 144 DPI box, 70 pixels on the first and 98 on the second.
    let across = displays(false, 144, 96);
    let start = (rect(-400, 315, -232, 500), 144);
    let ((at, dpi), changes) = drain(start, &[(-70, 500)], &across);
    assert_eq!(
        (changes, dpi, across(&at).expect("a display")),
        (1, 96, 96),
        "Windows' rect stands where her corner would set her DPI back"
    );
    // One over the other, 144 DPI above 96: her 96 DPI box, her bottom 50 pixels down the second.
    let down = displays(true, 144, 96);
    let start = (rect(200, 300, 312, 423), 96);
    let ((at, dpi), changes) = drain(start, &[(200, 50)], &down);
    assert_eq!(
        (changes, dpi, at.left, at.bottom, at.bottom - at.top),
        (1, 144, 200, 50, 185),
        "her corner kept on the edge"
    );
}

#[test]
fn places_that_pile_up_across_two_displays_drain_in_her_dpis_size() {
    let sizes = |(at, dpi): (RECT, u32)| {
        let drawn = (at.right - at.left, at.bottom - at.top);
        (drawn, sized(dpi as f32 / 96.0))
    };
    // Side by side, 96 DPI then 144; two places wait before either runs, as round 3 set them.
    let across = displays(false, 96, 144);
    let start = (rect(-400, 377, -288, 500), 96);
    let places = [(0, 500), (-70, 500)];
    let (window, _) = drain(start, &places, &across);
    let (drawn, own) = sizes(window);
    assert_eq!(
        (drawn, window.0.left, window.0.bottom),
        (own, -70, 500),
        "a keep whose DPI has passed moves her nowhere; she stands on the last corner"
    );
    // Side by side, 144 DPI then 96: the first takes her to 144, and the second lands there.
    let across = displays(false, 144, 96);
    let start = (rect(300, 377, 412, 500), 96);
    let places = [(-100, 500), (-60, 500)];
    let (drawn, own) = sizes(drain(start, &places, &across).0);
    assert_eq!(
        drawn, own,
        "her size is her DPI's, whatever DPI a place was posted at"
    );
}

#[test]
fn the_large_framed_popup_gpui_makes_stands_bare_in_her_box_on_an_edge_of_the_pane() {
    let main = window(WS_OVERLAPPEDWINDOW, 0);
    // gpui's popup: style 0, an overlapped window, which Windows gives a caption.
    let pet = window(0, WS_EX_TOOLWINDOW | WS_EX_TOPMOST);
    assert_eq!(
        info(pet).dwStyle & WS_CAPTION,
        WS_CAPTION,
        "framed as gpui makes it"
    );
    unsafe { SetWindowLongPtrW(pet, GWLP_WNDPROC, standin as *const () as isize) };
    let glue = Glue::hold(pet, main).expect("held");
    let work = work_area(main);
    move_main(main, (work.left + 200, work.top + 400));
    let pane = Bounds::new(point(px(0.), px(0.)), size(px(380.), px(280.)));
    let roomy = glue.put(1.0, pane, 140.0).expect("placed");
    let made = info(pet).rcWindow;
    deliver(pet);
    let (her, under) = (info(pet), info(main));
    let own = unsafe { GetDpiForWindow(pet) } as f32 / 96.0;
    // New DPIs, each with a rect Windows suggests 30 and 40 pixels off, 100 by 60.
    let display = display_dpi(&her.rcWindow).expect("the display under her");
    let change = |dpi: u32| {
        let at = info(pet).rcWindow;
        let suggested = RECT {
            left: at.left + 30,
            top: at.top + 40,
            right: at.left + 130,
            bottom: at.top + 100,
        };
        let pointer = &suggested as *const RECT as LPARAM;
        unsafe { SendMessageW(pet, WM_DPICHANGED, (dpi | dpi << 16) as WPARAM, pointer) };
        let taken = info(pet).rcWindow;
        deliver(pet);
        (suggested, taken, info(pet).rcWindow)
    };
    let (_, _, kept) = change(display);
    let (suggested, taken, stood) = change(display + 48);
    // At the work area's top the main window leaves her no room above it.
    move_main(main, (work.left + 200, work.top));
    let cramped = glue.put(1.0, pane, 140.0).expect("placed again");
    deliver(pet);
    let (inside, low) = (info(pet).rcWindow, info(main).rcClient);
    unsafe {
        DestroyWindow(pet);
        DestroyWindow(main);
    }
    let (outer, inner) = (her.rcWindow, her.rcClient);
    let frame = (
        inner.left - outer.left,
        inner.top - outer.top,
        outer.right - inner.right,
        outer.bottom - inner.bottom,
    );
    let drawn = (outer.right - outer.left, outer.bottom - outer.top);
    let own_box = (
        (WIDTH * own).round() as i32,
        ((HEAD + ROOM) * own).round() as i32,
    );
    let edge = (
        under.rcClient.left + left(px(380.), 140.0).round() as i32,
        under.rcClient.top,
    );
    let rect = |rect: RECT| (rect.left, rect.top, rect.right, rect.bottom);
    assert_eq!(
        (
            her.dwStyle & WS_CAPTION,
            frame,
            drawn,
            (outer.left, outer.bottom),
            rect(kept)
        ),
        (0, (0, 0, 0, 0), own_box, edge, rect(outer)),
        "bare, in her own box, her bottom on the pane's top edge, kept there at her display's DPI"
    );
    assert_eq!(
        (rect(taken), rect(stood)),
        (rect(suggested), rect(suggested)),
        "a keep at a DPI not her window's moves her nowhere: the rect gpui took from Windows stands"
    );
    assert_ne!(
        (made.right - made.left, made.bottom - made.top),
        own_box,
        "no size until her window takes the posted place"
    );
    assert_eq!(
        (roomy, cramped, inside.left, inside.bottom),
        (Edge::Top, Edge::Bottom, low.left + 140, low.top + 280),
        "with no room above, inside on the pane's bottom edge"
    );
}
