//! A session's status mark: 3 by 3 dots in its state's color, every mark stepping on one clock.

use std::{
    cmp::Ordering,
    f64::consts::TAU,
    sync::OnceLock,
    time::{Duration, Instant},
};

use gpui::{Div, Hsla, ParentElement, Styled, div};

/// The dots' beat: while a mark moves, the window draws at each step, not at every frame.
pub const STEP: Duration = Duration::from_millis(80);

/// What a mark shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    /// A ripple from the centre; with progress, the outer ring fills clockwise from the top.
    Working(Option<u8>),
    /// Lit, with two quick dips.
    Waiting,
    /// A program's error, or a failed last command.
    Error,
    Done,
    Idle,
}

/// One ripple, in seconds, and how far a dot one dot out from the centre lags it.
const RIPPLE: f64 = 1.2;
const LAG: f64 = 0.3;
/// Steps in one round of a waiting mark's dips.
const BLINK: u64 = 20;
/// A working dot at its lowest, a waiting mark in a dip, and a dot outside a settled shape.
const DIM: f32 = 0.35;
const DIP: f32 = 0.25;
const FAINT: f32 = 0.12;
/// Steps a settled shape's dot takes to light, and between one dot and the next.
const FADE: u64 = 2;
const STAGGER: u64 = 1;
/// The outer ring clockwise from the top, then the centre: the order progress fills.
const FILL: [usize; 9] = [1, 2, 5, 8, 7, 6, 3, 0, 4];

/// A settled look's dots in the order they light, and how bright they end; none for a look that keeps moving.
fn shape(look: Look) -> Option<(&'static [usize], f32)> {
    match look {
        Look::Working(_) | Look::Waiting => None,
        Look::Error => Some((&[0, 4, 8, 2, 6], 1.0)),
        Look::Done => Some((&[3, 7, 5, 2], 1.0)),
        Look::Idle => Some((&FILL[..8], 0.6)),
    }
}

fn steps(time: Duration) -> u64 {
    (time.as_millis() / STEP.as_millis()) as u64
}

/// A dot's brightness `t` seconds into its ripple.
fn crest(t: f64) -> f32 {
    let wave = 0.5 + 0.5 * (TAU * t / RIPPLE).cos();
    DIM + (1.0 - DIM) * wave as f32
}

/// Each dot's opacity, row by row, `clock` on the marks' clock and `held` after the look began.
pub fn dots(look: Look, clock: Duration, held: Duration) -> [f32; 9] {
    let t = steps(clock) as f64 * STEP.as_secs_f64();
    let mut levels = [DIM; 9];
    match look {
        Look::Working(None) => {
            for (dot, level) in levels.iter_mut().enumerate() {
                let (x, y) = ((dot % 3) as f64 - 1.0, (dot / 3) as f64 - 1.0);
                *level = crest(t - x.hypot(y) * LAG);
            }
        }
        Look::Working(Some(progress)) => {
            let filled = (usize::from(progress) * FILL.len() / 100).min(FILL.len() - 1);
            for (rank, &dot) in FILL.iter().enumerate() {
                levels[dot] = match rank.cmp(&filled) {
                    Ordering::Less => 1.0,
                    Ordering::Equal => crest(t),
                    Ordering::Greater => DIM,
                };
            }
        }
        Look::Waiting => {
            let dip = matches!(steps(clock) % BLINK, 0 | 1 | 4 | 5);
            levels = [if dip { DIP } else { 1.0 }; 9];
        }
        settled => {
            let (shape, bright) = shape(settled).expect("a settled look has a shape");
            levels = [FAINT; 9];
            let held = steps(held);
            for (rank, &dot) in shape.iter().enumerate() {
                let lit = held.saturating_sub(rank as u64 * STAGGER).min(FADE) as f32 / FADE as f32;
                levels[dot] = bright * lit + FAINT * (1.0 - lit);
            }
        }
    }
    levels
}

/// Whether a mark still changes `held` after its look began: work and waiting always, a settled shape until it is drawn.
pub fn moves(look: Look, held: Duration) -> bool {
    shape(look).is_none_or(|(shape, _)| steps(held) < (shape.len() as u64 - 1) * STAGGER + FADE)
}

/// Time on the clock every mark steps to, so marks move together.
fn clock(now: Instant) -> Duration {
    static START: OnceLock<Instant> = OnceLock::new();
    now.saturating_duration_since(*START.get_or_init(|| now))
}

