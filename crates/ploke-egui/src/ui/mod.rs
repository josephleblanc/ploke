//! egui-facing projection and interaction modules.
//!
//! The UI layer is deliberately split from `graph`: `graph` owns the semantic
//! execution object, while these modules own view state, layout, and the eframe
//! application shell.

pub mod app;
pub(crate) mod charts;
pub mod dashboard;
pub(crate) mod diff;
pub(crate) mod eval_protocol;
pub(crate) mod id_display;
pub mod inspector;
pub(crate) mod render;
pub(crate) mod text;
pub(crate) mod theme;
pub mod view;
