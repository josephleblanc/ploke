//! Minimal crate for cfg-based compilation-unit mask tests.

pub fn always_present() -> u32 {
    1
}

#[cfg(feature = "foo")]
pub fn only_when_foo() -> u32 {
    2
}
