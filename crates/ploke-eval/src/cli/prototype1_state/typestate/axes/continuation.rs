use std::marker::PhantomData;

use super::super::Private;

pub(crate) mod selection {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Maybe<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Selected<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod decision {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Stopped<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Allowed<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod handoff {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Recorded<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}
