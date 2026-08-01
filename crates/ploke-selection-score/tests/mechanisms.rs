use ploke_selection_score::{
    EvalEvidence, Evidence, FrontierConfig, Lane, Route, choose_route, eval_gate, evidence_gate,
    frontier_weights, normalize_weights,
    papers::raser::{self, RouteKind, RoutePrediction},
    rank_quality,
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

#[test]
fn raser2_uses_bridge_threshold() {
    assert_eq!(
        raser::raser2_select(0.5128, 0.20).expect("valid probability"),
        RouteKind::Prune
    );
    assert_eq!(
        raser::raser2_select(0.164, 0.20).expect("valid probability"),
        RouteKind::OneShotRag
    );
    assert_eq!(
        raser::raser2_select(0.20, 0.20).expect("threshold is inclusive"),
        RouteKind::Prune
    );
}

#[test]
fn bridge_label_requires_margin() {
    assert!(raser::bridgeable_label(0.62, 0.50, 0.10).expect("valid F1"));
    assert!(!raser::bridgeable_label(0.60, 0.50, 0.10).expect("strict margin"));
    assert!(raser::bridgeable_label(0.61, 0.50, 0.10).expect("valid F1"));
    assert!(raser::bridgeable_label(0.50, 0.50, -0.01).is_none());
}

#[test]
fn raser3_uses_paper_cost_example() {
    let route = [
        RoutePrediction {
            route: RouteKind::OneShotRag,
            predicted_f1: -0.04,
            cost: 1222.0,
        },
        RoutePrediction {
            route: RouteKind::Prune,
            predicted_f1: 0.33,
            cost: 3819.0,
        },
        RoutePrediction {
            route: RouteKind::IrcotStar,
            predicted_f1: 0.36,
            cost: 4315.0,
        },
    ];

    let choice = raser::raser3_select(&route, 1e-4).expect("valid route set");

    assert_eq!(choice.index, 1);
    assert_eq!(choice.route, RouteKind::Prune);
    assert!((choice.score - -0.0519).abs() < 1e-12);
    assert!((raser::raser3_score(&route[0], 1e-4).expect("valid route") - -0.1622).abs() < 1e-12);
    assert!((raser::raser3_score(&route[2], 1e-4).expect("valid route") - -0.0715).abs() < 1e-12);
}

#[test]
fn raser3_rejects_invalid_cost_inputs() {
    let route = [RoutePrediction {
        route: RouteKind::Prune,
        predicted_f1: 0.33,
        cost: -1.0,
    }];

    assert!(raser::raser3_select(&route, 1e-4).is_none());
    assert!(raser::raser3_select(&route, -1e-4).is_none());
    assert!(raser::raser3_select(&[], 1e-4).is_none());
}
