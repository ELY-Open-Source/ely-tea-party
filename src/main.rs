// A release build on Windows opens no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use ely_tea_party::{
    app, handoff,
    settings::{self, SettingsStore, StartIn},
    skins,
    term::shell::{ShellIntegration, Start, launch},
};
use futures::{StreamExt, channel::mpsc::unbounded};
use gpui::App;

/// A Linux app with no display server fails at start instead of running headless.
#[cfg(target_os = "linux")]
fn application() -> gpui::Application {
    gpui_platform::linux(gpui::WindowingModes::WAYLAND | gpui::WindowingModes::X11)
}

#[cfg(not(target_os = "linux"))]
fn application() -> gpui::Application {
    gpui_platform::application()
}

fn run(cx: &mut App) -> anyhow::Result<()> {
    app::start(cx)?;
    let settings = SettingsStore::get(cx);
    // No session shows yet, so Session's folder means home.
    let start = match settings.start_in {
        StartIn::Folder => Start::In(settings::start_folder(&settings.start_folder)?),
        StartIn::Home | StartIn::Session => Start::Home,
    };
    let launch = launch(&settings.shell, start, &cx.global::<ShellIntegration>().0)?;
    app::open(launch, true, cx)?;
    cx.activate(true);
    Ok(())
}

/// `ely-tea-party hook AGENT` reports an agent's hook; `ely-tea-party hooks install [AGENT…]` installs them.
#[cfg(unix)]
fn command() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("hook") => {
            ely_tea_party::agents::hooks::run(&args[1..]);
            Some(0)
        }
        Some("hooks") if args.get(1).map(String::as_str) == Some("install") => {
            Some(ely_tea_party::agents::install::cli(&args[2..]))
        }
        _ => None,
    }
}

#[cfg(windows)]
fn command() -> Option<i32> {
    None
}

fn main() {
    if let Some(code) = command() {
        std::process::exit(code);
    }
    let links = handoff::links();
    #[cfg(not(target_os = "macos"))]
    if !links.is_empty() && handoff::forward(&links) {
        return;
    }
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("ely_tea_party=info,ely_gpui_component=info"),
    )
    .init();
    let (sender, mut received) = unbounded();
    for link in links {
        sender.unbounded_send(link).expect("the receiver is held");
    }
    let application = application().with_assets(ely_gpui_component::Assets);
    let opened = sender.clone();
    application.on_open_urls(move |urls| {
        for url in urls {
            if opened.unbounded_send(url).is_err() {
                log::error!("handoff: a link came after the app stopped hearing them");
            }
        }
    });
    application.run(move |cx: &mut App| {
        ely_gpui_component::init(cx).expect("Ely failed to start");
        if let Err(error) = run(cx) {
            log::error!("{error:#}");
            std::process::exit(1);
        }
        handoff::listen(sender);
        cx.spawn(async move |cx| {
            while let Some(link) = received.next().await {
                cx.update(|cx| skins::market::open(&link, cx));
            }
        })
        .detach();
    });
}
