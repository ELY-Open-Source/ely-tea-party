//! The foot of the right column: how much of the shown agent's subscription is used.

use std::{
    collections::HashMap,
    time::{Instant, SystemTime},
};

use ely_gpui_component::{data_display::Meter, theme::ActiveTheme, typography::Caption};
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, Task, div};

use super::Workspace;
use crate::{
    sessions::Session,
    settings::SettingsStore,
    usage::{self, Plan, Usage},
};

/// The readings the right column shows, by plan.
#[derive(Default)]
pub(super) struct Meters {
    read: HashMap<Plan, (Result<Usage, String>, Instant)>,
    reading: Option<Plan>,
    _task: Option<Task<()>>,
}

/// The plan the session's agent runs on.
fn plan(session: Option<&Session>) -> Option<Plan> {
    Plan::of(session?.agent.as_ref()?.kind)
}

impl Workspace {
    /// Reads the shown agent's usage again once its last reading is stale, while the right column shows.
    pub(super) fn meter(&mut self, now: Instant, cx: &mut Context<Self>) {
        let Some(plan) = plan(self.sessions.get(self.active)) else {
            return;
        };
        let fresh = self
            .meters
            .read
            .get(&plan)
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) < plan.fresh());
        if fresh || self.meters.reading.is_some() || !SettingsStore::get(cx).show_panel {
            return;
        }
        self.meters.reading = Some(plan);
        let read = cx
            .background_executor()
            .spawn(async move { usage::read(plan) });
        self.meters._task = Some(cx.spawn(async move |this, cx| {
            let read = read.await;
            let kept = this.update(cx, |this, cx| {
                match &read {
                    Ok(usage) => log::debug!("usage: {plan:?} read: {usage:?}"),
                    Err(error) => log::info!("{error:#}"),
                }
                let read = read.map_err(|error| format!("{error:#}"));
                this.meters.read.insert(plan, (read, Instant::now()));
                this.meters.reading = None;
                cx.notify();
            });
            if kept.is_err() {
                log::debug!("usage: the window closed before {plan:?}'s reading came");
            }
        }));
    }

    /// The shown agent's usage, for agents whose plan can be read.
    pub(super) fn usage(&self, cx: &App) -> Option<AnyElement> {
        let plan = plan(self.sessions.get(self.active))?;
        let theme = cx.theme();
        let foot = div()
            .flex_none()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_3()
            .border_t_1()
            .border_color(theme.colors.border);
        let Some((read, _)) = self.meters.read.get(&plan) else {
            return Some(
                foot.child(Caption::new(format!("读取 {} 用量…", plan.name())))
                    .into_any_element(),
            );
        };
        let usage = match read {
            Ok(usage) => usage,
            Err(error) => {
                let said = error.strip_prefix("usage: ").unwrap_or(error);
                return Some(
                    foot.child(Caption::new(format!("{} 用量读不到：{said}", plan.name())))
                        .into_any_element(),
                );
            }
        };
        let title = match &usage.plan {
            Some(name) => format!("{} 用量 · {name}", plan.name()),
            None => format!("{} 用量", plan.name()),
        };
        let now = SystemTime::now();
        let meters = usage.limits.iter().enumerate().map(|(index, limit)| {
            let detail = match limit.resets {
                Some(resets) => {
                    format!("{:.0}% · {}", limit.percent, usage::resets_in(resets, now))
                }
                None => format!("{:.0}%", limit.percent),
            };
            Meter::new(("usage", index), limit.label.clone(), limit.percent / 100.).detail(detail)
        });
        let credits = usage
            .credits
            .as_ref()
            .map(|credits| Caption::new(format!("Credits 余额 {credits}")));
        Some(
            foot.child(
                div()
                    .text_size(theme.text_size(ely_gpui_component::theme::TextSize::Sm))
                    .text_color(theme.colors.fg_muted)
                    .child(title),
            )
            .children(meters)
            .children(credits)
            .children(usage.note.as_ref().map(|note| Caption::new(note.clone())))
            .into_any_element(),
        )
    }
}
