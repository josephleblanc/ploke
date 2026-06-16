use std::marker::PhantomData;

use super::super::Private;

pub(crate) mod authority {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Received<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod schedule {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Ready<Budget, Mode> {
        _budget: PhantomData<Budget>,
        _mode: PhantomData<Mode>,
        _private: Private,
    }
}
