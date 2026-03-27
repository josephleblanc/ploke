//! Two imports that share the leaf name `Result` but use different paths (`std::io` vs `std::fmt`).
//! Placed in separate modules so both `use` lines are valid Rust.

mod io_user {
    use std::io::Result;

    pub fn _f() -> Result<()> {
        Ok(())
    }
}

mod fmt_user {
    use std::fmt::Result;

    pub fn _g() -> Result {
        Ok(())
    }
}
