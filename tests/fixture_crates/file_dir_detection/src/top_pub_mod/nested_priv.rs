// Corresponds to `mod nested_priv;` in top_pub_mod.rs

// Public function, but only visible within `top_pub_mod` due to private module
pub fn nested_priv_pub_func() {}

fn nested_priv_priv_func() {}
