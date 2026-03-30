//! Minimal valid-Rust repro: two `fn` bodies in the same `impl` each declare a local
//! `static EMPTY` with the same name (mirrors Graphite `document_metadata.rs`).

pub struct Document;

impl Document {
    pub fn layer_outline(&self) -> impl Iterator<Item = u32> {
        static EMPTY: Vec<u32> = Vec::new();
        let _ = &EMPTY;
        core::iter::empty()
    }

    pub fn layer_with_free_points_outline(&self) -> impl Iterator<Item = u32> {
        static EMPTY: Vec<u32> = Vec::new();
        let _ = &EMPTY;
        core::iter::empty()
    }
}

pub fn exercise_fixture() -> usize {
    let d = Document;
    d.layer_outline().count() + d.layer_with_free_points_outline().count()
}
