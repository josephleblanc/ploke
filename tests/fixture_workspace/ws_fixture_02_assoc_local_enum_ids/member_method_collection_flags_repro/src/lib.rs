pub struct Vm;

impl Vm {
    fn inherit_patma_flags() {
        const COLLECTION_FLAGS: u32 = 1;
        let _ = COLLECTION_FLAGS;
    }

    fn check_abc_tpflags() {
        const COLLECTION_FLAGS: u32 = 2;
        let _ = COLLECTION_FLAGS;
    }
}

pub fn exercise_fixture() {
    Vm::inherit_patma_flags();
    Vm::check_abc_tpflags();
}
