//! Build script for mock_serde
//! 
//! This build script demonstrates build-time code generation patterns
//! similar to the real serde crate.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    // Tell cargo to rerun this script only if build.rs changes
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    
    // Generate a simple private module at build time
    let private_module = r#"
#[doc(hidden)]
pub mod __private {
    #[doc(hidden)]
    pub use crate::private::*;
}
"#;

    fs::write(out_dir.join("private.rs"), private_module).unwrap();
}
