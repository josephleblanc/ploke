//! Focused fixture for call-graph syntax forms that are absent from the larger parser fixtures.

pub fn dynamic_calls() -> i32 {
    let closure = || 7;
    let from_binding = (closure)();
    let from_literal = (|| 11)();
    from_binding + from_literal
}
