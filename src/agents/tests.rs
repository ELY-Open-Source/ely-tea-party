use std::time::{Duration, Instant};

use super::*;

fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}

#[test]
fn an_unhooked_agent_works_while_it_draws() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Claude, start);
    assert!(agent.guess(start + secs(1), start + secs(1)));
    assert_eq!(agent.status, Status::Working);
    assert!(
        !agent.guess(start + secs(1), start + secs(2)),
        "still within the window"
    );
    assert!(agent.guess(start + secs(1), start + secs(4)), "quiet: idle");
    assert_eq!(agent.status, Status::Idle);
    assert_eq!(agent.since, start + secs(1), "idle since it last drew");
}

#[test]
fn a_bell_waits_for_the_next_output() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Gpt, start);
    agent.set(Status::NeedsInput, start + secs(5));
    assert!(
        !agent.guess(start + secs(4), start + secs(30)),
        "older output keeps it waiting"
    );
    assert!(agent.guess(start + secs(31), start + secs(31)));
    assert_eq!(agent.status, Status::Working);
}

#[test]
fn a_hooked_agent_ignores_output() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Claude, start);
    agent.hooked = true;
    assert!(!agent.guess(start, start));
    assert_eq!(agent.status, Status::Idle);
}

#[test]
fn poses_follow_status_and_quiet() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Kimi, start);
    assert_eq!(agent.pose(start, 0), Pose::Idle);
    assert_eq!(agent.pose(start + secs(601), 0), Pose::Sleeping);
    agent.hooked = true;
    agent.set(Status::Working, start);
    assert_eq!(agent.pose(start + secs(1), 0), Pose::Working);
    assert_eq!(agent.pose(start + secs(7), 0), Pose::Thinking);
    assert_eq!(agent.pose(start + secs(901), 0), Pose::Idle, "a lost Stop");
    agent.set(Status::NeedsInput, start);
    assert_eq!(agent.pose(start + secs(3600), 0), Pose::NeedsInput);
}

#[test]
fn working_poses_turn_and_sessions_stagger() {
    let start = Instant::now();
    let mut agent = Agent::new(Kind::Pi, start);
    agent.set(Status::Working, start);
    assert_eq!(agent.pose(start, 0), Pose::Working);
    assert_eq!(agent.pose(start + secs(15), 0), Pose::Research);
    assert_eq!(agent.pose(start, 1), Pose::Research);
}

#[test]
fn every_pose_has_a_label() {
    for kind in Kind::ALL {
        for pose in Pose::ALL {
            assert!(!kind.label(pose).is_empty());
        }
    }
    assert_eq!(Kind::named("Codex"), Some(Kind::Gpt));
}

#[test]
fn elapsed_time_reads_short() {
    assert_eq!(ago(secs(12)), "12s");
    assert_eq!(ago(secs(300)), "5m");
    assert_eq!(ago(secs(3 * 3600)), "3h");
    assert_eq!(ago(secs(2 * 86400)), "2d");
}
