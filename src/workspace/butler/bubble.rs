use ely_gpui_component::{
    i18n::text,
    motion::{self, ease_out_cubic},
    theme::{ActiveTheme, Elevation, Radius},
};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, HighlightStyle, InteractiveElement,
    InteractiveText, IntoElement, ParentElement, PathBuilder, StatefulInteractiveElement, Styled,
    StyledText, UnderlineStyle, Window, canvas, div, point, px, rems,
};

use gpui::prelude::FluentBuilder;

use crate::butler::links;
use crate::workspace::Workspace;

impl Workspace {
    pub(in crate::workspace) fn butler_bubble(
        &self,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        if !self.butler.visible {
            return None;
        }
        let theme = cx.theme();
        let word = |key| text(cx, key, &[]);
        let this = cx.entity();
        let bottom = self.butler.anchor.bottom;
        let layout = self.butler.anchor.place(
            window.viewport_size().width.into(),
            window.viewport_size().height.into(),
            window.rem_size().into(),
        );
        let mut output = self.butler.text.clone();
        if let Some(error) = &self.butler.error {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&text(cx, "et.butler.failed", &[("error", error.as_str())]));
        } else if output.is_empty() && self.butler.running.is_some() {
            output = word("et.butler.waiting").to_string();
        }
        let has_output = !output.is_empty();
        let open: Vec<_> = self.sessions.iter().map(|s| s.link_id).collect();
        let reply = links::render(&output, &open, self.butler.running.is_some());
        let ranges: Vec<_> = reply.links.iter().map(|(range, _)| range.clone()).collect();
        let style = HighlightStyle {
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(theme.colors.info),
                wavy: false,
            }),
            ..Default::default()
        };
        let message = InteractiveText::new(
            "butler-message",
            StyledText::new(reply.text)
                .with_highlights(ranges.iter().cloned().map(|range| (range, style))),
        )
        .on_click(ranges, {
            let this = this.clone();
            move |index, window, cx| {
                if let Some((_, id)) = reply.links.get(index) {
                    this.update(cx, |this, cx| this.open_session_link(*id, window, cx));
                }
            }
        });
        let explanation = has_output.then(|| {
            let label = word(if self.butler.cached {
                "et.butler.cached"
            } else {
                "et.butler.explanation"
            });
            let tokens = self
                .butler
                .tokens
                .filter(|_| !self.butler.cached)
                .map(|tokens| text(cx, "et.butler.tokens", &[("count", &tokens.to_string())]));
            div()
                .ml(rems(2.5))
                .p_3()
                .rounded(theme.radius(Radius::Md))
                .bg(theme.colors.info.opacity(0.05))
                .line_height(rems(1.4))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.fg_muted)
                        .child(label),
                )
                .child(message)
                .children(tokens.map(|tokens| {
                    div()
                        .text_xs()
                        .text_color(theme.colors.fg_muted)
                        .child(tokens)
                }))
                .into_any_element()
        });
        let paper = theme.colors.overlay;
        let edge = theme.colors.border;
        let shadow = theme.elevation(Elevation::Floating);
        let tail = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let at = |x: f32, y: f32| {
                    bounds.origin
                        + point(
                            bounds.size.width * x,
                            bounds.size.height * if bottom { 1. - y } else { y },
                        )
                };
                for stroke in [false, true] {
                    let mut path = if stroke {
                        PathBuilder::stroke(px(1.))
                    } else {
                        PathBuilder::fill()
                    };
                    path.move_to(at(0., 1.));
                    path.line_to(at(0.5, 0.));
                    path.line_to(at(1., 1.));
                    if !stroke {
                        path.close();
                    }
                    match path.build() {
                        Ok(path) => window.paint_path(path, if stroke { edge } else { paper }),
                        Err(error) => log::error!("butler: cannot draw bubble tail: {error:?}"),
                    }
                }
            },
        )
        .absolute()
        .left(px(layout.tail))
        .when(!bottom, |tail| tail.top(-rems(0.4)))
        .when(bottom, |tail| tail.bottom(-rems(0.4)))
        .w(rems(0.8))
        .h(rems(0.45));
        let offset = px(layout.offset);
        Some(
            div()
                .occlude()
                .absolute()
                .left(px(layout.left))
                .w(px(layout.width))
                .rounded(theme.radius(Radius::Xl))
                .bg(paper)
                .text_color(theme.colors.fg)
                .border_1()
                .border_color(edge)
                .shadow(shadow)
                .on_mouse_down_out(move |_, _, cx| {
                    this.update(cx, |this, cx| this.dismiss_butler(cx));
                })
                .child(tail)
                .child(self.butler_header(cx))
                .child(
                    div()
                        .id("butler-report")
                        .max_h(px((layout.content_height
                            - f32::from(window.rem_size()) * 4.5)
                            .max(0.)
                            .min(f32::from(window.rem_size()) * 28.)))
                        .overflow_y_scroll()
                        .text_sm()
                        .line_height(rems(1.4))
                        .child(self.butler_cards(explanation, cx)),
                )
                .child(self.butler_footer(cx))
                .with_animation(
                    ("butler-appear", self.butler.generation),
                    Animation::new(motion::duration(motion::SLOW, cx)).with_easing(ease_out_cubic),
                    move |bubble, t| {
                        let travel = motion::NUDGE * (1. - t);
                        bubble
                            .opacity(t)
                            .when(!bottom, |bubble| bubble.top(offset + travel))
                            .when(bottom, |bubble| bubble.bottom(offset - travel))
                    },
                )
                .into_any_element(),
        )
    }
}
