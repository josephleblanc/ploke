use ploke_selection_score::papers::axiom::{self, Outcome};

#[test]
fn trust_score_counts_wrong_attempts_over_attempted_non_abstentions() {
    assert_eq!(axiom::trust_score(1, 4), Some(0.75));
    assert_eq!(axiom::trust_score(4, 4), Some(0.0));
    assert!(axiom::trust_score(0, 0).is_none());
    assert!(axiom::trust_score(5, 4).is_none());
}

#[test]
fn axiom_decide_abstains_unless_route_translate_and_verify_all_hold() {
    assert_eq!(axiom::decide(true, true, true), Outcome::Verified);
    assert_eq!(axiom::decide(false, true, true), Outcome::Abstain);
    assert_eq!(axiom::decide(true, false, true), Outcome::Abstain);
    assert_eq!(axiom::decide(true, true, false), Outcome::Abstain);
}
