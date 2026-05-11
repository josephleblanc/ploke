//! egui-facing projection and interaction modules.
//!
//! The UI layer is deliberately split from `graph`: `graph` owns the semantic
//! execution object, while these modules own view state, layout, and the eframe
//! application shell.

pub mod app;
pub mod view;
