use std::time::{Duration, Instant};

use ely_gpui_component::{
    buttons::Button,
    feedback::InlineMessage,
    forms::{Choice, NumberInput, Select},
    i18n::text,
    primitives::Severity,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize, Radius},
};
use gpui::{
    App, Context, Div, IntoElement, ObjectFit, ParentElement, Styled, StyledImage, Task, div, img,
    px, rems,
};

use super::SettingsWindow;
use crate::skins::{self, Layout, Library, Slot};

mod jobs;
use jobs::Action;

pub(super) struct Fields {
    slot: Slot,
    pose: String,
}

impl Default for Fields {
    fn default() -> Self {
        Self {
            slot: Slot::Claude,
            pose: "idle".into(),
        }
    }
}

pub(super) fn animate(cx: &mut Context<SettingsWindow>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(83))
                .await;
            if this
                .update(cx, |this, cx| {
                    if this.section == "characters" && !cx.reduce_motion() {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        }
    })
}

impl SettingsWindow {
    pub(super) fn characters_page(&self, cx: &Context<Self>) -> Div {
        let slot = self.characters.slot;
        let pose = &self.characters.pose;
        let library = cx.global::<Library>();
        let skin = skins::custom(slot, cx);
        let busy = library.busy;
        let word = |key| text(cx, key, &[]);
        let choices = Slot::ALL.map(|slot| {
            Choice::new(
                slot.id(),
                slot.kind()
                    .map_or_else(|| word("et.mascot.whale"), |kind| kind.name().into()),
            )
        });
        let selected = Select::new("character-slot", choices)
            .selected(slot.id())
            .disabled(busy)
            .on_change({
                let this = cx.entity();
                move |id, _, cx| {
                    this.update(cx, |this, cx| {
                        this.characters.slot = Slot::ALL
                            .into_iter()
                            .find(|slot| slot.id() == id.as_ref())
                            .expect("listed slot");
                        this.characters.pose = "idle".into();
                        cx.notify();
                    })
                }
            });
        let poses = Select::new(
            "character-pose",
            slot.poses()
                .iter()
                .map(|pose| Choice::new(*pose, text(cx, pose_key(pose), &[]))),
        )
        .selected(pose.clone())
        .disabled(busy)
        .on_change({
            let this = cx.entity();
            move |pose, _, cx| {
                this.update(cx, |this, cx| {
                    this.characters.pose = pose.to_string();
                    cx.notify();
                })
            }
        });
        let button = |id, label, action: Action| {
            Button::new(id, word(label))
                .size(ControlSize::Sm)
                .disabled(busy)
                .on_click(move |_, _, cx| jobs::start(slot, action.clone(), cx))
        };
        let actions = div()
            .flex()
            .flex_wrap()
            .gap_2()
            .child(button(
                "character-image",
                "et.characters.image",
                Action::Image,
            ))
            .child(button(
                "character-import",
                "et.characters.import",
                Action::Import,
            ))
            .child(button(
                "character-export",
                "et.characters.export",
                Action::Export,
            ))
            .child(button(
                "character-reset",
                "et.characters.reset",
                Action::Reset,
            ));
        let pose_image = button(
            "character-pose-image",
            "et.characters.pose_image",
            Action::Pose(pose.clone()),
        )
        .disabled(busy || skin.is_none());
        let status = if busy {
            "et.characters.busy"
        } else if skin.is_some() {
            "et.characters.custom"
        } else {
            "et.characters.builtin"
        };
        let mut page = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().colors.fg_muted)
                    .child(word("et.characters.hint")),
            )
            .child(
                SettingsSection::new(word("et.characters.title")).row(
                    SettingsRow::new(word("et.characters.choose"))
                        .description(word(status))
                        .control(div().w(rems(18.)).child(selected)),
                ),
            )
            .child(actions)
            .children(
                library
                    .error
                    .as_ref()
                    .map(|error| InlineMessage::new(Severity::Danger, error.clone())),
            )
            .children(
                library
                    .notice
                    .map(|key| InlineMessage::new(Severity::Success, word(key))),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .items_center()
                    .child(preview(slot, pose, cx))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .flex_1()
                            .child(poses)
                            .child(pose_image)
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().colors.fg_muted)
                                    .child(word(if skin.is_none() {
                                        "et.characters.pose_hint"
                                    } else if skin
                                        .is_some_and(|skin| !skin.bytes.contains_key(pose))
                                    {
                                        "et.characters.fallback"
                                    } else {
                                        "et.characters.action_hint"
                                    })),
                            ),
                    ),
            );
        if slot == Slot::Whale {
            page = page.child(layout_controls(
                slot,
                skin.map_or(Layout::default(), |skin| skin.layout),
                busy || skin.is_none(),
                cx,
            ));
        }
        page.child(
            div()
                .text_sm()
                .text_color(cx.theme().colors.fg_muted)
                .child(word("et.characters.limits")),
        )
    }
}

