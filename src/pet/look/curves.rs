//! The timings and eases Ely's moves take, after the films' tweens.

use std::f32::consts::PI;

use super::Motion;

/// How far her eyes close in a blink.
pub(super) const SHUT: f32 = 0.08;
/// A hop's height: 12 points.
const HOP: f32 = 31.0;
/// Seconds between blinks, a round she repeats; the short gap blinks twice.
const BLINKS: [f32; 6] = [3.4, 4.6, 0.32, 5.2, 2.8, 4.1];

/// How open an eye is `t` seconds into the blink round: shut in 70 ms, open again in 120 ms.
pub(super) fn blink(t: f64) -> f32 {
    let round: f32 = BLINKS.iter().sum();
    let at = fold(t, round);
    // A round ends in a blink; the first has none before it.
    let mut last = if t < round as f64 {
        f32::NEG_INFINITY
    } else {
        0.0
    };
    let mut time = 0.0;
    for gap in BLINKS {
        time += gap;
        if time <= at {
            last = time;
        }
    }
    match at - last {
        s if s < 0.07 => 1.0 - (1.0 - SHUT) * (s / 0.07).powi(2),
        s if s < 0.1 => SHUT,
        s if s < 0.22 => SHUT + (1.0 - SHUT) * (1.0 - (1.0 - (s - 0.1) / 0.12).powi(2)),
        _ => 1.0,
    }
}

/// A shape that opens from a line in 150 ms, `t` seconds after it came.
pub(super) fn opening(t: f32) -> f32 {
    SHUT + (1.0 - SHUT) * smooth((t / 0.15).clamp(0.0, 1.0))
}

/// A mouth or mark that pops in over 180 ms, `t` seconds after it came.
pub(super) fn popping(t: f32) -> f32 {
    0.6 + 0.4 * overshoot((t / 0.18).clamp(0.0, 1.0))
}

/// `count` hops in 0.6 s, `since` they began.
pub(super) fn hops(since: f32, count: f32) -> f32 {
    if !(0.0..0.6).contains(&since) {
        return 0.0;
    }
    (since / 0.6 * count * PI).sin().abs() * HOP
}

/// A shake of `amp` units that dies away in 0.6 s, `since` it began.
pub(super) fn shaking(since: f32, amp: f32) -> f32 {
    if !(0.0..0.6).contains(&since) {
        return 0.0;
    }
    let u = since / 0.6;
    (u * 6.0 * PI).sin() * amp * (1.0 - u)
}

/// The ahoge's wiggle, `since` it began: -16°, 11°, -6°, then still, in 0.7 s.
pub(super) fn wiggle(since: f32) -> f32 {
    const KEYS: [(f32, f32); 5] = [
        (0.0, 0.0),
        (0.12, -16.0),
        (0.28, 11.0),
        (0.46, -6.0),
        (0.7, 0.0),
    ];
    let Some(i) = (KEYS.windows(2)).position(|pair| (pair[0].0..pair[1].0).contains(&since)) else {
        return 0.0;
    };
    let ((t0, v0), (t1, v1)) = (KEYS[i], KEYS[i + 1]);
    v0 + (v1 - v0) * smooth((since - t0) / (t1 - t0))
}

/// A finger's tap, `since` it began: down 4 units in 60 ms, up in 160 ms.
pub(super) fn tap(since: f32) -> f32 {
    if since < 0.06 {
        4.0 * (since / 0.06).powi(2)
    } else {
        4.0 * (1.0 - smooth(((since - 0.06) / 0.16).min(1.0)))
    }
}

/// A z that floats up from its place and fades every 2.4 s, `phase` of a round late.
pub(super) fn floating(t: f64, phase: f32) -> Motion {
    let f = (fold(t, 2.4) / 2.4 + phase) % 1.0;
    Motion {
        at: (f * 6.0, -f * 22.0),
        scale: 0.8 + 0.4 * f,
        turn: 0.0,
        alpha: (f * PI).sin() * (early(t) / 0.8).min(1.0),
    }
}

/// A sparkle that grows in and twinkles, `phase` of a 0.9 s round late.
pub(super) fn twinkling(t: f32, phase: f32) -> Motion {
    let pulse = (2.0 * PI * (t / 0.9 + phase)).sin();
    Motion {
        at: (0.0, 0.0),
        scale: overshoot((t / 0.25).min(1.0)) * (0.85 + 0.25 * pulse),
        turn: pulse * 12.0,
        alpha: 1.0,
    }
}

/// `t` folded into a round of `period` seconds, which keeps its steps fine after weeks.
pub(super) fn fold(t: f64, period: f32) -> f32 {
    (t % period as f64) as f32
}

/// `t` while it is short; past a minute, what follows a start has long ended.
pub(super) fn early(t: f64) -> f32 {
    t.min(60.0) as f32
}

/// 1 inside `from..to`, 0 outside, eased in and out over 0.35 s.
pub(super) fn between(at: f32, from: f32, to: f32) -> f32 {
    smooth(
        ((at - from) / 0.35)
            .clamp(0.0, 1.0)
            .min(((to - at) / 0.35).clamp(0.0, 1.0)),
    )
}

pub(super) fn smooth(x: f32) -> f32 {
    x * x * (3.0 - 2.0 * x)
}

/// An ease that runs past its end and settles back, as GSAP's back.out.
pub(super) fn overshoot(x: f32) -> f32 {
    const C: f32 = 1.70158;
    1.0 + (C + 1.0) * (x - 1.0).powi(3) + C * (x - 1.0).powi(2)
}
