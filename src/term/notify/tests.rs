use super::*;

fn shown(ask: Option<Ask>) -> Notice {
    match ask {
        Some(Ask::Show(notice)) => notice,
        other => panic!("no notification: {other:?}"),
    }
}

#[test]
fn osc_9_is_iterm2_s_notification_unless_it_is_conemu_s_command() {
    assert_eq!(shown(nine(b"Hello world")).title, "Hello world");
    for command in [
        &b"1;100"[..],
        b"10",
        b"10;2",
        b"11;note",
        b"12",
        b"2;box",
        b"3;",
        b"3;title",
        b"5",
        b"5x",
        b"6;macro",
        b"7;cmd",
        b"8;VAR",
        b"9;/path",
    ] {
        assert_eq!(nine(command), None, "{}", String::from_utf8_lossy(command));
    }
    for text in [
        "1", "1a", "10x", "10;5", "13", "2", "2a", "4", "4;", "4;9", "x;y",
    ] {
        assert_eq!(shown(nine(text.as_bytes())).title, text);
    }
    assert_eq!(nine(b""), None);
    assert_eq!(nine(b"a\xffb"), None, "not UTF-8");
}

#[test]
fn osc_9_4_reads_progress_as_ghostty_does() {
    let progress = |body: &[u8]| match nine(body) {
        Some(Ask::Progress(progress)) => progress,
        other => panic!("no progress: {other:?}"),
    };
    assert_eq!(progress(b"4;0"), Progress::Remove);
    assert_eq!(progress(b"4;1"), Progress::Set(Some(0)));
    assert_eq!(progress(b"4;1;40"), Progress::Set(Some(40)));
    assert_eq!(progress(b"4;1;900"), Progress::Set(Some(100)));
    assert_eq!(progress(b"4;1;x"), Progress::Set(None));
    assert_eq!(
        progress(b"4;1;99999999999999999999999"),
        Progress::Set(None)
    );
    assert_eq!(progress(b"4;1;1_0"), Progress::Set(None));
    assert_eq!(progress(b"4;2"), Progress::Error(None));
    assert_eq!(progress(b"4;2;50"), Progress::Error(Some(50)));
    assert_eq!(progress(b"4;3;7"), Progress::Indeterminate);
    assert_eq!(progress(b"4;4;10"), Progress::Pause(Some(10)));
}

#[test]
fn osc_777_is_rxvt_s_notify() {
    let notice = shown(rxvt(b"notify;Title;Body;more"));
    assert_eq!(
        (notice.title.as_str(), notice.body.as_str()),
        ("Title", "Body;more")
    );
    assert_eq!(rxvt(b"other;x;y"), None);
    assert_eq!(rxvt(b"notify;Title"), None, "no body");
    assert_eq!(rxvt(b"notify;;"), None, "nothing to say");
}

#[test]
fn osc_99_gathers_chunks_by_id_and_reads_its_options() {
    let mut kitty = Kitty::default();
    assert_eq!(kitty.read(b"i=a:d=0;Hel"), None);
    assert_eq!(kitty.read(b"i=b:d=0:p=body;other"), None);
    assert_eq!(kitty.read(b"i=a:d=0:p=body;Wor"), None);
    let notice = shown(kitty.read(b"i=a:p=body:o=unfocused:a=report,-focus:c=1:w=5000;ld"));
    assert_eq!(
        (
            notice.id.as_deref(),
            notice.title.as_str(),
            notice.body.as_str()
        ),
        (Some("a"), "Hel", "World")
    );
    assert_eq!(
        (notice.focus, notice.report, notice.close, notice.expire),
        (false, true, true, Some(5000)),
        "o passes by"
    );
    let other = shown(kitty.read(b"i=b:p=body;!"));
    assert_eq!(
        (other.title.as_str(), other.body.as_str()),
        ("other!", ""),
        "a lone body is the title"
    );
}

#[test]
fn osc_99_decodes_base64_and_takes_plain_text() {
    let mut kitty = Kitty::default();
    let notice = shown(kitty.read(b"e=1:p=body;SGVsbG8gd29ybGQ="));
    assert_eq!((notice.title.as_str(), notice.id), ("Hello world", None));
    let plain = shown(kitty.read(b";Hello world"));
    assert_eq!(
        (plain.title.as_str(), plain.focus, plain.report),
        ("Hello world", true, false)
    );
    assert_eq!(
        kitty.read(b"bad:i=c;x").map(|_| ()),
        Some(()),
        "a malformed pair passes by"
    );
}

