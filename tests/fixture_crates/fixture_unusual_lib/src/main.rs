#[cfg(not(feature = "weird_lib"))]
use fixture_unusual_lib::function_in_common_file;

#[cfg(feature = "weird_lib")]
use fixture_unusual_lib::{function_in_uncommon_file};

fn main() {
    in_bin();

    #[cfg(not(feature = "weird_lib"))]
    println!("{}", function_in_common_file());

    #[cfg(feature = "weird_lib")]
    function_in_uncommon_file();
}

struct BinStruct;

pub fn in_bin() {
    println!("Hello from bin")
}
