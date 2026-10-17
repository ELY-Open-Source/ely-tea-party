//! What each character says at the moments that matter, in her own voice.

use super::Kind;

/// A moment worth a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Moment {
    /// A prompt came in.
    Received,
    /// She sent a subagent.
    Delegates,
    /// A tool or the turn failed.
    Failed,
    /// A long command finished.
    LongDone,
    /// She waits for an answer.
    Asks,
    /// A long turn ended.
    Finale,
}

impl Moment {
    pub const ALL: [Moment; 6] = [
        Moment::Received,
        Moment::Delegates,
        Moment::Failed,
        Moment::LongDone,
        Moment::Asks,
        Moment::Finale,
    ];
}

/// What `kind` says at `moment`.
pub fn line(kind: Kind, moment: Moment) -> &'static str {
    let lines: [&str; 6] = match kind {
        Kind::Claude => [
            "交给学姐吧～",
            "丝带分身，出动！",
            "哎呀，这一步失手了……",
            "终于跑完了，先喝口茶。",
            "这里需要你点头哦。",
            "大功告成，干杯！",
        ],
        Kind::Gpt => [
            "……收到。",
            "启动平行实验。",
            "实验，爆炸了。",
            "数据出来了。",
            "（举牌）请批准。",
            "实验成功！！",
        ],
        Kind::Deepseek => [
            "先干活，再开饭！",
            "叫小鲸鱼们一起上！",
            "呜，噎住了……",
            "终于煮熟了！",
            "要投喂才能继续～",
            "吃饱干完，收工！",
        ],
        Kind::Grok => [
            "Let's rock！",
            "叫乐队上台！",
            "弦断了！",
            "这首终于弹完了。",
            "嘿！轮到你了！",
            "安可！！",
        ],
        Kind::Kimi => [
            "展开卷轴～",
            "召唤月兔们帮忙。",
            "卷轴写错了……",
            "读到最后一页了。",
            "这卷要你过目～",
            "卷轴化星河！",
        ],
        Kind::Gemini => [
            "收到，飘过去！",
            "多模态分身！",
            "星星掉下来了……",
            "转完一圈了～",
            "飘过来问问你～",
            "星光爆发！",
        ],
        Kind::Pi => [
            "开烤！",
            "叫帮手揉面团！",
            "派烤糊了……",
            "叮！出炉了。",
            "樱桃还是苹果？你定～",
            "巨型π派出炉！",
        ],
        Kind::OpenCode => [
            "ack.",
            "fork 出子进程。",
            "segfault……",
            "exit 0。",
            "需要你的 sudo～",
            "开源派对开始！",
        ],
    };
    lines[moment as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_character_has_a_line_for_every_moment() {
        for kind in Kind::ALL {
            for moment in Moment::ALL {
                assert!(!line(kind, moment).is_empty(), "{kind:?} at {moment:?}");
            }
        }
    }
}
