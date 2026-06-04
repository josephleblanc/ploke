//! 2606.02488 — RASER recoverability-aware routing.

/// Paper route/action labels for RASER.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteKind {
    OneShotRag,
    Prune,
    IrcotStar,
}

/// RASER-3 route-specific prediction available at inference time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoutePrediction {
    pub route: RouteKind,
    pub predicted_f1: f64,
    pub cost: f64,
}

/// Selected RASER-3 route plus its cost-adjusted score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteChoice {
    pub index: usize,
    pub route: RouteKind,
    pub score: f64,
}

/// Candidate evidence route with predicted benefit and cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    pub benefit: f64,
    pub cost: f64,
}

/// Scores one candidate RASER route as predicted answer quality minus cost.
///
/// This is the scalar term inside the RASER-3 decision rule from
/// "RASER: Recoverability-Aware Selective Escalation Router for Multi-Hop
/// Question Answering" (arXiv:2606.02488). In the paper, the router first runs
/// a cheap one-shot RAG pass and builds a small feature state from information
/// that already exists after that pass: the draft answer, retrieval similarity
/// scores, bridge-like question cues, and a simple question type. For the
/// three-action router, RASER trains one lightweight predictor per route. Each
/// predictor estimates the answer F1 that would be obtained if the current
/// question state were sent to that route: plain one-shot RAG, the PRUNE bridge
/// route, or the iterative IRCoT* route.
///
/// Given those route-specific predictions, RASER-3 chooses the route with the
/// largest value of
///
/// ```text
/// predicted_f1(route, state) - lambda * route_cost(route)
/// ```
///
/// This helper implements exactly that inner score for one already-materialized
/// route. `Route::benefit` is the caller's predicted quality term, corresponding
/// to the paper's `f_hat_r(s)`; `Route::cost` is the token or compute cost term,
/// corresponding to `c_r`; and `lambda` is the exchange rate between predicted
/// F1 and budget. Lower `lambda` values make the selector more willing to pay
/// for expensive retrieval. Higher values require an expensive route to buy back
/// more predicted F1 before it can beat the cheap one-shot route. The paper
/// treats this as a cost/accuracy dial rather than as one fixed universal score:
/// sweeping `lambda` traces a cost-F1 frontier.
///
/// The paper distinguishes this RASER-3 score from the simpler RASER-2 router.
/// RASER-2 is a binary classifier that runs PRUNE only when
/// `p(BRIDGEABLE | state)` exceeds a threshold; its training label is positive
/// when PRUNE improves one-shot RAG by more than a small F1 margin. RASER-3
/// generalizes that idea from "should we escalate?" to "which of several routes
/// is worth its cost?" by comparing predicted F1 after subtracting route cost.
///
/// Reported performance claims should be read with that budget-aware framing.
/// Across six LLMs and three multi-hop QA benchmarks, the paper reports that
/// RASER keeps F1 competitive with more expensive always-escalate baselines
/// while spending substantially fewer tokens. In particular, RASER-2 is reported
/// to stay close to always-PRUNE while using roughly 39-57% of always-PRUNE's
/// token budget across the main comparisons, and the abstract summarizes the
/// overall token range as 41-49% of always-prune tokens. RASER-3 is presented as
/// a higher-budget operating point when iterative retrieval is useful: on
/// smaller or mid-tier LLMs where IRCoT* adds real F1, it captures much of the
/// iterative-route benefit while spending far fewer tokens than always-IRCoT*.
/// The paper's ablations also argue that the routing decision is mostly driven
/// by cheap retrieval-quality features (`score_gap` and `score_top1` dominate
/// feature importance), and that simple GBM predictors are competitive with
/// heavier alternatives such as XGBoost, Ridge, logistic regression, and MLP
/// variants.
///
/// This crate intentionally keeps the executable surface narrow: it does not
/// train the predictors, estimate feature state, normalize token costs, or
/// encode the paper's benchmark tables. It exposes the source-supported
/// threshold, label, score, and argmax rules so Ploke can replay the same kind
/// of recoverability-aware selector against already-materialized candidate
/// routes.
pub fn route_score(route: &Route, lambda: f64) -> Option<f64> {
    if !lambda.is_finite() || !route.benefit.is_finite() || !route.cost.is_finite() {
        None
    } else {
        Some(route.benefit - lambda * route.cost)
    }
}

/// Chooses the route maximizing `benefit - lambda * cost`.
pub fn choose_route(route: &[Route], lambda: f64) -> Option<usize> {
    if route.is_empty() || !lambda.is_finite() {
        return None;
    }
    if route
        .iter()
        .any(|item| !item.benefit.is_finite() || !item.cost.is_finite())
    {
        return None;
    }

    route
        .iter()
        .enumerate()
        .max_by(|left, right| {
            let left_score = route_score(left.1, lambda).expect("finite route");
            let right_score = route_score(right.1, lambda).expect("finite route");
            left_score.total_cmp(&right_score)
        })
        .map(|(idx, _)| idx)
}

/// Returns the RASER-2 bridgeability training label.
///
/// The paper labels a sample bridgeable when PRUNE improves one-shot RAG by more
/// than the margin `tau`, using `y = I[F1_PRUNE - F1_ONE_SHOT_RAG > tau]`.
/// Observed F1 values and `tau` must be finite values in `[0, 1]`.
pub fn bridgeable_label(prune_f1: f64, one_shot_f1: f64, tau: f64) -> Option<bool> {
    if !unit_value(prune_f1) || !unit_value(one_shot_f1) || !unit_value(tau) {
        return None;
    }

    Some(prune_f1 - one_shot_f1 > tau)
}

/// Applies the RASER-2 inclusive bridgeability threshold.
///
/// `prob` is the binary classifier's `p(BRIDGEABLE | state)`. When it is at
/// least `threshold`, RASER-2 escalates to PRUNE; otherwise it keeps one-shot
/// RAG. Both values must be finite probabilities in `[0, 1]`.
pub fn raser2_select(prob: f64, threshold: f64) -> Option<RouteKind> {
    if !unit_value(prob) || !unit_value(threshold) {
        return None;
    }

    if prob >= threshold {
        Some(RouteKind::Prune)
    } else {
        Some(RouteKind::OneShotRag)
    }
}

/// Scores one RASER-3 route prediction with the paper's cost-aware objective.
///
/// The regressor output is allowed to be any finite value because the paper's
/// worked example includes a negative predicted F1 for one-shot RAG. Costs and
/// `lambda` must be finite and non-negative because they represent a token/compute
/// price and its F1 exchange rate.
pub fn raser3_score(route: &RoutePrediction, lambda: f64) -> Option<f64> {
    if !route.predicted_f1.is_finite() || !nonnegative(route.cost) || !nonnegative(lambda) {
        return None;
    }

    Some(route.predicted_f1 - lambda * route.cost)
}

/// Selects the RASER-3 route maximizing `predicted_f1 - lambda * cost`.
///
/// Ties keep the earliest route in the provided slice, matching the crate's
/// generic finite-argmax tie policy. The function expects already-computed route
/// predictions; it does not train or run the GBM regressors from the paper.
pub fn raser3_select(route: &[RoutePrediction], lambda: f64) -> Option<RouteChoice> {
    if route.is_empty() {
        return None;
    }

    let mut best: Option<RouteChoice> = None;
    for (index, item) in route.iter().enumerate() {
        let score = raser3_score(item, lambda)?;
        if best.is_none_or(|choice| score > choice.score) {
            best = Some(RouteChoice {
                index,
                route: item.route,
                score,
            });
        }
    }

    best
}

fn unit_value(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
