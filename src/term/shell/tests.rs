use std::time::{Duration, Instant};

use alacritty_terminal::{event::Event, grid::Dimensions, index::Line};
use futures::channel::mpsc::TryRecvError;

use super::*;
use crate::{
    settings::Settings,
    term::{Events, Pty, PtyEvent, config, spawn},
};

/// A folder name with a space, a percent, a bell and a newline.
const INNER: &str = "in ner%\u{7}x\ny";

/// A fresh folder under the temp dir, by its real path.
fn folder(name: &str) -> PathBuf {
    let folder = env::temp_dir().join(format!("ely-tea-party-{name}-{}", std::process::id()));
    if folder.exists() {
        fs::remove_dir_all(&folder).expect("a stale test folder goes");
    }
    fs::create_dir_all(&folder).expect("a test folder");
    folder.canonicalize().expect("a real path")
}

fn installed(shell: &str) -> String {
    let found = std::process::Command::new("/bin/sh")
        .args(["-c", &format!("command -v {shell}")])
        .output()
        .expect("sh runs");
    assert!(found.status.success(), "{shell} is not installed");
    String::from_utf8(found.stdout)
        .expect("utf-8")
        .trim()
        .to_string()
}

/// A shell started with integration under a fake HOME, and what it reported.
struct Run {
    shell: String,
    pty: Pty,
    events: Events,
    root: PathBuf,
    start: PathBuf,
    folders: Vec<PathBuf>,
    titles: Vec<String>,
    deadline: Instant,
}

impl Run {
    /// Starts `shell` in a fresh folder; `files` go under its HOME first.
    fn start(shell: &str, files: &[(&str, &str)]) -> Self {
        crate::testlog::init();
        let root = folder(&format!(
            "shell-{}",
            shell.rsplit('/').next().expect("a name")
        ));
        let (dir, home, start) = (
            root.join("integration"),
            root.join("home"),
            root.join("start"),
        );
        fs::create_dir_all(start.join(INNER)).expect("the inner folder");
        fs::create_dir_all(&home).expect("home");
        for (name, text) in files {
            let path = home.join(name);
            fs::create_dir_all(path.parent().expect("a folder")).expect("a home folder");
            fs::write(path, text).expect("a home file");
        }
        install(&dir).expect("scripts install");
        let mut launch = launch(shell, start.clone(), &dir).expect("a launch");
        launch.env.push(("HOME".into(), home.display().to_string()));
        let (pty, events) = spawn(&launch, (100, 30), (8, 16), config(&Settings::default()))
            .expect("the shell starts");
        Self {
            shell: shell.into(),
            pty,
            events,
            root,
            start,
            folders: Vec::new(),
            titles: Vec::new(),
            deadline: Instant::now() + Duration::from_secs(30),
        }
    }

