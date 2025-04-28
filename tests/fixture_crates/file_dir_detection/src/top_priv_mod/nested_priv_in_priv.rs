// Corresponds to `mod nested_priv_in_priv;` in top_priv_mod.rs

// Public function, but only visible within `top_priv_mod`
pub fn nested_priv_pub_func() {}

fn nested_priv_priv_func() {}
