use alacritty_terminal::{
    event::VoidListener,
    term::{Config, test::TermSize},
    vte::ansi::Processor,
};

use super::*;

/// A terminal `lines` high keeping `history` lines of scrollback.
fn term(lines: usize, history: usize) -> (Term<VoidListener>, Processor) {
    let config = Config {
        scrolling_history: history,
        ..Config::default()
    };
    let term = Term::new(config, &TermSize::new(40, lines), VoidListener);
    (term, Processor::new())
}

fn places(turns: &Turns) -> Vec<Place> {
    turns.list.iter().map(|turn| turn.place).collect()
}

#[test]
fn a_prompt_s_row_is_found_and_scrolls_with_its_text() {
    let (mut term, mut parser) = term(4, 100);
    let mut turns = Turns::default();
    let now = Instant::now();
    turns.prompted("fix the  build\nplease".into());
    turns.follow(&mut term, now);
    assert_eq!(places(&turns), [Place::Seeking]);
    parser.advance(&mut term, b"old\r\n> fix the build\r\n");
    turns.follow(&mut term, now + SEEK_EVERY);
    assert_eq!(places(&turns), [Place::At(1)]);
    parser.advance(&mut term, b"a\r\nb\r\nc\r\nd\r\n");
    turns.follow(&mut term, now + SEEK_EVERY * 2);
    assert_eq!(places(&turns), [Place::At(-2)]);
}

#[test]
fn wide_characters_and_placeholders_still_find_the_row() {
    let (mut term, mut parser) = term(4, 100);
    let mut turns = Turns::default();
    turns.prompted("[Image #1] 我觉得cmux这个展示挺好的".into());
    parser.advance(
        &mut term,
        "> [Image #1] 我觉得cmux这个展示挺好的\r\n".as_bytes(),
    );
    turns.follow(&mut term, Instant::now());
    assert_eq!(places(&turns), [Place::At(0)]);
}

#[test]
fn a_row_the_agent_redraws_is_sought_again_after_the_last_prompt_only() {
    let (mut term, mut parser) = term(6, 100);
    let mut turns = Turns::default();
    let now = Instant::now();
    turns.prompted("first".into());
    parser.advance(&mut term, b"> first\r\n");
    turns.follow(&mut term, now);
    turns.prompted("second".into());
    parser.advance(&mut term, b"> second\r\n");
    turns.follow(&mut term, now + SEEK_EVERY);
    assert_eq!(places(&turns), [Place::At(0), Place::At(1)]);
    // The agent clears its lines and prints them again lower.
    parser.advance(
        &mut term,
        b"\x1b[1;1H\x1b[2K\x1b[2;1H\x1b[2K\x1b[4;1H> first\r\n> second\r\n",
    );
    turns.follow(&mut term, now + SEEK_EVERY * 2);
    assert_eq!(places(&turns), [Place::Gone, Place::At(4)]);
}

#[test]
fn a_prompt_trimmed_from_the_scrollback_is_gone() {
    let (mut term, mut parser) = term(2, 3);
    let mut turns = Turns::default();
    let now = Instant::now();
    turns.prompted("one".into());
    parser.advance(&mut term, b"> one\r\n");
    turns.follow(&mut term, now);
    turns.prompted("two".into());
    parser.advance(&mut term, b"1\r\n2\r\n3\r\n4\r\n5\r\n> two\r\n");
    turns.follow(&mut term, now + SEEK_EVERY);
    assert_eq!(places(&turns), [Place::Gone, Place::At(0)]);
}

#[test]
fn the_same_words_again_find_the_row_after_the_last_prompt() {
    let (mut term, mut parser) = term(6, 100);
    let mut turns = Turns::default();
    let now = Instant::now();
    turns.prompted("go on".into());
    parser.advance(&mut term, b"> go on\r\nok\r\n");
    turns.follow(&mut term, now);
    turns.prompted("go on".into());
    parser.advance(&mut term, b"> go on\r\n");
    turns.follow(&mut term, now + SEEK_EVERY);
    assert_eq!(places(&turns), [Place::At(0), Place::At(2)]);
}

#[test]
fn prompts_never_shown_stop_being_sought() {
    let (mut term, _) = term(4, 100);
    let mut turns = Turns::default();
    for prompt in ["a1", "b2", "c3", "d4"] {
        turns.prompted(prompt.into());
    }
    turns.follow(&mut term, Instant::now());
    let expected = [Place::Lost, Place::Seeking, Place::Seeking, Place::Seeking];
    assert_eq!(places(&turns), expected);
}

#[test]
fn a_probe_skips_placeholders_and_collapses_spaces() {
    assert_eq!(probe("\n  [Pasted text #1 +20 lines]  a   b\nc"), "a b");
    assert_eq!(probe("[Image #1][Image #2] x"), "x");
    assert_eq!(probe("abcdefghijklmnopqrstuvwxyz"), "abcdefghijklmnop");
    assert_eq!(probe(""), "");
}
