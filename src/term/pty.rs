use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc};

#[cfg(windows)]
use alacritty_terminal::tty;
use alacritty_terminal::{
    event::{Event, EventListener, Notify, OnResize, WindowSize},
    event_loop::{EventLoop, Msg, Notifier},
    sync::FairMutex,
    term::{Config, Term, test::TermSize},
    vte::ansi::CursorShape,
};
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

use super::{mouse::MouseMode, tap::Tapped};
use crate::settings::{CursorStyle, Settings};

/// What a session hears from its pty.
#[derive(Debug)]
pub enum PtyEvent {
    Term(Event),
    Folder(PathBuf),
    Mouse(MouseMode),
}

/// Hands the grid's events to the session; `pid` names the program in logs.
#[derive(Clone)]
pub struct Listener {
    sender: UnboundedSender<PtyEvent>,
    pid: u32,
}

impl EventListener for Listener {
    fn send_event(&self, event: Event) {
        if let Event::ChildExit(status) = &event {
            log::info!("pty {}: the program ended, {status}", self.pid);
        }
        if self.sender.unbounded_send(PtyEvent::Term(event)).is_err() {
            log::debug!("pty {}: an event after its session closed", self.pid);
        }
    }
}

/// A program, the folder it starts in, and variables over the app's own.
#[derive(Clone, Debug)]
pub struct Launch {
    /// None runs the login shell.
    pub program: Option<(String, Vec<String>)>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    /// Shell integration makes it report its folder.
    pub reports: bool,
}

/// A running program on a pseudo-terminal and the grid it draws into.
pub struct Pty {
    pub term: Arc<FairMutex<Term<Listener>>>,
    pub pid: u32,
    notifier: Notifier,
    #[cfg(unix)]
    master: std::fs::File,
}

/// What a pty reports, in order.
pub type Events = UnboundedReceiver<PtyEvent>;

#[cfg(unix)]
type Raw = super::unix::Pty;
#[cfg(windows)]
type Raw = tty::Pty;

/// The grid's options from the settings: scrollback, the cursor programs start from, kitty keys.
pub fn config(settings: &Settings) -> Config {
    let shape = match settings.cursor_style {
        CursorStyle::Block => CursorShape::Block,
        CursorStyle::Bar => CursorShape::Beam,
        CursorStyle::Underline => CursorShape::Underline,
        CursorStyle::Hollow => CursorShape::HollowBlock,
    };
    Config {
        scrolling_history: settings.scrollback as usize,
        default_cursor_style: alacritty_terminal::vte::ansi::CursorStyle {
            shape,
            blinking: settings.cursor_blink,
        },
        kitty_keyboard: true,
        ..Config::default()
    }
}

/// Starts `launch` on a pty of `columns` by `lines` cells, each `cell` pixels.
pub fn spawn(
    launch: &Launch,
    (columns, lines): (usize, usize),
    cell: (u16, u16),
    config: Config,
) -> anyhow::Result<(Pty, Events)> {
    let mut env = HashMap::from([
        ("TERM".to_string(), "xterm-256color".to_string()),
        ("COLORTERM".to_string(), "truecolor".to_string()),
        ("TERM_PROGRAM".to_string(), "ely-tea-party".to_string()),
        (
            "TERM_PROGRAM_VERSION".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        ),
        // Programs that know no such name, as Node's supports-hyperlinks, still send OSC 8.
        ("FORCE_HYPERLINK".to_string(), "1".to_string()),
    ]);
    if std::env::var_os("LANG").is_none() {
        env.insert("LANG".into(), "en_US.UTF-8".into());
    }
    env.extend(launch.env.iter().cloned());
    let size = fit(columns, lines, cell);
    let raw = start(launch, env, size).map_err(|error| {
        anyhow::anyhow!(
            "pty: cannot start {:?} in {}: {error}",
            launch.program,
            launch.cwd.display()
        )
    })?;
    let pid = child_id(&raw)?;
    #[cfg(unix)]
    let master = raw
        .master()
        .map_err(|error| anyhow::anyhow!("pty {pid}: no copy of the master: {error}"))?;
    log::info!(
        "pty {pid}: started {:?} in {}",
        launch.program,
        launch.cwd.display()
    );
    let (sender, events) = unbounded();
    let listener = Listener {
        sender: sender.clone(),
        pid,
    };
    let grid = TermSize::new(size.num_cols.into(), size.num_lines.into());
    let term = Arc::new(FairMutex::new(Term::new(config, &grid, listener.clone())));
    let event_loop = EventLoop::new(
        term.clone(),
        listener,
        Tapped::new(raw, sender, pid),
        true,
        false,
    )?;
    let notifier = Notifier(event_loop.channel());
    event_loop.spawn();
    Ok((
        Pty {
            term,
            pid,
            notifier,
            #[cfg(unix)]
            master,
        },
        events,
    ))
}

#[cfg(unix)]
fn start(launch: &Launch, env: HashMap<String, String>, size: WindowSize) -> std::io::Result<Raw> {
    super::unix::spawn(launch.program.as_ref(), &launch.cwd, &env, size)
}

