//! Prompt marks inside the terminal's left gutter; hovering previews a turn and clicking jumps to it.

use ely_gpui_component::theme::{ActiveTheme, Elevation, Radius, TextSize};
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder, px, relative,
};

use super::{
    turns::{Place, Turn},
    view::TerminalView,
};

/// The mark the pointer is over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pin {
    Turn(usize),
    /// The prompts trimmed from the scrollback, which share one mark at the top.
    Gone,
}

/// What the pointer does on the rail.
#[derive(Default)]
pub struct Rail {
    hovered: Option<Pin>,
}

impl Rail {
    pub(super) fn width(window: &Window, cx: &App) -> Pixels {
        cx.theme()
            .scrollbar_thickness()
            .to_pixels(window.rem_size())
            * 2.
    }
}

/// The scrollback's size and the screen's, in lines.
pub type Scrollback = (usize, usize);

impl TerminalView {
    /// Marks in a dedicated left gutter, with the hovered prompt's card opening into the terminal.
    pub(super) fn rail(
        &self,
        (history, lines): Scrollback,
        inset: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let turns = &self.turns.list;
        let placed = |(index, turn): (usize, &Turn)| match turn.place {
            Place::At(line) => Some((index, line)),
            _ => None,
        };
        let marks: Vec<(usize, i32)> = turns.iter().enumerate().filter_map(placed).collect();
        let gone = turns
            .iter()
            .filter(|turn| turn.place == Place::Gone)
            .count();
        if history == 0 && marks.is_empty() && gone == 0 {
            return Vec::new();
        }
        let width = Rail::width(window, cx);
        let colors = &cx.theme().colors;
        let total = (history + lines) as f32;
        let at = |line: i32| ((line + history as i32) as f32 + 0.5) / total;
        let mut rail = div().absolute().top_0().bottom_0().left_0().w(width);
        let pins = marks
            .iter()
            .map(|&(index, line)| (Pin::Turn(index), at(line)))
            .chain((gone > 0).then_some((Pin::Gone, 0.)));
        for (pin, place) in pins {
            let hovered = self.rail.hovered == Some(pin);
            let color = match (pin, hovered) {
                (Pin::Gone, _) => colors.fg_subtle,
                (_, true) => colors.accent_hover,
                (_, false) => colors.accent,
            };
            let id = match pin {
                Pin::Turn(index) => index,
                Pin::Gone => usize::MAX,
            };
            rail = rail.child(
                div()
                    .id(("turn", id))
                    .absolute()
                    .left_0()
                    .right_0()
                    .top(relative(place))
                    .mt(px(-4.))
                    .h(px(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .occlude()
                    .when(pin != Pin::Gone, |mark| mark.cursor_pointer())
                    .child(
                        div()
                            .w(if hovered { width } else { px(7.) })
                            .h(px(2.))
                            .rounded_full()
                            .bg(color),
                    )
                    .on_hover(cx.listener(move |view, over: &bool, _, cx| {
                        if *over {
                            view.rail.hovered = Some(pin);
                        } else if view.rail.hovered == Some(pin) {
                            view.rail.hovered = None;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            let Pin::Turn(index) = pin else { return };
                            if let Some(Place::At(line)) =
                                view.turns.list.get(index).map(|t| t.place)
                            {
                                view.scroll_to_turn(line, cx);
                            }
                        }),
                    ),
            );
        }
        let mut shown = vec![rail.into_any_element()];
        shown.extend(self.card(gone, (inset, width), at, cx));
        shown
    }

    /// The prompt under the pointer: its words, how its turn ended, and whether the scrollback still holds it.
    fn card(
        &self,
        gone: usize,
        (inset, width): (f32, Pixels),
        at: impl Fn(i32) -> f32,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let turns = &self.turns.list;
        let (turn, place) = match self.rail.hovered? {
            Pin::Turn(index) => {
                let turn = turns.get(index)?;
                let Place::At(line) = turn.place else {
                    return None;
                };
                (turn, at(line))
            }
            Pin::Gone => (turns.iter().rfind(|turn| turn.place == Place::Gone)?, 0.),
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let line = |text: String, size, color| {
            div()
                .max_w_full()
                .truncate()
                .text_size(theme.text_size(size))
                .text_color(color)
                .child(text)
        };
        let trimmed = (turn.place == Place::Gone).then(|| match gone {
            1 => "已不在回滚记录中".to_string(),
            more => format!("已不在回滚记录中 · 共 {more} 条"),
        });
        let card = div()
            .absolute()
            .left(width + px(6.))
            .max_w(px(380.))
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_2()
            .rounded(theme.radius(Radius::Md))
            .border_1()
            .border_color(colors.border)
            .bg(colors.overlay)
            .shadow(theme.elevation(Elevation::Floating))
            .child(
                line(turn.prompt.clone(), TextSize::Base, colors.fg)
                    .font_weight(gpui::FontWeight::SEMIBOLD),
            )
            .children(
                turn.reply
                    .clone()
                    .map(|reply| line(reply, TextSize::Sm, colors.fg_muted)),
            )
            .children(trimmed.map(|text| line(text, TextSize::Xs, colors.fg_subtle)));
        // Below the mark in the view's upper part, above it lower down.
        let card = if place < 0.7 {
            card.top(relative(place)).mt(px(inset - 6.))
        } else {
            card.bottom(relative(1. - place)).mb(px(inset - 6.))
        };
        Some(card.into_any_element())
    }
}
