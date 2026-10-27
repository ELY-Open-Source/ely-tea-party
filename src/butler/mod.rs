pub mod cache;
pub mod companion;
pub mod config;
pub mod links;
pub mod placement;
pub mod process;
pub mod state;
pub mod tasks;

pub use config::{Config, Credentials};
pub use state::Briefing;

fn dismiss_other_windows(current: gpui::AnyWindowHandle, cx: &mut gpui::App) {
    let others: Vec<_> = cx
        .windows()
        .into_iter()
        .filter(|window| *window != current)
        .filter_map(|window| window.downcast::<crate::workspace::Workspace>())
        .filter_map(|window| window.entity(cx).ok())
        .collect();
    for workspace in others {
        workspace.update(cx, |workspace, cx| workspace.dismiss_butler(cx));
    }
}

pub fn outside_click_listener() -> impl gpui::IntoElement {
    use gpui::Styled;
    gpui::canvas(
        |_, _, _| {},
        |_, _, window, _| {
            window.on_mouse_event(|_: &gpui::MouseDownEvent, phase, window, cx| {
                if phase == gpui::DispatchPhase::Capture {
                    dismiss_other_windows(window.window_handle(), cx);
                }
            });
        },
    )
    .absolute()
    .size_full()
}
