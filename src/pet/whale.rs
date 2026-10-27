//! The whale girl in Ely's place: the sessions column's 鲸鱼娘. Ely's act picks her animation while the sessions are busy; at rest she stands still, and the longer nobody touches the app, the more she gets up to, a stroll among it.

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
pub(super) const BORED: f64 = 60.0;
pub(super) const ASLEEP: f64 = 300.0;
/// Seconds each turn of her antics lasts: a snack, a rest, a stroll, a rest, slacking off, a rest.
const TURN: f64 = 20.0;
/// Seconds a sleeper's z takes to rise and fade, and how many rise at once.
const RISE: f64 = 2.4;
const ZS: usize = 3;

/// How often she moves to her art's next frame: 12 a second.
const FRAME: Duration = Duration::from_millis(83);

/// Takes her stroll on and draws her at each of her art's frames, whether or not the app is active, unless the system asks for less motion: gpui gives an inactive app's windows no animation frames, which froze her.
pub fn frames(cx: &mut Context<Pet>) -> Task<()> {
    cx.spawn(async move |pet, cx| {
        loop {
            cx.background_executor().timer(FRAME).await;
            let drawn = pet.update(cx, |pet, cx| {
                pet.stroll(Instant::now(), cx);
                pet.sync_butler(cx);
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

/// Which turn of her antics `quiet` seconds of rest fall in, from 0.
fn turn(quiet: f64) -> u64 {
    ((quiet - BORED) / TURN) as u64
}

/// The turn her stroll begins in after `quiet` seconds of rest, while it lasts.
pub fn stroll_turn(quiet: f64) -> Option<u64> {
    (BORED..ASLEEP)
        .contains(&quiet)
        .then(|| turn(quiet))
        .filter(|turn| turn % 6 == 2)
}

/// The loop that plays `act` after `quiet` seconds with nothing new: busy sessions pick it; at rest she stands, then snacks, strolls and slacks by turns, then sleeps.
fn drawn(act: Act, quiet: f64) -> &'static str {
    match act {
        Act::Cheer | Act::Wink => "working-special",
        Act::Work => "working",
        Act::Wait => "needs_input",
        Act::Fret => "thinking",
        Act::Calm | Act::Doze if quiet >= ASLEEP => "sleeping",
        Act::Calm | Act::Doze if quiet < BORED => "calm",
        Act::Calm | Act::Doze => match turn(quiet) % 6 {
            0 => "idle",
            4 => "working-slack",
            _ => "calm",
        },
    }
}

/// How far each z has risen `quiet` seconds into her sleep, 0 to 1, each a third behind the last.
fn zs(quiet: f64) -> [f32; ZS] {
    std::array::from_fn(|k| ((quiet / RISE + k as f64 / ZS as f64) % 1.0) as f32)
}

/// Her, in `pose` at `now`, after `quiet` seconds with nothing new, or on her stroll's `walk`, filling her window.
pub fn whale(
    pose: &Pose,
    quiet: f64,
    walk: Option<&'static str>,
    now: Instant,
    cx: &App,
) -> impl IntoElement + use<> {
    let name = walk.unwrap_or_else(|| drawn(pose.act, quiet));
    let frame = crate::agents::assets::mascot(name, now, cx);
    // Standing still, she breathes as Ely does.
    let breath = if name == "calm" { pose.bob } else { 0.0 };
    let top = ROOM + HEAD - SIDE + SUNK - (pose.lift + breath) * UNIT;
    let layout = crate::skins::custom(crate::skins::Slot::Whale, cx).map(|skin| skin.layout);
    let side = layout.map_or(SIDE, |layout| SIDE * layout.scale);
    let top = layout.map_or(top, |layout| top - SUNK + SIDE - side + layout.y);
    let left = layout.map_or(0.0, |layout| (SIDE - side) / 2.0 + layout.x);
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
                .left(px(left + pose.shake * UNIT))
                .top(px(top))
                .size(px(side))
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
    fn at_rest_she_stands_then_snacks_strolls_and_slacks_by_turns_then_sleeps() {
        let at = |turn: f64| BORED + turn * TURN + 5.0;
        assert_eq!(drawn(Act::Calm, 5.0), "calm");
        assert_eq!(drawn(Act::Calm, at(0.0)), "idle");
        assert_eq!(drawn(Act::Calm, at(1.0)), "calm");
        assert_eq!(stroll_turn(at(2.0)), Some(2), "her stroll's turn");
        assert_eq!(drawn(Act::Calm, at(3.0)), "calm");
        assert_eq!(drawn(Act::Calm, at(4.0)), "working-slack");
        assert_eq!(drawn(Act::Calm, at(6.0)), "idle");
        assert_eq!(stroll_turn(at(8.0)), Some(8));
        assert_eq!(stroll_turn(at(3.0)), None);
        assert_eq!(stroll_turn(5.0), None, "too soon");
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
