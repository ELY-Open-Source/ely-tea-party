use std::time::Instant;

use ely_gpui_component::{
    buttons::{Button, ButtonVariant, IconButton},
    i18n::text,
    primitives::{Icon, IconName, Tooltip},
    theme::{ActiveTheme, ControlSize, IconSize, Radius},
};
use gpui::{AnyElement, Context, Div, FontWeight, div, prelude::*, rems};

use crate::{
    agents::timeline::Step,
    butler::tasks::{self, Status},
    sessions,
    settings::SettingsStore,
    workspace::Workspace,
};

impl Workspace {
    pub(super) fn butler_header(&self, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let count = self
            .sessions
            .iter()
            .filter(|session| {
                matches!(
                    tasks::observed(session, cx),
                    Status::Error | Status::Waiting
                )
            })
            .count();
        div()
            .flex()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(theme.colors.border.opacity(0.6))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(rems(2.25))
                    .rounded(theme.radius(Radius::Lg))
                    .bg(theme.colors.info.opacity(0.1))
                    .text_color(theme.colors.info)
                    .child(Icon::new(IconName::Sparkles).size(IconSize::Md)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(text(
                        cx,
                        "et.butler.overview",
                        &[],
                    )))
                    .child(div().text_xs().text_color(theme.colors.fg_muted).child(
                        if count == 0 {
                            text(cx, "et.butler.no_attention", &[])
                        } else {
                            text(cx, "et.butler.tasks", &[("count", &count.to_string())])
                        },
                    )),
            )
            .child(
                IconButton::new("butler-memory", IconName::BookOpen)
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .tooltip(text(cx, "et.memory.title", &[]))
                    .on_click(cx.listener(|this, _, _, cx| {
                        let folder = this.sessions.get(this.active).map(|s| s.folder.clone());
                        crate::memory::window::open(folder, None, cx);
                    })),
            )
            .child(
                IconButton::new("butler-close", IconName::X)
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .tooltip(text(cx, "et.butler.close", &[]))
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_butler(cx))),
            )
    }

    pub(super) fn butler_footer(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let enabled = SettingsStore::get(cx).butler.enabled;
        div()
            .id("butler-local-note")
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(theme.colors.border.opacity(0.6))
            .text_xs()
            .text_color(theme.colors.fg_muted)
            .child(
                div()
                    .size(rems(0.375))
                    .rounded_full()
                    .bg(theme.colors.success),
            )
            .child(text(cx, "et.butler.local_short", &[]))
            .child(div().flex_1())
            .child(text(
                cx,
                if enabled {
                    "et.butler.ai_on_demand"
                } else {
                    "et.butler.ai_optional"
                },
                &[],
            ))
            .tooltip(Tooltip::text(text(
                cx,
                if enabled {
                    "et.butler.explain_hint"
                } else {
                    "et.butler.enable_hint"
                },
                &[],
            )))
            .into_any_element()
    }

    pub(super) fn butler_cards(
        &self,
        mut explanation: Option<AnyElement>,
        cx: &Context<Self>,
    ) -> Div {
        let theme = cx.theme();
        let word = |key| text(cx, key, &[]);
        let enabled = SettingsStore::get(cx).butler.enabled;
        let home = dirs::home_dir();
        let now = Instant::now();
        let mut tasks: Vec<_> = self
            .sessions
            .iter()
            .map(|session| (session, tasks::observed(session, cx)))
            .collect();
        tasks.sort_by_key(|(_, status)| *status);
        let attention = tasks
            .iter()
            .filter(|(_, status)| matches!(status, Status::Error | Status::Waiting))
            .count();
        div()
            .flex()
            .flex_col()
            .px_2()
            .py_2()
            .children(
                tasks
                    .into_iter()
                    .enumerate()
                    .map(|(index, (session, status))| {
                        let id = session.link_id;
                        let expanded = self.butler.expanded == Some(id);
                        let prompt = session
                            .log
                            .entries
                            .iter()
                            .rev()
                            .find_map(|entry| match &entry.step {
                                Step::Prompt(prompt) => Some(prompt.as_deref()),
                                _ => None,
                            })
                            .flatten()
                            .map(tasks::excerpt)
                            .filter(|prompt| !prompt.is_empty());
                        let latest = session
                            .log
                            .entries
                            .back()
                            .and_then(|entry| tasks::latest(&entry.step))
                            .or_else(|| {
                                let (_, record) = session.terminal.read(cx).records().shown()?;
                                record.msg.as_deref().map(|message| {
                                    ("et.butler.task_message", tasks::excerpt(message))
                                })
                            });
                        let preview = latest
                            .as_ref()
                            .map(|(key, detail)| text(cx, key, &[("detail", detail)]))
                            .or_else(|| {
                                prompt
                                    .as_ref()
                                    .map(|prompt| word_value(cx, "et.butler.task_goal", prompt))
                            })
                            .unwrap_or_else(|| word(status.label()));
                        let color = match status {
                            Status::Error => theme.colors.danger,
                            Status::Waiting => theme.colors.warning,
                            Status::Ended => theme.colors.success,
                            Status::Working => theme.colors.info,
                            _ => theme.colors.fg_muted,
                        };
                        let avatar = if let Some(agent) = &session.agent {
                            sessions::avatar(
                                agent,
                                agent.pose(now, session.id),
                                rems(2.),
                                (session, now),
                                cx,
                            )
                            .into_any_element()
                        } else {
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .size(rems(2.))
                                .rounded(theme.radius(Radius::Lg))
                                .bg(theme.colors.surface)
                                .text_color(theme.colors.fg_muted)
                                .child(Icon::new(IconName::Terminal).size(IconSize::Sm))
                                .into_any_element()
                        };
                        let status_label = match status {
                            Status::Error => "et.butler.badge_error",
                            Status::Waiting => "et.butler.badge_waiting",
                            Status::Ended => "et.butler.badge_ended",
                            Status::Working => "et.butler.task_working",
                            Status::Idle => "et.status.idle",
                            Status::Unknown => "et.butler.badge_unknown",
                        };
                        let heading = div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().flex_shrink_0().child(avatar))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .id(("butler-title", session.id))
                                            .truncate()
                                            .font_weight(FontWeight::MEDIUM)
                                            .cursor_pointer()
                                            .hover(|style| style.underline())
                                            .child(tasks::excerpt(session.named()))
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.open_session_link(id, window, cx)
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id(("butler-folder", session.id))
                                            .truncate()
                                            .text_xs()
                                            .text_color(theme.colors.fg_muted)
                                            .child(sessions::brief(
                                                &session.folder,
                                                home.as_deref(),
                                            ))
                                            .tooltip(Tooltip::text(sessions::shown(
                                                &session.folder,
                                                home.as_deref(),
                                            ))),
                                    ),
                            )
                            .child(
                                div()
                                    .id(("butler-status", session.id))
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .px_2()
                                    .py_1()
                                    .rounded(theme.radius(Radius::Sm))
                                    .text_xs()
                                    .text_color(color)
                                    .bg(color.opacity(0.08))
                                    .child(word(status_label))
                                    .tooltip(Tooltip::text(word(status.label()))),
                            )
                            .child(
                                IconButton::new(
                                    ("butler-open", session.id),
                                    IconName::ArrowUpRight,
                                )
                                .size(ControlSize::Sm)
                                .variant(ButtonVariant::Ghost)
                                .tooltip(word("et.butler.open_task"))
                                .on_click(cx.listener(
                                    move |this, _, window, cx| {
                                        this.open_session_link(id, window, cx)
                                    },
                                )),
                            );
                        let row = div()
                            .id(("butler-task", session.id))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .px_3()
                            .py_2()
                            .rounded(theme.radius(Radius::Lg))
                            .hover(|style| style.bg(theme.colors.fg.opacity(0.025)))
                            .when(expanded, |row| row.bg(theme.colors.surface.opacity(0.55)))
                            .child(heading)
                            .child(
                                div()
                                    .ml(rems(2.5))
                                    .text_color(theme.colors.fg_muted)
                                    .line_height(rems(1.35))
                                    .when(!expanded, |preview| preview.line_clamp(2))
                                    .child(preview),
                            )
                            .when(expanded && latest.is_some(), |row| {
                                row.children(prompt.map(|prompt| {
                                    div()
                                        .ml(rems(2.5))
                                        .text_xs()
                                        .text_color(theme.colors.fg_muted)
                                        .child(word_value(cx, "et.butler.task_goal", &prompt))
                                }))
                            })
                            .child(
                                div()
                                    .ml(rems(2.5))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        Button::new(
                                            ("butler-details", session.id),
                                            word(if expanded {
                                                "et.butler.collapse"
                                            } else {
                                                "et.butler.details"
                                            }),
                                        )
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .trailing_icon(if expanded {
                                            IconName::ChevronUp
                                        } else {
                                            IconName::ChevronDown
                                        })
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.butler.expanded = (!expanded).then_some(id);
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .when(enabled, |actions| {
                                        actions.child(
                                            Button::new(
                                                ("butler-explain", session.id),
                                                word("et.butler.explain"),
                                            )
                                            .size(ControlSize::Sm)
                                            .variant(ButtonVariant::Ghost)
                                            .icon(IconName::Sparkles)
                                            .disabled(self.butler.running.is_some())
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    this.explain_task(id, cx)
                                                }),
                                            ),
                                        )
                                    }),
                            )
                            .children(if self.butler.target == Some(id) {
                                explanation.take()
                            } else {
                                None
                            });
                        div()
                            .when(index == 0 || index == attention, |group| {
                                group.child(
                                    div()
                                        .px_3()
                                        .pt_2()
                                        .pb_1()
                                        .text_xs()
                                        .font_weight(FontWeight::MEDIUM)
                                        .text_color(theme.colors.fg_muted)
                                        .child(word(if index < attention {
                                            "et.butler.needs_attention"
                                        } else {
                                            "et.butler.other_tasks"
                                        })),
                                )
                            })
                            .when(index > 0 && index != attention, |group| {
                                group.child(
                                    div()
                                        .mx_3()
                                        .h(gpui::px(1.))
                                        .bg(theme.colors.border.opacity(0.4)),
                                )
                            })
                            .child(row)
                    }),
            )
    }
}

fn word_value(cx: &Context<Workspace>, key: &str, detail: &str) -> gpui::SharedString {
    text(cx, key, &[("detail", detail)])
}
