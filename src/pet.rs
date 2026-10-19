//! Ely, the icon's girl, peeking over the main window's top edge from a window of her own.

mod look;
#[cfg(target_os = "macos")]
mod macos;
mod rig;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

use std::{cell::RefCell, rc::Rc, time::Instant};

use gpui::{
    App, AppContext as _, Bounds, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Render, Styled, Subscription, Window, WindowBackgroundAppearance,
    WindowBounds, WindowHandle, WindowKind, WindowOptions, canvas, div, point, popup, px, size,
};

#[cfg(target_os = "macos")]
use self::macos as platform;
#[cfg(windows)]
use self::windows as platform;
#[cfg(target_os = "linux")]
use self::x11 as platform;
use self::{
    look::{Act, EASE, Mood, Pose, blend, mood, pose},
    rig::Rig,
};
use crate::{settings::SettingsStore, workspace::Workspace};

/// Her head's box, in points, and the room above it her hops take.
const WIDTH: f32 = 112.0;
const HEAD: f32 = 107.4;
const ROOM: f32 = 16.0;
/// How far in from the pane's left edge she holds its top edge.
const LEFT: f32 = 140.0;

/// Ely in her window: she moves all the time, her acts follow the sessions' marks, and a press pets her.
pub struct Pet {
    rig: Rc<Rig>,
    mood: Mood,
    /// When the mood began, when she came, and when she was last pressed.
    since: Instant,
    shown: Instant,
    petted: Option<Instant>,
    /// What she was doing, her last pose, and the pose a change of act eases her from, with its time.
    act: Act,
    last: Option<Pose>,
    eased: Option<(Pose, Instant)>,
    /// How far in from the pane's left edge she opened.
    left: f32,
    /// What keeps her window on the main window's edge, where the platform does not.
    glue: Option<platform::Glue>,
    _watch: Vec<Subscription>,
}

/// Shows Ely over `main` while the settings want her.
pub fn follow(main: WindowHandle<Workspace>, cx: &mut App) {
    let shown: Rc<RefCell<Option<WindowHandle<Pet>>>> = Rc::default();
    let sync = {
        let shown = shown.clone();
        move |cx: &mut App| {
            let open = *shown.borrow();
            match (SettingsStore::get(cx).ely, open) {
                (true, None) => *shown.borrow_mut() = open_pet(main, cx),
                (false, Some(pet)) => {
                    *shown.borrow_mut() = None;
                    close(pet, "as the settings say", cx);
                }
                _ => {}
            }
        }
    };
    sync(cx);
    cx.observe_global::<SettingsStore>(sync).detach();
    // A Wayland popup keeps the place it opened at, so she opens again where a new width puts her.
    let moved = main.update(cx, |_, window, cx| {
        cx.observe_window_bounds(window, move |_, window, cx| {
            let Some(pet) = *shown.borrow() else {
                return;
            };
            let at = left(pane(window).size.width);
            if wayland(window) && pet.read(cx).is_ok_and(|pet| pet.left != at) {
                let shown = shown.clone();
                cx.defer(move |cx| {
                    close(pet, "to stand where the new width puts her", cx);
                    *shown.borrow_mut() = open_pet(main, cx);
                });
            }
        })
    });
    match moved {
        Ok(moved) => moved.detach(),
        Err(error) => log::error!("pet: the main window's width goes unwatched: {error:#}"),
    }
}

/// Opens her window, logging why when it does not open.
fn open_pet(main: WindowHandle<Workspace>, cx: &mut App) -> Option<WindowHandle<Pet>> {
    show(main, cx)
        .inspect_err(|error| log::error!("pet: Ely does not show: {error:#}"))
        .ok()
}

fn close(pet: WindowHandle<Pet>, why: &str, cx: &mut App) {
    log::info!("pet: Ely goes, {why}");
    if pet
        .update(cx, |_, window, _| window.remove_window())
        .is_err()
    {
        log::debug!("pet: her window had gone");
    }
}

/// How far in from the pane's left edge she stands on a pane `width` points wide: `LEFT`, or less on a narrow pane, so she holds its edge.
fn left(width: Pixels) -> f32 {
    LEFT.min(f32::from(width) - WIDTH).max(0.0)
}

