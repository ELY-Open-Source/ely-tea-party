pub const MESSAGES: &[(&str, &str)] = &[
    ("et.drawer.share", "发送给 Agent"),
    (
        "et.drawer.share_hint",
        "预览脱敏后的输出，再传给上方的 Agent。请先在上方启动 Agent。",
    ),
    ("et.drawer.preview", "发送终端内容给 Agent"),
    (
        "et.drawer.privacy",
        "已在本机遮盖常见密钥、邮箱和用户目录名，请检查并编辑可能遗漏的信息。确认后填入 Agent 输入区，由你按 Enter 提交。",
    ),
    (
        "et.drawer.scope",
        "最近的输出 · 最多 400 行终端内容 / 32 KB",
    ),
    ("et.drawer.truncated", "较早的输出已省略。"),
    ("et.drawer.cancel", "取消"),
    ("et.drawer.insert", "填入 Agent"),
    (
        "et.drawer.redacted_again",
        "编辑后的内容已再次脱敏，请检查更新后的预览，再次点击填入。",
    ),
    (
        "et.drawer.target_changed",
        "接收的会话或 Agent 已改变，请返回原会话或重新打开预览。",
    ),
    (
        "et.drawer.no_paste",
        "Agent 尚未启用安全粘贴，未发送任何内容，请等输入区就绪后重试。",
    ),
];
