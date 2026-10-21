//! The Activity tab: the shown session's rounds, newest first, each its prompt, its steps in order and the agent's last words.

use std::time::{Duration, Instant};

use ely_gpui_component::{
    primitives::{Icon, IconName},
    theme::{ActiveTheme, IconSize, Palette, TextSize},
    typography::Caption,
};
use gpui::{
    AnyElement, Context, Div, FontWeight, Hsla, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, relative, rems,
};

use crate::{
    agents::{
        self, Status,
        activity::Doing,
        timeline::{Entry, Round, Step},
    },
    sessions::{self, Session},
    workspace::Workspace,
};

/// How a round stands.
#[derive(Clone, Copy, PartialEq)]
enum Standing {
    Done,
    Failed,
    Running,
    /// Its agent left, or moved on, before it ended.
    Unended,
}

/// The color of what a step does: reading blue, writing purple, running cyan, the rest muted.
fn tone(doing: Doing, colors: &Palette) -> Hsla {
    match doing {
        Doing::Reading | Doing::Searching | Doing::Browsing => colors.info,
        Doing::Editing => colors.syntax.keyword,
        Doing::Running => colors.syntax.type_name,
        Doing::Delegating | Doing::Planning | Doing::Other => colors.fg_muted,
    }
}

/// The least time a round's bars measure against, so a quick round's steps keep short bars.
const FLOOR: Duration = Duration::from_secs(5);

/// What a step does, two characters at most, so its text lines up.
fn tag(doing: Doing) -> &'static str {
    match doing {
        Doing::Browsing => "网页",
        Doing::Delegating => "分身",
        Doing::Planning => "计划",
        Doing::Other => "工具",
        doing => doing.verb(),
    }
}

/// `took` short: `0.4s`, `12s`, then as rows count time.
fn short(took: Duration) -> String {
    if took < Duration::from_secs(10) {
        format!("{:.1}s", took.as_secs_f32())
    } else {
        agents::ago(took)
    }
}

/// The shown session's rounds, newest first: the newest open, the others closed to a summary, a press on the chevron flipping one.
pub(super) fn activity(session: Option<&Session>, cx: &mut Context<Workspace>) -> AnyElement {
    let Some(session) = session else {
        return Caption::new("No session.").px_1().pt_2().into_any_element();
    };
    let rounds = session.log.rounds();
    if rounds.is_empty() {
        let hint = match session.agent {
            Some(_) => "Nothing yet: steps show once the agent's hooks report.",
            None => "No agent runs in this session.",
        };
        return Caption::new(hint).px_1().pt_2().into_any_element();
    }
    let now = Instant::now();
    let working = session
        .agent
        .as_ref()
        .is_some_and(|agent| agent.status != Status::Idle);
    let last = rounds.len() - 1;
    // Prompts with text after each round's, which counts it back in the terminal's list.
    let mut later = 0;
    let mut cards = Vec::with_capacity(rounds.len());
    for (index, round) in rounds.iter().enumerate().rev() {
        let standing = match round.end {
            Some((true, ..)) => Standing::Failed,
            Some(_) => Standing::Done,
            None if index == last && working => Standing::Running,
            None => Standing::Unended,
        };
        let open = (index == last) != session.log.flipped.contains(&round.at);
        let back = matches!(round.prompt, Some(Some(_))).then_some(later);
        cards.push(card((index, round), (standing, open, back), now, cx));
        later += usize::from(back.is_some());
    }
    div()
        .id("activity")
        .size_full()
        .overflow_y_scroll()
        .px_1()
        .flex()
        .flex_col()
        .gap_3()
        .children(cards)
        .into_any_element()
}

