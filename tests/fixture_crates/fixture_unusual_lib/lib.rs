pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

pub fn in_lib() {
    println!("in lib");
}

#[cfg(not(feature = "weird_lib"))]
struct WeirdLibStruct;

#[cfg(not(feature = "weird_lib"))]
pub fn function_in_common_file() -> &'static str {
    "Hello from lib.rs outside src"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }

    #[test]
    fn is_it_uncommon() {
        common_file::function_in_uncommon_file();
    }
}

#[cfg_attr(feature = "weird_lib", path = "src/mod.rs")]
#[cfg_attr(not(feature = "weird_lib"), path = "../common_file.rs")]
mod common_file;

pub use common_file::*;
