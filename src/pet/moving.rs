//! Her move from one edge of the pane to the other: she dives behind the edge she leaves, her window moves, and she rises from the new one.

use std::time::{Duration, Instant};

use gpui::{App, Context, Window};

use super::{Edge, Pet};

/// How long her dive behind the edge she leaves takes.
const DIVE: Duration = Duration::from_millis((super::look::DIVE * 1000.0) as u64);

impl Pet {
    /// Keeps her window on the edge she stands on, and starts her dive once the room gives her the other, or calls it off once it gives hers back.
    pub(super) fn place(&mut self, main: &Window, cx: &mut App) {
        let Some(glue) = &self.glue else {
            return;
        };
        let room = match glue.place(main, self.at, Some(self.edge)) {
            Ok(room) => room,
            Err(error) => return log::error!("{error:#}"),
        };
        self.heard(cx);
        match self.moving {
            None if room != self.edge => self.dive(room, Instant::now()),
            Some(_) if room == self.edge => {
                log::info!("pet: Ely stays on the pane's {} edge", name(room));
                self.moving = None;
                self.ease(Instant::now());
            }
            _ => {}
        }
    }

    /// Has her window read its bounds and scale after this update: gpui drops what the platform reports of a move made inside one, so she kept the scale of a display she left.
    pub(super) fn heard(&self, cx: &mut App) {
        let Some(own) = self.own else {
            return;
        };
        cx.defer(move |cx| {
            if own
                .update(cx, |_, window, cx| window.bounds_changed(cx))
                .is_err()
            {
                log::debug!("pet: her window went before it read its move");
            }
        });
    }

    fn dive(&mut self, to: Edge, now: Instant) {
        log::info!("pet: Ely dives, to move to the pane's {} edge", name(to));
        self.moving = Some((to, now));
    }

    /// Whether her dive is over, starting one for `to` if none is under way; a Wayland popup cannot move, so it opens again on `to` after.
    pub(super) fn dived(&mut self, to: Edge, now: Instant) -> bool {
        match self.moving {
            Some((_, at)) => now.saturating_duration_since(at) >= DIVE,
            None => {
                self.dive(to, now);
                false
            }
        }
    }

    /// Places her, and lands her once a dive that starts is over.
    pub(super) fn keep(&mut self, main: &Window, cx: &mut Context<Self>) {
        self.place(main, cx);
        self.wait_to_land(cx);
    }

    /// Lands her by the clock once her dive is over, and again after any dive the landing starts; gpui gives a window it hides no frames to do it in.
    fn wait_to_land(&mut self, cx: &mut Context<Self>) {
        if self.moving.is_none() || self.landing {
            return;
        }
        self.landing = true;
        let main = self.main;
        cx.spawn(async move |pet, cx| {
            loop {
                let left = |pet: &Pet, _: &App| {
                    (pet.moving).map(|(_, at)| DIVE.saturating_sub(at.elapsed()))
                };
                match cx.update(|cx| pet.read_with(cx, left)) {
                    Ok(Some(left)) if !left.is_zero() => cx.background_executor().timer(left).await,
                    Ok(Some(_)) => {
                        let landed = cx.update(|cx| {
                            main.update(cx, |_, main, cx| {
                                pet.update(cx, |pet, cx| pet.arrive(main, cx))
                            })
                        });
                        if let Err(error) = landed.and_then(|landed| landed) {
                            log::debug!("pet: she does not land, a window gone: {error:#}");
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(_) => return,
                }
            }
            let done = cx.update(|cx| pet.update(cx, |pet, _| pet.landing = false));
            done.unwrap_or_else(|_| log::debug!("pet: her window went as she landed"));
        })
        .detach();
    }

    /// Stands her on the edge she dove for, where she rises as she does when she comes.
    fn arrive(&mut self, main: &Window, cx: &mut App) {
        let Some((to, _)) = self.moving.take() else {
            return;
        };
        log::info!("pet: Ely moves to the pane's {} edge", name(to));
        self.edge = to;
        self.shown = Instant::now();
        self.place(main, cx);
    }
}

fn name(edge: Edge) -> String {
    format!("{edge:?}").to_lowercase()
}
