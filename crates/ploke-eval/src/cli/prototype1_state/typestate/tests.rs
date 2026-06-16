use std::cell::Cell;

use super::{Step, transition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S0(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S1(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S2(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct S3(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestError {
    Stop,
}

#[test]
fn transition_applies_single_typed_edge() {
    let edge = transition(|state: S0| -> Result<S1, TestError> { Ok(S1(state.0 + 1)) });

    let result = edge.apply(S0(10));

    assert_eq!(result, Ok(S1(11)));
}

#[test]
fn then_composes_adjacent_typed_edges() {
    let pipeline = transition(|state: S0| -> Result<S1, TestError> { Ok(S1(state.0 + 1)) }).then(
        transition(|state: S1| -> Result<S2, TestError> { Ok(S2(state.0 * 2)) }),
    );

    let result = pipeline.apply(S0(10));

    assert_eq!(result, Ok(S2(22)));
}

#[test]
fn composed_chain_preserves_final_output() {
    let pipeline = transition(|state: S0| -> Result<S1, TestError> { Ok(S1(state.0 + 1)) })
        .then(transition(|state: S1| -> Result<S2, TestError> {
            Ok(S2(state.0 * 2))
        }))
        .then(transition(|state: S2| -> Result<S3, TestError> {
            Ok(S3(state.0 + 3))
        }));

    let result = pipeline.apply(S0(10));

    assert_eq!(result, Ok(S3(25)));
}

#[test]
fn chain_short_circuits_on_error() {
    let ran_second = Cell::new(false);
    let pipeline = transition(|_state: S0| -> Result<S1, TestError> { Err(TestError::Stop) }).then(
        transition(|state: S1| -> Result<S2, TestError> {
            ran_second.set(true);
            Ok(S2(state.0))
        }),
    );

    let result = pipeline.apply(S0(10));

    assert_eq!(result, Err(TestError::Stop));
    assert!(!ran_second.get());
}
