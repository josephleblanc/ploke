use fixture_multi_target_cu::{BinOrLib, lib_only};

pub struct BinStruct;

fn bin_only() -> BinStruct {
    let _ = lib_only();
    BinStruct
}


fn bin_or_lib() -> BinOrLib {
    println!("bin_or_lib from bin.rs scoped with with BinOrLib scoped by `use fixture_multi_target_cu::BinOrLib;`");
    BinOrLib
}

mod private {
    use crate::BinOrLib;

    pub fn semi_private_bin() -> BinOrLib {
        println!("bin_or_lib from `mod private` in semi_private_bin in bin.rs with BinOrLib scoped with `use crate::BinOrLib;`");
        BinOrLib
    }
}

fn main() {
    let _ = bin_only();
    let _ = bin_or_lib();
    let _ = fixture_multi_target_cu::bin_or_lib();
    let _ = private::semi_private_bin();
}
