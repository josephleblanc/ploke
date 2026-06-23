use std::marker::PhantomData;

use super::super::Private;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct None {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity<T> {
    _carrier: PhantomData<T>,
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Facts {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Emitted<T> {
    _carrier: PhantomData<T>,
    _private: Private,
}
