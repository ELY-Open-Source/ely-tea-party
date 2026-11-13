//! Prompt marks inside the terminal's left gutter; hovering previews a turn and clicking jumps to it.

use ely_gpui_component::{
    i18n::text,
    theme::{ActiveTheme, Elevation, Radius, TextSize},
};
use gpui::{
    AnyElement, App, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels,
    ScrollHandle, StatefulInteractiveElement, Styled, Window, anchored, div, point,
    prelude::FluentBuilder, px, relative,
};

use super::{turns::Place, view::TerminalView};

/// What the pointer does on the rail.
#[derive(Default)]
pub struct Rail {
    hovered: Option<usize>,
    current: Option<usize>,
    scroll: ScrollHandle,
}

impl Rail {
    pub(super) fn width(window: &Window, cx: &App) -> Pixels {
        cx.theme()
            .scrollbar_thickness()
            .to_pixels(window.rem_size())
            * 8.
    }
}

impl TerminalView {
    /// Compact turn marks, centered independently of terminal output lengths.
    pub(super) fn rail(
        &mut self,
        inset: f32,
        offset: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let turns = &self.turns.list;
        if turns.is_empty() {
            return Vec::new();
        }
        let current = current_turn(turns.iter().map(|turn| turn.place), offset);
        if current != self.rail.current {
            self.rail.current = current;
            if let Some(index) = current {
                self.rail.scroll.scroll_to_item(index);
            }
        }
        let width = Rail::width(window, cx);
        let unit = width / 8.;
        let pitch = unit * (10. / 6.);
        let theme = cx.theme();
        let colors = &theme.colors;
        let mut list = div()
            .id("turn-rail")
            .w_full()
            .h(pitch * turns.len() as f32)
            .max_h(relative(0.7))
            .flex()
            .flex_col()
            .overflow_y_scroll()
            .occlude()
            .track_scroll(&self.rail.scroll)
            .on_scroll_wheel(cx.listener(|view, _, _, cx| {
                view.rail.hovered = None;
                cx.notify();
            }));
        for (index, turn) in turns.iter().enumerate() {
            let distance = self.rail.hovered.map(|hovered| hovered.abs_diff(index));
            let length = match distance {
                Some(0) => 26.,
                Some(1) => 20.,
                Some(2) => 14.,
                Some(3) => 10.,
                _ => 6.,
            };
            let opacity = if distance == Some(0) {
                1.
            } else if current == Some(index) && self.rail.hovered.is_none() {
                0.6
            } else {
                0.4
            };
            let located = matches!(turn.place, Place::At(_));
            list = list.child(
                div()
                    .id(("turn", index))
                    .flex_none()
                    .h(pitch)
                    .w_full()
                    .pl(unit * (16. / 6.))
                    .flex()
                    .items_center()
                    .block_mouse_except_scroll()
                    .when(located, |mark| mark.cursor_pointer())
                    .child(
                        div()
                            .w(unit * (length / 6.))
                            .h(theme.hairline() * 2.)
                            .rounded_full()
                            .bg(colors.fg.opacity(opacity)),
                    )
                    .on_hover(cx.listener(move |view, over: &bool, _, cx| {
                        log::trace!("terminal {}: turn {index} hovered {over}", view.pty.pid);
                        if *over {
                            view.rail.hovered = Some(index);
                        } else if view.rail.hovered == Some(index) {
                            view.rail.hovered = None;
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |view, _, _, cx| {
                            cx.stop_propagation();
                            if let Some(Place::At(line)) =
                                view.turns.list.get(index).map(|t| t.place)
                            {
                                view.scroll_to_turn(line, cx);
                            }
                        }),
                    ),
            );
        }
        vec![
            div()
                .absolute()
                .top(px(inset))
                .bottom(px(inset))
                .left_0()
                .w(width)
                .flex()
                .flex_col()
                .justify_center()
                .child(list)
                .children(self.card(cx))
                .into_any_element(),
        ]
    }

    /// Preview beside the hovered tick, outside the scrolling list.
    fn card(&self, cx: &Context<Self>) -> Option<AnyElement> {
        let turns = &self.turns.list;
        let index = self.rail.hovered?;
        let turn = turns.get(index)?;
        let bounds = self.rail.scroll.bounds_for_item(index)?;
        let list = self.rail.scroll.bounds();
        let y = bounds.top() + self.rail.scroll.offset().y;
        if y < list.top() || y >= list.bottom() {
            return None;
        }
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
        let unavailable = match turn.place {
            Place::At(_) => None,
            Place::Gone => Some("et.terminal.turn_gone"),
            Place::Seeking | Place::Lost => Some("et.terminal.turn_unlocated"),
        };
        let card = div()
            .w(theme.sheet_size())
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
            .children(unavailable.map(|key| {
                line(
                    text(cx, key, &[]).to_string(),
                    TextSize::Xs,
                    colors.fg_subtle,
                )
            }));
        Some(
            anchored()
                .position(point(list.right(), y))
                .snap_to_window()
                .child(card)
                .into_any_element(),
        )
    }
}

fn current_turn(places: impl Iterator<Item = Place>, offset: usize) -> Option<usize> {
    let located: Vec<_> = places
        .enumerate()
        .filter_map(|(index, place)| match place {
            Place::At(line) => Some((index, line)),
            _ => None,
        })
        .collect();
    if offset == 0 {
        return located.last().map(|&(index, _)| index);
    }
    located
        .iter()
        .rev()
        .find(|&&(_, line)| line <= 1 - offset as i32)
        .or_else(|| located.first())
        .map(|&(index, _)| index)
}

#[cfg(test)]
mod tests;