fn preview(slot: Slot, pose: &str, cx: &App) -> Div {
    let skin = skins::custom(slot, cx);
    let layout = if slot == Slot::Whale {
        skin.map_or(Layout::default(), |skin| skin.layout)
    } else {
        Layout::default()
    };
    let side = 112.0 * layout.scale;
    let frame = skins::preview(slot, pose, Instant::now(), cx);
    let y = if slot == Slot::Whale {
        112.0 - side + layout.y
    } else {
        0.0
    };
    div()
        .p_4()
        .rounded(cx.theme().radius(Radius::Lg))
        .bg(cx.theme().colors.surface)
        .child(
            div()
                .relative()
                .size(px(112.))
                .overflow_hidden()
                .children(frame.map(|frame| {
                    img(frame)
                        .absolute()
                        .left(px((112.0 - side) / 2.0 + layout.x))
                        .top(px(y))
                        .size(px(side))
                        .object_fit(ObjectFit::Contain)
                })),
        )
}

fn layout_controls(slot: Slot, layout: Layout, disabled: bool, cx: &App) -> SettingsSection {
    let mut section = SettingsSection::new(text(cx, "et.characters.placement", &[]));
    for (field, label, value, min, max) in [
        (
            "scale",
            "et.characters.scale",
            layout.scale * 100.0,
            50.0,
            100.0,
        ),
        ("x", "et.characters.x", layout.x, -30.0, 30.0),
        ("y", "et.characters.y", layout.y, -30.0, 30.0),
    ] {
        let control = NumberInput::new(field, f64::from(value))
            .range(min, max)
            .step(1.0)
            .size(ControlSize::Sm)
            .on_commit(move |value, _, cx| {
                if disabled {
                    return;
                }
                let Some(skin) = skins::custom(slot, cx) else {
                    return;
                };
                let mut layout = skin.layout;
                match field {
                    "scale" => layout.scale = value as f32 / 100.0,
                    "x" => layout.x = value as f32,
                    _ => layout.y = value as f32,
                }
                if layout == skin.layout {
                    return;
                }
                jobs::start(slot, Action::Layout(layout), cx);
            });
        let control = if disabled {
            div().child(format!("{value:.0}")).into_any_element()
        } else {
            control.into_any_element()
        };
        section = section.row(SettingsRow::new(text(cx, label, &[])).control(control));
    }
    section
}

fn pose_key(pose: &str) -> &'static str {
    match pose {
        "idle" => "et.characters.idle",
        "calm" => "et.characters.calm",
        "working" => "et.characters.working",
        "working-research" => "et.characters.research",
        "working-special" => "et.characters.special",
        "working-slack" => "et.characters.slack",
        "thinking" => "et.characters.thinking",
        "needs_input" => "et.characters.needs_input",
        "sleeping" => "et.characters.sleeping",
        "walking" => "et.characters.walking",
        "walking-left" => "et.characters.walking_left",
        "puzzled-up" => "et.characters.look_up",
        "puzzled-down" => "et.characters.look_down",
        _ => unreachable!("listed pose"),
    }
}
