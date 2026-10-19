use super::*;
use crate::term::mouse::MouseMode;

/// What the scanner finds in `chunks`, each one read: a body, or `RESET`.
fn scan(chunks: &[&[u8]]) -> Vec<String> {
    let mut scanner = Osc::default();
    let mut found = Vec::new();
    for chunk in chunks {
        scanner.feed(chunk, &mut |thing| {
            found.push(match thing {
                Found::Osc(body, _) => String::from_utf8(body.to_vec()).expect("utf-8"),
                Found::Reset => "RESET".into(),
                Found::Push => "PUSH".into(),
                Found::Pop => "POP".into(),
            })
        });
    }
    found
}

#[test]
fn finds_a_full_reset_between_bodies() {
    assert_eq!(
        scan(&[b"\x1b]0;a\x07\x1bc\x1b", b"c\x1b]0;b\x1b\\"]),
        ["0;a", "RESET", "RESET", "0;b"]
    );
    assert!(
        scan(&[b"\x1b[c\x1b]0;c"]).is_empty(),
        "DA1 and an unfinished body"
    );
    // vte passes DEL and high bytes by in an escape, and the reset still comes.
    assert_eq!(scan(&[b"\x1b\x7fc\x1b\xff", b"c"]), ["RESET", "RESET"]);
}

#[test]
fn a_sequence_counts_from_osc_through_st_skipped_controls_too() {
    let mut lengths = Vec::new();
    let mut osc = b"\x1b]7501;".to_vec();
    osc.extend([0; 5000]);
    osc.extend_from_slice(b"state=done\x07\x1b]0;t\x1b\\");
    Osc::default().feed(&osc, &mut |found| {
        if let Found::Osc(body, length) = found {
            lengths.push((body.len(), length));
        }
    });
    assert_eq!(lengths, [(15, 5018), (3, 7)]);
}

#[test]
fn finds_each_osc_body_across_reads_ended_as_vte_ends_it() {
    assert_eq!(scan(&[b"ab\x1b]7;file:///a\x07cd"]), ["7;file:///a"]);
    assert_eq!(
        scan(&[b"\x1b]7;file:", b"///b\x1b", b"\\"]),
        ["7;file:///b"]
    );
    assert_eq!(
        scan(&[b"\x1b", b"]133;A\x07\x1b]0;t\x1b\\"]),
        ["133;A", "0;t"]
    );
    assert_eq!(scan(&[b"\x1b]7;x\x18\x1b]7;y\x1a"]), ["7;x", "7;y"]);
    assert_eq!(scan(&[b"\x1b]7;cut\x1b]133;D;1\x07"]), ["7;cut", "133;D;1"]);
    assert_eq!(scan(&[b"\x1b]0;t\x1b[31m"]), ["0;t"]);
}

#[test]
fn controls_vte_skips_stay_out_of_a_body() {
    assert_eq!(scan(&[b"\x1b\x00]1\x0033;\x1fA\x07"]), ["133;A"]);
    assert_eq!(scan(&[b"\x1b[31m\x1bc"]), ["RESET"]);
}

#[test]
fn an_overlong_body_is_dropped() {
    let mut long = b"\x1b]7;".to_vec();
    long.extend(std::iter::repeat_n(b'a', MOST + 1));
    long.push(0x07);
    assert!(scan(&[&long]).is_empty());
    assert_eq!(scan(&[&long, b"\x1b]7;b\x07"]), ["7;b"]);
}

#[test]
fn an_osc_99_body_holds_its_metadata_and_a_full_payload() {
    let osc = |length: usize| [&b"\x1b]99;"[..], &vec![b'a'; length - 3], b"\x07"].concat();
    assert_eq!(scan(&[&osc(notify::BODY)]).concat().len(), notify::BODY);
    assert!(scan(&[&osc(notify::BODY + 1)]).is_empty());
}

