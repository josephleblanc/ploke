use ploke_selection_score::{
    EvalEvidence, Evidence, FrontierConfig, Lane, Route, choose_route, eval_gate, evidence_gate,
    frontier_weights, normalize_weights, rank_quality,
};

#[test]
fn required_gate_rejects_missing_lane() {
    let req = [Lane::Operational, Lane::Oracle];
    let mut set = std::collections::BTreeMap::new();
    set.insert(Lane::Operational, Evidence::ready());

    assert!(!evidence_gate(&req, &set));

    set.insert(Lane::Oracle, Evidence::ready());

    assert!(evidence_gate(&req, &set));
}

#[test]
fn required_gate_rejects_invalid_lane() {
    let req = [Lane::Operational];
    let mut set = std::collections::BTreeMap::new();
    set.insert(
        Lane::Operational,
        Evidence {
            present: true,
            matched: true,
            valid: false,
            replayable: true,
        },
    );

    assert!(!evidence_gate(&req, &set));
}

#[test]
fn rank_quality_maps_order_to_percentiles() {
    let score = [100.0, 40.0, 40.0, 0.0];

    let rank = rank_quality(&score).expect("finite scores should rank");

    assert_eq!(rank, vec![1.0, 0.5, 0.5, 0.0]);
}

#[test]
fn rank_quality_uses_midpoint_for_all_ties() {
    let score = [7.0, 7.0, 7.0];

    let rank = rank_quality(&score).expect("finite scores should rank");

    assert_eq!(rank, vec![0.5, 0.5, 0.5]);
}

#[test]
fn frontier_weights_favor_quality_around_midpoint() {
    let cfg = FrontierConfig {
        top_m: 2,
        lambda: 10.0,
    };
    let qual = [1.0, 0.5, 0.0];
    let child = [0, 0, 0];

    let weight = frontier_weights(&qual, &child, cfg).expect("valid vectors should score");
    let prob = normalize_weights(&weight);

    assert!(weight[0] > weight[1]);
    assert!(weight[1] > weight[2]);
    assert!((prob.iter().sum::<f64>() - 1.0).abs() < 1e-12);
}

#[test]
fn frontier_weights_dampen_existing_children() {
    let cfg = FrontierConfig {
        top_m: 1,
        lambda: 10.0,
    };
    let qual = [0.8, 0.8];
    let child = [0, 3];

    let weight = frontier_weights(&qual, &child, cfg).expect("valid vectors should score");

    assert!((weight[0] / weight[1] - 4.0).abs() < 1e-12);
}

#[test]
fn evaluator_gate_requires_reliable_versioned_evidence() {
    let ev = EvalEvidence {
        success: 3,
        failure: 1,
        prior_success: 1,
        prior_failure: 1,
        versioned: true,
        replayable: true,
        calibrated: true,
    };

    assert!(eval_gate(&ev, 0.6));

    let stale = EvalEvidence {
        versioned: false,
        ..ev
    };

    assert!(!eval_gate(&stale, 0.6));
}

#[test]
fn route_choice_uses_benefit_minus_cost() {
    let routes = [
        Route {
            benefit: 0.4,
            cost: 0.1,
        },
        Route {
            benefit: 0.9,
            cost: 1.0,
        },
        Route {
            benefit: 0.6,
            cost: 0.2,
        },
    ];

    let choice = choose_route(&routes, 0.5).expect("nonempty route set should choose");

    assert_eq!(choice, 2);
}
