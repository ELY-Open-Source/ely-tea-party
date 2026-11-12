pub const MESSAGES: &[(&str, &str)] = &[
    ("et.drawer.share", "Send to Agent"),
    (
        "et.drawer.share_hint",
        "Preview redacted output for the agent above. Start an agent there first.",
    ),
    ("et.drawer.preview", "Terminal context for Agent"),
    (
        "et.drawer.privacy",
        "Common secrets, emails and home directory names are redacted locally. Review and edit for anything missed. Inserts into the agent input; press Enter there to submit.",
    ),
    (
        "et.drawer.scope",
        "Recent output only · up to 400 terminal rows / 32 KB",
    ),
    ("et.drawer.truncated", "Older output was omitted."),
    ("et.drawer.cancel", "Cancel"),
    ("et.drawer.insert", "Insert into Agent"),
    (
        "et.drawer.redacted_again",
        "Your edits were redacted. Review the updated preview, then insert again.",
    ),
    (
        "et.drawer.target_changed",
        "The receiving session or agent changed. Return to the original session or reopen the preview.",
    ),
    (
        "et.drawer.no_paste",
        "The agent has not enabled bracketed paste. Nothing was sent; retry when its input is ready.",
    ),
];