#[test]
fn osc_99_gathers_base64_chunked_before_or_after_encoding() {
    let title = |chunks: &[&str]| {
        let mut kitty = Kitty::default();
        let (last, first) = chunks.split_last().expect("a chunk");
        for chunk in first {
            assert_eq!(kitty.read(format!("i=a:d=0:e=1;{chunk}").as_bytes()), None);
        }
        shown(kitty.read(format!("i=a:e=1;{last}").as_bytes())).title
    };
    assert_eq!(title(&["SGVsb", "G8="]), "Hello", "split inside a group");
    assert_eq!(title(&["8J+Y", "gA=="]), "😀", "split inside a character");
    assert_eq!(
        title(&["SGVsbG8=", "IHdvcmxk"]),
        "Hello world",
        "each chunk padded"
    );
    assert_eq!(title(&["SGVsbG", "8"]), "Hello", "the last chunk unpadded");
    let mut kitty = Kitty::default();
    assert_eq!(kitty.read(b"i=n:d=0;Title"), None);
    let notice = shown(kitty.read(b"i=n:p=body:e=1;TGluZSAxCkxpbmUgMg=="));
    assert_eq!(
        (notice.title.as_str(), notice.body.as_str()),
        ("Title", "Line 1\nLine 2"),
        "base64 carries any UTF-8"
    );
}

#[test]
fn osc_99_asks_to_close_and_what_is_alive_and_supported() {
    let mut kitty = Kitty::default();
    assert_eq!(kitty.read(b"i=x:p=close;"), Some(Ask::Close("x".into())));
    assert_eq!(kitty.read(b"p=close;"), None, "no id, no close");
    assert_eq!(kitty.read(b"i=q:p=?;"), Some(Ask::Query("q".into())));
    assert_eq!(kitty.read(b"p=?;"), Some(Ask::Query("0".into())));
    assert_eq!(kitty.read(b"i=y:p=alive;"), Some(Ask::Alive("y".into())));
    assert_eq!(
        kitty.read(b"p=icon;x"),
        None,
        "an unknown payload passes by"
    );
}

#[test]
fn osc_99_drops_what_breaks_its_rules() {
    let mut kitty = Kitty::default();
    let long = |length: usize, metadata: &str| {
        let mut body = format!("{metadata};").into_bytes();
        body.extend(std::iter::repeat_n(b'a', length));
        body
    };
    for body in [
        b"no payload".to_vec(),
        b"i=a b;x".to_vec(),
        b"i=a\"b;x".to_vec(),
        b"e=1;!!!".to_vec(),
        b"e=1;/w==".to_vec(),
        "x\u{85}y"
            .as_bytes()
            .iter()
            .copied()
            .fold(b";".to_vec(), |mut body, byte| {
                body.push(byte);
                body
            }),
        b";a\tb".to_vec(),
        b"e=1;SGVsbG8gS".to_vec(),
        long(PLAIN + 1, ""),
        long(ENCODED + 4, "e=1"),
        long(1, &"x".repeat(METADATA + 1)),
    ] {
        assert_eq!(
            kitty.read(&body),
            None,
            "{}",
            String::from_utf8_lossy(&body)
        );
    }
    assert_eq!(kitty.read(b"i=k:d=0;Hel"), None);
    assert_eq!(kitty.read(b"i=k:d=0:e=1;!!!!"), None);
    assert!(kitty.pending.is_empty(), "a bad chunk drops what gathered");
    for at in 0..PENDING {
        assert_eq!(kitty.read(format!("i=p{at}:d=0;x").as_bytes()), None);
    }
    assert_eq!(kitty.read(b"i=over:d=0;x"), None);
    assert!(
        kitty.pending.iter().all(|(id, _)| id != "over"),
        "past the cap nothing gathers"
    );
    let mut big = Kitty::default();
    for _ in 0..GATHERED / PLAIN {
        assert_eq!(big.read(&long(PLAIN, "i=g:d=0")), None);
    }
    assert_eq!(
        big.read(&long(PLAIN, "i=g")),
        None,
        "over the gathered bound"
    );
    assert!(big.pending.is_empty());
}

#[test]
fn the_answers_name_the_asker_s_id() {
    let offered = if CLOSES {
        "p=title,body,close,?,alive:w=1"
    } else {
        "p=title,body,?,alive"
    };
    assert_eq!(
        answer("q"),
        format!("\x1b]99;i=q:p=?;a=focus,report:o=invisible:{offered}\x1b\\")
    );
    assert_eq!(alive("0", &[]), "\x1b]99;i=0:p=alive;\x1b\\");
    assert_eq!(alive("z", &["a", "b"]), "\x1b]99;i=z:p=alive;a,b\x1b\\");
    assert_eq!(untracked("n1"), "\x1b]99;i=n1:p=close;untracked\x1b\\");
    assert_eq!(activated("0"), "\x1b]99;i=0;\x1b\\");
}