/// How long until the clock's next step, a little past it.
pub fn wait(now: Instant) -> Duration {
    let into = clock(now).as_nanos() % STEP.as_nanos();
    STEP - Duration::from_nanos(into as u64) + Duration::from_millis(2)
}

/// The mark's dots in `color`, `now`, its look shown since `since`.
pub fn lattice(look: Look, color: Hsla, now: Instant, since: Instant) -> Div {
    let levels = dots(look, clock(now), now.saturating_duration_since(since));
    div()
        .grid()
        .grid_cols(3)
        .gap_0p5()
        .children(levels.map(|level| div().size_1().rounded_full().bg(color.opacity(level))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(steps: u32) -> Duration {
        STEP * steps
    }

    #[test]
    fn work_ripples_from_the_centre_to_the_edges_then_the_corners() {
        let levels = |steps| dots(Look::Working(None), at(steps), Duration::ZERO);
        let peak = |dot: usize| {
            (0..15)
                .max_by(|&a, &b| levels(a)[dot].total_cmp(&levels(b)[dot]))
                .expect("steps")
        };
        let (centre, edge, corner) = (peak(4), peak(1), peak(0));
        assert_eq!(centre, 0);
        assert!(centre < edge && edge < corner, "{centre} {edge} {corner}");
        assert_eq!(peak(3), edge, "every edge together");
        assert_eq!(peak(8), corner, "every corner together");
        let again = levels(15);
        assert!(
            levels(0)
                .iter()
                .zip(again)
                .all(|(a, b)| (a - b).abs() < 1e-4),
            "one ripple every 15 steps"
        );
    }

    #[test]
    fn progress_fills_the_ring_clockwise_from_the_top_then_the_centre() {
        let lit = |progress, steps| dots(Look::Working(Some(progress)), at(steps), Duration::ZERO);
        let half = lit(50, 9);
        assert!([1, 2, 5, 8].iter().all(|&dot| half[dot] == 1.0), "{half:?}");
        assert!([6, 3, 0, 4].iter().all(|&dot| half[dot] == DIM), "{half:?}");
        assert!(lit(50, 0)[7] > half[7], "the next dot pulses");
        let full = lit(100, 9);
        assert!(FILL[..8].iter().all(|&dot| full[dot] == 1.0), "{full:?}");
        assert!(lit(100, 0)[4] > full[4], "then the centre pulses");
        assert!(lit(0, 0)[1] > lit(0, 9)[1], "at 0% the top pulses");
    }

    #[test]
    fn waiting_stays_lit_and_dips_twice() {
        let blink = |steps| dots(Look::Waiting, at(steps), Duration::ZERO);
        let dips: Vec<u32> = (0..20).filter(|&steps| blink(steps)[0] == DIP).collect();
        assert_eq!(dips, [0, 1, 4, 5]);
        assert!((0..20).all(|steps| blink(steps).iter().all(|&level| level == blink(steps)[0])));
        assert_eq!(blink(2), [1.0; 9]);
    }

    #[test]
    fn a_settled_look_draws_its_shape_dot_by_dot_then_holds() {
        let done = |steps| dots(Look::Done, at(1000), at(steps));
        assert_eq!(done(0), [FAINT; 9], "nothing lit at first");
        assert!(
            done(1)[3] > FAINT && done(1)[2] == FAINT,
            "the check draws from its left end"
        );
        let check = [3, 7, 5, 2];
        let drawn = done(5);
        assert!((0..9).all(|dot| drawn[dot] == if check.contains(&dot) { 1.0 } else { FAINT }));
        assert_eq!(done(500), drawn, "then it holds still");
        assert!(moves(Look::Done, at(4)) && !moves(Look::Done, at(5)));
        let idle = dots(Look::Idle, Duration::ZERO, at(9));
        assert_eq!(idle[4], FAINT, "idle is a ring");
        assert!(FILL[..8].iter().all(|&dot| idle[dot] == 0.6));
        assert!(!moves(Look::Idle, at(9)) && moves(Look::Idle, at(8)));
    }

    #[test]
    fn work_and_waiting_never_stop() {
        for look in [Look::Working(None), Look::Working(Some(40)), Look::Waiting] {
            assert!(moves(look, at(1_000_000)), "{look:?}");
        }
    }
}
