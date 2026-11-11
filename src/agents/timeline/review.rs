use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::{Entry, Step, Timeline};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Review {
    #[default]
    Unseen,
    Viewed,
    Accepted,
}

impl Timeline {
    pub fn retain_reply(&mut self, reply: Option<String>) {
        if let Some(reply) = reply
            && let Some(Entry {
                step: Step::Ended { said, .. },
                ..
            }) = self.entries.back_mut()
            && said.as_ref() != Some(&reply)
        {
            *said = Some(reply);
            self.changes += 1;
        }
    }

    pub fn completion(&self) -> Option<&Entry> {
        self.entries
            .back()
            .filter(|entry| matches!(entry.step, Step::Ended { .. }))
    }

    pub fn review(&mut self, at: Instant, review: Review) -> bool {
        let Some(entry) = self.entries.back_mut().filter(|entry| {
            entry.at == at && matches!(entry.step, Step::Ended { failed: false, .. })
        }) else {
            return false;
        };
        if entry.review == review || (review == Review::Viewed && entry.review != Review::Unseen) {
            return false;
        }
        entry.review = review;
        self.changes += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn viewing_never_accepts_and_duplicate_ends_preserve_review() {
        let now = Instant::now();
        let mut log = Timeline::default();
        log.record("Stop", (None, Some("done"), None), Some(now), now);
        assert!(log.review(now, Review::Viewed));
        assert_eq!(log.completion().unwrap().review, Review::Viewed);
        log.record("agent-turn-complete", (None, None, None), None, now);
        assert_eq!(log.completion().unwrap().review, Review::Viewed);
        assert!(log.review(now, Review::Accepted));
        assert!(!log.review(now, Review::Viewed));
        assert!(log.review(now, Review::Unseen));
    }

    #[test]
    fn old_result_actions_cannot_accept_new_work_or_failures() {
        let now = Instant::now();
        let later = now + Duration::from_secs(1);
        let mut log = Timeline::default();
        log.record("Stop", (None, None, None), Some(now), now);
        log.record("UserPromptSubmit", (None, Some("next"), None), None, later);
        assert!(log.completion().is_none());
        assert!(!log.review(now, Review::Accepted));
        log.record("Stop", (None, None, None), Some(later), later);
        assert!(!log.review(now, Review::Accepted));
        assert_eq!(log.completion().unwrap().review, Review::Unseen);
        log.record("StopFailure", (None, None, None), Some(later), later);
        assert!(!log.review(later, Review::Accepted));
    }
}
