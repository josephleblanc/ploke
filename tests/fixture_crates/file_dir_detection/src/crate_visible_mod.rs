// Corresponds to `pub(crate) mod crate_visible_mod;` in main.rs

pub fn crate_vis_func() {}

// Nested private module
mod nested_priv {
    fn func() {}
}

// Nested crate visible module
pub(crate) mod nested_crate_vis {
    pub fn func() {}
}
