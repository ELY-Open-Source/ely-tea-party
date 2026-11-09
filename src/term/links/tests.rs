use alacritty_terminal::{
    event::VoidListener,
    index::{Column, Line},
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;

fn target<T>(term: &Term<T>, point: Point) -> Option<String> {
    link_at(term, point).map(|target| match target {
        Target::Url(url) => url.to_string(),
        Target::Path { path, .. } => path.to_string(),
    })
}

fn link(bytes: &str, line: i32, column: usize) -> Option<String> {
    let mut term = Term::new(Config::default(), &TermSize::new(12, 4), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, bytes.as_bytes());
    link_at(&term, Point::new(Line(line), Column(column))).map(|target| match target {
        Target::Url(url) => url.to_string(),
        Target::Path { path, .. } => path.to_string(),
    })
}

#[test]
fn a_path_names_its_file_without_the_line() {
    assert_eq!(link("at src/a.rs:4:2", 0, 6).as_deref(), Some("src/a.rs"));
    assert_eq!(link("at src/a.rs:4:2", 0, 1), None);
}

#[test]
fn a_path_opens_only_where_a_file_is() {
    let folder = std::env::temp_dir().join(format!("ely-links-{}", std::process::id()));
    std::fs::create_dir_all(folder.join("src")).unwrap();
    std::fs::write(folder.join("src/a.rs"), "").unwrap();
    assert_eq!(
        file("src/a.rs", Some(&folder)),
        Some(folder.join("src/a.rs"))
    );
    assert_eq!(file("src/b.rs", Some(&folder)), None, "no such file");
    assert_eq!(file("src/a.rs", None), None, "no folder to start from");
    let absolute = folder.join("src/a.rs");
    assert_eq!(file(absolute.to_str().unwrap(), None), Some(absolute));
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn a_word_is_the_run_of_text_under_the_point() {
    let text = "see 中文/a.rs now";
    assert_eq!(word(text, 4), Some("中文/a.rs"));
    assert_eq!(word(text, text.find("a.rs").unwrap()), Some("中文/a.rs"));
    assert_eq!(word(text, 3), None, "a blank is no word");
    assert_eq!(word(text, text.len()), None, "past the text");
    assert_eq!(word("a\u{3000}b", 4), Some("b"), "a wide blank ends it");
}

#[test]
fn a_word_loses_what_surrounds_its_path() {
    assert_eq!(bare("(src/a.rs:4:2),"), Some("src/a.rs"));
    assert_eq!(bare("\"README.md\"."), Some("README.md"));
    assert_eq!(bare("src/term/"), Some("src/term/"));
    assert_eq!(bare(".env"), Some(".env"));
    assert_eq!(bare("../a"), Some("../a"));
    for mark in ["/", "..", "~/", "--", ":4"] {
        assert_eq!(bare(mark), None, "{mark} names no file");
    }
}

#[test]
fn loose_text_names_what_the_pattern_does_not() {
    let folder = std::env::temp_dir().join(format!("ely-named-{}", std::process::id()));
    std::fs::create_dir_all(folder.join("my docs/src")).unwrap();
    std::fs::write(folder.join("Makefile"), "").unwrap();
    std::fs::write(folder.join("my docs/a.txt"), "").unwrap();
    let here = Some(folder.as_path());
    assert_eq!(named("Makefile:3", here), Some(folder.join("Makefile")));
    assert_eq!(named("my docs/src", here), Some(folder.join("my docs/src")));
    assert_eq!(
        named("  'my docs/a.txt'\n", here),
        Some(folder.join("my docs/a.txt")),
        "a selection's own blanks go"
    );
    assert_eq!(named("Makefile\nmy docs", here), None, "two lines");
    assert_eq!(named("nothing", here), None, "no such file");
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn a_file_url_opens_as_its_file_while_it_is_here() {
    let path = std::env::temp_dir().join(format!("ely-local {}", std::process::id()));
    std::fs::write(&path, "").unwrap();
    let name = path
        .to_str()
        .unwrap()
        .replace('\\', "/")
        .replace(' ', "%20");
    #[cfg(windows)]
    let name = format!("/{name}");
    assert_eq!(local(&format!("file://{name}")), Some(path.clone()));
    std::fs::remove_file(&path).unwrap();
    assert_eq!(local(&format!("file://{name}")), None, "no longer there");
    assert_eq!(local("file://elsewhere.test/tmp"), None, "another host's");
    assert_eq!(local("https://a.b/c"), None);
}

#[test]
fn an_osc_8_link_names_its_target() {
    let text = "a \x1b]8;;https://ely.dev/x\x1b\\ely\x1b]8;;\x1b\\ b";
    assert_eq!(link(text, 0, 3).as_deref(), Some("https://ely.dev/x"));
    assert_eq!(link(text, 0, 0), None);
}

#[test]
fn a_prompt_s_own_link_is_no_target() {
    let text = "\x1b]8;;\x7f\x1b\\$ https://ely.dev\x1b]8;;\x1b\\";
    assert_eq!(link(text, 0, 0), None);
    assert_eq!(link(text, 0, 4).as_deref(), Some("https://ely.dev"));
}

#[test]
fn a_url_ends_before_its_sentence_does() {
    assert_eq!(link("(a.io/x).", 0, 2), None, "no scheme, no link");
    let text = "(http://a.b)";
    assert_eq!(link(text, 0, 4).as_deref(), Some("http://a.b"));
    assert_eq!(link(text, 0, 11), None);
}

#[test]
fn a_url_keeps_its_combining_marks() {
    let text = "https://e.test/cafe\u{301}/x";
    assert_eq!(link(text, 1, 1).as_deref(), Some(text));
}

#[test]
fn a_url_runs_across_wrapped_rows_and_wide_cells() {
    let text = "\u{4e2d} https://ely.dev/abc";
    assert_eq!(link(text, 1, 3).as_deref(), Some("https://ely.dev/abc"));
    assert_eq!(link(text, 0, 3).as_deref(), Some("https://ely.dev/abc"));
    assert_eq!(link(text, 0, 1), None);
    let wide = "file://a/\u{4e2d} x";
    assert_eq!(
        link(wide, 0, 10).as_deref(),
        Some("file://a/\u{4e2d}"),
        "a wide cell's spacer is its char"
    );
}

#[test]
fn a_zero_width_space_ends_a_url() {
    let text = "doc (\u{200b}https://a.io/x\u{200b})";
    assert_eq!(link(text, 0, 8).as_deref(), Some("https://a.io/x"));
}

fn hit(text: &str, line: i32, column: usize) -> RangeInclusive<Point> {
    let mut term = Term::new(Config::default(), &TermSize::new(12, 4), VoidListener);
    <Processor>::new().advance(&mut term, text.as_bytes());
    hit_at(&term, at(line, column)).expect("a link")
}

fn at(line: i32, column: usize) -> Point {
    Point::new(Line(line), Column(column))
}

#[test]
fn hover_covers_the_wrapped_url_without_its_punctuation() {
    assert_eq!(hit("中 (http://a.b).", 0, 8), at(0, 4)..=at(1, 1));
    assert_eq!(hit("file://a/中 x", 0, 10), at(0, 0)..=at(0, 10));
    assert_eq!(
        hit("https://e.test/cafe\u{301}/x", 1, 1),
        at(0, 0)..=at(1, 8)
    );
}

#[test]
fn osc_8_hover_covers_its_label_and_keeps_separate_links_apart() {
    let text = "a \x1b]8;;https://a.b\x07abcdefghijklmn\x1b]8;;\x07 z";
    assert_eq!(hit(text, 1, 1), at(0, 2)..=at(1, 3));
    let text = "\x1b]8;;https://a.b\x07one\x1b]8;;\x07 \x1b]8;;https://a.b\x07two\x1b]8;;\x07";
    assert_eq!(hit(text, 0, 1), at(0, 0)..=at(0, 2));
    assert_eq!(hit(text, 0, 5), at(0, 4)..=at(0, 6));
}

#[test]
fn a_long_osc_8_link_is_searched_only_inside_the_viewport() {
    use alacritty_terminal::grid::Scroll;

    let mut term = Term::new(Config::default(), &TermSize::new(12, 4), VoidListener);
    let text = format!(
        "\x1b]8;;https://a.b\x07{}\x1b]8;;\x07",
        "x".repeat(12 * 2048)
    );
    <Processor>::new().advance(&mut term, text.as_bytes());
    assert!(term.grid().history_size() > 1000);
    for scroll in [Scroll::Bottom, Scroll::Delta(100), Scroll::Top] {
        term.scroll_display(scroll);
        let top = -(term.grid().display_offset() as i32);
        let hover = hit_at(&term, at(top + 1, 5)).expect("a visible link");
        assert_eq!(hover, at(top, 0)..=at(top + 3, 11));
        let outside = if top == 0 {
            Point::new(term.grid().topmost_line(), Column(0))
        } else {
            Point::new(term.grid().bottommost_line(), Column(0))
        };
        assert!(hit_at(&term, outside).is_none());
        assert_eq!(target(&term, outside).as_deref(), Some("https://a.b"));
    }
}

fn parsed(text: &str) -> Term<VoidListener> {
    let mut term = Term::new(Config::default(), &TermSize::new(12, 4), VoidListener);
    <Processor>::new().advance(&mut term, text.as_bytes());
    term
}

#[test]
fn a_long_plain_logical_line_stops_at_the_cell_budget() {
    use alacritty_terminal::grid::Scroll;

    let mut term = parsed(&"x".repeat(12 * 2048));
    for scroll in [Scroll::Bottom, Scroll::Delta(100), Scroll::Top] {
        term.scroll_display(scroll);
        let point = at(-(term.grid().display_offset() as i32) + 1, 5);
        assert!(token(&term, point).is_none());
        assert!(hit_at(&term, point).is_none());
        assert!(link_at(&term, point).is_none());
    }
}

#[test]
fn an_oversized_url_is_not_opened_or_underlined_as_a_truncated_url() {
    let url = format!("https://a.b/{}", "x".repeat(URL_CELLS));
    let term = parsed(&url);
    for point in [Point::new(term.grid().topmost_line(), Column(3)), at(2, 5)] {
        assert!(link_at(&term, point).is_none());
    }
    assert!(hit_at(&term, at(2, 5)).is_none());
    let url = format!("https://a.b/{}", "x".repeat(URL_CELLS - 12));
    let term = parsed(&url);
    assert_eq!(target(&term, at(2, 5)).as_deref(), Some(url.as_str()));
}

#[test]
fn a_short_wrapped_url_is_found_inside_a_long_logical_line() {
    let url = "https://a.b/cafe\u{301}/中";
    let term = parsed(&format!("{} ({url}).", "x".repeat(URL_CELLS * 2)));
    let point = at(2, 0);
    assert_eq!(target(&term, point).as_deref(), Some(url));
    assert!(hit_at(&term, point).is_some());
}

#[test]
fn hard_newlines_do_not_join_url_fragments() {
    let term = parsed("https://a.b/\r\npath");
    assert!(link_at(&term, at(1, 1)).is_none());
    assert_eq!(target(&term, at(0, 4)).as_deref(), Some("https://a.b/"));
}

#[test]
fn combining_marks_cannot_exceed_the_text_budget() {
    let term = parsed(&format!("https://a.b/e{}", "\u{301}".repeat(URL_BYTES)));
    assert!(token(&term, at(0, 5)).is_some());
    assert!(url_at(&term, at(0, 5)).is_none());
    assert!(link_at(&term, at(0, 5)).is_none());
    assert!(hit_at(&term, at(0, 5)).is_none());
}

#[test]
fn a_short_url_in_compact_json_is_not_rejected_with_a_long_field() {
    let term = parsed(&format!(
        r#"{{"padding":"{}","url":"https://a.b"}}"#,
        "中".repeat(5000)
    ));
    let point = term
        .renderable_content()
        .display_iter
        .find(|cell| cell.cell.c == 'h')
        .expect("the URL is visible")
        .point;
    assert_eq!(target(&term, point).as_deref(), Some("https://a.b"));
    assert_eq!(
        hit_at(&term, point),
        Some(point..=point.add(&term, Boundary::Grid, 10))
    );
}

#[test]
fn url_delimiters_isolate_long_neighbors_on_both_sides() {
    use alacritty_terminal::grid::Scroll;

    for delimiter in ['"', '\'', '<', '>', '`'] {
        for trailing in [false, true] {
            let padding = "x".repeat(5000);
            let text = if trailing {
                format!("{delimiter}https://a.b{delimiter}{padding}")
            } else {
                format!("{padding}{delimiter}https://a.b{delimiter}")
            };
            let mut term = parsed(&text);
            if trailing {
                term.scroll_display(Scroll::Top);
            }
            let point = term
                .renderable_content()
                .display_iter
                .find(|cell| cell.cell.c == 'h')
                .expect("the URL is visible")
                .point;
            assert_eq!(target(&term, point).as_deref(), Some("https://a.b"));
            assert_eq!(
                hit_at(&term, point),
                Some(point..=point.add(&term, Boundary::Grid, 10))
            );
            for edge in [
                point.sub(&term, Boundary::Grid, 1),
                point.add(&term, Boundary::Grid, 11),
            ] {
                assert!(link_at(&term, edge).is_none());
                assert!(hit_at(&term, edge).is_none());
            }
        }
    }
}
