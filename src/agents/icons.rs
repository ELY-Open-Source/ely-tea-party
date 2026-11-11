use std::borrow::Cow;

use ely_gpui_component::{
    primitives::{Icon, IconName},
    theme::{ActiveTheme, IconSize, TextSize},
};
use gpui::{AnyElement, App, AssetSource, IntoElement, ParentElement, SharedString, Styled, div};

use super::Kind;

pub struct Icons;

const FILES: &[(&str, &[u8])] = &[
    (
        "agent-icons/claude.svg",
        include_bytes!("../../assets/agent-icons/claude.svg"),
    ),
    (
        "agent-icons/gpt.svg",
        include_bytes!("../../assets/agent-icons/gpt.svg"),
    ),
    (
        "agent-icons/deepseek.svg",
        include_bytes!("../../assets/agent-icons/deepseek.svg"),
    ),
    (
        "agent-icons/gemini.svg",
        include_bytes!("../../assets/agent-icons/gemini.svg"),
    ),
    (
        "agent-icons/kimi.svg",
        include_bytes!("../../assets/agent-icons/kimi.svg"),
    ),
];

impl AssetSource for Icons {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| (*name).into())
            .collect())
    }
}

pub fn name(kind: Kind) -> &'static str {
    match kind {
        Kind::Claude => "Claude Code",
        Kind::Gpt => "Codex",
        Kind::Deepseek => "DSCODE",
        Kind::Grok => "Grok",
        Kind::Kimi => "Kimi",
        Kind::Gemini => "Gemini CLI",
        Kind::Pi => "Pi",
        Kind::OpenCode => "OpenCode",
    }
}

pub fn icon(kind: Option<Kind>, cx: &App) -> AnyElement {
    let mark = match kind {
        Some(Kind::Grok) => Icon::new(IconName::Zap),
        Some(Kind::OpenCode) => Icon::new(IconName::Code),
        Some(Kind::Pi) => {
            return div()
                .size(cx.theme().icon_size(IconSize::Sm))
                .flex()
                .items_center()
                .justify_center()
                .text_size(cx.theme().text_size(TextSize::Base))
                .text_color(cx.theme().colors.fg)
                .child("π")
                .into_any_element();
        }
        Some(kind) => Icon::from_path(format!("agent-icons/{}.svg", kind.stem())),
        None => Icon::new(IconName::Terminal),
    };
    mark.size(IconSize::Sm)
        .color(cx.theme().colors.fg)
        .into_any_element()
}
