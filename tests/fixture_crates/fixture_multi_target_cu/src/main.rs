use fixture_multi_target_cu::lib_only;

pub struct BinStruct;

fn bin_only() -> BinStruct {
    let _ = lib_only();
    BinStruct
}

fn main() {
    let _ = bin_only();
}
