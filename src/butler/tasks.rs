use crate::{agents::timeline::Step, sessions::orb::Look};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Error,
    Waiting,
    Ended,
    Working,
    Idle,
    Unknown,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "et.butler.task_error",
            Self::Waiting => "et.butler.task_waiting",
            Self::Ended => "et.butler.task_ended",
            Self::Working => "et.butler.task_working",
            Self::Idle => "et.butler.task_idle",
            Self::Unknown => "et.butler.task_unknown",
        }
    }
}

pub fn status(terminal: Option<Look>, agent: Option<Look>) -> Status {
    let looks = [terminal, agent];
    if looks.contains(&Some(Look::Error)) {
        Status::Error
    } else if looks.contains(&Some(Look::Waiting)) {
        Status::Waiting
    } else if looks
        .iter()
        .any(|look| matches!(look, Some(Look::Working(_))))
    {
        Status::Working
    } else if terminal == Some(Look::Done) {
        Status::Ended
    } else if looks.contains(&Some(Look::Idle)) {
        Status::Idle
    } else {
        Status::Unknown
    }
}

pub fn excerpt(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

pub fn observed(session: &crate::sessions::Session, cx: &gpui::App) -> Status {
    let status = status(session.looks(cx), session.agent_look());
    if session.agent.as_ref().is_some_and(|agent| agent.hooked) {
        reported(status, session.log.entries.back().map(|entry| &entry.step))
    } else {
        status
    }
}

fn reported(status: Status, latest: Option<&Step>) -> Status {
    if matches!(status, Status::Idle | Status::Unknown)
        && let Some(Step::Ended { failed, .. }) = latest
    {
        if *failed {
            Status::Error
        } else {
            Status::Ended
        }
    } else {
        status
    }
}

pub fn latest(step: &Step) -> Option<(&'static str, String)> {
    let (key, value) = match step {
        Step::Prompt(_) => return None,
        Step::Tool { name, detail, .. } => {
            ("et.butler.task_tool", detail.as_deref().unwrap_or(name))
        }
        Step::Asked(detail) => ("et.butler.task_question", detail.as_deref()?),
        Step::Ended { said, .. } => ("et.butler.task_reported", said.as_deref()?),
    };
    let value = excerpt(value);
    (!value.is_empty()).then_some((key, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_or_missing_signals_never_claim_completion() {
        assert_eq!(status(None, Some(Look::Idle)), Status::Idle);
        assert_eq!(status(None, None), Status::Unknown);
        assert_eq!(status(Some(Look::Done), None), Status::Ended);
        assert_eq!(
            status(Some(Look::Done), Some(Look::Working(None))),
            Status::Working
        );
        assert_eq!(
            status(Some(Look::Error), Some(Look::Waiting)),
            Status::Error
        );
        assert_eq!(
            status(Some(Look::Working(None)), Some(Look::Waiting)),
            Status::Waiting
        );
    }

    #[test]
    fn agent_claims_stay_attributed_and_long_labels_are_bounded() {
        let step = Step::Ended {
            failed: false,
            took: None,
            said: Some("Tests passed".into()),
        };
        assert_eq!(
            latest(&step),
            Some(("et.butler.task_reported", "Tests passed".into()))
        );
        assert_eq!(reported(Status::Idle, Some(&step)), Status::Ended);
        assert_eq!(reported(Status::Working, Some(&step)), Status::Working);
        assert_eq!(reported(Status::Idle, None), Status::Idle);
        let failed = Step::Ended {
            failed: true,
            took: None,
            said: None,
        };
        assert_eq!(reported(Status::Idle, Some(&failed)), Status::Error);
        assert_eq!(excerpt(&"鲸\n".repeat(400)).chars().count(), 180);
        assert!(!excerpt("line\nnext\tword").contains('\n'));
    }
}
