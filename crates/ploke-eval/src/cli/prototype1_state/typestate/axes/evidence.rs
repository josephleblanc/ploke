use std::marker::PhantomData;

use super::super::Private;

pub(crate) mod parent_start {
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

pub(crate) mod baseline {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Ready<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod policy {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Ready<Policy, Budget> {
        _policy: PhantomData<Policy>,
        _budget: PhantomData<Budget>,
        _private: Private,
    }
}

pub(crate) mod selection {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    /// `PlannedChildren` is the current live bundle produced by child-plan
    /// resolution. It includes fields that the global carrier would likely
    /// split apart later.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Plan<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    /// Placeholder for private `ActiveSelectionStrategy`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Strategy {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Evidence<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Seal<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod completion {
    use super::Private;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Recorded {
        _private: Private,
    }
}
