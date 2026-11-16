// A release build on Windows opens no console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Context as _;
use ely_tea_party::{
    app, handoff,
    sessions::Program,
    settings::{self, SettingsStore, StartIn},
    skins,
    term::shell::{ShellIntegration, Start, command, launch},
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
    #[cfg(windows)]
    if app::args::embedded(std::env::args_os().skip(1)) {
        log::info!("app: COM started this copy for a console session");
        app::start(cx)?;
        ely_tea_party::defterm::listen(cx)?;
        return wait_for_handoff(cx);
    }
    // A link goes to the market once the app runs; only a folder or a program picks the first session.
    let words = std::env::args_os()
        .skip(1)
        .filter(|arg| !arg.to_str().is_some_and(handoff::is_link));
    let asked = app::args::parse(words)?;
    app::start(cx)?;
    #[cfg(windows)]
    if let Err(error) = ely_tea_party::defterm::listen(cx) {
        log::error!("{error:#}");
    }
    let restore = match asked.folder.is_some() || asked.command.is_some() {
        true => app::Restore::Behind,
        false => app::Restore::First,
    };
    let settings = SettingsStore::get(cx);
    let (launch, program) = if let Some((shell, args)) = asked.command {
        let folder = match asked.folder {
            Some(folder) => folder,
            None => std::env::current_dir().context("app: no current folder for -e")?,
        };
        let launch = command(&shell, &args, folder)?;
        let program = Program {
            shell,
            args,
            written: true,
        };
        (launch, program)
    } else {
        // No session shows yet, so Session's folder means home.
        let start = match (asked.folder, settings.start_in) {
            (Some(folder), _) => Start::In(folder),
            (None, StartIn::Folder) => Start::In(settings::start_folder(&settings.start_folder)?),
            (None, StartIn::Home | StartIn::Session) => Start::Home,
        };
        let launch = launch(
            &settings.shell,
            &[],
            start,
            &cx.global::<ShellIntegration>().0,
        )?;
        let program = Program {
            shell: settings.shell.clone(),
            ..Program::default()
        };
        (launch, program)
    };
    app::open(launch, program, restore, cx)?;
    cx.activate(true);
    Ok(())
}

/// `ely-tea-party hook AGENT` reports an agent's hook; `ely-tea-party hooks install [AGENT…]` installs them.
#[cfg(unix)]
fn hooks_command() -> Option<i32> {
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
fn hooks_command() -> Option<i32> {
    None
}

/// A copy COM started that gets no session in 30 seconds stops.
#[cfg(windows)]
fn wait_for_handoff(cx: &mut App) -> anyhow::Result<()> {
    cx.spawn(async |cx| {
        cx.background_executor()
            .timer(std::time::Duration::from_secs(30))
            .await;
        if cx.update(|cx| cx.windows().is_empty()) {
            log::error!("app: COM started this copy, and no console session came in 30 seconds");
            std::process::exit(1);
        }
    })
    .detach();
    Ok(())
}

fn main() {
    if let Some(code) = ely_tea_party::communication::command() {
        std::process::exit(code);
    }
    if let Some(code) = hooks_command() {
        std::process::exit(code);
    }
    if let Some(code) = ely_tea_party::control::command_line() {
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
    let application = application().with_assets(ely_gpui_component::Assets::before(
        ely_tea_party::agents::icons::Icons,
    ));
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
