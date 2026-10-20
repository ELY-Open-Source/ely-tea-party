//! The whale girl in Ely's place: the sessions column's 鲸鱼娘. Ely's act picks her animation while the sessions are busy; at rest she stands still, and the longer nobody touches the app, the more she gets up to.

use std::time::{Duration, Instant};

use gpui::{
    App, Context, FontWeight, Global, IntoElement, ObjectFit, ParentElement, Styled, StyledImage,
    Task, div, img, prelude::FluentBuilder, px,
};

use super::{
    HEAD, Pet, ROOM,
    look::{Act, Pose},
    rig::UNIT,
};
use crate::{
    agents::Kind,
    settings::{Mascot, SettingsStore},
};

/// Her art's side, in points.
const SIDE: f32 = 112.0;
/// How far her feet stand below her window's bottom, so she peeks over the main window's edge.
const SUNK: f32 = 30.0;
/// Seconds of quiet before she gets up to something, and before she falls asleep.
const BORED: f64 = 60.0;
const ASLEEP: f64 = 300.0;
/// Seconds each turn of her antics lasts: a snack, a rest, slacking off, a rest.
const TURN: f64 = 20.0;
/// Seconds a sleeper's z takes to rise and fade, and how many rise at once.
const RISE: f64 = 2.4;
const ZS: usize = 3;

/// How often she moves to her art's next frame: 12 a second.
const FRAME: Duration = Duration::from_millis(83);

/// Draws her at each of her art's frames, whether or not the app is active, unless the system asks for less motion: gpui gives an inactive app's windows no animation frames, which froze her.
pub fn frames(cx: &mut Context<Pet>) -> Task<()> {
    cx.spawn(async move |pet, cx| {
        loop {
            cx.background_executor().timer(FRAME).await;
            let drawn = pet.update(cx, |_, cx| {
                if SettingsStore::get(cx).mascot == Mascot::Whale && !cx.reduce_motion() {
                    cx.notify();
                }
            });
            if drawn.is_err() {
                break;
            }
        }
    })
}

/// When the user last typed into a terminal or pressed the mouse in the main window.
pub struct Touched(Instant);

impl Global for Touched {}

/// Notes a key or a press in the main window, which wakes her and starts her quiet over.
pub fn touch(cx: &mut App) {
    cx.set_global(Touched(Instant::now()));
}

/// When the user last touched the main window, if ever.
pub fn touched(cx: &App) -> Option<Instant> {
    cx.try_global::<Touched>().map(|touched| touched.0)
}

/// The loop that plays `act` after `quiet` seconds with nothing new: busy sessions pick it; at rest she stands, then snacks and slacks by turns, then sleeps.
fn drawn(act: Act, quiet: f64) -> &'static str {
    match act {
        Act::Cheer | Act::Wink => "working-special",
        Act::Work => "working",
        Act::Wait => "needs_input",
        Act::Fret => "thinking",
        Act::Calm | Act::Doze if quiet >= ASLEEP => "sleeping",
        Act::Calm | Act::Doze if quiet < BORED => "calm",
        Act::Calm | Act::Doze => match ((quiet - BORED) / TURN) as u64 % 4 {
            0 => "idle",
            2 => "working-slack",
            _ => "calm",
        },
    }
}

/// How far each z has risen `quiet` seconds into her sleep, 0 to 1, each a third behind the last.
fn zs(quiet: f64) -> [f32; ZS] {
    std::array::from_fn(|k| ((quiet / RISE + k as f64 / ZS as f64) % 1.0) as f32)
}

/// Her, in `pose` at `now`, after `quiet` seconds with nothing new, filling her window.
pub fn whale(pose: &Pose, quiet: f64, now: Instant) -> impl IntoElement + use<> {
    let name = drawn(pose.act, quiet);
    let frame = crate::agents::assets::mascot(name, now);
    // Standing still, she breathes as Ely does.
    let breath = if name == "calm" { pose.bob } else { 0.0 };
    let top = ROOM + HEAD - SIDE + SUNK - (pose.lift + breath) * UNIT;
    // Her color, light enough to read over any window behind her.
    let color = gpui::Hsla {
        l: 0.78,
        ..Kind::Deepseek.color()
    };
    let z = |rise: f32| {
        div()
            .absolute()
            .left(px(SIDE * 0.62 + rise * 14.0))
            .top(px(top + 18.0 - rise * 26.0))
            .text_size(px(9.0 + rise * 7.0))
            .font_weight(FontWeight::BOLD)
            .text_color(color.opacity(1.0 - rise))
            .child("z")
    };
    div()
        .size_full()
        .relative()
        .overflow_hidden()
        .children(frame.map(|frame| {
            img(frame)
                .absolute()
                .left(px(pose.shake * UNIT))
                .top(px(top))
                .size(px(SIDE))
                .object_fit(ObjectFit::Contain)
        }))
        .when(name == "sleeping", |her| her.children(zs(quiet).map(z)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_sessions_pick_her_loop_whatever_the_quiet() {
        assert_eq!(drawn(Act::Work, 1000.0), "working");
        assert_eq!(drawn(Act::Wait, 0.0), "needs_input");
        assert_eq!(drawn(Act::Fret, 0.0), "thinking");
        assert_eq!(drawn(Act::Wink, 0.0), "working-special");
    }

    #[test]
    fn at_rest_she_stands_then_snacks_and_slacks_by_turns_then_sleeps() {
        assert_eq!(drawn(Act::Calm, 5.0), "calm");
        assert_eq!(drawn(Act::Calm, BORED + 5.0), "idle");
        assert_eq!(drawn(Act::Calm, BORED + TURN + 5.0), "calm");
        assert_eq!(drawn(Act::Calm, BORED + 2.0 * TURN + 5.0), "working-slack");
        assert_eq!(drawn(Act::Calm, BORED + 3.0 * TURN + 5.0), "calm");
        assert_eq!(drawn(Act::Calm, BORED + 4.0 * TURN + 5.0), "idle");
        assert_eq!(drawn(Act::Calm, ASLEEP + 1.0), "sleeping");
        assert_eq!(
            drawn(Act::Doze, 30.0),
            "calm",
            "a touch wakes her though Ely would doze"
        );
    }

    #[test]
    fn a_sleeper_s_zs_rise_a_third_apart() {
        let [a, b, c] = zs(0.0);
        assert_eq!((a, b), (0.0, 1.0 / 3.0));
        assert!((c - 2.0 / 3.0).abs() < 1e-6);
        assert!(zs(RISE * 0.5).iter().all(|rise| (0.0..1.0).contains(rise)));
    }
}
