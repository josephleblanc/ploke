//! Pullback-like composition over shared keys.

/// A value with a key used for compatibility-preserving composition.
pub trait Keyed<Key> {
    fn key(&self) -> &Key;
}

/// Failure to construct a pullback because the two sides do not agree in the
/// shared key domain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullbackError<Key> {
    left: Key,
    right: Key,
}

impl<Key> PullbackError<Key> {
    pub fn new(left: Key, right: Key) -> Self {
        Self { left, right }
    }

    pub fn left(&self) -> &Key {
        &self.left
    }

    pub fn right(&self) -> &Key {
        &self.right
    }

    pub fn into_parts(self) -> (Key, Key) {
        (self.left, self.right)
    }
}

/// Pair of independently produced values that agree over a shared key.
///
/// This is a small, practical carrier for the pullback idea:
///
/// `left` and `right` may come from different surfaces, but the constructor
/// admits the pair only if both map to the same key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pullback<Left, Right, Key> {
    left: Left,
    right: Right,
    key: Key,
}

impl<Left, Right, Key> Pullback<Left, Right, Key> {
    pub fn try_new(left: Left, right: Right) -> Result<Self, PullbackError<Key>>
    where
        Left: Keyed<Key>,
        Right: Keyed<Key>,
        Key: Clone + Eq,
    {
        let left_key = left.key().clone();
        let right_key = right.key().clone();
        if left_key != right_key {
            return Err(PullbackError::new(left_key, right_key));
        }

        Ok(Self {
            left,
            right,
            key: left_key,
        })
    }

    pub fn left(&self) -> &Left {
        &self.left
    }

    pub fn right(&self) -> &Right {
        &self.right
    }

    pub fn key(&self) -> &Key {
        &self.key
    }

    pub fn into_parts(self) -> (Left, Right, Key) {
        (self.left, self.right, self.key)
    }
}

#[cfg(test)]
mod tests {
    use super::{Keyed, Pullback};

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Row {
        key: &'static str,
        value: &'static str,
    }

    impl Keyed<&'static str> for Row {
        fn key(&self) -> &&'static str {
            &self.key
        }
    }

    #[test]
    fn pullback_requires_matching_keys() {
        let left = Row {
            key: "runtime-1",
            value: "journal",
        };
        let right = Row {
            key: "runtime-1",
            value: "evaluation",
        };

        let joined = Pullback::try_new(left, right).unwrap();
        assert_eq!(joined.key(), &"runtime-1");
        assert_eq!(joined.left().value, "journal");
        assert_eq!(joined.right().value, "evaluation");
    }

    #[test]
    fn pullback_rejects_mismatched_keys() {
        let left = Row {
            key: "runtime-1",
            value: "journal",
        };
        let right = Row {
            key: "runtime-2",
            value: "evaluation",
        };

        let err = Pullback::try_new(left, right).unwrap_err();
        assert_eq!(err.left(), &"runtime-1");
        assert_eq!(err.right(), &"runtime-2");
    }
}
