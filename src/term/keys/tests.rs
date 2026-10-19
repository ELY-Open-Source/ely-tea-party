use super::*;

/// A key pressed with modifiers written as `ctrl+alt+shift+name`.
fn key(spec: &'static str, text: &'static str) -> Key<'static> {
    let mut parts: Vec<&str> = spec.split('+').collect();
    let name = parts.pop().expect("a name");
    Key {
        name,
        text,
        shift: parts.contains(&"shift"),
        ctrl: parts.contains(&"ctrl"),
        alt: parts.contains(&"alt"),
        platform: parts.contains(&"cmd"),
        action: Action::Press,
        unshifted: None,
    }
}

fn legacy(key: Key, app_cursor: bool) -> Option<String> {
    encode(&key, app_cursor, Kitty::default()).map(|bytes| String::from_utf8(bytes).expect("utf-8"))
}

fn kitty(key: Key, flags: Kitty) -> Option<String> {
    encode(&key, false, flags).map(|bytes| String::from_utf8(bytes).expect("utf-8"))
}

const DISAMBIGUATE: Kitty = Kitty {
    disambiguate: true,
    events: false,
    alternates: false,
    all: false,
    text: false,
};

#[test]
fn plain_text_is_left_to_the_input_method() {
    assert_eq!(legacy(key("a", "a"), false), None);
    assert_eq!(legacy(key("shift+a", "A"), false), None);
    assert_eq!(legacy(key("space", " "), false), None);
    assert_eq!(legacy(key("ctrl+a", ""), false).as_deref(), Some("\x01"));
    let released = Key {
        action: Action::Release,
        ..key("enter", "\n")
    };
    assert_eq!(legacy(released, false), None, "legacy sends no releases");
}

#[test]
fn cursor_keys_follow_decckm_and_modifiers() {
    assert_eq!(legacy(key("up", ""), false).as_deref(), Some("\x1b[A"));
    assert_eq!(legacy(key("up", ""), true).as_deref(), Some("\x1bOA"));
    assert_eq!(
        legacy(key("ctrl+up", ""), true).as_deref(),
        Some("\x1b[1;5A")
    );
    assert_eq!(
        legacy(key("alt+shift+left", ""), false).as_deref(),
        Some("\x1b[1;4D")
    );
    assert_eq!(legacy(key("home", ""), true).as_deref(), Some("\x1bOH"));
    assert_eq!(legacy(key("end", ""), false).as_deref(), Some("\x1b[F"));
}

#[test]
fn function_and_editing_keys_send_xterm_sequences() {
    let cases = [
        ("f1", "\x1bOP"),
        ("ctrl+f1", "\x1b[1;5P"),
        ("f3", "\x1bOR"),
        ("shift+f3", "\x1b[13;2~"),
        ("f4", "\x1bOS"),
        ("f5", "\x1b[15~"),
        ("shift+f5", "\x1b[15;2~"),
        ("f12", "\x1b[24~"),
        ("f25", "\x1b[46~"),
        ("insert", "\x1b[2~"),
        ("delete", "\x1b[3~"),
        ("ctrl+pageup", "\x1b[5;5~"),
        ("pagedown", "\x1b[6~"),
    ];
    for (spec, want) in cases {
        assert_eq!(
            legacy(key(spec, ""), false).as_deref(),
            Some(want),
            "{spec}"
        );
    }
    assert_eq!(legacy(key("f26", ""), false), None);
}

#[test]
fn enter_tab_escape_and_backspace_carry_their_modifiers() {
    let cases: [(&str, &str, &[u8]); 15] = [
        ("enter", "\n", b"\r"),
        ("shift+enter", "\n", b"\x1b[27;2;13~"),
        ("alt+enter", "\n", b"\x1b\r"),
        ("ctrl+enter", "", b"\x1b[27;5;13~"),
        ("tab", "\t", b"\t"),
        ("shift+tab", "", b"\x1b[Z"),
        ("alt+tab", "\t", b"\x1b\t"),
        ("ctrl+tab", "", b"\x1b[27;5;9~"),
        ("escape", "", b"\x1b"),
        ("alt+escape", "", b"\x1b\x1b"),
        ("shift+escape", "", b"\x1b[27;2;27~"),
        ("backspace", "", b"\x7f"),
        ("ctrl+backspace", "", b"\x08"),
        ("alt+backspace", "", b"\x1b\x7f"),
        ("ctrl+alt+backspace", "", b"\x1b\x08"),
    ];
    for (spec, text, want) in cases {
        assert_eq!(
            encode(&key(spec, text), false, Kitty::default()).as_deref(),
            Some(want),
            "{spec}"
        );
    }
}