#[cfg(windows)]
fn start(launch: &Launch, env: HashMap<String, String>, size: WindowSize) -> std::io::Result<Raw> {
    let options = tty::Options {
        shell: launch
            .program
            .clone()
            .map(|(program, args)| tty::Shell::new(program, args)),
        working_directory: Some(launch.cwd.clone()),
        drain_on_exit: true,
        env,
        escape_args: true,
    };
    tty::new(&options, size, 0)
}

impl Pty {
    /// Sends bytes to the program, as typing does.
    pub fn write(&self, bytes: impl Into<Cow<'static, [u8]>>) {
        self.notifier.notify(bytes);
    }

    /// Fits the grid and tells the program its new size; both get the same counts.
    pub fn resize(&mut self, (columns, lines): (usize, usize), cell: (u16, u16)) {
        let size = fit(columns, lines, cell);
        self.notifier.on_resize(size);
        self.term
            .lock()
            .resize(TermSize::new(size.num_cols.into(), size.num_lines.into()));
    }

    /// The process group the terminal hands its keys to: the shell, or the job it runs.
    #[cfg(unix)]
    pub fn foreground(&self) -> Option<u32> {
        use std::os::fd::AsRawFd;
        let group = unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) };
        u32::try_from(group).ok().filter(|group| *group > 0)
    }

    #[cfg(windows)]
    pub fn foreground(&self) -> Option<u32> {
        None
    }
}

#[cfg(all(test, unix))]
impl Pty {
    /// Whether the program reads raw keys, canonical mode off.
    pub(crate) fn reads_keys(&self) -> bool {
        use rustix_openpty::rustix::termios::{LocalModes, tcgetattr};
        let modes = tcgetattr(&self.master).expect("the pty's modes");
        !modes.local_modes.contains(LocalModes::ICANON)
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if self.notifier.0.send(Msg::Shutdown).is_err() {
            log::debug!("pty {}: the event loop had already stopped", self.pid);
        } else {
            log::info!("pty {}: closing", self.pid);
        }
    }
}

#[cfg(unix)]
fn child_id(pty: &Raw) -> anyhow::Result<u32> {
    Ok(pty.id())
}

#[cfg(windows)]
fn child_id(pty: &Raw) -> anyhow::Result<u32> {
    pty.child_watcher()
        .pid()
        .map(u32::from)
        .ok_or_else(|| anyhow::anyhow!("pty: Windows gave the program no id"))
}

