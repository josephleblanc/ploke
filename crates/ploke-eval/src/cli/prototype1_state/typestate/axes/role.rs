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
pub(crate) struct Unresolved;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity<T> {
    value: T,
    _private: Private,
}

impl<T> Identity<T> {
    pub(in crate::cli::prototype1_state::typestate) fn new(value: T) -> Self {
        Self {
            value,
            _private: Private,
        }
    }

    pub(in crate::cli::prototype1_state::typestate) fn into_inner(self) -> T {
        self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Initialized<T> {
    value: T,
    _private: Private,
}

impl<T> Initialized<T> {
    pub(in crate::cli::prototype1_state::typestate) fn new(value: T) -> Self {
        Self {
            value,
            _private: Private,
        }
    }

    pub(in crate::cli::prototype1_state::typestate) fn into_inner(self) -> T {
        self.value
    }
}