#[test]
fn ctrl_makes_c0_bytes_or_fixterms_csi_u() {
    let bytes = |spec| encode(&key(spec, ""), false, Kitty::default());
    assert_eq!(bytes("ctrl+c"), Some(vec![0x03]));
    assert_eq!(bytes("ctrl+alt+c"), Some(vec![0x1B, 0x03]));
    assert_eq!(bytes("ctrl+space"), Some(vec![0x00]));
    assert_eq!(bytes("ctrl+_"), Some(vec![31]));
    assert_eq!(bytes("ctrl+?"), Some(vec![127]));
    assert_eq!(bytes("ctrl+2"), Some(vec![0x00]));
    assert_eq!(bytes("ctrl+["), Some(b"\x1b[91;5u".to_vec()));
    assert_eq!(bytes("ctrl+i"), Some(b"\x1b[105;5u".to_vec()));
    assert_eq!(bytes("ctrl+m"), Some(b"\x1b[109;5u".to_vec()));
    assert_eq!(bytes("ctrl+shift+a"), Some(b"\x1b[97;6u".to_vec()));
}

#[test]
fn alt_prefixes_escape_to_what_the_key_types() {
    assert_eq!(legacy(key("alt+a", "a"), false).as_deref(), Some("\x1ba"));
    assert_eq!(
        legacy(key("alt+shift+a", ""), false).as_deref(),
        Some("\x1bA")
    );
    assert_eq!(legacy(key("alt+e", "é"), false).as_deref(), Some("\x1bé"));
    assert_eq!(
        legacy(key("alt+space", " "), false).as_deref(),
        Some("\x1b ")
    );
}

#[test]
fn kitty_disambiguate_keeps_text_and_the_legacy_three() {
    assert_eq!(kitty(key("a", "a"), DISAMBIGUATE), None);
    assert_eq!(kitty(key("shift+a", "A"), DISAMBIGUATE), None);
    assert_eq!(
        kitty(key("enter", "\n"), DISAMBIGUATE).as_deref(),
        Some("\r")
    );
    assert_eq!(kitty(key("tab", "\t"), DISAMBIGUATE).as_deref(), Some("\t"));
    assert_eq!(
        kitty(key("backspace", ""), DISAMBIGUATE).as_deref(),
        Some("\x7f")
    );
    assert_eq!(
        kitty(key("escape", ""), DISAMBIGUATE).as_deref(),
        Some("\x1b[27u")
    );
}

#[test]
fn kitty_reports_modified_keys_as_csi_u() {
    let cases = [
        ("ctrl+a", "", "\x1b[97;5u"),
        ("alt+a", "a", "\x1b[97;3u"),
        ("ctrl+shift+a", "", "\x1b[97;6u"),
        ("shift+enter", "\n", "\x1b[13;2u"),
        ("shift+tab", "", "\x1b[9;2u"),
        ("up", "", "\x1b[A"),
        ("ctrl+up", "", "\x1b[1;5A"),
        ("f1", "", "\x1b[P"),
        ("f5", "", "\x1b[15~"),
        ("ctrl+f5", "", "\x1b[15;5~"),
        ("f13", "", "\x1b[57376u"),
        ("f25", "", "\x1b[57388u"),
    ];
    for (spec, text, want) in cases {
        assert_eq!(
            kitty(key(spec, text), DISAMBIGUATE).as_deref(),
            Some(want),
            "{spec}"
        );
    }
}

#[test]
fn kitty_events_report_repeats_and_releases() {
    let flags = Kitty {
        events: true,
        ..DISAMBIGUATE
    };
    let with = |spec, text, action| Key {
        action,
        ..key(spec, text)
    };
    assert_eq!(
        kitty(with("a", "a", Action::Repeat), flags),
        None,
        "a repeat types again"
    );
    assert_eq!(
        kitty(with("a", "a", Action::Release), flags).as_deref(),
        Some("\x1b[97;1:3u")
    );
    assert_eq!(
        kitty(with("ctrl+a", "", Action::Repeat), flags).as_deref(),
        Some("\x1b[97;5:2u")
    );
    assert_eq!(kitty(with("enter", "\n", Action::Release), flags), None);
    assert_eq!(
        kitty(with("up", "", Action::Release), flags).as_deref(),
        Some("\x1b[1;1:3A")
    );
    assert_eq!(kitty(with("a", "a", Action::Release), DISAMBIGUATE), None);
}

#[test]
fn kitty_report_all_alternates_and_text() {
    let all = Kitty {
        all: true,
        ..DISAMBIGUATE
    };
    assert_eq!(kitty(key("a", "a"), all).as_deref(), Some("\x1b[97u"));
    assert_eq!(
        kitty(key("shift+a", "A"), all).as_deref(),
        Some("\x1b[97;2u")
    );
    assert_eq!(kitty(key("enter", "\n"), all).as_deref(), Some("\x1b[13u"));
    let alternates = Kitty {
        alternates: true,
        ..all
    };
    assert_eq!(
        kitty(key("shift+a", "A"), alternates).as_deref(),
        Some("\x1b[97:65;2u")
    );
    let text = Kitty { text: true, ..all };
    assert_eq!(kitty(key("a", "a"), text).as_deref(), Some("\x1b[97;;97u"));
    assert_eq!(
        kitty(key("shift+a", "A"), text).as_deref(),
        Some("\x1b[97;2;65u")
    );
    assert_eq!(
        kitty(key("ctrl+a", ""), text).as_deref(),
        Some("\x1b[97;5u")
    );
    let released = Key {
        action: Action::Release,
        ..key("enter", "\n")
    };
    let everything = Kitty {
        events: true,
        ..all
    };
    assert_eq!(kitty(released, everything).as_deref(), Some("\x1b[13;1:3u"));
}