#[test]
fn mouse_modes_follow_private_modes_in_order_across_reads() {
    use crate::term::mouse::{Format, Tracking};
    let mut modes = Modes::default();
    assert_eq!(modes.feed(b"a\x1b[?"), None);
    assert!(modes.feed(b"1006;9h").is_some());
    assert_eq!(modes.mode.format, Format::Sgr);
    assert_eq!(modes.mode.tracking, Tracking::X10);
    assert_eq!(modes.feed(b"\x1b[9m\x1b[?25l\x1b[?9"), None);
    assert!(modes.feed(b"l").is_some());
    assert_eq!(modes.mode.tracking, Tracking::Off);
    modes.feed(b"\x1b[?1000h\x1b[?9h\x1b[?9l");
    assert_eq!(modes.mode.tracking, Tracking::Off, "9 replaced 1000");
    modes.feed(b"\x1b[?1002h");
    let reset = modes.feed(b"\x1bc");
    assert_eq!(reset, Some(MouseMode::default()), "a reset ends it");
    modes.feed(b"\x1b[?09;\x1b[?1003h");
    assert_eq!(modes.mode.tracking, Tracking::Any);
}

#[test]
fn mouse_modes_read_through_controls_as_the_parser_does() {
    use crate::term::mouse::{Format, Tracking};
    let mut modes = Modes::default();
    modes.feed(b"\x1b[?1000\x00h\x1b[?1006");
    modes.feed(b"\x00h");
    let on = MouseMode {
        tracking: Tracking::Normal,
        format: Format::Sgr,
    };
    assert_eq!(
        modes.mode, on,
        "NUL runs inside a sequence, across reads too"
    );
    modes.feed(b"\x1b[?1000\x00l\x1b[?1006\x7f\xc3\xa9l");
    assert_eq!(
        modes.mode,
        MouseMode::default(),
        "DEL and high bytes are skipped"
    );
    modes.feed(b"\x1b\x07[?1002;12:9");
    modes.feed(b"9:1;9h");
    assert_eq!(
        modes.mode.tracking,
        Tracking::X10,
        "subparameters are skipped"
    );
    modes.feed(b"\x1b[?1003\x18h\x1b[?1003\x1ah");
    assert_eq!(modes.mode.tracking, Tracking::X10, "CAN and SUB cancel");
    let normal = |subparameters: usize| {
        [b"\x1b[?1000".as_slice(), &b":0".repeat(subparameters), b"h"].concat()
    };
    modes.feed(&normal(32));
    assert_eq!(
        modes.mode.tracking,
        Tracking::X10,
        "33 values: vte drops it"
    );
    modes.feed(&normal(31));
    assert_eq!(modes.mode.tracking, Tracking::Normal, "32 values hold");
}

#[test]
fn a_local_uri_gives_its_decoded_folder() {
    assert_eq!(
        folder(b"file:///tmp/ely%20dir"),
        Some(PathBuf::from("/tmp/ely dir"))
    );
    assert_eq!(folder(b"file://localhost/x"), Some(PathBuf::from("/x")));
    let host = gethostname::gethostname().into_string().expect("host");
    let named = format!("file://{host}/y");
    assert_eq!(folder(named.as_bytes()), Some(PathBuf::from("/y")));
    assert_eq!(
        folder("file:///%E4%B8%AD".as_bytes()),
        Some(PathBuf::from("/中"))
    );
}

#[test]
fn other_hosts_and_bad_uris_give_none() {
    assert_eq!(folder(b"file://elsewhere.invalid/tmp"), None);
    assert_eq!(folder(b"http://x/y"), None);
    assert_eq!(folder(b"file:///%FF"), None);
}

/// vte's own reading of `bytes`, under the same mode rules.
fn parsed(bytes: &[u8]) -> MouseMode {
    use alacritty_terminal::vte::{Params, Parser, Perform};
    struct Modes(MouseMode);
    impl Perform for Modes {
        fn csi_dispatch(&mut self, params: &Params, between: &[u8], ignore: bool, action: char) {
            if !ignore && between == b"?" && matches!(action, 'h' | 'l') {
                for param in params.iter() {
                    self.0.apply(u32::from(param[0]), action == 'h');
                }
            }
        }

        fn esc_dispatch(&mut self, between: &[u8], _: bool, byte: u8) {
            if between.is_empty() && byte == b'c' {
                self.0 = MouseMode::default();
            }
        }
    }
    let mut modes = Modes(MouseMode::default());
    Parser::new().advance(&mut modes, bytes);
    modes.0
}

