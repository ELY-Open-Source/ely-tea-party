use serde::Serialize;

use crate::sessions::orb::Look;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Error,
    Waiting,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Attention {
    pub id: String,
    pub title: String,
    pub kind: Kind,
}

pub fn kind(terminal: Option<Look>, agent: Option<Look>) -> Option<Kind> {
    match (terminal, agent) {
        (Some(Look::Error), _) | (_, Some(Look::Error)) => Some(Kind::Error),
        (Some(Look::Waiting), _) | (_, Some(Look::Waiting)) => Some(Kind::Waiting),
        _ => None,
    }
}

pub fn report_sessions(ids: &[String], attention: &[Attention]) -> Vec<usize> {
    let mut indices: Vec<_> = (0..ids.len()).collect();
    if !attention.is_empty() {
        indices.retain(|&index| attention.iter().any(|a| a.id == ids[index]));
    }
    indices.sort_by_key(|&index| {
        attention
            .iter()
            .position(|a| a.id == ids[index])
            .unwrap_or(usize::MAX)
    });
    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_and_waiting_need_attention_but_work_and_idle_do_not() {
        assert_eq!(
            kind(Some(Look::Error), Some(Look::Waiting)),
            Some(Kind::Error)
        );
        assert_eq!(kind(None, Some(Look::Waiting)), Some(Kind::Waiting));
        assert_eq!(kind(Some(Look::Waiting), None), Some(Kind::Waiting));
        assert_eq!(kind(Some(Look::Working(None)), Some(Look::Idle)), None);
        assert_eq!(kind(Some(Look::Done), None), None);
    }

    #[test]
    fn alerts_beyond_the_snapshot_limit_are_read_first_by_stable_id() {
        let ids: Vec<_> = (0..40).map(|id| id.to_string()).collect();
        let attention = vec![
            Attention {
                id: "39".into(),
                title: "Failed".into(),
                kind: Kind::Error,
            },
            Attention {
                id: "35".into(),
                title: "Question".into(),
                kind: Kind::Waiting,
            },
        ];
        assert_eq!(report_sessions(&ids, &attention), vec![39, 35]);
        assert_eq!(report_sessions(&ids, &[]), (0..40).collect::<Vec<_>>());
    }
}
