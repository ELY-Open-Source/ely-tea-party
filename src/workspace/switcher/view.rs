use std::ops::Range;

use ely_gpui_component::{
    i18n::text,
    primitives::{Backdrop, FocusScope, Icon, IconName},
    theme::{ActiveTheme, Elevation, IconSize, Radius, TextSize},
    typography::Ellipsis,
};
use gpui::{
    App, Context, FontWeight, HighlightStyle, MouseButton, Render, Role, SharedString, StyledText,
    Window, div, prelude::*, rems,
};

use super::{Switcher, SwitcherNext, SwitcherPrevious};
use crate::agents::icons;

fn marked(value: &str, hits: &[Range<usize>], cx: &App) -> StyledText {
    let style = HighlightStyle {
        color: Some(cx.theme().colors.accent),
        font_weight: Some(FontWeight::SEMIBOLD),
        ..Default::default()
    };
    StyledText::new(SharedString::from(value.to_owned()))
        .with_highlights(hits.iter().cloned().map(|hit| (hit, style)))
}

impl Render for Switcher {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let margin = rems(2.).to_pixels(window.rem_size());
        let card = div()
            .id("session-switcher-card")
            .key_context("SessionSwitcher")
            .role(Role::Dialog)
            .aria_label(text(cx, "et.switcher.title", &[]))
            .flex()
            .flex_col()
            .w(theme.palette_size().width)
            .h(theme.palette_size().height)
            .max_w(window.viewport_size().width - margin)
            .max_h(window.viewport_size().height - margin)
            .rounded(theme.radius(Radius::Xl))
            .bg(colors.overlay)
            .border_1()
            .border_color(colors.border)
            .shadow(theme.elevation(Elevation::Modal))
            .text_size(theme.text_size(TextSize::Sm))
            .text_color(colors.fg)
            .on_action(cx.listener(|this, _: &SwitcherPrevious, _, cx| {
                cx.stop_propagation();
                this.step(-1, cx);
            }))
            .on_action(cx.listener(|this, _: &SwitcherNext, _, cx| {
                cx.stop_propagation();
                this.step(1, cx);
            }))
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    cx.stop_propagation();
                    this.dismiss(window, cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap_3()
                    .px_4()
                    .py_3()
                    .border_b_1()
                    .border_color(colors.border)
                    .child(
                        Icon::new(IconName::Search)
                            .size(IconSize::Sm)
                            .color(colors.fg_muted),
                    )
                    .child(div().flex_1().min_w_0().child(self.input.clone())),
            )
            .child(
                div()
                    .id("session-switcher-list")
                    .role(Role::ListBox)
                    .aria_label(text(cx, "et.switcher.title", &[]))
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .p_2()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .children(self.rows.iter().map(|row| {
                        let entry = &row.entry;
                        let id = entry.id;
                        let active = self.selected == Some(id);
                        let status = text(cx, entry.state.label(), &[]);
                        div()
                            .id(("switch-session", id))
                            .role(Role::ListBoxOption)
                            .aria_selected(active)
                            .aria_label(format!(
                                "{} · {} · {} · {}",
                                entry.title, entry.folder, entry.agent, status
                            ))
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap_3()
                            .px_3()
                            .py_2()
                            .rounded(theme.radius(Radius::Md))
                            .when(active, |row| row.bg(colors.active))
                            .hover(|row| row.bg(colors.hover))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    window.prevent_default();
                                    cx.stop_propagation();
                                    this.selected = Some(id);
                                    this.pick(window, cx);
                                }),
                            )
                            .child(div().flex_none().child(icons::icon(entry.kind, cx)))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(div().text_ellipsis().child(marked(
                                        &entry.title,
                                        &row.matched.hits[0],
                                        cx,
                                    )))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .text_size(theme.text_size(TextSize::Xs))
                                            .text_color(colors.fg_muted)
                                            .child(div().flex_none().child(marked(
                                                &entry.agent,
                                                &row.matched.hits[2],
                                                cx,
                                            )))
                                            .child(div().flex_1().min_w_0().text_ellipsis().child(
                                                marked(&entry.folder, &row.matched.hits[1], cx),
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .flex()
                                    .flex_col()
                                    .items_end()
                                    .gap_1()
                                    .text_size(theme.text_size(TextSize::Xs))
                                    .text_color(entry.state.color(colors))
                                    .child(status)
                                    .when(self.current == Some(id), |column| {
                                        column.child(div().text_color(colors.fg_muted).child(text(
                                            cx,
                                            "et.switcher.current",
                                            &[],
                                        )))
                                    }),
                            )
                    }))
                    .when(self.rows.is_empty(), |list| {
                        list.child(div().p_4().text_color(colors.fg_muted).child(text(
                            cx,
                            "et.switcher.empty",
                            &[],
                        )))
                    }),
            )
            .child(
                div()
                    .flex_none()
                    .px_4()
                    .py_2()
                    .border_t_1()
                    .border_color(colors.border)
                    .text_size(theme.text_size(TextSize::Xs))
                    .text_color(colors.fg_muted)
                    .child(Ellipsis::new(text(cx, "et.switcher.help", &[]))),
            );
        let switcher = cx.entity();
        Backdrop::new("session-switcher")
            .on_dismiss(move |window, cx| switcher.update(cx, |this, cx| this.dismiss(window, cx)))
            .child(FocusScope::new(&self.focus).trap().child(card))
    }
}
