# WASM Protocol Dashboard Plan

This directory contains the gated implementation plan for making `ploke-egui` the browser/WASM-facing Prototype 1 protocol/debug frontend.

- [`main-plan.md`](main-plan.md): task breakdown, gate matrix, and execution workflow.

The plan deliberately treats `ploke-tree-egui` and `PlaybackBrowserModel` as prior art only. The implementation source boundary is `&ploke_tree::Graph` plus typed graph-owned projections.
