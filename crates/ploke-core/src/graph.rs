#![cfg(feature = "ghost_data_graphs")]

use std::marker::PhantomData;

trait GraphState {}
struct Unvalidated;
impl GraphState for Unvalidated {}
struct Validated;
impl GraphState for Validated {}
struct Resolved;
impl GraphState for Resolved {}

// Generic graph structure parameterized by state
struct Graph<Nodes, Relations, State: GraphState> {
    nodes: Nodes,
    relations: Relations,
    _state: PhantomData<State>, // Zero-sized marker
}

pub enum ValidationError {
    ExampleError,
}

// Validation function changes the phantom state type
fn validate<N, R>(
    graph: Graph<N, R, Unvalidated>,
) -> Result<Graph<N, R, Validated>, ValidationError> {
    // ... perform validation on graph.nodes, graph.relations ...
    Ok(Graph {
        nodes: graph.nodes,
        relations: graph.relations,
        _state: PhantomData, // Transition to Validated state
    })
}
// Downstream function requires a validated graph
fn analyze<N, R>(graph: &Graph<N, R, Validated>) {
    // Compiler guarantees graph._state is Validated
    // ... analysis logic ...
}
