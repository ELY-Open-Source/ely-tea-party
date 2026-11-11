pub(super) const MESSAGES: &[(&str, &str)] = &[
    ("et.messaging.title", "Agent 通信集成"),
    (
        "et.messaging.description",
        "Ely 终端自带 App 的 CLI。为各 agent 安装通信 skill 后，受管理的入口会随 Ely 更新。接收方主动读取收件箱；若 agent 尚未发现 skill，请重新启动它。",
    ),
    ("et.messaging.missing", "未安装"),
    ("et.messaging.synced", "已安装并校验"),
    ("et.messaging.update", "需要更新"),
    ("et.messaging.modified", "用户已修改，保留原文件"),
    ("et.messaging.failed", "校验失败"),
    ("et.messaging.install", "安装／更新"),
    ("et.messaging.remove", "移除"),
    ("et.messaging.refresh", "检查并同步"),
];
