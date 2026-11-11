use ely_gpui_component::theme::{ActiveTheme, Palette};
use gpui::{App, Hsla};

use crate::{
    agents::timeline::Review,
    butler::tasks::{self, Status},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskState {
    Error,
    Waiting,
    Unseen,
    Review,
    Reported,
    Working,
    Accepted,
    Idle,
    Unknown,
}

impl TaskState {
    pub fn from_status(status: Status, review: Option<Review>) -> Self {
        match status {
            Status::Error => Self::Error,
            Status::Waiting => Self::Waiting,
            Status::Ended => match review {
                Some(Review::Unseen) => Self::Unseen,
                Some(Review::Viewed) => Self::Review,
                Some(Review::Accepted) => Self::Accepted,
                None => Self::Reported,
            },
            Status::Working => Self::Working,
            Status::Idle => Self::Idle,
            Status::Unknown => Self::Unknown,
        }
    }

    pub fn priority(self) -> u8 {
        match self {
            Self::Error => 0,
            Self::Waiting => 1,
            Self::Unseen | Self::Review | Self::Reported => 2,
            Self::Working => 3,
            Self::Accepted => 4,
            Self::Idle => 5,
            Self::Unknown => 6,
        }
    }

    pub fn needs_attention(self) -> bool {
        matches!(
            self,
            Self::Error | Self::Waiting | Self::Unseen | Self::Review | Self::Reported
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Error => "et.butler.badge_error",
            Self::Waiting => "et.butler.badge_waiting",
            Self::Unseen => "et.task.unseen",
            Self::Review => "et.task.review",
            Self::Accepted => "et.task.accepted",
            Self::Reported => "et.butler.badge_ended",
            Self::Working => "et.butler.task_working",
            Self::Idle => "et.status.idle",
            Self::Unknown => "et.butler.badge_unknown",
        }
    }

    pub fn color(self, colors: &Palette) -> Hsla {
        match self {
            Self::Error => colors.danger,
            Self::Waiting | Self::Unseen | Self::Review | Self::Reported => colors.warning,
            Self::Working => colors.info,
            Self::Accepted => colors.success,
            Self::Idle | Self::Unknown => colors.fg_muted,
        }
    }
}

impl super::Session {
    pub fn task_state(&self, cx: &App) -> TaskState {
        TaskState::from_status(
            tasks::observed(self, cx),
            self.log.completion().map(|entry| entry.review),
        )
    }

    pub fn task_color(&self, cx: &App) -> Hsla {
        self.task_state(cx).color(&cx.theme().colors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_a_result_does_not_move_it_below_other_unseen_results() {
        let mut rows = [(1, TaskState::Review), (2, TaskState::Unseen)];
        rows.sort_by_key(|(_, state)| state.priority());
        assert_eq!(rows[0].0, 1);
    }

    #[test]
    fn review_never_hides_live_work_or_a_problem() {
        for review in [Review::Unseen, Review::Viewed, Review::Accepted] {
            assert_eq!(
                TaskState::from_status(Status::Working, Some(review)),
                TaskState::Working
            );
            assert_eq!(
                TaskState::from_status(Status::Error, Some(review)),
                TaskState::Error
            );
            assert_eq!(
                TaskState::from_status(Status::Waiting, Some(review)),
                TaskState::Waiting
            );
        }
    }

    #[test]
    fn all_views_use_the_same_priority_and_attention_rule() {
        let mut states = [
            TaskState::Accepted,
            TaskState::Working,
            TaskState::Review,
            TaskState::Unseen,
            TaskState::Waiting,
            TaskState::Error,
        ];
        states.sort();
        assert_eq!(
            states,
            [
                TaskState::Error,
                TaskState::Waiting,
                TaskState::Unseen,
                TaskState::Review,
                TaskState::Working,
                TaskState::Accepted
            ]
        );
        assert!(TaskState::Review.needs_attention());
        assert!(!TaskState::Accepted.needs_attention());
        assert_eq!(TaskState::from_status(Status::Idle, None), TaskState::Idle);
        assert_eq!(
            TaskState::from_status(Status::Unknown, None),
            TaskState::Unknown
        );
    }
}
