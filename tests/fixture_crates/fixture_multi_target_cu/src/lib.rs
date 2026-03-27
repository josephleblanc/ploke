pub struct LibStruct;

pub fn lib_only() -> LibStruct {
    LibStruct
}

#[cfg(feature = "extra")]
pub fn only_with_extra_feature() -> &'static str {
    "extra"
}

pub struct BinOrLib;

pub fn bin_or_lib() -> BinOrLib {
    println!("bin_or_lib from lib.rs scoped ");
    BinOrLib
}

mod private_lib {
    // leads to error
    // fn semi_private_lib() -> BinStruct {}
}
