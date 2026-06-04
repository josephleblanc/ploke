//! 2606.02488 — RASER recoverability-aware routing.

/// Candidate evidence route with predicted benefit and cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Route {
    pub benefit: f64,
    pub cost: f64,
}

/// Scores a route as `benefit - lambda * cost`.
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
