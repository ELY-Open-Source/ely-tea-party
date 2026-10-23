//! The whale girl's stroll at rest: she walks along her edge to a spot over the terminal, looks at what it shows without making head or tail of it, and walks back to where she stands.

use std::time::Instant;

use gpui::{App, Context};

use super::{Edge, Pet, WIDTH, look::Act, pane, whale};
use crate::settings::{Mascot, SettingsStore};

/// How fast she walks, in points a second, and how much faster a touch or a busy session calls her back.
const SPEED: f32 = 36.0;
const HURRY: f32 = 4.0;
/// Seconds she looks at the terminal.
const PUZZLE: f64 = 6.0;
/// How far a spot may lie from the terminal's middle, as a share of its width.
const SPREAD: f32 = 0.3;

/// Where her stroll is: walking to a spot, looking since then, or walking back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Leg {
    Out(f32),
    Look(Instant),
    Back,
}

/// Her stroll under way, and which way she faces as she walks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroll {
    pub leg: Leg,
    pub left: bool,
}

impl Stroll {
    /// The animation she plays on this leg, standing on `edge`: the terminal lies below the top edge and above the bottom one.
    pub fn drawn(&self, edge: Edge) -> &'static str {
        match (self.leg, edge) {
            (Leg::Look(_), Edge::Top) => "puzzled-down",
            (Leg::Look(_), Edge::Bottom) => "puzzled-up",
            _ if self.left => "walking-left",
            _ => "walking",
        }
    }
}

/// Where she stands to look at a terminal from `from` to `to` points in from the pane's left, on a pane `width` wide, for stroll `turn`.
fn spot(from: f32, to: f32, width: f32, turn: u64) -> f32 {
    // The golden ratio scatters each turn's spot over the terminal.
    let share = (turn as f32 * 0.618_034).fract() * 2.0 - 1.0;
    let middle = (from + to) / 2.0 + share * SPREAD * (to - from);
    (middle - WIDTH / 2.0).clamp(0.0, (width - WIDTH).max(0.0))
}

/// `at` moved `by` points toward `to`, and whether it got there.
fn toward(at: f32, to: f32, by: f32) -> (f32, bool) {
    if (to - at).abs() <= by {
        (to, true)
    } else {
        (at + by * (to - at).signum(), false)
    }
}

impl Pet {
    /// Takes her stroll a step on at `now`: one a turn of her antics, back early once something calls her, none while she is held, diving, unglued or still.
    pub(super) fn stroll(&mut self, now: Instant, cx: &mut Context<Self>) {
        let step = now
            .saturating_duration_since(self.stepped)
            .as_secs_f32()
            .min(0.25);
        self.stepped = now;
        let settings = SettingsStore::get(cx);
        let home = settings.ely_left;
        let free = settings.mascot == Mascot::Whale && !cx.reduce_motion();
        if !free || self.glue.is_none() || self.drag.is_some() || self.moving.is_some() {
            if self.stroll.take().is_some() && self.drag.is_none() {
                log::info!("pet: the whale girl's stroll stops");
                self.at = home;
                self.go(cx);
            }
            return;
        }
        let quiet = self.quiet(now, cx);
        let calm = matches!(self.act, Act::Calm | Act::Doze);
        if self.stroll.is_none()
            && calm
            && let Some(turn) = whale::stroll_turn(quiet)
            && self.strolled != Some(turn)
        {
            self.strolled = Some(turn);
            let Some(to) = self.over_terminal(turn, cx) else {
                return;
            };
            log::info!("pet: the whale girl strolls to {to} points in");
            self.stroll = Some(Stroll {
                leg: Leg::Out(to),
                left: to < self.at,
            });
        }
        let Some(mut stroll) = self.stroll else {
            return;
        };
        // A touch, a busy session or sleep.
        let called = !calm || !(whale::BORED..whale::ASLEEP).contains(&quiet);
        let speed = SPEED * if called { HURRY } else { 1.0 } * step;
        stroll.leg = match stroll.leg {
            Leg::Out(_) | Leg::Look(_) if called => {
                log::info!(
                    "pet: the whale girl hurries back from {} points in",
                    self.at
                );
                Leg::Back
            }
            Leg::Out(to) => match toward(self.at, to, speed) {
                (at, true) => {
                    self.at = at;
                    let way = if self.edge == Edge::Top { "down" } else { "up" };
                    log::info!(
                        "pet: the whale girl looks {way} at the terminal from {at} points in"
                    );
                    Leg::Look(now)
                }
                (at, false) => {
                    self.at = at;
                    Leg::Out(to)
                }
            },
            Leg::Look(since) if now.saturating_duration_since(since).as_secs_f64() >= PUZZLE => {
                log::info!("pet: the whale girl gives up on the terminal and walks back");
                Leg::Back
            }
            look @ Leg::Look(_) => look,
            Leg::Back => {
                let (at, home) = toward(self.at, home, speed);
                self.at = at;
                if home {
                    log::info!("pet: the whale girl is back from her stroll");
                    self.stroll = None;
                    return self.go(cx);
                }
                Leg::Back
            }
        };
        if stroll.leg == Leg::Back {
            stroll.left = home < self.at;
        }
        self.stroll = Some(stroll);
        self.go(cx);
    }

    /// A spot over the terminal column for stroll `turn`, from the pane's width and the side columns the settings show.
    fn over_terminal(&self, turn: u64, cx: &mut App) -> Option<f32> {
        let settings = SettingsStore::get(cx).clone();
        let width = self
            .main
            .update(cx, |_, main, _| f32::from(pane(main).size.width))
            .inspect_err(|error| log::debug!("pet: the main window does not read: {error:#}"))
            .ok()?;
        let from = if settings.show_sessions {
            settings.sessions_width
        } else {
            0.0
        };
        let to = width
            - if settings.show_panel {
                settings.panel_width
            } else {
                0.0
            };
        Some(spot(from, to.max(from), width, turn))
    }

    /// Moves her window to where she now stands.
    fn go(&mut self, cx: &mut App) {
        let main = self.main;
        if let Err(error) = main.update(cx, |_, main, cx| self.place(main, cx)) {
            log::debug!("pet: the main window does not read: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_turn_s_spot_lies_over_the_terminal_and_inside_the_pane() {
        for turn in 0..20 {
            let at = spot(288.0, 960.0, 1200.0, turn);
            assert!((288.0 - WIDTH..=960.0).contains(&at), "turn {turn} at {at}");
        }
        assert_ne!(spot(288.0, 960.0, 1200.0, 1), spot(288.0, 960.0, 1200.0, 2));
        assert_eq!(spot(0.0, 80.0, 80.0, 3), 0.0, "a pane narrower than her");
    }

    #[test]
    fn a_step_never_overshoots() {
        assert_eq!(toward(100.0, 110.0, 4.0), (104.0, false));
        assert_eq!(toward(100.0, 90.0, 4.0), (96.0, false));
        assert_eq!(toward(100.0, 102.0, 4.0), (102.0, true));
    }

    #[test]
    fn she_looks_down_from_the_top_edge_and_up_from_the_bottom() {
        let look = Stroll {
            leg: Leg::Look(Instant::now()),
            left: true,
        };
        assert_eq!(look.drawn(Edge::Top), "puzzled-down");
        assert_eq!(look.drawn(Edge::Bottom), "puzzled-up");
        let walk = Stroll {
            leg: Leg::Back,
            left: true,
        };
        assert_eq!(walk.drawn(Edge::Top), "walking-left");
        let walk = Stroll {
            left: false,
            ..walk
        };
        assert_eq!(walk.drawn(Edge::Bottom), "walking");
    }
}
