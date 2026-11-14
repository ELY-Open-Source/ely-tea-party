use super::*;
use crate::term::frame::{Row, Underline};

#[test]
fn spans_count_columns_not_bytes() {
    let style = Style {
        fg: gpui::black(),
        bg: None,
        bold: false,
        italic: false,
        underline: Underline::None,
        underline_color: gpui::black(),
        strike: false,
    };
    let bold = Style {
        bold: true,
        ..style
    };
    let row = Row {
        text: "中 ab".into(),
        runs: vec![(4, style), (2, bold)],
        alone: Vec::new(),
    };
    let spans: Vec<_> = spans(&row).into_iter().map(|(at, n, _)| (at, n)).collect();
    assert_eq!(spans, [(0, 2), (2, 2)]);
}

#[test]
fn icons_live_in_the_private_use_areas() {
    assert!(private('\u{E0B0}'));
    assert!(private('\u{F0035}'));
    assert!(!private('a'));
    assert!(!private('中'));
}

#[test]
fn bold_sits_300_above_the_text_up_to_900() {
    let style = Style {
        fg: gpui::black(),
        bg: None,
        bold: false,
        italic: false,
        underline: Underline::None,
        underline_color: gpui::black(),
        strike: false,
    };
    let bold = Style {
        bold: true,
        ..style
    };
    let weight = |text: f32, style: &Style| {
        face(
            style,
            &gpui::Font {
                weight: FontWeight(text),
                ..gpui::font("JetBrains Mono")
            },
        )
        .weight
        .0
    };
    assert_eq!(weight(400., &style), 400.);
    assert_eq!(weight(400., &bold), 700.);
    assert_eq!(weight(300., &bold), 600.);
    assert_eq!(weight(700., &bold), 900.);
}

#[test]
fn an_emoji_in_one_cell_shrinks_only_beside_a_character() {
    let style = Style {
        fg: gpui::black(),
        bg: None,
        bold: false,
        italic: false,
        underline: Underline::None,
        underline_color: gpui::black(),
        strike: false,
    };
    let look = Look {
        font: gpui::font("JetBrains Mono"),
        font_id: gpui::FontId(0),
        icons: gpui::font("JetBrains Mono"),
        cjk: None,
        size: px(13.),
        cell: size(px(8.), px(16.)),
        baseline: px(12.),
        descent: px(3.),
        rule: px(1.),
        cursor: gpui::black(),
        cursor_text: gpui::black(),
        fg: gpui::black(),
        bg: gpui::black(),
    };
    let row = |text: &str| Row {
        text: text.into(),
        runs: vec![(text.len(), style)],
        alone: vec![(0, "\u{26A0}\u{FE0F}".into(), style)],
    };
    let fitted = |text: &str| glyphs::narrowed(&row(text), 0, "\u{26A0}\u{FE0F}", &look);
    assert!(fitted("  a").is_none(), "a free cell takes the overhang");
    assert!(fitted(" ").is_none(), "the row's end");
    let small = fitted(" a").expect("a character beside it");
    assert!(small.size <= look.cell.width && small.baseline < look.baseline);
    assert!(glyphs::narrowed(&row(" a"), 0, "\u{2500}", &look).is_none());
}
