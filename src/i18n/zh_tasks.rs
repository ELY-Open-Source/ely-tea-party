pub const MESSAGES: &[(&str, &str)] = &[
    ("et.task.unseen", "完成 · 未查看"),
    ("et.task.review", "已查看 · 待验收"),
    ("et.task.accepted", "用户已确认处理"),
    ("et.task.party", "Tea Party · {count} 项待处理"),
    ("et.task.result", "查看结果"),
    ("et.task.reply", "Agent 的结束回复（已记录片段）"),
    ("et.task.no_reply", "没有收到结束回复，请查看终端。"),
    (
        "et.task.evidence",
        "以下是 hooks 记录的回复和工具事件，不代表独立验收。",
    ),
    ("et.task.runs", "本轮运行记录"),
    ("et.task.run_unknown", "未收到结束事件"),
    ("et.task.run_failed", "报告失败"),
    ("et.task.run_returned", "已返回"),
    (
        "et.task.no_runs",
        "本轮没有收到运行命令的记录，不能据此判断验证是否通过。",
    ),
    ("et.task.changes", "当前工作区改动"),
    ("et.task.activity", "查看 Activity 原记录"),
    (
        "et.task.shared_changes",
        "Git 展示当前工作区的全部改动，可能包含你或其他 agent 的工作。",
    ),
    ("et.task.accept", "我已检查，标为已处理"),
    ("et.task.reopen", "重新列入待处理"),
];
