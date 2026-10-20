use alacritty_terminal::{
    event::VoidListener,
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;
use crate::term::prompts::Prompts;

const PROMPT: &str = "\x1b]133;A\x07$ \x1b]133;B\x07";

/// A terminal `columns` wide after `text` passed the prompts' rewrite, as the tap passes it.
fn term(columns: usize, text: &str) -> Term<VoidListener> {
    let mut term = Term::new(Config::default(), &TermSize::new(columns, 4), VoidListener);
    let mut parser: Processor = Processor::new();
    parser.advance(&mut term, Prompts::default().pass(text.as_bytes()));
    term
}

fn click(line: i32, column: usize) -> Point {
    Point::new(Line(line), Column(column))
}

#[test]
fn a_click_on_the_typed_line_counts_the_characters_to_it() {
    let typed = term(20, &format!("{PROMPT}echo hello"));
    assert_eq!(arrows(&typed, click(0, 7)), Some(-5), "back to hello");
    assert_eq!(
        arrows(&typed, click(0, 2)),
        Some(-10),
        "the first character"
    );
    assert_eq!(
        arrows(&typed, click(0, 12)),
        Some(0),
        "the cursor's own cell"
    );
    let moved = term(20, &format!("{PROMPT}echo hello\x1b[5D"));
    assert_eq!(arrows(&moved, click(0, 11)), Some(4), "on to the last o");
    assert_eq!(
        arrows(&moved, click(0, 18)),
        Some(5),
        "past the end, the end"
    );
}

#[test]
fn a_click_off_the_input_moves_nothing() {
    let typed = term(20, &format!("{PROMPT}ls"));
    assert_eq!(arrows(&typed, click(0, 0)), None, "on the prompt");
    assert_eq!(arrows(&typed, click(0, 1)), None, "on the prompt's space");
    assert_eq!(arrows(&typed, click(2, 3)), None, "another line");
    let ran = term(20, &format!("{PROMPT}cat\r\nhello"));
    assert_eq!(arrows(&ran, click(1, 2)), None, "a line no prompt shows");
    let output = term(20, "$ plain text");
    assert_eq!(arrows(&output, click(0, 3)), None, "no marks at all");
    let full = term(20, &format!("{PROMPT}vim\x1b[?1049h"));
    assert_eq!(arrows(&full, click(0, 3)), None, "the alternate screen");
}

#[test]
fn a_wide_character_takes_one_arrow() {
    let wide = term(20, &format!("{PROMPT}中文ab"));
    assert_eq!(arrows(&wide, click(0, 3)), Some(-4), "the right half of 中");
    assert_eq!(arrows(&wide, click(0, 4)), Some(-3), "文");
    assert_eq!(arrows(&wide, click(0, 7)), Some(-1), "b");
}

#[test]
fn soft_wrapped_rows_are_one_line() {
    let wrapped = term(10, &format!("{PROMPT}abcdefghijkl"));
    assert_eq!(wrapped.grid().cursor.point, click(1, 4));
    assert_eq!(arrows(&wrapped, click(0, 3)), Some(-11), "back to b");
    let back = term(10, &format!("{PROMPT}abcdefghijkl\x1b[3D"));
    assert_eq!(arrows(&back, click(0, 3)), Some(-8), "up to b");
    assert_eq!(arrows(&back, click(1, 3)), Some(2), "on to l");
}

#[test]
fn trailing_spaces_count_only_up_to_the_cursor() {
    let spaced = term(20, &format!("{PROMPT}ls  "));
    assert_eq!(arrows(&spaced, click(0, 15)), Some(0));
    assert_eq!(arrows(&spaced, click(0, 4)), Some(-2));
}

#[test]
fn a_prompt_over_two_lines_keeps_its_input_on_the_last() {
    let two = term(20, "\x1b]133;A\x07~/src\r\n> \x1b]133;B\x07make");
    assert_eq!(arrows(&two, click(1, 2)), Some(-4));
    assert_eq!(arrows(&two, click(0, 3)), None, "the prompt's first line");
}

/// The press `term` sends for a click at `at`, as text.
fn sent(term: &Term<VoidListener>, at: Point, relative: bool) -> Option<String> {
    press(term, at, relative).map(|bytes| String::from_utf8(bytes).expect("ASCII"))
}

#[test]
fn a_shell_that_takes_clicks_hears_the_press_where_it_landed() {
    let marked = "\x1b]133;A;click_events=1\x07$ \x1b]133;B\x07";
    let typed = term(20, &format!("out\r\n{marked}echo hi"));
    assert_eq!(
        sent(&typed, click(1, 7), false).as_deref(),
        Some("\x1b[<0;8;2M")
    );
    assert_eq!(
        sent(&typed, click(1, 7), true).as_deref(),
        Some("\x1b[<0;8;1M")
    );
    assert_eq!(
        sent(&typed, click(2, 3), true),
        None,
        "a row below the input"
    );
    assert_eq!(sent(&typed, click(0, 1), false), None, "before the prompt");
    assert_eq!(sent(&typed, click(1, 0), false), None, "on the prompt");
    assert_eq!(
        sent(&typed, click(1, 1), false),
        None,
        "on the prompt's space"
    );
    let past = sent(&typed, click(1, 15), false);
    assert_eq!(
        past.as_deref(),
        Some("\x1b[<0;16;2M"),
        "past the end, the shell decides"
    );
    let right = term(
        20,
        &format!("{marked}\x1b[1;17H\x1b]133;A\x07~/a\x1b]133;B\x07\x1b[1;3Hls"),
    );
    assert_eq!(
        sent(&right, click(0, 18), false),
        None,
        "a right prompt after the cursor"
    );
    assert_eq!(
        sent(&right, click(0, 3), false).as_deref(),
        Some("\x1b[<0;4;1M")
    );
    assert_eq!(sent(&right, click(0, 1), false), None, "on the left prompt");
    let empty = term(
        20,
        "\x1b[1;17H\x1b]133;A;click_events=1\x07~/a\x1b]133;B\x07\x1b[1;1Hls",
    );
    let left = sent(&empty, click(0, 1), false);
    assert_eq!(left.as_deref(), Some("\x1b[<0;2;1M"), "only a right prompt");
    assert_eq!(
        sent(&empty, click(0, 17), false),
        None,
        "on that right prompt"
    );
    let two = term(
        20,
        "\x1b]133;A;click_events=2\x07~/src\r\n> \x1b]133;B\x07ls",
    );
    let relative = sent(&two, click(1, 3), true);
    assert_eq!(
        relative.as_deref(),
        Some("\x1b[<0;4;2M"),
        "from the prompt's first row"
    );
    let full = term(20, &format!("{marked}vim\x1b[?1049h"));
    assert_eq!(
        sent(&full, click(0, 3), false),
        None,
        "the alternate screen"
    );
    let scrolled = term(20, &format!("{marked}a\r\nb\r\nc\r\nd\r\ne"));
    assert_eq!(
        sent(&scrolled, click(-1, 0), false),
        None,
        "a row in the history"
    );
}

#[test]
fn input_reads_the_line_its_prompt_started() {
    assert_eq!(Shell::Busy.reads(), Shell::Reading(Click::Arrows));
    assert_eq!(
        Shell::Prompting(Click::Absolute).reads(),
        Shell::Reading(Click::Absolute)
    );
    assert_eq!(
        Shell::Reading(Click::Relative).reads(),
        Shell::Reading(Click::Relative)
    );
}

#[test]
fn a_prompt_that_prints_nothing_starts_the_input_at_its_first_typed_cell() {
    let empty = "\x1b]133;A;click_events=1\x07\x1b]133;B\x07";
    let typed = term(20, &format!("echo hi\r\nhi\r\n{empty}abcdef"));
    let at = sent(&typed, click(2, 3), false);
    assert_eq!(at.as_deref(), Some("\x1b[<0;4;3M"), "on the d");
    let relative = sent(&typed, click(2, 3), true);
    assert_eq!(
        relative.as_deref(),
        Some("\x1b[<0;4;1M"),
        "from the input's row"
    );
    assert_eq!(sent(&typed, click(1, 1), false), None, "output above");
    assert_eq!(
        sent(&typed, click(0, 2), false),
        None,
        "the line run before"
    );
    let moved = term(20, &format!("{empty}abcdef\x1b[6D"));
    let back = sent(&moved, click(0, 4), false);
    assert_eq!(
        back.as_deref(),
        Some("\x1b[<0;5;1M"),
        "the cursor at the start"
    );
    let blank = term(20, &format!("echo hi\r\n{empty}"));
    assert_eq!(
        sent(&blank, click(0, 2), false),
        None,
        "nothing typed, nothing after"
    );
}

#[test]
fn a_prompt_drawn_again_after_the_typed_text_keeps_its_line() {
    let marked = "\x1b]133;A;click_events=1\x07";
    let right = format!("\x1b[1;17H{marked}RIGHT\x1b]133;B\x07\x1b[1;7H");
    let repainted = term(24, &format!("{marked}\x1b]133;B\x07abcdef{right}"));
    let at = sent(&repainted, click(0, 3), false);
    assert_eq!(
        at.as_deref(),
        Some("\x1b[<0;4;1M"),
        "fish's right prompt again"
    );
    assert_eq!(
        sent(&repainted, click(0, 17), false),
        None,
        "on that prompt"
    );
    let ran = "\x1b]133;C\x07\x1b]133;D;0\x07";
    let after = format!("$ \x1b]133;B\x07echo hi{ran}\r\nhi\r\n{marked}\x1b]133;B\x07abcdef");
    let below = term(20, &format!("{marked}{after}"));
    assert_eq!(sent(&below, click(1, 1), false), None, "the output between");
    assert_eq!(
        sent(&below, click(0, 4), false),
        None,
        "the line run before"
    );
    let typed = sent(&below, click(2, 2), true);
    assert_eq!(
        typed.as_deref(),
        Some("\x1b[<0;3;1M"),
        "from the input's own row"
    );
}

#[test]
fn the_newest_line_typed_is_known_by_its_number_not_its_place() {
    let marked = "\x1b]133;A;click_events=1\x07$ \x1b]133;B\x07";
    let ran = "\x1b]133;C\x07\x1b]133;D;0\x07";
    let home = format!("\r\n\r\n\r\n{marked}oldlower{ran}\x1b[H{marked}abcdef");
    let above = term(20, &home);
    let at = sent(&above, click(0, 5), false);
    assert_eq!(
        at.as_deref(),
        Some("\x1b[<0;6;1M"),
        "the prompt at the top, old input below"
    );
    assert_eq!(sent(&above, click(3, 4), false), None, "the old line below");
}