/// Opens her window on `main`'s top edge: a popup Wayland's compositor places, else a window kept there.
fn show(main: WindowHandle<Workspace>, cx: &mut App) -> anyhow::Result<WindowHandle<Pet>> {
    let (bounds, pane, wayland) = main.update(cx, |_, window, _| {
        (window.bounds(), pane(window), wayland(window))
    })?;
    let at = left(pane.size.width);
    let room = size(px(WIDTH), px(HEAD + ROOM));
    let kind = if wayland {
        WindowKind::AnchoredPopup(popup::PopupOptions {
            parent: main.into(),
            anchor_rect: Bounds::new(
                point(pane.origin.x + px(at), pane.origin.y),
                size(px(1.), px(1.)),
            ),
            anchor: popup::PopupAnchor::TopLeft,
            gravity: popup::PopupGravity::TopRight,
            constraint_adjustment: popup::PopupConstraintAdjustment::empty(),
            offset: point(px(0.), px(0.)),
            grab: false,
        })
    } else {
        WindowKind::PopUp
    };
    let origin = bounds.origin + pane.origin + point(px(at), -room.height);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(origin, room))),
        titlebar: None,
        focus: false,
        show: true,
        kind,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    };
    let workspace = main.entity(cx)?;
    let feeling = |workspace: &Workspace, cx: &App| {
        mood(
            workspace
                .sessions()
                .iter()
                .flat_map(|session| [session.looks(cx), session.agent_look()])
                .flatten(),
        )
    };
    let start = feeling(workspace.read(cx), cx);
    let pet = cx.open_window(options, |_, cx| {
        cx.new(|cx| {
            let feel = cx.observe(&workspace, move |pet: &mut Pet, workspace, cx| {
                let mood = feeling(workspace.read(cx), cx);
                if mood != pet.mood {
                    pet.feel(mood, Instant::now());
                    cx.notify();
                }
            });
            Pet {
                rig: Rc::new(Rig::new()),
                mood: start,
                since: Instant::now(),
                shown: Instant::now(),
                petted: None,
                act: Act::Calm,
                last: None,
                eased: None,
                left: at,
                glue: None,
                _watch: vec![feel],
            }
        })
    })?;
    if !wayland {
        glue(main, pet, cx)?;
    }
    let way = if wayland {
        "a Wayland popup"
    } else {
        "a window kept there"
    };
    log::info!("pet: Ely peeks over the main window, {way}, feeling {start:?}");
    Ok(pet)
}

/// Keeps her window on `main`'s top edge as it moves, comes forward and hides.
fn glue(main: WindowHandle<Workspace>, pet: WindowHandle<Pet>, cx: &mut App) -> anyhow::Result<()> {
    main.update(cx, |_, main, cx| {
        pet.update(cx, |pet, window, cx| {
            let glue = platform::Glue::new(window, main)?;
            glue.place(main)?;
            pet.glue = Some(glue);
            pet._watch
                .push(cx.observe_window_bounds(main, |pet, main, _| pet.place(main)));
            pet._watch
                .push(cx.observe_window_activation(main, |pet, main, _| pet.place(main)));
            #[cfg(target_os = "linux")]
            pet._watch.push(
                cx.observe_window_visibility(main, |pet, visibility, main, _| {
                    pet.shown(visibility.is_visible(), main)
                }),
            );
            anyhow::Ok(())
        })?
    })?
}

/// The main window's pane, in its own points.
fn pane(window: &Window) -> Bounds<Pixels> {
    crate::blur::pane(
        window.viewport_size(),
        window.client_inset(),
        window.window_decorations(),
    )
}

/// Whether `window` is a Wayland surface, where the compositor places windows.
#[cfg(target_os = "linux")]
fn wayland(window: &Window) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    HasWindowHandle::window_handle(window)
        .is_ok_and(|handle| matches!(handle.as_raw(), RawWindowHandle::Wayland(_)))
}

#[cfg(not(target_os = "linux"))]
fn wayland(_: &Window) -> bool {
    false
}

impl Pet {
    /// Takes up `mood` at `now`, its act beginning there, eased from where she is.
    fn feel(&mut self, mood: Mood, now: Instant) {
        log::debug!("pet: Ely feels {mood:?}, was {:?}", self.mood);
        (self.mood, self.since) = (mood, now);
        self.ease(now);
    }

    /// A press at `now`, which winks her, eased from where she is.
    fn press(&mut self, now: Instant) {
        log::debug!("pet: Ely is pressed");
        self.petted = Some(now);
        self.ease(now);
    }

    /// Eases her from her last pose, from `now`, as a move restarts.
    fn ease(&mut self, now: Instant) {
        self.eased = self.last.clone().map(|last| (last, now));
    }

