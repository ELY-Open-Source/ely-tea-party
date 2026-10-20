//! A drag that moves her along the edge she stands on.

use gpui::{App, Pixels};

use super::{Pet, WIDTH, pane};
use crate::settings::SettingsStore;

impl Pet {
    /// Takes hold of her for a drag along her edge, the pointer `local` in her window.
    pub(super) fn grab(&mut self, local: Pixels, scale: f32) {
        match self.pointer(local, scale) {
            Ok(x) => self.drag = Some((x, self.at)),
            Err(error) => log::error!("{error:#}"),
        }
    }

    /// Moves her along her edge after the pointer, inside the pane.
    pub(super) fn slide(&mut self, local: Pixels, scale: f32, cx: &mut App) {
        let Some((from, at)) = self.drag else {
            return;
        };
        let x = match self.pointer(local, scale) {
            Ok(x) => x,
            Err(error) => {
                self.drag = None;
                return log::error!("{error:#}");
            }
        };
        let main = self.main;
        let slid = main.update(cx, |_, main, _| {
            let room = (f32::from(pane(main).size.width) - WIDTH).max(0.0);
            // Whole points, so the kept place carries no float noise.
            self.at = (at + x - from).round().clamp(0.0, room.floor());
            self.place(main);
        });
        if let Err(error) = slid {
            log::debug!("pet: the main window does not read: {error:#}");
        }
    }

    /// Lets go of her, the settings keeping where the drag left her, where a Wayland popup then opens again.
    pub(super) fn release(&mut self, cx: &mut App) {
        if self.drag.take().is_none() || SettingsStore::get(cx).ely_left == self.at {
            return;
        }
        let at = self.at;
        match SettingsStore::update(cx, |settings| settings.ely_left = at) {
            Ok(()) => log::info!("pet: Ely stands {at} points in from the pane's left edge"),
            Err(error) => log::error!("pet: where Ely stands is not kept: {error:#}"),
        }
    }

    /// The pointer's x in points: on the screen where her glue moves her window, else in her window, which a drag leaves in place.
    fn pointer(&self, local: Pixels, scale: f32) -> anyhow::Result<f32> {
        match &self.glue {
            Some(glue) => glue.pointer(scale),
            None => Ok(local.into()),
        }
    }
}