#[test]
fn the_tap_reads_mouse_modes_as_vte_does_byte_by_byte() {
    let cases: [&[u8]; 10] = [
        b"\x1b[?1000;1006h\x1b[?1000\x00l",
        b"\x1b[?1000\x00h\x1b[?1006h\x1b[?1005\x0bh",
        b"\x1b\x00\x7f[?\x07100\xc3\xa90h\x1b[?1006\x08l",
        b"\x1b[?1002;12:99:1;9h\x1b[?:9h",
        b"\x1b[?1003\x18h\x1b[?1003\x1ah\x1b[?1003\x1b[?1002h",
        b"\x1b[?9h\x1b[?1000$h\x1b[?1000<h\x1b[>1000h\x1b[1000h",
        b"\x1b]0;x\x1b[?1000h\x07\x1bP?9h\x1b\\\x1b(c\x1bc",
        b"\x1b[?1000h\x1b[?1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18;19;20;21;22;23;24;25;26;27;28;29;30;31;1003h",
        b"\x1b[?1000h\x1b[?1;2;3;4;5;6;7;8;9;10;11;12;13;14;15;16;17;18;19;20;21;22;23;24;25;26;27;28;29;30;31;32;1003l",
        b"\x1b[?65537;1000h\x1b[?01002h\x1b[?h\x1b[?;1003h",
    ];
    let values = |subparameters: usize| {
        [
            b"\x1b[?1000h\x1b[?1003".as_slice(),
            &b":0".repeat(subparameters),
            b"h",
        ]
        .concat()
    };
    for case in cases
        .map(<[u8]>::to_vec)
        .into_iter()
        .chain([values(31), values(32)])
    {
        let case = case.as_slice();
        let mut modes = Modes::default();
        for end in 1..=case.len() {
            modes.feed(&case[end - 1..end]);
            assert_eq!(modes.mode, parsed(&case[..end]), "{case:?} to byte {end}");
        }
    }
}

#[test]
fn finds_the_title_stack_s_push_and_pop_as_vte_reads_them() {
    assert_eq!(scan(&[b"\x1b[22;0t\x1b[23;2t"]), ["PUSH", "POP"]);
    assert_eq!(
        scan(&[b"\x1b[2", b"2:1\x07t\x1b[23t"]),
        ["PUSH", "POP"],
        "across reads, a control run"
    );
    for other in [
        &b"\x1b[?22t"[..],
        b"\x1b[22$t",
        b"\x1b[1;22t",
        b"\x1b[t",
        b"\x1b[22\x18t",
        b"\x1b[24t",
    ] {
        assert!(
            scan(&[other]).is_empty(),
            "{}",
            String::from_utf8_lossy(other)
        );
    }
    let mut many = b"\x1b[22".to_vec();
    many.extend(std::iter::repeat_n(&b";1"[..], 32).flatten());
    many.push(b't');
    assert!(scan(&[&many]).is_empty(), "vte drops one past 32 fields");
    assert_eq!(
        scan(&[b"\x1b[22t\x1b]2;t\x07\x1b[23t"]),
        ["PUSH", "2;t", "POP"]
    );
}

#[test]
fn a_title_s_body_holds_65536_bytes_and_another_s_4096() {
    let osc = |number: &str, length: usize| {
        [
            format!("\x1b]{number};").as_bytes(),
            &vec![b'a'; length],
            b"\x07",
        ]
        .concat()
    };
    let longest = scan(&[&osc("2", titles::LONGEST - 2)]);
    assert_eq!(longest.concat().len(), titles::LONGEST);
    assert!(scan(&[&osc("0", titles::LONGEST - 1)]).is_empty());
    assert!(
        scan(&[&osc("7", MOST - 1)]).is_empty(),
        "another stays at 4096"
    );
}

#[cfg(windows)]
#[test]
fn win32_input_mode_follows_conptys_ask_until_a_full_reset() {
    let mut modes = Modes::default();
    modes.feed(b"\x1b[c\x1b[?1004h\x1b[?9001h");
    assert!(modes.win32);
    modes.feed(b"\x1b[?9001l");
    assert!(!modes.win32);
    modes.feed(b"\x1b[?9001h\x1bc");
    assert!(!modes.win32, "a full reset ends it");
}