/// One round: its header, then its steps and last words while open, else a summary.
fn card(
    (index, round): (usize, &Round),
    (standing, open, back): (Standing, bool, Option<usize>),
    now: Instant,
    cx: &mut Context<Workspace>,
) -> Div {
    let theme = cx.theme();
    let colors = &theme.colors;
    let started = round.at;
    let took = match round.end {
        Some((_, took, _)) => took,
        None if standing == Standing::Running => Some(now.saturating_duration_since(started)),
        None => None,
    };
    let (mark, mark_color) = match standing {
        Standing::Done => ("✓", colors.success),
        Standing::Failed => ("✗ 出错", colors.danger),
        Standing::Running => ("进行中", colors.info),
        Standing::Unended => ("未结束", colors.fg_subtle),
    };
    let when = [Some(agents::updated(started, now)), took.map(short)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
    let prompt = match round.prompt {
        Some(Some(prompt)) => sessions::one_line(prompt),
        Some(None) => "新的一轮".into(),
        None => "开始之前".into(),
    };
    let chevron = if open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    let title = div()
        .id(("round-prompt", index))
        .flex_1()
        .min_w_0()
        .truncate()
        .font_weight(FontWeight::MEDIUM)
        .text_color(colors.fg)
        .child(prompt)
        .when_some(back, |title, back| {
            title
                .cursor_pointer()
                .hover(|style| style.underline())
                .on_click(cx.listener(move |this, _, _, cx| this.show_prompt(back, cx)))
        });
    let header = div()
        .flex()
        .items_start()
        .gap_1()
        .child(
            div()
                .id(("round-flip", index))
                .flex_none()
                .pt_0p5()
                .cursor_pointer()
                .child(
                    Icon::new(chevron)
                        .size(IconSize::Sm)
                        .color(colors.fg_subtle),
                )
                .on_click(cx.listener(move |this, _, _, cx| this.flip_round(started, cx))),
        )
        .child(
            div().flex_1().min_w_0().child(title).child(
                div()
                    .flex()
                    .gap_1()
                    .text_size(theme.text_size(TextSize::Xs))
                    .text_color(colors.fg_subtle)
                    .child(when)
                    .child(div().text_color(mark_color).child(mark)),
            ),
        );
    let longest = (round.steps.iter())
        .filter_map(|entry| ran(entry, standing, now))
        .max()
        .unwrap_or_default();
    let body = match open {
        true => div()
            .flex()
            .flex_col()
            .gap_0p5()
            .children(
                round
                    .steps
                    .iter()
                    .map(|entry| step(entry, (standing, longest), now, colors)),
            )
            .children(round.end.and_then(|(_, _, said)| said).map(|said| {
                line(("结论", colors.success), colors.fg_muted).child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .line_clamp(4)
                        .child(said.to_string()),
                )
            })),
        false => summary(round, colors),
    };
    div()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(theme.text_size(TextSize::Sm))
        .child(header)
        .child(
            // A rule down the steps' left joins them to their round.
            body.ml_1p5()
                .pl_2()
                .border_l_1()
                .border_color(colors.border),
        )
}

/// A row that leads with `verb` in its color, its text in `color`.
fn line((verb, tone): (&str, Hsla), color: Hsla) -> Div {
    div().flex().items_start().gap_2().text_color(color).child(
        div()
            .w_7()
            .flex_none()
            .font_weight(FontWeight::MEDIUM)
            .text_color(tone)
            .child(verb.to_string()),
    )
}

/// A step: what it did to what, and how long it took, or how long it has run while its round does.
fn step(
    entry: &Entry,
    (standing, longest): (Standing, Duration),
    now: Instant,
    colors: &Palette,
) -> Div {
    match &entry.step {
        Step::Tool {
            doing,
            name,
            detail,
            took,
            failed,
        } => {
            let what = sessions::one_line(detail.as_deref().unwrap_or(name));
            let ran = ran(entry, standing, now);
            let (time, color, fill) = match (ran, took) {
                (Some(ran), _) if *failed => {
                    (format!("✗ {}", short(ran)), colors.danger, colors.danger)
                }
                (Some(ran), Some(_)) => (short(ran), colors.fg_subtle, tone(*doing, colors)),
                (Some(ran), None) => (format!("{}…", short(ran)), colors.info, colors.info),
                (None, _) => (String::new(), colors.fg_subtle, colors.fg_subtle),
            };
            let share = ran.map_or(0.0, |ran| share(ran, longest));
            line((tag(*doing), tone(*doing, colors)), colors.fg)
                .child(div().flex_1().min_w_0().truncate().child(what))
                .child(bar(share, fill, colors))
                .child(
                    div()
                        .flex_none()
                        .w(rems(3.25))
                        .flex()
                        .justify_end()
                        .text_color(color)
                        .child(time),
                )
        }
        Step::Asked(asked) => line(("问你", colors.warning), colors.fg).child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(sessions::one_line(asked.as_deref().unwrap_or("需要你回复"))),
        ),
        Step::Prompt(_) | Step::Ended { .. } => div(),
    }
}

