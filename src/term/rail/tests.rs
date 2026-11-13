use super::{Place, current_turn};

#[test]
fn current_turn_follows_scrollback_and_skips_missing_prompts() {
    let places = [
        Place::Gone,
        Place::At(-100),
        Place::Lost,
        Place::At(-30),
        Place::At(5),
    ];
    let current = |offset| current_turn(places.into_iter(), offset);
    assert_eq!(current(150), Some(1));
    assert_eq!(current(101), Some(1));
    assert_eq!(current(50), Some(1));
    assert_eq!(current(31), Some(3));
    assert_eq!(current(0), Some(4));
}

#[test]
fn unlocated_turns_do_not_claim_the_current_position() {
    assert_eq!(
        current_turn([Place::Gone, Place::Seeking, Place::Lost].into_iter(), 0),
        None
    );
    assert_eq!(current_turn([].into_iter(), 10), None);
}
