//! Agent-colored light trails confined to the active terminal's edges.

use std::{f32::consts::TAU, sync::OnceLock, time::Instant};

use ely_gpui_component::theme::ActiveTheme;
use gpui::{App, Bounds, Hsla, IntoElement, Pixels, Styled, Window, canvas, fill, point, px, size};

use crate::agents::{Kind, speed::Speed};

struct Aura {
    main: Hsla,
    echo: Hsla,
    core: Hsla,
    ultra: bool,
    reduced: bool,
    unit: f32,
}

pub(super) fn border(kind: Kind, speed: Speed, cx: &App) -> impl IntoElement {
    let dark = cx.theme().colors.bg.l < 0.5;
    let mut main = kind.color();
    main.l = if dark {
        main.l.max(0.57)
    } else {
        main.l.min(0.43)
    };
    let mut echo = main;
    echo.h = (main.h + 0.085).fract();
    let mut core = main;
    core.l = if dark { 0.9 } else { 0.24 };
    core.s *= 0.55;
    let reduced = cx.reduce_motion();
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            static EPOCH: OnceLock<Instant> = OnceLock::new();
            let elapsed = EPOCH.get_or_init(Instant::now).elapsed().as_secs_f32();
            let aura = Aura {
                main,
                echo,
                core,
                reduced,
                ultra: kind == Kind::Gpt && speed == Speed::Ultrafast,
                unit: f32::from(window.rem_size()) / 16.,
            };
            paint(bounds, elapsed, &aura, window);
            if !reduced {
                window.request_animation_frame();
            }
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

fn paint(bounds: Bounds<Pixels>, elapsed: f32, aura: &Aura, window: &mut Window) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    if w < 16. * aura.unit || h < 16. * aura.unit {
        return;
    }
    let perimeter = 2. * (w + h);
    let heads = if aura.ultra { 4 } else { 2 };
    let period = if aura.ultra { 1.55 } else { 4.8 };
    let phase = (elapsed / period).fract();
    for depth in (0..4).rev() {
        let thickness = (1. + depth as f32 * 1.6) * aura.unit;
        let alpha = match (depth, aura.ultra) {
            (0, true) => 0.28,
            (0, false) => 0.16,
            (_, true) => 0.035,
            (_, false) => 0.02,
        };
        edge(
            bounds,
            0.,
            perimeter,
            thickness,
            aura.main.opacity(alpha),
            window,
        );
    }
    if aura.reduced {
        return;
    }
    for head in 0..heads {
        let front = perimeter * (phase + head as f32 / heads as f32);
        let color = if aura.ultra && head % 2 == 1 {
            aura.echo
        } else {
            aura.main
        };
        let tail = (perimeter * if aura.ultra { 0.16 } else { 0.14 })
            .min(if aura.ultra { 440. } else { 300. } * aura.unit);
        for step in 0..32 {
            let strength = (1. - step as f32 / 32.).powf(1.4);
            let length = tail / 32.;
            let at = front - step as f32 * length;
            for (width, alpha) in [(7., 0.07), (3.5, 0.24), (1.5, 0.95)] {
                edge(
                    bounds,
                    at,
                    length + 0.4,
                    width * aura.unit,
                    color.opacity(strength * alpha),
                    window,
                );
            }
        }
        edge(
            bounds,
            front,
            8. * aura.unit,
            2. * aura.unit,
            aura.core.opacity(if aura.ultra { 0.95 } else { 0.65 }),
            window,
        );
    }
    if aura.ultra {
        counterflow(bounds, elapsed, aura, window);
        corners(bounds, elapsed, aura, window);
    }
}

fn counterflow(bounds: Bounds<Pixels>, elapsed: f32, aura: &Aura, window: &mut Window) {
    let inset = 3.5 * aura.unit;
    let inner = Bounds::new(
        bounds.origin + point(px(inset), px(inset)),
        size(
            bounds.size.width - px(2. * inset),
            bounds.size.height - px(2. * inset),
        ),
    );
    let perimeter = 2. * f32::from(inner.size.width + inner.size.height);
    for spark in 0..6 {
        let at = perimeter * ((-elapsed / 2.4).rem_euclid(1.) + spark as f32 / 6.);
        for step in 0..8 {
            let strength = (1. - step as f32 / 8.).powi(2);
            let position = at + step as f32 * 6. * aura.unit;
            edge(
                inner,
                position,
                6.5 * aura.unit,
                3. * aura.unit,
                aura.echo.opacity(strength * 0.12),
                window,
            );
            edge(
                inner,
                position,
                6.5 * aura.unit,
                aura.unit,
                aura.core.opacity(strength * 0.75),
                window,
            );
        }
    }
}

fn corners(bounds: Bounds<Pixels>, elapsed: f32, aura: &Aura, window: &mut Window) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let span = (24. * aura.unit).min(w.min(h) * 0.12);
    for (index, at) in [0., w, w + h, 2. * w + h].into_iter().enumerate() {
        let pulse = 0.5 + 0.5 * (elapsed * TAU / 2.4 - index as f32 * TAU / 4.).sin();
        let color = if index % 2 == 0 { aura.main } else { aura.echo };
        for step in 0..8 {
            let strength = (1. - step as f32 / 8.) * (0.25 + pulse * 0.5);
            let length = span / 8.;
            for position in [at + step as f32 * length, at - (step + 1) as f32 * length] {
                edge(
                    bounds,
                    position,
                    length + 0.3,
                    8. * aura.unit,
                    color.opacity(strength * 0.12),
                    window,
                );
                edge(
                    bounds,
                    position,
                    length + 0.3,
                    2. * aura.unit,
                    color.opacity(strength),
                    window,
                );
            }
        }
    }
}

fn edge(
    bounds: Bounds<Pixels>,
    start: f32,
    length: f32,
    thickness: f32,
    color: Hsla,
    window: &mut Window,
) {
    let w = f32::from(bounds.size.width);
    let h = f32::from(bounds.size.height);
    let sides = [w, h, w, h];
    let perimeter = 2. * (w + h);
    let mut at = start.rem_euclid(perimeter);
    let mut left = length.min(perimeter);
    for _ in 0..5 {
        if left <= 0. {
            break;
        }
        let mut side = 0;
        let mut offset = at;
        while side < 3 && offset >= sides[side] {
            offset -= sides[side];
            side += 1;
        }
        let take = left.min(sides[side] - offset);
        let (x, y, width, height) = match side {
            0 => (offset, 0., take, thickness),
            1 => (w - thickness, offset, thickness, take),
            2 => (w - offset - take, h - thickness, take, thickness),
            _ => (0., h - offset - take, thickness, take),
        };
        window.paint_quad(fill(
            Bounds::new(
                bounds.origin + point(px(x), px(y)),
                size(px(width), px(height)),
            ),
            color,
        ));
        left -= take;
        at = (at + take).rem_euclid(perimeter);
    }
}
