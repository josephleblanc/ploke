pub struct Initializer;

impl Initializer {
    pub fn init_array(&self, use_zero: bool) -> u64 {
        enum InitializationKind {
            ZeroedConst,
            InRegisterValue(u64),
        }

        let init_kind = if use_zero {
            InitializationKind::ZeroedConst
        } else {
            InitializationKind::InRegisterValue(1)
        };

        match init_kind {
            InitializationKind::ZeroedConst => 0,
            InitializationKind::InRegisterValue(value) => value,
        }
    }

    pub fn init_slice(&self, use_zero: bool) -> u64 {
        enum InitializationKind {
            ZeroedConst,
            InRegisterValue(u64),
            ElemByElem,
        }

        let init_kind = if use_zero {
            InitializationKind::ZeroedConst
        } else if self.init_array(false) > 1 {
            InitializationKind::ElemByElem
        } else {
            InitializationKind::InRegisterValue(2)
        };

        match init_kind {
            InitializationKind::ZeroedConst => 0,
            InitializationKind::InRegisterValue(value) => value,
            InitializationKind::ElemByElem => 3,
        }
    }
}

pub fn exercise_fixture() -> u64 {
    let initializer = Initializer;
    initializer.init_array(false) + initializer.init_slice(false)
}