    /// Reads events, answering the shell's terminal queries, until `done` holds.
    fn until(&mut self, what: &str, done: impl Fn(&Self) -> bool) {
        while !done(self) {
            assert!(
                Instant::now() < self.deadline,
                "{}: no {what}; folders {:?}, titles {:?}, screen\n{}",
                self.shell,
                self.folders,
                self.titles,
                self.screen()
            );
            match self.events.try_recv() {
                Ok(PtyEvent::Folder(folder)) => self.folders.push(folder),
                Ok(PtyEvent::Term(Event::Title(title))) => self.titles.push(title),
                Ok(PtyEvent::Term(Event::PtyWrite(reply))) => self.pty.write(reply.into_bytes()),
                Ok(PtyEvent::Term(Event::ChildExit(status))) => {
                    panic!("{} ended, {status}:\n{}", self.shell, self.screen())
                }
                Ok(_) => {}
                Err(TryRecvError::Closed) => panic!("{}'s events ended", self.shell),
                Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }

    /// Types `line` once the line editor reads raw keys.
    fn send(&mut self, line: &str) {
        self.until("line editor", |run| run.pty.reads_keys());
        self.pty.write(format!("{line}\r").into_bytes());
    }

    fn shows(&self, text: &str) -> bool {
        self.screen().contains(text)
    }

    fn screen(&self) -> String {
        let term = self.pty.term.lock();
        (0..term.screen_lines())
            .map(|line| {
                let row = &term.grid()[Line(line as i32)];
                let text: String = row.into_iter().map(|cell| cell.c).collect();
                text.trim_end().to_string()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The prompt's folder, the script loaded by `probe`, then a cd into the inner folder.
    fn walk(&mut self, probe: &str) {
        let start = self.start.clone();
        self.send(probe);
        self.until("first folder", |run| run.folders.contains(&start));
        assert_eq!(self.folders[0], start);
        self.until("loaded script", |run| run.shows("ELY-LOADED"));
        self.send("cd in*");
        let inner = start.join(INNER);
        self.until("inner folder", |run| run.folders.contains(&inner));
    }

    fn finish(mut self) {
        self.send("exit");
        loop {
            assert!(
                Instant::now() < self.deadline,
                "{} did not exit",
                self.shell
            );
            match self.events.try_recv() {
                Ok(PtyEvent::Term(Event::ChildExit(_))) | Err(TryRecvError::Closed) => break,
                Ok(PtyEvent::Term(Event::PtyWrite(reply))) => self.pty.write(reply.into_bytes()),
                Ok(_) => {}
                Err(TryRecvError::Empty) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        let root = self.root.clone();
        drop(self);
        fs::remove_dir_all(&root).expect("cleanup");
    }
}

#[test]
fn the_scripts_install_once_and_mend_a_changed_one() {
    let dir = folder("install");
    install(&dir).expect("installs");
    let zshenv = dir.join("zsh/.zshenv");
    assert_eq!(fs::read_to_string(&zshenv).expect("written"), SCRIPTS[0].1);
    fs::write(&zshenv, [0xff, 0xfe]).expect("write");
    install(&dir).expect("installs again");
    assert_eq!(fs::read_to_string(&zshenv).expect("mended"), SCRIPTS[0].1);
    fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn a_script_it_cannot_read_fails_the_install() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("unreadable");
    install(&dir).expect("installs");
    let bash = dir.join("bash/ely-tea-party.bash");
    fs::set_permissions(&bash, fs::Permissions::from_mode(0o200)).expect("200");
    let error = install(&dir).expect_err("an unreadable script fails");
    fs::set_permissions(&bash, fs::Permissions::from_mode(0o600)).expect("600");
    assert!(
        format!("{error:#}").contains("bash/ely-tea-party.bash"),
        "{error:#}"
    );
    fs::remove_dir_all(dir).expect("cleanup");
}

#[test]
fn zsh_reports_its_folder_byte_for_byte_and_its_titles() {
    let mut run = Run::start(&installed("zsh"), &[]);
    run.walk("(( $+functions[_ely_precmd] )) && echo ELY-LOAD''ED");
    run.until("title", |run| {
        run.titles.iter().any(|title| title == "in ner%xy")
    });
    assert!(
        run.titles.contains(&"cd in*".to_string()),
        "{:?}",
        run.titles
    );
    run.finish();
}

#[test]
fn bash_reports_its_folder_byte_for_byte_and_keeps_the_status() {
    let status = r#"PROMPT_COMMAND='printf "USER_STATUS=%s\n" "$?"'"#;
    let files = [(".bashrc", status), (".bash_profile", status)];
    let mut run = Run::start(&installed("bash"), &files);
    run.walk("declare -F _ely_prompt >/dev/null && echo ELY-LOAD''ED");
    run.until("title", |run| {
        run.titles.iter().any(|title| title == "in ner%xy")
    });
    run.send("false");
    run.until("kept status", |run| run.shows("USER_STATUS=1"));
    run.finish();
}

#[test]
fn fish_loads_its_script_and_restores_its_data_dirs() {
    let mut run = Run::start(&installed("fish"), &[]);
    run.walk("functions -q _ely_prompt; and echo ELY-LOAD''ED");
    let previous = env::var("XDG_DATA_DIRS").unwrap_or_default();
    run.send(r#"echo "DIRS[$ELY_FISH_XDG_DATA_DIRS][$XDG_DATA_DIRS]""#);
    let restored = format!("DIRS[][{previous}]");
    run.until("restored dirs", |run| run.shows(&restored));
    run.finish();
}

#[cfg(target_os = "macos")]
#[test]
fn powershell_reports_its_folder_byte_for_byte_and_keeps_the_status() {
    let prompt = r#"function global:prompt { "USER_SUCCESS=$? > " }"#;
    let files = [(
        ".config/powershell/Microsoft.PowerShell_profile.ps1",
        prompt,
    )];
    let mut run = Run::start(&installed("pwsh"), &files);
    run.walk("if ($global:ElyPrompt) { 'ELY-LOAD' + 'ED' }");
    run.until("title", |run| {
        run.titles.iter().any(|title| title == "in ner%xy")
    });
    run.send("/usr/bin/false");
    run.until("kept status", |run| run.shows("USER_SUCCESS=False"));
    run.finish();
}

#[test]
fn a_shell_named_bare_runs_from_its_full_path() {
    let started = launch("sh", PathBuf::from("/"), Path::new("/nonexistent")).expect("a launch");
    let (program, _) = started.program.expect("a program");
    assert_eq!(program, crate::settings::program_path("sh").expect("sh"));
    assert!(Path::new(&program).is_absolute(), "{program}");
    assert!(launch("no-such-shell", PathBuf::from("/"), Path::new("/")).is_err());
}

#[test]
fn an_unknown_shell_starts_without_integration() {
    let dir = PathBuf::from("/nonexistent");
    let launch = launch("/bin/sh", PathBuf::from("/"), &dir).expect("a launch");
    assert_eq!(launch.program, Some(("/bin/sh".into(), Vec::new())));
    assert!(launch.env.is_empty());
}
