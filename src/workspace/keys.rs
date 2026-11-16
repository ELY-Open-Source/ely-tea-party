use gpui::{Action, App, actions};

actions!(
    ely_tea_party,
    [
        QuickSwitch,
        ToggleSessions,
        TogglePanel,
        NewSession,
        CloseSession,
        NextSession,
        PreviousSession,
        LastSession,
        JumpToWaiting,
        ToggleDrawer,
        ToggleParty,
        SplitRight,
        SplitDown,
        NextPane,
        PreviousPane
    ]
);

/// Shows the session at this place, from 0; past the last one, the key goes on to the program.
#[derive(Clone, Debug, PartialEq, Action)]
#[action(namespace = ely_tea_party, no_json)]
pub struct SelectSession(pub usize);

/// The switcher's own keys, bound before `keymap::init` so its table keeps them; the commands' keys are `keymap`'s.
pub fn bind_keys(cx: &mut App) {
    super::switcher::bind_keys(cx);
}
