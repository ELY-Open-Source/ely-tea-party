use ely_gpui_component::{
    buttons::Button,
    feedback::InlineMessage,
    i18n::text,
    primitives::Severity,
    settings::{SettingsRow, SettingsSection},
    theme::ControlSize,
};
use gpui::{App, BorrowAppContext, Global, ParentElement, Styled, div};

use super::skills::{self, State, Status};

#[derive(Default)]
pub struct Integrations {
    busy: bool,
    statuses: Vec<Status>,
    error: Option<String>,
}
impl Global for Integrations {}

pub fn init(cx: &mut App) {
    cx.default_global::<Integrations>();
    run(None, cx);
}

pub fn section(cx: &App) -> gpui::Div {
    let state = cx.global::<Integrations>();
    let mut section = SettingsSection::new(text(cx, "et.messaging.title", &[]));
    for status in &state.statuses {
        let agent = status.agent.clone();
        let key = match status.state {
            State::Missing => "et.messaging.missing",
            State::Synced => "et.messaging.synced",
            State::UpdateAvailable => "et.messaging.update",
            State::Modified => "et.messaging.modified",
            State::Failed => "et.messaging.failed",
        };
        let path = skills::home()
            .ok()
            .and_then(|home| {
                status
                    .path
                    .strip_prefix(home)
                    .ok()
                    .map(|path| path.to_path_buf())
            })
            .map_or_else(
                || status.path.display().to_string(),
                |path| format!("~{}{}", std::path::MAIN_SEPARATOR, path.display()),
            );
        let mut description = format!("{}\n{path}", text(cx, key, &[]));
        if status.state == State::Failed {
            description.push_str(&format!("\n{}", status.detail));
        }
        let button = Button::new(
            format!("messaging-install-{agent}"),
            text(cx, "et.messaging.install", &[]),
        )
        .size(ControlSize::Sm)
        .disabled(state.busy || status.state == State::Modified)
        .on_click(move |_, _, cx| run(Some((agent.clone(), true)), cx));
        let agent = status.agent.clone();
        let remove = Button::new(
            format!("messaging-remove-{agent}"),
            text(cx, "et.messaging.remove", &[]),
        )
        .size(ControlSize::Sm)
        .disabled(state.busy || matches!(status.state, State::Missing | State::Modified))
        .on_click(move |_, _, cx| run(Some((agent.clone(), false)), cx));
        section = section.row(
            SettingsRow::new(if status.agent == "claude" {
                "Claude Code"
            } else {
                "Codex"
            })
            .description(description)
            .control(div().flex().gap_2().child(button).child(remove)),
        );
    }
    let refresh = Button::new("messaging-sync", text(cx, "et.messaging.refresh", &[]))
        .size(ControlSize::Sm)
        .loading(state.busy)
        .disabled(state.busy)
        .on_click(|_, _, cx| run(None, cx));
    let mut page = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(InlineMessage::new(
            Severity::Info,
            text(cx, "et.messaging.description", &[]),
        ))
        .child(section)
        .child(refresh);
    if let Some(error) = &state.error {
        page = page.child(InlineMessage::new(Severity::Danger, error.clone()));
    }
    page
}

fn run(action: Option<(String, bool)>, cx: &mut App) {
    if cx.global::<Integrations>().busy {
        return;
    }
    cx.update_global::<Integrations, _>(|state, _| {
        state.busy = true;
        state.error = None;
    });
    let work = cx.background_executor().spawn(async move {
        let home = match skills::home() {
            Ok(home) => home,
            Err(error) => return (Vec::new(), Some(format!("{error:#}"))),
        };
        let error = action.and_then(|(agent, install)| {
            let result = skills::targets(&home)
                .into_iter()
                .find(|(name, _)| name == &agent)
                .ok_or_else(|| anyhow::anyhow!("skills: unknown agent {agent}"))
                .and_then(|(_, path)| {
                    if install {
                        skills::install(&path)
                    } else {
                        skills::uninstall(&path)
                    }
                });
            result.err().map(|error| format!("{error:#}"))
        });
        (skills::refresh(&home), error)
    });
    cx.spawn(async move |cx| {
        let (statuses, error) = work.await;
        if let Some(error) = &error {
            log::error!("skills: {error}");
        }
        cx.update(|cx| {
            cx.update_global::<Integrations, _>(|state, _| {
                state.busy = false;
                state.statuses = statuses;
                state.error = error;
            })
        });
    })
    .detach();
}