#[test]
fn flags_that_only_add_fields_keep_legacy_keys() {
    let alternates = Kitty {
        alternates: true,
        ..Kitty::default()
    };
    let bytes = |spec, flags, app_cursor| encode(&key(spec, ""), app_cursor, flags);
    assert_eq!(bytes("ctrl+c", alternates, false), Some(vec![0x03]));
    assert_eq!(bytes("escape", alternates, false), Some(b"\x1b".to_vec()));
    assert_eq!(bytes("up", alternates, true), Some(b"\x1bOA".to_vec()));
    assert_eq!(bytes("f1", alternates, false), Some(b"\x1bOP".to_vec()));
    assert_eq!(
        bytes("ctrl+shift+a", alternates, false),
        Some(b"\x1b[97:65;6u".to_vec())
    );
    let events = Kitty {
        events: true,
        ..Kitty::default()
    };
    let released = |spec| Key {
        action: Action::Release,
        ..key(spec, "")
    };
    assert_eq!(bytes("ctrl+c", events, false), Some(vec![0x03]));
    assert_eq!(
        encode(&released("ctrl+c"), false, events),
        Some(b"\x1b[99;5:3u".to_vec())
    );
    assert_eq!(bytes("f1", events, false), Some(b"\x1b[P".to_vec()));
    assert_eq!(bytes("escape", events, false), Some(b"\x1b".to_vec()));
    assert_eq!(
        encode(&released("escape"), false, events),
        Some(b"\x1b[27;1:3u".to_vec())
    );
}

#[test]
fn kitty_reports_the_key_shift_turned_into_a_symbol() {
    // gpui names Shift-[ by its `{` and drops Shift; the platform gives back `[`.
    let made = |spec, text, action| Key {
        unshifted: Some('['),
        action,
        ..key(spec, text)
    };
    let herdr = Kitty {
        events: true,
        alternates: true,
        ..DISAMBIGUATE
    };
    assert_eq!(
        kitty(made("cmd+ctrl+{", "", Action::Press), herdr).as_deref(),
        Some("\x1b[91:123;14u")
    );
    assert_eq!(
        kitty(made("cmd+ctrl+{", "", Action::Release), herdr).as_deref(),
        Some("\x1b[91:123;14:3u")
    );
    assert_eq!(
        kitty(made("ctrl+{", "", Action::Press), DISAMBIGUATE).as_deref(),
        Some("\x1b[91;6u")
    );
    assert_eq!(
        kitty(made("{", "{", Action::Press), herdr),
        None,
        "it types"
    );
    let all = Kitty {
        all: true,
        alternates: true,
        ..DISAMBIGUATE
    };
    assert_eq!(
        kitty(made("{", "{", Action::Press), all).as_deref(),
        Some("\x1b[91:123;2u")
    );
    assert_eq!(
        legacy(made("ctrl+{", "", Action::Press), false).as_deref(),
        Some("\x1b[123;5u"),
        "legacy keeps fixterms' shifted character"
    );
    let events = Kitty {
        events: true,
        ..Kitty::default()
    };
    assert_eq!(
        kitty(made("alt+{", "", Action::Press), events).as_deref(),
        Some("\x1b{"),
        "a flag that adds no field keeps the legacy bytes gpui's name gives"
    );
    let bang = Key {
        unshifted: Some('1'),
        ..key("alt+!", "")
    };
    assert_eq!(kitty(bang, events).as_deref(), Some("\x1b!"));
    let same = Key {
        unshifted: Some('7'),
        ..key("ctrl+7", "")
    };
    assert_eq!(
        kitty(same, herdr).as_deref(),
        Some("\x1b[55;6u"),
        "no alternate where Shift made the same character"
    );
}

#[test]
fn super_is_kittys_eighth_bit() {
    let events = Kitty {
        events: true,
        ..DISAMBIGUATE
    };
    assert_eq!(
        kitty(key("cmd+ctrl+c", ""), DISAMBIGUATE).as_deref(),
        Some("\x1b[99;13u")
    );
    let released = Key {
        action: Action::Release,
        ..key("cmd+ctrl+c", "")
    };
    assert_eq!(kitty(released, events).as_deref(), Some("\x1b[99;13:3u"));
    assert_eq!(legacy(key("cmd+c", ""), false), None, "Cmd types nothing");
    assert_eq!(
        legacy(key("cmd+up", ""), false).as_deref(),
        Some("\x1b[1;9A")
    );
    assert_eq!(
        legacy(key("cmd+enter", ""), false).as_deref(),
        Some("\x1b[27;9;13~")
    );
}

#[test]
fn pastes_end_lines_with_cr_or_go_bracketed_without_escapes() {
    assert_eq!(paste("a\r\nb\nc", false), b"a\rb\rc");
    assert_eq!(
        paste("x\n\x1b[201~rm\x03", true),
        b"\x1b[200~x\n[201~rm\x1b[201~"
    );
}
