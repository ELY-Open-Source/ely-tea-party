//! The title bar's update button, shown while the updater has something to say.

use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    primitives::IconName,
    theme::ControlSize,
};
use gpui::App;

use crate::update::{self, State};

/// The button for the updater's state, if it shows one.
pub(super) fn update_button(cx: &App) -> Option<Button> {
    let (label, variant, icon) = match update::state(cx) {
        State::Idle if update::asked(cx) => {
            let button = Button::new("update", "检查更新…")
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .loading(true);
            return Some(button);
        }
        State::Idle => return None,
        State::Current => (
            format!("已是最新 {}", update::VERSION),
            ButtonVariant::Ghost,
            IconName::Check,
        ),
        State::Unchecked(_) => (
            "检查更新失败".to_string(),
            ButtonVariant::Ghost,
            IconName::CircleAlert,
        ),
        State::Available(release) => (
            format!("更新到 {}", release.version),
            ButtonVariant::Primary,
            IconName::Download,
        ),
        State::Installing(release) => {
            let button = Button::new("update", format!("正在更新到 {}", release.version))
                .variant(ButtonVariant::Primary)
                .size(ControlSize::Sm)
                .loading(true);
            return Some(button);
        }
        State::Ready(_) => (
            "重启以完成更新".to_string(),
            ButtonVariant::Success,
            IconName::RefreshCw,
        ),
        State::Failed(..) => (
            "更新失败，去下载页".to_string(),
            ButtonVariant::Danger,
            IconName::ExternalLink,
        ),
    };
    Some(
        Button::new("update", label)
            .variant(variant)
            .size(ControlSize::Sm)
            .icon(icon)
            .on_click(|_, _, cx| update::press(cx)),
    )
}