/// How long a tool step ran: until its result, or until now while its round runs.
fn ran(entry: &Entry, standing: Standing, now: Instant) -> Option<Duration> {
    match (&entry.step, standing) {
        (
            Step::Tool {
                took: Some(took), ..
            },
            _,
        ) => Some(*took),
        (Step::Tool { took: None, .. }, Standing::Running) => {
            Some(now.saturating_duration_since(entry.at))
        }
        _ => None,
    }
}

/// The part of its track a step's bar fills: its time against its round's longest, or [`FLOOR`] in a quick round.
fn share(ran: Duration, longest: Duration) -> f32 {
    (ran.as_secs_f32() / longest.max(FLOOR).as_secs_f32()).clamp(0.0, 1.0)
}

/// A step's time as a bar in its track, which it never passes.
fn bar(share: f32, fill: Hsla, colors: &Palette) -> Div {
    div()
        .flex_none()
        .w(rems(2.5))
        .h(rems(0.25))
        .self_center()
        .rounded_full()
        .bg(colors.border)
        .when(share > 0.0, |track| {
            track.child(
                div()
                    .h_full()
                    .w(relative(share))
                    .min_w(rems(0.125))
                    .rounded_full()
                    .bg(fill.opacity(0.8)),
            )
        })
}

/// A closed round's steps counted by kind, each in its color, and its failures.
fn summary(round: &Round, colors: &Palette) -> Div {
    let mut counts: Vec<(&str, Hsla, usize)> = Vec::new();
    let mut failed = 0;
    for entry in &round.steps {
        let (verb, color) = match &entry.step {
            Step::Tool {
                doing, failed: f, ..
            } => {
                failed += usize::from(*f);
                (tag(*doing), tone(*doing, colors))
            }
            Step::Asked(_) => ("问你", colors.warning),
            _ => continue,
        };
        match counts.iter_mut().find(|(seen, ..)| *seen == verb) {
            Some((_, _, count)) => *count += 1,
            None => counts.push((verb, color, 1)),
        }
    }
    let failed = (failed > 0).then_some(("失败", colors.danger, failed));
    div().flex().flex_wrap().gap_x_2().children(
        counts
            .into_iter()
            .chain(failed)
            .map(|(verb, color, count)| div().text_color(color).child(format!("{verb} {count}"))),
    )
}

impl Workspace {
    /// Opens or closes a round of the shown session's Activity.
    fn flip_round(&mut self, round: Instant, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.get_mut(self.active) {
            session.log.flip(round);
            cx.notify();
        }
    }

    /// Scrolls the shown terminal to the prompt `back` prompts before its last.
    fn show_prompt(&mut self, back: usize, cx: &mut Context<Self>) {
        if let Some(session) = self.sessions.get(self.active) {
            session
                .terminal
                .update(cx, |view, cx| view.show_prompt(back, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_measure_against_the_longest_step_or_the_floor_and_never_pass_their_track() {
        let secs = Duration::from_secs_f32;
        assert_eq!(share(secs(1.0), secs(0.5)), 0.2, "a quick round");
        assert_eq!(share(secs(30.0), secs(60.0)), 0.5);
        assert_eq!(share(secs(60.0), secs(60.0)), 1.0, "the longest fills it");
        assert_eq!(share(secs(90.0), secs(60.0)), 1.0, "capped");
    }
}
