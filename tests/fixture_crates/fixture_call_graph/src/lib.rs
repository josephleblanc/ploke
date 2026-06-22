//! Focused fixture for call-graph syntax forms that are absent from the larger parser fixtures.

pub fn dynamic_calls() -> i32 {
    let closure = || 7;
    let from_binding = (closure)();
    let from_literal = (|| 11)();
    from_binding + from_literal
}

pub fn local_target() -> i32 {
    3
}

pub fn call_crate_local_target() -> i32 {
    crate::local_target()
}

pub mod local_mod {
    pub fn nested_target() -> i32 {
        5
    }

    pub fn call_self_nested_target() -> i32 {
        self::nested_target()
    }
}
