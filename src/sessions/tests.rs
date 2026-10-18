use super::*;

#[test]
fn folders_under_home_read_from_a_tilde() {
    let home = PathBuf::from("/Users/ely");
    let shown = |folder: &str| shown(Path::new(folder), Some(&home));
    assert_eq!(shown("/Users/ely"), "~");
    assert_eq!(
        shown("/Users/ely/src/app"),
        format!("~{MAIN_SEPARATOR}src/app")
    );
    assert_eq!(
        shown("/Users/elyse"),
        "/Users/elyse",
        "a sibling is no child"
    );
    assert_eq!(shown("/tmp"), "/tmp");
    assert_eq!(
        shown("/Users/ely/line\nbreak"),
        format!("~{MAIN_SEPARATOR}line␊break")
    );
}

#[test]
fn long_folders_cut_all_but_the_last_name_to_a_letter() {
    let home = PathBuf::from("/Users/ely");
    let sep = MAIN_SEPARATOR;
    // Windows names folders with either separator, so each test folder is written in its own.
    let brief = |folder: &str| {
        brief(
            Path::new(&folder.replace('/', std::path::MAIN_SEPARATOR_STR)),
            Some(&home),
        )
    };
    assert_eq!(
        brief("/Users/ely/Documents/notes"),
        format!("~{sep}Documents{sep}notes"),
        "short enough"
    );
    assert_eq!(
        brief("/Users/ely/Workspace/other/ely-tea-party/ELYTerminal"),
        format!("~{sep}W{sep}o{sep}e{sep}ELYTerminal")
    );
    assert_eq!(
        brief("/Users/ely/.config/opencode/plugins/x"),
        format!("~{sep}.c{sep}o{sep}p{sep}x"),
        "a hidden folder keeps its dot"
    );
    assert_eq!(
        brief("/opt/homebrew/lib/node_modules/gemini"),
        format!("{sep}o{sep}h{sep}l{sep}n{sep}gemini"),
        "outside home"
    );
}

#[test]
fn control_characters_show_as_their_pictures() {
    assert_eq!(one_line("a\nb\tc\rd\0e\u{7f}"), "a␊b␉c␍d␀e␡");
    assert_eq!(one_line("café ✓"), "café ✓", "printable text stays");
}

#[test]
fn moving_keeps_the_shown_session_shown() {
    assert_eq!(after_move(0, 0, 2), 2, "the shown one moved");
    assert_eq!(after_move(1, 0, 2), 0, "one from before it went past it");
    assert_eq!(after_move(1, 2, 0), 2, "one from after it went before it");
    assert_eq!(after_move(3, 0, 2), 3, "the move happened above it");
}

#[test]
fn closing_shows_the_next_session_or_keeps_the_shown_one() {
    assert_eq!(after_close(2, 0, 3), 1, "one before it went");
    assert_eq!(after_close(1, 1, 3), 1, "it went: the next one");
    assert_eq!(after_close(3, 3, 3), 2, "the last went: the one before");
    assert_eq!(after_close(1, 2, 3), 1, "one after it went");
}

#[test]
fn the_mark_shows_the_most_pressing_look() {
    let record = |state, progress| Record {
        state,
        kind: None,
        progress,
        app: None,
        title: None,
        msg: None,
    };
    let waiting = look(Some(&record(State::Blocked, None)), true);
    assert_eq!(
        waiting,
        Some(Look::Waiting),
        "waiting outranks a failed command"
    );
    let working = look(Some(&record(State::Working, Some(40))), false);
    assert_eq!(working, Some(Look::Working(Some(40))));
    let idle = record(State::Idle, None);
    assert_eq!(
        look(Some(&idle), true),
        Some(Look::Error),
        "a failed command outranks idle"
    );
    assert_eq!(look(Some(&idle), false), Some(Look::Idle));
    assert_eq!(
        look(Some(&record(State::Done, None)), true),
        Some(Look::Done)
    );
    assert_eq!(
        look(Some(&record(State::Error, None)), false),
        Some(Look::Error)
    );
    assert_eq!(look(None, false), None);
}

#[test]
fn hover_shows_a_program_s_text_as_text() {
    assert_eq!(plain("evil\u{202e}txt.exe"), "eviltxt.exe");
    assert_eq!(
        plain("a\u{200b}b\u{feff}c\u{e0041}d\u{85}e\u{2066}f"),
        "abcdef"
    );
    assert_eq!(
        plain("👨\u{200d}💻 two\nlines"),
        "👨\u{200d}💻 two␊lines",
        "joiners stay"
    );
    assert_eq!(plain(&"x".repeat(SHORT)), "x".repeat(SHORT));
    assert_eq!(
        plain(&"é".repeat(SHORT + 1)),
        format!("{}…", "é".repeat(SHORT))
    );
}

#[test]
fn hover_lists_each_record_with_its_app_then_a_failed_command() {
    let record =
        |state: State, kind: Option<Kind>, progress: Option<u8>, texts: [Option<&str>; 3]| {
            let [app, title, msg] = texts.map(|text| text.map(String::from));
            Record {
                state,
                kind,
                progress,
                app,
                title,
                msg,
            }
        };
    let mut records = Records::default();
    records.report(
        String::new(),
        record(
            State::Working,
            None,
            Some(40),
            [Some("deploy"), None, Some("Deploying")],
        ),
    );
    records.report(
        "eu".into(),
        record(
            State::Blocked,
            Some(Kind::Permission),
            None,
            [None, Some("EU West"), Some("Approve?")],
        ),
    );
    records.report(
        "eu/db".into(),
        record(State::Done, None, None, [Some("db"), None, None]),
    );
    let lines: Vec<String> = hover(&records, Some(2))
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        lines,
        [
            "Working, 40%: deploy · Deploying",
            "Waiting for permission: deploy · EU West · Approve?",
            "Done: db",
            "Last command exited with 2"
        ]
    );
    for at in 0..LISTED {
        records.report(format!("r{at}"), record(State::Idle, None, None, [None; 3]));
    }
    let lines = hover(&records, None);
    assert_eq!(lines.len(), LISTED + 1);
    assert_eq!(lines[LISTED].to_string(), "3 more");
}