    /// Her pose now, eased from where she was while a change of act or mood, or a press, is fresh.
    fn pose(&mut self, now: Instant) -> Pose {
        let since = |at: Instant| now.saturating_duration_since(at).as_secs_f64();
        let mut pose = pose(
            self.mood,
            since(self.since),
            since(self.shown),
            self.petted.map(since),
        );
        if pose.act != self.act {
            log::debug!("pet: Ely goes from {:?} to {:?}", self.act, pose.act);
            self.act = pose.act;
            self.ease(now);
        }
        if let Some((from, at)) = &self.eased {
            let into = since(*at) as f32;
            if into < EASE {
                pose = blend(from, pose, into);
            } else {
                self.eased = None;
            }
        }
        self.last = Some(pose.clone());
        pose
    }

    fn place(&self, main: &Window) {
        if let Some(glue) = &self.glue
            && let Err(error) = glue.place(main)
        {
            log::error!("{error:#}");
        }
    }

    #[cfg(target_os = "linux")]
    fn shown(&self, visible: bool, main: &Window) {
        if let Some(glue) = &self.glue {
            match glue.shown(visible) {
                Ok(()) => log::debug!("pet: Ely shows {visible} with the main window"),
                Err(error) => log::error!("{error:#}"),
            }
        }
        if visible {
            self.place(main);
        }
    }
}

impl Render for Pet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let pose = self.pose(Instant::now());
        // gpui gives a hidden window no frames, so she rests while unseen.
        window.request_animation_frame();
        let rig = self.rig.clone();
        let drawn = canvas(
            |_, _, _| {},
            move |bounds, _, window, cx| {
                let head = point(bounds.origin.x, bounds.origin.y + px(ROOM));
                rig.paint(head, &pose, window, cx);
            },
        );
        div().size_full().child(drawn.size_full()).on_mouse_down(
            MouseButton::Left,
            cx.listener(|pet, _, _, cx| {
                pet.press(Instant::now());
                cx.notify();
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{look::Mark, *};

    /// Her, `mood` since `since`, come long before.
    fn ely(mood: Mood, since: Instant) -> Pet {
        Pet {
            rig: Rc::new(Rig::new()),
            mood,
            since,
            shown: since - Duration::from_secs(60),
            petted: None,
            act: Act::Calm,
            last: None,
            eased: None,
            left: LEFT,
            glue: None,
            _watch: Vec::new(),
        }
    }

    #[test]
    fn a_change_of_mood_or_a_press_eases_her_from_where_she_is_though_the_act_stays() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        // A wait hops her, a press winks, and a program starts working inside the wink.
        let mut pet = ely(Mood::Waiting, start);
        pet.press(at(2250));
        let before = pet.pose(at(2750));
        pet.feel(Mood::Working, at(2750));
        let after = pet.pose(at(2751));
        assert_eq!((before.act, after.act), (Act::Wink, Act::Wink));
        assert!(
            (after.lift - before.lift).abs() < 1.0,
            "no drop: {} then {}",
            before.lift,
            after.lift
        );
        let marks: Vec<Mark> = after.marks.iter().map(|(mark, _)| *mark).collect();
        assert!(
            marks.contains(&Mark::Bang),
            "the wait's mark fades: {marks:?}"
        );
        // An old finish clears mid-glance: calm before and after, her eyes turn back at her pace.
        let mut pet = ely(Mood::Done, start);
        let before = pet.pose(at(13_600));
        pet.feel(Mood::Calm, at(13_600));
        let after = pet.pose(at(13_601));
        assert_eq!((before.act, after.act), (Act::Calm, Act::Calm));
        assert!(
            before.look.0 < -5.0 && (after.look.0 - before.look.0).abs() < 0.5,
            "no snap: {:?} then {:?}",
            before.look,
            after.look
        );
        let later = pet.pose(at(14_000));
        assert!(
            pet.eased.is_none() && later.look.0.abs() < 0.5,
            "eased in by then"
        );
        // A second press mid-hop restarts the wink without a drop.
        let mut pet = ely(Mood::Calm, start);
        pet.press(at(1000));
        let before = pet.pose(at(1300));
        pet.press(at(1300));
        let after = pet.pose(at(1301));
        assert!(
            before.lift > 10.0 && (after.lift - before.lift).abs() < 1.0,
            "no drop: {} then {}",
            before.lift,
            after.lift
        );
    }

    #[test]
    fn she_holds_the_edge_of_any_pane() {
        assert_eq!(left(px(1200.)), LEFT);
        assert_eq!(left(px(200.)), 200.0 - WIDTH);
        assert_eq!(
            left(px(100.)),
            0.0,
            "narrower than she is, she stands at its left"
        );
    }
}