/// A pty's size, at least 2 by 1; counts past what the kernel holds are cut and logged.
pub fn fit(columns: usize, lines: usize, cell: (u16, u16)) -> WindowSize {
    let cells = |count: usize, side: u16| {
        let most = (i16::MAX as u16).min(u16::MAX / side.max(1));
        let kept = u16::try_from(count).unwrap_or(u16::MAX).min(most);
        if usize::from(kept) < count {
            log::error!("pty: {count} cells of {side} px pass what a pty holds; {kept} kept");
        }
        kept
    };
    WindowSize {
        num_lines: cells(lines.max(1), cell.1),
        num_cols: cells(columns.max(2), cell.0),
        cell_width: cell.0,
        cell_height: cell.1,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use alacritty_terminal::{
        grid::Dimensions,
        index::{Column, Line},
    };
    use futures::StreamExt;

    use super::*;

    struct Ran {
        folders: Vec<PathBuf>,
        text: String,
        success: bool,
        pid: u32,
        columns: usize,
    }

    fn run(script: &str, size: (usize, usize)) -> Ran {
        crate::testlog::init();
        let launch = Launch {
            program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
            cwd: std::env::temp_dir(),
            env: vec![("ELY".into(), "ok".into())],
            reports: false,
        };
        let (pty, mut events) =
            spawn(&launch, size, (8, 16), config(&Settings::default())).expect("sh starts");
        let (folders, success) = futures::executor::block_on(async {
            let (mut folders, mut success) = (Vec::new(), false);
            while let Some(event) = events.next().await {
                match event {
                    PtyEvent::Folder(folder) => folders.push(folder),
                    PtyEvent::Term(Event::ChildExit(status)) => success = status.success(),
                    PtyEvent::Term(Event::Exit) => break,
                    PtyEvent::Term(_) | PtyEvent::Mouse(_) => {}
                }
            }
            (folders, success)
        });
        let term = pty.term.lock();
        let row = &term.grid()[Line(0)];
        let text: String = (0..24).map(|column| row[Column(column)].c).collect();
        Ran {
            folders,
            text: text.trim_end().to_string(),
            success,
            pid: pty.pid,
            columns: term.columns(),
        }
    }

    #[test]
    fn a_program_prints_into_the_grid_with_its_variables_and_id() {
        let ran = run("printf 'ely %s %s' \"$ELY\" $$", (40, 4));
        assert!(ran.folders.is_empty());
        assert_eq!(ran.text, format!("ely ok {}", ran.pid));
        assert!(ran.success);
    }

    #[test]
    fn osc_7_reaches_the_session_as_a_folder() {
        let ran = run(
            "printf '\\033]7;file:///tmp/a%%20b\\007x\\033]7;file://elsewhere.invalid/c\\033\\\\'",
            (40, 4),
        );
        assert_eq!(ran.folders, [PathBuf::from("/tmp/a b")]);
        assert_eq!(ran.text, "x");
    }

    #[test]
    fn a_size_past_the_kernel_is_one_size_for_grid_and_program() {
        let ran = run("stty size", (9000, 8));
        assert_eq!(ran.columns, usize::from(u16::MAX / 8));
        assert_eq!(ran.text, format!("8 {}", u16::MAX / 8));
    }

    /// Starts `script`, waits for it to print READY, closes it, and waits for the reap's log.
    fn close_after_ready(script: &str, reaped: &str) {
        crate::testlog::init();
        let launch = Launch {
            program: Some(("/bin/sh".into(), vec!["-c".into(), script.into()])),
            cwd: std::env::temp_dir(),
            env: Vec::new(),
            reports: false,
        };
        let (pty, mut events) =
            spawn(&launch, (20, 4), (8, 16), config(&Settings::default())).expect("sh starts");
        futures::executor::block_on(async {
            while let Some(event) = events.next().await {
                let term = pty.term.lock();
                let text: String = (0..5)
                    .map(|column| term.grid()[Line(0)][Column(column)].c)
                    .collect();
                drop(term);
                if matches!(event, PtyEvent::Term(Event::Wakeup)) && text == "READY" {
                    break;
                }
            }
        });
        let pid = pty.pid;
        drop(pty);
        let hung_up = format!("pty {pid}: hung up, {reaped}");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !crate::testlog::logged(&hung_up) {
            assert!(std::time::Instant::now() < deadline, "no `{hung_up}`");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(crate::testlog::logged(&format!("pty {pid}: closing")));
        let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
        assert!(!alive, "the program was reaped");
    }

    #[test]
    fn closing_a_session_hangs_up_its_program() {
        close_after_ready("printf READY; exec cat", "signal: 1 (SIGHUP)");
    }

    #[test]
    fn a_program_that_ignores_sighup_ends_when_the_pty_closes() {
        // cat reads EOF on macOS and EIO on Linux; the shell's own exit holds on both.
        close_after_ready("trap '' HUP; printf READY; cat; exit 7", "exit status: 7");
    }

    #[test]
    fn a_missing_folder_stops_the_start() {
        let launch = Launch {
            program: None,
            cwd: "/nonexistent/ely".into(),
            env: Vec::new(),
            reports: false,
        };
        let error = spawn(&launch, (20, 4), (8, 16), config(&Settings::default()))
            .err()
            .expect("a missing folder is an error");
        assert!(error.to_string().contains("/nonexistent/ely"));
    }

    #[test]
    fn a_folder_that_cannot_be_entered_stops_the_start() {
        use std::os::unix::fs::PermissionsExt;
        let folder =
            std::env::temp_dir().join(format!("ely-tea-party-shut-{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a temp folder");
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o600)).expect("600");
        let launch = Launch {
            program: Some(("/bin/sh".into(), vec!["-c".into(), "true".into()])),
            cwd: folder.clone(),
            env: Vec::new(),
            reports: false,
        };
        let started = spawn(&launch, (20, 4), (8, 16), config(&Settings::default()));
        std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700)).expect("700");
        std::fs::remove_dir(&folder).expect("the temp folder goes");
        let error = started.err().expect("a folder it cannot enter is an error");
        let error = error.to_string();
        assert!(error.contains("Permission denied"), "{error}");
        assert!(error.contains(&folder.display().to_string()), "{error}");
    }

    #[test]
    fn sizes_hold_at_least_two_by_one_and_cap_at_the_kernel() {
        let size = fit(70_000, 24, (8, 16));
        assert_eq!((size.num_cols, size.num_lines), (u16::MAX / 8, 24));
        let size = fit(0, 0, (8, 16));
        assert_eq!((size.num_cols, size.num_lines), (2, 1));
    }
}

#[cfg(test)]
mod scrollback {
    use alacritty_terminal::{event::VoidListener, grid::Dimensions, vte::ansi::Processor};

    use super::*;

    #[test]
    fn the_setting_caps_history_and_a_change_trims_it() {
        let settings = Settings {
            scrollback: 5,
            ..Settings::default()
        };
        let mut term = Term::new(config(&settings), &TermSize::new(10, 3), VoidListener);
        let mut parser: Processor = Processor::new();
        let lines: String = (1..=20).map(|line| format!("{line}\r\n")).collect();
        parser.advance(&mut term, lines.as_bytes());
        assert_eq!(term.grid().history_size(), 5);
        term.set_options(config(&Settings {
            scrollback: 2,
            ..settings
        }));
        assert_eq!(term.grid().history_size(), 2);
    }
}
