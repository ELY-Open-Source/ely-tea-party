use gpui::App;

use crate::settings::{FONT_SIZE, SettingsStore};

/// The font size the terminal draws.
pub fn size(cx: &App) -> f32 {
    let settings = SettingsStore::get(cx);
    shown(settings.font_size, settings.font_zoom)
}

/// Moves the font `by` points; none resets it.
pub fn step(by: Option<f32>, cx: &mut App) {
    let settings = SettingsStore::get(cx);
    let setting = settings.font_size;
    let enlarged = by.map_or(0.0, |by| moved(setting, settings.font_zoom, by));
    if let Err(error) = SettingsStore::update(cx, |settings| settings.font_zoom = enlarged) {
        log::error!("zoom: cannot save terminal zoom: {error:#}");
        return;
    }
    log::info!(
        "zoom: the terminal's font is {} points, {enlarged:+} on the setting",
        shown(setting, enlarged)
    );
}

fn shown(setting: f32, enlarged: f32) -> f32 {
    (setting + enlarged).clamp(*FONT_SIZE.start(), *FONT_SIZE.end())
}

/// The offset after moving `by` from the shown size.
fn moved(setting: f32, enlarged: f32, by: f32) -> f32 {
    shown(shown(setting, enlarged) + by, 0.0) - setting
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_step_moves_the_font_inside_the_range() {
        assert_eq!(moved(13.0, 0.0, 1.0), 1.0);
        assert_eq!(moved(13.0, 2.0, -1.0), 1.0);
        assert_eq!(moved(71.0, 0.0, 1.0), 1.0);
        assert_eq!(moved(72.0, 0.0, 1.0), 0.0, "the top holds");
        assert_eq!(moved(6.0, 0.0, -1.0), 0.0, "the bottom holds");
        assert_eq!(
            shown(70.0, 5.0),
            72.0,
            "a larger setting under a zoom stays in range"
        );
        assert_eq!(moved(70.0, 5.0, -1.0), 1.0, "a step starts from what shows");
    }
    #[test]
    fn saved_zoom_rejects_invalid_offsets_and_clamps_effective_size() {
        for zoom in [f32::NAN, f32::INFINITY, -67.0, 67.0] {
            let settings = crate::settings::Settings {
                font_zoom: zoom,
                ..Default::default()
            };
            assert!(
                settings
                    .validate()
                    .unwrap_err()
                    .to_string()
                    .contains("font_zoom")
            );
        }
        for (base, zoom, expected) in [
            (13.0, 2.0, 15.0),
            (13.0, -2.0, 11.0),
            (72.0, 66.0, 72.0),
            (6.0, -66.0, 6.0),
        ] {
            let settings = crate::settings::Settings {
                font_size: base,
                font_zoom: zoom,
                ..Default::default()
            };
            settings.validate().unwrap();
            assert_eq!(shown(settings.font_size, settings.font_zoom), expected);
        }
    }
}
