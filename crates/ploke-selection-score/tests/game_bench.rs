use ploke_selection_score::protocols::game_bench::{self, Status};

#[test]
fn status_prioritizes_format_error_and_submit_outcomes() {
    assert_eq!(
        game_bench::status(false, true, true, 0, 3),
        Status::FormatError
    );
    assert_eq!(game_bench::status(true, true, true, 0, 3), Status::Success);
    assert_eq!(game_bench::status(true, true, false, 0, 3), Status::Failure);
    assert_eq!(
        game_bench::status(true, false, false, 2, 3),
        Status::Continue
    );
    assert_eq!(
        game_bench::status(true, false, false, 3, 3),
        Status::Timeout
    );
}

#[test]
fn average_success_turns_uses_only_successful_episodes() {
    let status = [
        Status::Success,
        Status::Failure,
        Status::Success,
        Status::Timeout,
    ];
    let turns = [2, 9, 6, 10];

    let avg = game_bench::average_success_turns(&status, &turns)
        .expect("successful episodes should have average turns");

    assert!((avg - 4.0).abs() < 1e-12);
}

#[test]
fn average_success_turns_rejects_empty_mismatched_or_no_success_inputs() {
    assert!(game_bench::average_success_turns(&[], &[]).is_none());
    assert!(game_bench::average_success_turns(&[Status::Success], &[]).is_none());
    assert!(game_bench::average_success_turns(&[Status::Failure], &[3]).is_none());
}

#[test]
fn efficiency_combines_success_rate_and_average_turns() {
    let status = [
        Status::Success,
        Status::Failure,
        Status::Success,
        Status::Timeout,
    ];
    let success_rate = game_bench::success_rate(&status).expect("nonempty status set should score");
    let avg_turns =
        game_bench::average_success_turns(&status, &[2, 9, 6, 10]).expect("successes exist");
    let efficiency =
        game_bench::efficiency(success_rate, avg_turns).expect("positive turns should score");

    assert!((success_rate - 0.5).abs() < 1e-12);
    assert!((efficiency - 0.125).abs() < 1e-12);
}
