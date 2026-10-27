use super::MemoryWindow;
use crate::settings::SettingsStore;
use ely_gpui_component::{
    buttons::{Button, ButtonVariant},
    forms::{Choice, Input, Select},
    i18n::text,
    theme::{ActiveTheme, ControlSize, Radius},
};
use gpui::{Context, Div, Window, div, prelude::*, rems};

impl MemoryWindow {
    pub(super) fn content(&self, _window: &Window, cx: &Context<Self>) -> Div {
        let theme = cx.theme();
        let enabled = SettingsStore::get(cx).butler.memory;
        let selected = self
            .folder
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "global".into());
        let mut folders = self.overview.projects.clone();
        if selected != "global" && !folders.contains(&selected) {
            folders.insert(0, selected.clone());
        }
        let project = Select::new(
            "memory-project",
            std::iter::once(Choice::new("global", text(cx, "et.memory.global", &[])))
                .chain(folders.into_iter().map(|p| Choice::new(p.clone(), p))),
        )
        .selected(selected)
        .on_change(cx.listener(|this, value: &gpui::SharedString, _, cx| {
            if this.busy {
                return;
            }
            this.folder = (value.as_ref() != "global").then(|| value.to_string().into());
            this.editing = None;
            this.refresh(cx);
        }));
        let kind = Select::new(
            "memory-kind",
            [
                Choice::new("preference", text(cx, "et.memory.preference", &[])),
                Choice::new("note", text(cx, "et.memory.note", &[])),
                Choice::new("unresolved", text(cx, "et.memory.unresolved", &[])),
                Choice::new("decision", text(cx, "et.memory.decision", &[])),
            ],
        )
        .selected(self.kind.clone())
        .on_change(cx.listener(|this, value: &gpui::SharedString, _, cx| {
            if this.editing.is_none() {
                this.kind = value.to_string();
                cx.notify();
            }
        }));
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .pt_8()
            .bg(theme.colors.bg)
            .text_color(theme.colors.fg)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_lg().child(text(cx, "et.memory.title", &[])))
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.colors.fg_muted)
                            .child(text(
                                cx,
                                "et.memory.storage",
                                &[(
                                    "size",
                                    &format!("{:.1}", self.overview.bytes as f64 / 1048576.),
                                )],
                            )),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.fg_muted)
                    .child(text(cx, "et.memory.hint", &[])),
            )
            .when(!enabled, |panel| {
                panel.child(
                    Button::new("enable-memory", text(cx, "et.memory.enable", &[])).on_click(
                        cx.listener(|this, _, _, cx| {
                            if let Err(error) =
                                SettingsStore::update(cx, |s| s.butler.memory = true)
                            {
                                this.error = Some(error.to_string());
                            }
                            cx.notify();
                        }),
                    ),
                )
            })
            .when(self.suggestion.is_some(), |panel| {
                panel.child(
                    Button::new("memory-reject", text(cx, "et.memory.reject", &[]))
                        .disabled(self.busy || !enabled)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(id) = this.suggestion.clone() {
                                this.perform(move |store| store.reject(&id), cx);
                            }
                        })),
                )
            })
            .child(project)
            .children(self.quote.as_ref().map(|quote| {
                div()
                    .p_2()
                    .rounded(theme.radius(Radius::Md))
                    .bg(theme.colors.surface)
                    .text_sm()
                    .child(quote.clone())
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(kind))
                    .child(
                        Button::new(
                            "memory-save",
                            text(
                                cx,
                                if self.editing.is_some() {
                                    "et.memory.update"
                                } else {
                                    "et.memory.save"
                                },
                                &[],
                            ),
                        )
                        .disabled(
                            self.busy
                                || !enabled
                                || (self.kind != "preference" && self.folder.is_none()),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                    ),
            )
            .child(Input::new(&self.input))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .items_center()
                    .child(div().flex_1().child(Input::new(&self.search)))
                    .child(
                        Button::new("memory-search", text(cx, "et.memory.search", &[]))
                            .disabled(self.busy)
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    ),
            )
            .children(self.error.as_ref().map(|error| {
                div()
                    .text_sm()
                    .text_color(theme.colors.danger)
                    .child(error.clone())
            }))
            .child(
                div()
                    .id("memory-items")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(self.overview.items.is_empty(), |items| {
                        items.child(div().text_sm().child(text(cx, "et.memory.empty", &[])))
                    })
                    .children(self.overview.items.iter().map(|item| {
                        let id = item.id.clone();
                        let edit = item.clone();
                        let undo_id = id.clone();
                        let revision = item.revision;
                        div()
                            .p_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .rounded(theme.radius(Radius::Lg))
                            .bg(theme.colors.surface)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.fg_muted)
                                    .child(text(
                                        cx,
                                        if item.status == "explicit" {
                                            "et.memory.explicit"
                                        } else {
                                            "et.memory.reported"
                                        },
                                        &[],
                                    )),
                            )
                            .child(div().text_sm().child(item.body.clone()))
                            .children(item.sources.iter().map(|source| {
                                let at = chrono::DateTime::from_timestamp(source.at, 0)
                                    .map(|at| at.format("%Y-%m-%d %H:%M UTC").to_string())
                                    .unwrap_or_default();
                                div()
                                    .text_xs()
                                    .text_color(theme.colors.fg_muted)
                                    .child(format!(
                                        "{} · {} · {}\n{}",
                                        at,
                                        source.kind,
                                        text(
                                            cx,
                                            if source.retained {
                                                "et.memory.source"
                                            } else {
                                                "et.memory.excerpt"
                                            },
                                            &[]
                                        ),
                                        source.excerpt
                                    ))
                            }))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .when(item.can_undo, |actions| {
                                        actions.child(
                                            Button::new(
                                                gpui::SharedString::from(format!("undo-{undo_id}")),
                                                text(cx, "et.memory.undo", &[]),
                                            )
                                            .size(ControlSize::Sm)
                                            .variant(ButtonVariant::Ghost)
                                            .disabled(self.busy)
                                            .on_click(
                                                cx.listener(move |this, _, _, cx| {
                                                    let id = undo_id.clone();
                                                    this.perform(
                                                        move |store| store.undo(&id, revision),
                                                        cx,
                                                    );
                                                }),
                                            ),
                                        )
                                    })
                                    .child(
                                        Button::new(
                                            gpui::SharedString::from(format!("edit-memory-{id}")),
                                            text(cx, "et.memory.edit", &[]),
                                        )
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .disabled(self.busy || !enabled)
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                this.editing =
                                                    Some((edit.id.clone(), edit.revision));
                                                this.kind = edit.kind.clone();
                                                this.quote = None;
                                                this.input.update(cx, |input, cx| {
                                                    input.set_text(edit.body.clone(), cx)
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(
                                        Button::new(
                                            gpui::SharedString::from(format!("forget-memory-{id}")),
                                            text(cx, "et.memory.forget", &[]),
                                        )
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .disabled(self.busy)
                                        .on_click(
                                            cx.listener(move |this, _, _, cx| {
                                                let id = id.clone();
                                                this.perform(move |store| store.forget(&id), cx);
                                            }),
                                        ),
                                    ),
                            )
                    })),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.fg_muted)
                    .child(text(cx, "et.memory.delete_hint", &[])),
            )
            .child(
                Button::new(
                    "memory-clear",
                    text(
                        cx,
                        if self.clear_pending {
                            "et.memory.clear_confirm"
                        } else {
                            "et.memory.clear"
                        },
                        &[],
                    ),
                )
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .disabled(self.busy)
                .on_click(cx.listener(|this, _, _, cx| {
                    if this.clear_pending {
                        this.perform(|store| store.clear(), cx);
                    } else {
                        this.clear_pending = true;
                        cx.notify();
                    }
                })),
            )
            .child(div().h(rems(0.25)))
    }
}
