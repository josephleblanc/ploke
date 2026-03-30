//! Minimal repro for duplicate synthetic `Const` IDs: same identifier in a closure body
//! and in a sibling method on the same `impl` (see Graphite `vector-types` `subpath/core.rs`).

pub struct Subpath;

impl Subpath {
    pub fn new_rounded_rectangle() {
        let new_arc = || {
            // Constant from https://pomax.github.io/bezierinfo/#circles_cubic
            const HANDLE_OFFSET_FACTOR: f64 = 0.551784777779014;
            let _ = HANDLE_OFFSET_FACTOR;
        };
        new_arc();
    }

    pub fn new_ellipse() {
        // Based on https://pomax.github.io/bezierinfo/#circles_cubic
        const HANDLE_OFFSET_FACTOR: f64 = 0.551784777779014;
        let _ = HANDLE_OFFSET_FACTOR;
    }
}

pub fn exercise_fixture() {
    Subpath::new_rounded_rectangle();
    Subpath::new_ellipse();
}
