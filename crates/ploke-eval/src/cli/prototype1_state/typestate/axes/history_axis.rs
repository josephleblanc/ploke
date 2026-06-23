use std::marker::PhantomData;

use super::super::Private;

pub(crate) mod startup {
    use super::{PhantomData, Private};

    /// Startup/admission has not been observed by this path.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    /// Current live startup is in progress or not yet validated.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Pending {
        _private: Private,
    }

    /// Current gen0 setup produced checkout identity only. It did not create
    /// the intended genesis History block.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Identity<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    /// Current live startup validated enough to enter `Parent<Ready>`.
    /// This is not yet the final `Parent<Ruling>` model.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Validated<Kind> {
        _kind: PhantomData<Kind>,
        _private: Private,
    }

    /// Branch kind intentionally no longer tracked after convergence.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum Any {}
}

pub(crate) mod head {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Unobserved {
        _private: Private,
    }

    /// Startup validation consumed whatever head/absence observation it
    /// needed; no separate `LineageState` carrier remains in the live path.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct FromStartup {
        _private: Private,
    }

    /// History was read for candidate/selection purposes.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Read {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Observed<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Unchanged {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Advanced<T> {
        _carrier: PhantomData<T>,
        _private: Private,
    }
}

pub(crate) mod epoch {
    use super::{PhantomData, Private};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct None {
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Open<Block, Crown> {
        _block: PhantomData<Block>,
        _crown: PhantomData<Crown>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Locked<Block, Crown> {
        _block: PhantomData<Block>,
        _crown: PhantomData<Crown>,
        _private: Private,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Sealed<Block> {
        _block: PhantomData<Block>,
        _private: Private,
    }
}
