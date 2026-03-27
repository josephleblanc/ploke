pub struct LibStruct;

pub fn lib_only() -> LibStruct {
    LibStruct
}

#[cfg(feature = "extra")]
pub fn only_with_extra_feature() -> &'static str {
    "extra"
}
