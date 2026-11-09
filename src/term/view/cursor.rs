//! The cursor's blink and its glide between cells.

use std::time::{Duration, Instant};

use gpui::{Context, Task};

use super::{BLINK, TerminalView};

/// How long the cursor glides to a new cell.
const GLIDE: Duration = Duration::from_millis(90);

impl TerminalView {
    /// Where the cursor is drawn on its way to `to`, in cells.
    pub(in crate::term) fn glide_position(&self, to: (usize, usize)) -> (f32, f32) {
        let end = (to.0 as f32, to.1 as f32);
        match self.glide {
            Some((from, start)) if self.target == Some(to) => {
                let t = start.elapsed().as_secs_f32() / GLIDE.as_secs_f32();
                if t >= 1. {
                    return end;
                }
                let e = t * t * (3. - 2. * t);
                (from.0 + (end.0 - from.0) * e, from.1 + (end.1 - from.1) * e)
            }
            _ => end,
        }
    }

    /// Starts a glide from where the cursor is drawn when its cell changes.
    pub(super) fn follow(&mut self, target: Option<(usize, usize)>, smooth: bool) {
        match (self.target, target) {
            (_, None) => self.glide = None,
            (Some(from), Some(to)) if smooth && from != to => {
                self.glide = Some((self.glide_position(from), Instant::now()));
            }
            (_, Some(_)) if !smooth => self.glide = None,
            _ => {}
        }
        self.target = target;
    }

    pub(super) fn blink(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(BLINK).await;
                let ticked = view.update(cx, |view, cx| {
                    if view.blinking {
                        view.shown = !view.shown;
                        cx.notify();
                    }
                });
                if ticked.is_err() {
                    break;
                }
            }
        })
    }
}
