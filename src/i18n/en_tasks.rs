pub const MESSAGES: &[(&str, &str)] = &[
    ("et.task.unseen", "Done · Unseen"),
    ("et.task.review", "Viewed · Unreviewed"),
    ("et.task.accepted", "User acknowledged"),
    ("et.task.party", "Tea Party · {count} need attention"),
    ("et.task.result", "View result"),
    ("et.task.reply", "Captured closing reply"),
    (
        "et.task.no_reply",
        "No closing reply was captured. Open the terminal for context.",
    ),
    (
        "et.task.evidence",
        "Replies and tool events below are reported by hooks, not independent verification.",
    ),
    ("et.task.runs", "Commands in this turn"),
    ("et.task.run_unknown", "No completion reported"),
    ("et.task.run_failed", "Reported failure"),
    ("et.task.run_returned", "Returned"),
    (
        "et.task.no_runs",
        "No command runs were captured for this turn; verification is unknown.",
    ),
    ("et.task.changes", "Workspace changes"),
    ("et.task.activity", "Open Activity records"),
    (
        "et.task.shared_changes",
        "Git shows all current workspace changes, including yours or other agents’ work.",
    ),
    ("et.task.accept", "I reviewed this · Mark handled"),
    ("et.task.reopen", "Reopen for review"),
];
