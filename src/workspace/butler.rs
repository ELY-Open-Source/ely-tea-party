use std::{sync::mpsc, time::Duration};

use ely_gpui_component::{
    i18n::text,
    motion::{self, ease_out_cubic},
    theme::{ActiveTheme, Elevation, Mix, Radius},
};
use gpui::{
    Animation, AnimationExt, AnyElement, Context, HighlightStyle, InteractiveElement,
    InteractiveText, IntoElement, ParentElement, PathBuilder, StatefulInteractiveElement, Styled,
    StyledText, UnderlineStyle, Window, canvas, div, linear_color_stop, linear_gradient, point, px,
    rems,
};

use gpui::prelude::FluentBuilder;

use super::Workspace;
use crate::{
    butler::{
        Credentials,
        attention::{self, Attention},
        links,
        placement::Anchor,
        process::{self, Cancellation, Request},
    },
    settings::SettingsStore,
};

impl Workspace {
    pub fn butler_attention(&self, cx: &gpui::App) -> Vec<Attention> {
        let mut alerts: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|session| {
                let kind = attention::kind(session.looks(cx), session.agent_look())?;
                Some(Attention {
                    id: session.id.to_string(),
                    title: session.named().chars().take(160).collect(),
                    kind,
                })
            })
            .collect();
        alerts.sort_by_key(|a| a.kind);
        alerts
    }

    fn butler_select(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.sessions.iter().position(|s| s.id.to_string() == id) {
            self.select(index, window, cx);
        }
    }

    fn open_session_link(
        &mut self,
        id: crate::sessions::LinkId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(index) = self.sessions.iter().position(|s| s.link_id == id) {
            log::debug!("butler: opening session link {}", id.url());
            self.select(index, window, cx);
        } else {
            log::debug!("butler: linked session has closed");
        }
    }

    pub fn dismiss_butler(&mut self, cx: &mut Context<Self>) {
        if self.butler.visible {
            log::debug!("butler: bubble dismissed by outside click");
            self.butler.cancel();
            self.butler.visible = false;
            cx.notify();
        }
    }

    pub fn move_butler(&mut self, anchor: Anchor, cx: &mut Context<Self>) {
        if self.butler.visible && self.butler.anchor != anchor {
            log::debug!("butler: bubble follows mascot at {anchor:?}");
            self.butler.anchor = anchor;
            cx.notify();
        }
    }

    pub fn show_butler(&mut self, anchor: Anchor, window: &mut Window, cx: &mut Context<Self>) {
        if !SettingsStore::get(cx).butler.enabled {
            return;
        }
        if self.butler.visible {
            log::debug!("butler: mascot click keeps the current reply open");
            self.move_butler(anchor, cx);
            return;
        }
        let attention = self.butler_attention(cx);
        if let Some(first) = attention.first() {
            self.butler_select(&first.id, window, cx);
        }
        self.butler.attention = attention;
        self.butler.visible = true;
        self.butler.generation = self.butler.generation.wrapping_add(1);
        self.butler.anchor = anchor;
        self.butler.cancel();
        self.butler.error = None;
        self.butler.text.clear();
        self.butler.omitted = 0;
        let config = SettingsStore::get(cx).butler.clone();
        let language = SettingsStore::get(cx).language.tag().to_string();
        cx.default_global::<Credentials>();
        let key = match config.api_key(cx.global::<Credentials>()) {
            Ok(key) => key,
            Err(error) => {
                self.butler.error = Some(error.to_string());
                return cx.notify();
            }
        };
        let ids: Vec<_> = self.sessions.iter().map(|s| s.id.to_string()).collect();
        let selected = attention::report_sessions(&ids, &self.butler.attention);
        self.butler.omitted = selected.len().saturating_sub(32);
        let sources: Vec<_> = selected
            .into_iter()
            .take(32)
            .map(|index| {
                let session = &self.sessions[index];
                session.terminal.read(cx).snapshot_source(
                    session.id.to_string(),
                    session.named().chars().take(160).collect(),
                    session.folder.to_string_lossy().chars().take(512).collect(),
                    session.link_id.url(),
                )
            })
            .collect();
        let attention = self.butler.attention.iter().take(32).cloned().collect();
        let cancel = Cancellation::default();
        let flag = cancel.0.clone();
        let (tx, rx) = mpsc::channel();
        self.butler.running = Some(cancel);
        log::info!(
            "butler: report requested for {} terminal snapshots",
            sources.len()
        );
        let worker = cx.background_executor().spawn(async move {
            let snapshots = sources.into_iter().map(|source| source.capture()).collect();
            process::run(
                Request {
                    version: 1,
                    provider: config.provider,
                    model: config.model,
                    api_key: key,
                    language,
                    persona: config.persona,
                    attention,
                    snapshots,
                },
                flag,
                tx,
            );
        });
        self.butler.task = Some(cx.spawn(async move |this, cx| {
            let _worker = worker;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                let finished = this.update(cx, |this, cx| {
                    let mut finished = false;
                    let mut changed = false;
                    for event in rx.try_iter() {
                        changed = true;
                        finished |= this.butler.apply(event);
                        if finished {
                            break;
                        }
                    }
                    if changed {
                        cx.notify();
                    }
                    finished
                });
                if finished.unwrap_or(true) {
                    break;
                }
            }
        }));
        cx.notify();
    }

    pub(super) fn butler_bubble(&self, window: &Window, cx: &Context<Self>) -> Option<AnyElement> {
        if !self.butler.visible || !SettingsStore::get(cx).butler.enabled {
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
        if self.butler.omitted > 0 && self.butler.running.is_none() {
            output.push(' ');
            output.push_str(&word("et.butler.omitted"));
        }
        let open: Vec<_> = self.sessions.iter().map(|s| s.link_id).collect();
        let reply = links::render(&output, &open, self.butler.running.is_some());
        let ranges: Vec<_> = reply.links.iter().map(|(range, _)| range.clone()).collect();
        let style = HighlightStyle {
            underline: Some(UnderlineStyle {
                thickness: px(1.),
                color: Some(
                    theme
                        .colors
                        .fg
                        .mix(&crate::agents::Kind::Deepseek.color(), 0.4)
                        .opacity(0.6),
                ),
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
        let tint = crate::agents::Kind::Deepseek.color();
        let paper = theme.colors.overlay.mix(&tint, 0.1);
        let shade = theme.colors.bg.mix(&tint, 0.08);
        let edge = paper.mix(&theme.colors.fg, 0.16);
        let gleam = theme.colors.fg.opacity(0.12);
        let shadow: Vec<_> = theme
            .elevation(Elevation::Floating)
            .into_iter()
            .map(|mut layer| {
                layer.color = layer.color.opacity(0.4);
                layer
            })
            .collect();
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
                    path.cubic_bezier_to(at(0.3, 0.12), at(0.28, 0.9), at(0.32, 0.4));
                    path.cubic_bezier_to(at(0.4, 0.1), at(0.28, -0.02), at(0.35, -0.02));
                    path.cubic_bezier_to(at(1., 1.), at(0.6, 0.6), at(0.65, 0.95));
                    if !stroke {
                        path.close();
                    }
                    match path.build() {
                        Ok(path) => window.paint_path(
                            path,
                            if stroke {
                                edge
                            } else if bottom {
                                shade
                            } else {
                                paper
                            },
                        ),
                        Err(error) => log::error!("butler: cannot draw bubble tail: {error:?}"),
                    }
                }
            },
        )
        .absolute()
        .left(px(layout.tail))
        .when(!bottom, |tail| tail.top(-rems(0.7)))
        .when(bottom, |tail| tail.bottom(-rems(0.7)))
        .w(rems(1.8))
        .h(rems(0.8));
        let offset = px(layout.offset);
        Some(
            div()
                .occlude()
                .absolute()
                .left(px(layout.left))
                .w(px(layout.width))
                .px_6()
                .py_5()
                .rounded(theme.radius(Radius::Xl) * 2.5)
                .when(!bottom, |bubble| {
                    bubble.rounded_bl(theme.radius(Radius::Lg) * 1.5)
                })
                .when(bottom, |bubble| {
                    bubble.rounded_tl(theme.radius(Radius::Lg) * 1.5)
                })
                .bg(linear_gradient(
                    180.,
                    linear_color_stop(paper, 0.),
                    linear_color_stop(shade, 1.),
                ))
                .text_color(theme.colors.fg)
                .border_1()
                .border_color(edge)
                .shadow(shadow)
                .on_mouse_down_out(move |_, _, cx| {
                    this.update(cx, |this, cx| this.dismiss_butler(cx));
                })
                .child(tail)
                .child(
                    div()
                        .absolute()
                        .top(rems(0.45))
                        .right(rems(1.5))
                        .w(rems(2.))
                        .h(px(1.))
                        .rounded_full()
                        .bg(linear_gradient(
                            90.,
                            linear_color_stop(gleam, 0.),
                            linear_color_stop(gleam.opacity(0.), 1.),
                        )),
                )
                .child(
                    div()
                        .id("butler-report")
                        .max_h(px(layout.content_height))
                        .overflow_y_scroll()
                        .text_sm()
                        .line_height(rems(1.6))
                        .child(message),
                )
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
