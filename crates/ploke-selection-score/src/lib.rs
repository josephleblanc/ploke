//! Experimental scoring mechanisms for Ploke selection policies.
//!
//! This crate keeps the mechanisms small and pure so they can be replayed against
//! candidate-history snapshots before any selector is promoted into Ploke proper.

pub mod catalog;
pub mod common;
pub mod papers;
pub mod ploke;
pub mod protocols;

pub use catalog::{Exactness, MechanismKind, MechanismSpec, SourceRef};
pub use common::{ScoreError, normalize_weights, rank_quality};
pub use papers::raser::{
    Route, RouteChoice, RouteKind, RoutePrediction, bridgeable_label, choose_route, raser2_select,
    raser3_score, raser3_select, route_score,
};
pub use ploke::evidence::{EvalEvidence, Evidence, Lane, eval_gate, evidence_gate, reliability};
pub use ploke::frontier::{FrontierConfig, frontier_weights};
