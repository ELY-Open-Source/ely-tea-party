use ely_gpui_component::{
    buttons::Button,
    feedback::InlineMessage,
    i18n::text,
    primitives::Severity,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize},
};
use gpui::{App, BorrowAppContext, Global, ParentElement, Styled, div};

use crate::agents::install::desktop::{self, Report};

#[derive(Default)]
pub(super) struct Installation {
    running: bool,
    report: Option<Report>,
}

impl Global for Installation {}

pub(super) fn section(cx: &App) -> gpui::Div {
    let state = cx.global::<Installation>();
    let label = if state.running {
        "et.hooks.installing"
    } else {
        "et.hooks.install"
    };
    let button = Button::new("install-agent-hooks", text(cx, label, &[]))
        .size(ControlSize::Sm)
        .loading(state.running)
        .disabled(state.running)
        .on_click(|_, _, cx| install(cx));
    let mut section = div().flex().flex_col().gap_3().child(
        SettingsSection::new(text(cx, "et.hooks.title", &[])).row(
            SettingsRow::new(text(cx, "et.hooks.status", &[]))
                .description(text(cx, "et.hooks.description", &[]))
                .control(button),
        ),
    );
    if let Some(report) = &state.report {
        let (severity, key) = if report.success {
            (Severity::Success, "et.hooks.success")
        } else {
            (Severity::Danger, "et.hooks.failed")
        };
        section = section
            .child(InlineMessage::new(severity, text(cx, key, &[])))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().colors.fg_muted)
                    .child(report.output.clone()),
            );
    }
    section
}

fn install(cx: &mut App) {
    if cx.global::<Installation>().running {
        return;
    }
    log::info!("hooks: installation requested in settings");
    cx.update_global::<Installation, _>(|state, _| {
        state.running = true;
        state.report = None;
    });
    let work = cx.background_executor().spawn(async { desktop::run() });
    cx.spawn(async move |cx| {
        let report = work.await.unwrap_or_else(|error| Report {
            success: false,
            output: format!("{error:#}"),
        });
        if report.success {
            log::info!("hooks: installation completed");
        } else {
            log::error!("hooks: installation failed: {}", report.output);
        }
        cx.update(|cx| {
            cx.update_global::<Installation, _>(|state, _| {
                state.running = false;
                state.report = Some(report);
            });
        });
    })
    .detach();
}
