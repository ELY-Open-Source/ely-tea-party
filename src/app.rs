use std::borrow::Cow;

use anyhow::Context as _;
use ely_gpui_component::theme::ActiveTheme;
use gpui::{
    App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowHandle,
    WindowOptions, px, size,
};

#[cfg(target_os = "macos")]
use crate::term::mac;
use crate::{
    agents, blur, menus,
    settings::{self, SettingsStore},
    term::{ICONS, Launch, MONO, config, input, shell::ShellIntegration, spawn},
    themes,
    workspace::{self, TITLE, Workspace},
};

/// The id the macOS bundle and Linux desktop entry share.
pub const APP_ID: &str = "net.toddzheng.ely-tea-party";

/// Fonts, settings, integration scripts and the chosen font, each checked before a window opens.
pub fn start(cx: &mut App) -> anyhow::Result<()> {
    cx.text_system()
        .add_fonts(vec![
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
            Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
            Cow::Borrowed(include_bytes!(
                "../assets/fonts/JetBrainsMono-BoldItalic.ttf"
            )),
            Cow::Borrowed(include_bytes!(
                "../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf"
            )),
        ])
        .context("app: the bundled fonts failed to register")?;
    let names = cx.text_system().all_font_names();
    for family in [MONO, ICONS] {
        anyhow::ensure!(
            names.iter().any(|name| name == family),
            "app: {family} is not among the registered fonts"
        );
    }
    SettingsStore::init(cx)?;
    ShellIntegration::init(cx)?;
    #[cfg(unix)]
    {
        cx.set_global(workspace::Reports::new(agents::hooks::listen()?));
        cx.on_app_quit(|_| async { agents::hooks::close() })
            .detach();
    }
    input::bind_keys(cx);
    workspace::bind_keys(cx);
    settings::window::bind_keys(cx);
    menus::init(cx);
    // Settings alone hold no terminal, so they go with the last one.
    cx.on_window_closed(|cx, _| {
        let windows = cx.windows();
        if !windows
            .iter()
            .any(|window| window.downcast::<Workspace>().is_some())
        {
            log::info!("app: no terminal window left; quitting");
            cx.quit();
        }
    })
    .detach();
    #[cfg(target_os = "macos")]
    {
        mac::repeat_held_keys();
        mac::deliver_command_key_ups()?;
    }
    #[cfg(target_os = "linux")]
    themes::omarchy::start(cx)?;
    themes::apply(cx);
    let chosen = |settings: &settings::Settings| {
        let theme = (settings.theme.clone(), settings.dark_theme.clone());
        (theme, settings.background_opacity)
    };
    let mut worn = chosen(SettingsStore::get(cx));
    cx.observe_global::<SettingsStore>(move |cx| {
        let now = chosen(SettingsStore::get(cx));
        if now != worn {
            worn = now;
            themes::apply(cx);
        }
    })
    .detach();
    Ok(())
}

/// Opens the main window running `launch`.
pub fn open(mut launch: Launch, cx: &mut App) -> anyhow::Result<WindowHandle<Workspace>> {
    let bounds = Bounds::centered(None, size(px(1200.0), px(760.0)), cx);
    let background = blur::appearance(SettingsStore::get(cx));
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(TITLE.into()),
            appears_transparent: true,
            traffic_light_position: Some(cx.theme().traffic_light_origin()),
        }),
        // Ely's columns draw the controls; only Linux reads this, and must not draw its own.
        window_decorations: Some(WindowDecorations::Client),
        window_background: background,
        ..Default::default()
    };
    let id = agents::next_session();
    launch.env.extend(agents::env(id)?);
    let (pty, events) = spawn(&launch, (80, 24), (8, 16), config(SettingsStore::get(cx)))?;
    let window = cx.open_window(options, |window, cx| {
        window.set_app_id(APP_ID);
        window
            .observe_window_appearance(|_, cx| themes::apply(cx))
            .detach();
        cx.new(|cx| Workspace::new((id, pty, events), launch, window, cx))
    })?;
    log::info!(
        "app: main window open, blur: {background:?} on {}",
        std::env::consts::OS
    );
    Ok(window)
}
