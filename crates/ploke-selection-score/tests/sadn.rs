use ploke_selection_score::papers::sadn;

#[test]
fn advantage_update_matches_paper_td_form() {
    let updated = sadn::advantage_update(0.2, 1.0, 0.9, 0.8, 0.5, 0.1)
        .expect("finite SADN update inputs should score");

    assert!((updated - 0.302).abs() < 1e-12);
}

#[test]
fn partial_advantage_subtracts_prior_prefix_value() {
    let advantage = sadn::partial_advantage(4.5, 3.0)
        .expect("finite partial Q values should produce an advantage");

    assert!((advantage - 1.5).abs() < 1e-12);
}

#[test]
fn global_advantage_sums_ordered_agent_advantages() {
    let advantage =
        sadn::global_advantage(&[0.25, -0.5, 1.25]).expect("finite lane advantages should sum");

    assert!((advantage - 1.0).abs() < 1e-12);
}

#[test]
fn discounted_return_sums_shared_rewards() {
    let value = sadn::discounted_return(&[2.0, 3.0, 5.0], 0.5)
        .expect("finite rewards and gamma should produce a value");

    assert!((value - 4.75).abs() < 1e-12);
}

#[test]
fn expected_cost_weights_target_algorithm_cost_by_instance_distribution() {
    let cost = sadn::expected_cost(&[0.25, 0.75], &[10.0, 2.0])
        .expect("finite probability distribution and costs should score");

    assert!((cost - 4.0).abs() < 1e-12);
}

#[test]
fn sequential_greedy_chooses_by_prefix_conditioned_advantage() {
    let chosen = sadn::sequential_greedy(&[2, 3, 2], |agent, prefix, action| match agent {
        0 => [0.1, 0.9].get(action).copied(),
        1 if prefix == [1] => [None, Some(0.4), Some(0.8)][action],
        2 if prefix == [1, 2] => [0.3, 0.2].get(action).copied(),
        _ => None,
    })
    .expect("each ordered agent has at least one finite conditional action");

    assert_eq!(chosen, vec![1, 2, 0]);
}

#[test]
fn sequential_greedy_keeps_earliest_action_on_ties() {
    let chosen = sadn::sequential_greedy(&[3], |_agent, _prefix, _action| Some(1.0))
        .expect("tie still has finite actions");

    assert_eq!(chosen, vec![0]);
}

#[test]
fn sequential_greedy_rejects_empty_or_nonfinite_conditional_domains() {
    assert!(sadn::sequential_greedy(&[], |_agent, _prefix, _action| Some(0.0)).is_none());
    assert!(sadn::sequential_greedy(&[0], |_agent, _prefix, _action| Some(0.0)).is_none());
    assert!(sadn::sequential_greedy(&[2], |_agent, _prefix, _action| Some(f64::NAN)).is_none());
}
