use std::marker::PhantomData;

use super::super::Private;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unknown {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Parent {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Child {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Successor {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unresolved {
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Identity<T> {
    _carrier: PhantomData<T>,
    _private: Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Initialized<T> {
    _carrier: PhantomData<T>,
    _private: Private,
}
