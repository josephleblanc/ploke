//! Small algebraic carriers for projections, fibers, witnesses, and pullbacks.
//!
//! These traits are intentionally independent of Prototype 1 History, journals,
//! reports, or interventions. They name reusable structure:
//! - a projection maps a source into an image and declares its kernel;
//! - a fiber is the inverse image of one projected value;
//! - a witness selects a concrete source representative from that fiber;
//! - a pullback joins independently produced values only when their keys agree.
//!
//! Domain-specific modules decide what counts as admissible evidence, authority,
//! or History. This module only provides the algebraic shape those decisions can
//! depend on.

mod projection;
mod pullback;

pub use projection::{
    Fiber, Projected, Projection, Provenance, ReadSurface, ResolveWith, VerifiableWith, Witnessed,
};
pub use pullback::{Keyed, Pullback, PullbackError};
