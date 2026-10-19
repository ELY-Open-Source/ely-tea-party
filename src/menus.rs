use ely_gpui_component::forms::{Copy, Paste, SelectAll};
use gpui::{App, InteractiveElement, KeyBinding, Menu, MenuItem, OsAction, actions};

use crate::{
    settings::window::OpenSettings,
    term::{input::Find, zoom},
    update::CheckForUpdates,
    workspace::{CloseSession, NewSession, ToggleDrawer, TogglePanel, ToggleParty, ToggleSessions},
};

actions!(
    ely_tea_party,
    [Quit, Bigger, Smaller, ResetSize, Minimize, Zoom]
);

/// The PRD's menu bar.
pub fn menus() -> Vec<Menu> {
    let menu = |name: &str, items| Menu {
        name: name.to_string().into(),
        items,
        disabled: false,
    };
    vec![
        menu(
            "Ely Tea Party",
            vec![
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::action("Check for Updates…", CheckForUpdates),
                MenuItem::separator(),
                MenuItem::action("Quit Ely Tea Party", Quit),
            ],
        ),
        menu(
            "Shell",
            vec![
                MenuItem::action("New Session", NewSession),
                MenuItem::action("Close Session", CloseSession),
            ],
        ),
        menu(
            "Edit",
            vec![
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find", Find),
            ],
        ),
        menu(
            "View",
            vec![
                MenuItem::action("Toggle Sessions", ToggleSessions),
                MenuItem::action("Toggle Right Panel", TogglePanel),
                MenuItem::action("Toggle Drawer", ToggleDrawer),
                MenuItem::action("Tea Party", ToggleParty),
                MenuItem::separator(),
                MenuItem::action("Bigger", Bigger),
                MenuItem::action("Smaller", Smaller),
                MenuItem::action("Reset Size", ResetSize),
            ],
        ),
        // macOS lists the open windows here itself.
        menu(
            "Window",
            vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
            ],
        ),
    ]
}

/// Binds app commands before building their menus.
pub fn init(cx: &mut App) {
    let (quit, modifier) = if cfg!(target_os = "macos") {
        ("cmd-q", "cmd")
    } else {
        ("ctrl-shift-q", "ctrl")
    };
    cx.bind_keys([
        KeyBinding::new(quit, Quit, None),
        KeyBinding::new(&format!("{modifier}-="), Bigger, None),
        KeyBinding::new(&format!("{modifier}-+"), Bigger, None),
        KeyBinding::new(&format!("{modifier}--"), Smaller, None),
        KeyBinding::new(&format!("{modifier}-0"), ResetSize, None),
    ]);
    if cfg!(target_os = "macos") {
        cx.bind_keys([KeyBinding::new("cmd-m", Minimize, None)]);
    }
    cx.set_global(zoom::Enlarged::default());
    cx.on_action(|_: &Quit, cx| {
        log::info!("app: quit asked");
        cx.quit();
    });
    cx.on_action(|_: &Bigger, cx| zoom::step(Some(1.0), cx));
    cx.on_action(|_: &Smaller, cx| zoom::step(Some(-1.0), cx));
    cx.on_action(|_: &ResetSize, cx| zoom::step(None, cx));
    cx.set_menus(menus());
}

/// Minimize and Zoom, which only a window answers.
pub fn window_actions<E: InteractiveElement>(root: E) -> E {
    root.on_action(|_: &Minimize, window, _| {
        log::info!("window: minimized");
        window.minimize_window();
    })
    .on_action(|_: &Zoom, window, _| {
        log::info!("window: zoomed");
        window.zoom_window();
    })
}
