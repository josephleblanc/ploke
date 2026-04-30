//! Inner protocol carriers for cross-runtime messages.
//!
//! A `Message` is not just a persisted record. It is an obligation crossing a
//! runtime boundary: one runtime must pack it into a buffer, and another runtime
//! must read that buffer before taking a later state transition.

use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::history::{Block, HistoryError, OpenBlock, SealBlock, block};

/// Static filesystem address schema used by a cross-runtime message.
pub(crate) trait File {
    /// Runtime values needed to resolve this file address.
    type Params;

    /// Stable protocol label for this file address.
    const NAME: &'static str;

    /// Resolve this static file schema into a concrete runtime address.
    fn resolve(params: Self::Params) -> PathBuf;
}

/// One directed role/state edge.
pub(crate) trait Transition {
    /// State before this edge is crossed.
    type From;
    /// State after this edge is crossed.
    type To;
}

/// Static filesystem box for exactly one lock edge and one unlock edge.
pub(crate) trait MessageBox: File {
    /// Edge allowed to lock this box.
    type Lock: Transition;
    /// Edge allowed to unlock this box.
    type Unlock: Transition;
}

/// Crown authority state markers.
pub(crate) mod crown {
    use super::{Private, SealBlock};

    /// The current Parent may mutate the active lineage.
    #[derive(Debug)]
    pub(crate) struct Ruling {
        _private: Private,
    }

    impl Ruling {
        pub(super) fn new() -> Self {
            Self { _private: Private }
        }
    }

    /// The prior Parent has locked the Crown for successor verification.
    #[derive(Debug)]
    pub(crate) struct Locked {
        seal: SealBlock,
        _private: Private,
    }

    impl Locked {
        pub(super) fn new(seal: SealBlock) -> Self {
            Self {
                seal,
                _private: Private,
            }
        }

        pub(super) fn into_seal(self) -> SealBlock {
            self.seal
        }
    }
}

/// Exclusive authority over one lineage surface.
pub(crate) struct Crown<S> {
    lineage: LineageKey,
    state: S,
    _private: Private,
}

/// Crown-local lineage identity.
///
/// This deliberately wraps the current debug/source string so Crown code does
/// not depend on `String` as the long-term definition of lineage identity. The
/// backing value can be replaced once lineage is defined by a stronger object
/// such as an artifact/tree-backed coordinate or authenticated head key.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LineageKey {
    value: String,
}

impl fmt::Debug for LineageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("LineageKey").field(&self.value).finish()
    }
}

impl LineageKey {
    pub(super) fn from_debug_value(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub(crate) fn matches_debug_str(&self, value: &str) -> bool {
        self.value == value
    }
}

impl<S> fmt::Debug for Crown<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Crown")
            .field("lineage", &self.lineage)
            .finish_non_exhaustive()
    }
}

impl<S> Crown<S> {
    pub(crate) fn lineage_key(&self) -> &LineageKey {
        &self.lineage
    }
}

impl Crown<crown::Ruling> {
    fn for_lineage(lineage: LineageKey) -> Self {
        Self {
            lineage,
            state: crown::Ruling::new(),
            _private: Private,
        }
    }

    fn lock(self, seal: SealBlock) -> Crown<crown::Locked> {
        Crown {
            lineage: self.lineage,
            state: crown::Locked::new(seal),
            _private: Private,
        }
    }
}

impl Crown<crown::Locked> {
    pub(crate) fn into_seal_fields(self) -> SealBlock {
        self.state.into_seal()
    }
}

/// Transition that retires a selectable Parent and locks lineage authority.
///
/// This trait is implemented in this module so the raw `Crown<Ruling>`
/// constructor can stay private. Sibling modules may use the transition if they
/// hold the required role/state carrier, but they cannot mint a ruling Crown
/// from a lineage string.
pub(crate) trait LockCrown {
    type Retired;

    #[cfg(test)]
    fn lock_crown(self, seal: SealBlock) -> (Self::Retired, Crown<crown::Locked>);

    fn seal_block(
        self,
        open: OpenBlock,
        seal: SealBlock,
    ) -> Result<(Self::Retired, Block<block::Sealed>), HistoryError>;

    fn seal_block_with<F>(
        self,
        open: OpenBlock,
        seal: SealBlock,
        admit: F,
    ) -> Result<(Self::Retired, Block<block::Sealed>), HistoryError>
    where
        F: FnOnce(
            &Crown<crown::Ruling>,
            super::history::block::Claims,
        ) -> Result<super::history::block::Claims, HistoryError>;
}

impl LockCrown for super::parent::Parent<super::parent::Selectable> {
    type Retired = super::parent::Parent<super::parent::Retired>;

    #[cfg(test)]
    fn lock_crown(self, seal: SealBlock) -> (Self::Retired, Crown<crown::Locked>) {
        let (retired, lineage) = self.into_retired_and_lineage();
        (retired, Crown::for_lineage(lineage).lock(seal))
    }

    fn seal_block(
        self,
        open: OpenBlock,
        seal: SealBlock,
    ) -> Result<(Self::Retired, Block<block::Sealed>), HistoryError> {
        self.seal_block_with(open, seal, |_ruling, claims| Ok(claims))
    }

    fn seal_block_with<F>(
        self,
        open: OpenBlock,
        mut seal: SealBlock,
        admit: F,
    ) -> Result<(Self::Retired, Block<block::Sealed>), HistoryError>
    where
        F: FnOnce(
            &Crown<crown::Ruling>,
            super::history::block::Claims,
        ) -> Result<super::history::block::Claims, HistoryError>,
    {
        let (retired, lineage) = self.into_retired_and_lineage();
        let ruling = Crown::for_lineage(lineage);
        let block = ruling.open_block(open)?;
        seal.claims = admit(&ruling, seal.claims)?;
        let locked = ruling.lock(seal);
        let sealed = locked.seal(block)?;
        Ok((retired, sealed))
    }
}

#[cfg(test)]
impl Crown<crown::Locked> {
    pub(crate) fn test_locked(lineage: impl Into<String>) -> Self {
        Self::test_locked_with_seal(lineage, SealBlock::test())
    }

    pub(crate) fn test_locked_with_seal(lineage: impl Into<String>, seal: SealBlock) -> Self {
        Self {
            lineage: LineageKey::from_debug_value(lineage),
            state: crown::Locked::new(seal),
            _private: Private,
        }
    }
}

#[cfg(test)]
impl Crown<crown::Ruling> {
    pub(crate) fn test_ruling(lineage: impl Into<String>) -> Self {
        Self::for_lineage(LineageKey::from_debug_value(lineage))
    }
}

/// Static filesystem box that transfers the crown for one lineage.
pub(crate) trait LockBox: File {
    /// Lineage authority carried by this box.
    type Lineage;
    /// Edge that locks the crown into this box.
    type Lock: Transition;
    /// Edge that unlocks the crown from this box.
    type Unlock: Transition;
}

/// Concrete address for a static filesystem message location.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct At<F: File> {
    path: PathBuf,
    _file: PhantomData<F>,
    _private: Private,
}

impl<F: File> Clone for At<F> {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            _file: PhantomData,
            _private: Private,
        }
    }
}

impl<F: File> At<F> {
    pub(crate) fn resolve(params: F::Params) -> Self {
        Self {
            path: F::resolve(params),
            _file: PhantomData,
            _private: Private,
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn into_path(self) -> PathBuf {
        self.path
    }
}

impl<F: File> Serialize for At<F> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.path.serialize(serializer)
    }
}

impl<'de, F: File> Deserialize<'de> for At<F> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            path: PathBuf::deserialize(deserializer)?,
            _file: PhantomData,
            _private: Private,
        })
    }
}

/// Cross-runtime protocol message.
///
/// Implement this for records that are written by one runtime and read by
/// another runtime as a precondition for a later role/state transition.
pub(crate) trait Message {
    /// Static file identity used as the durable message box.
    type Box: MessageBox;
    /// Payload locked into and unlocked from the box.
    type Body;
    /// Role/state after packing failed.
    type SenderFailed;
    /// Error returned when a packed message is presented to the wrong receiver.
    type ReceiveError;

    /// Short stable label for diagnostics.
    const KIND: &'static str;

    /// Move the sender into the post-send role/state.
    fn close_sender(
        sender: <<Self::Box as MessageBox>::Lock as Transition>::From,
        at: &At<Self::Box>,
        body: &Self::Body,
    ) -> <<Self::Box as MessageBox>::Lock as Transition>::To;

    /// Move the sender into the failed-pack role/state.
    fn fail_sender(
        sender: <<Self::Box as MessageBox>::Lock as Transition>::From,
    ) -> Self::SenderFailed;

    /// Move the receiver into the post-read role/state after validating that
    /// this exact receiver is allowed to unlock this exact box.
    fn ready_receiver(
        receiver: <<Self::Box as MessageBox>::Unlock as Transition>::From,
        at: &At<Self::Box>,
        body: &Self::Body,
    ) -> Result<<<Self::Box as MessageBox>::Unlock as Transition>::To, Self::ReceiveError>;
}

/// Message that has been created but not yet packed into its buffer.
#[must_use = "Open<M> must be packed or converted into a typed failure before it is dropped"]
pub(crate) struct Open<M: Message> {
    armed: bool,
    sender: Option<<<M::Box as MessageBox>::Lock as Transition>::From>,
    body: Option<M::Body>,
    _message: PhantomData<M>,
    _private: Private,
}

impl<M: Message> fmt::Debug for Open<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Open")
            .field("kind", &M::KIND)
            .field("armed", &self.armed)
            .finish_non_exhaustive()
    }
}

impl<M: Message> Open<M> {
    /// Open a message by consuming its only valid sender role/state.
    pub(crate) fn from_sender(
        sender: <<M::Box as MessageBox>::Lock as Transition>::From,
        body: M::Body,
    ) -> Self {
        Self {
            armed: true,
            sender: Some(sender),
            body: Some(body),
            _message: PhantomData,
            _private: Private,
        }
    }

    /// Lock this message into its only durable box.
    pub(crate) fn lock<E, F>(
        mut self,
        at: At<M::Box>,
        write: F,
    ) -> Result<(<<M::Box as MessageBox>::Lock as Transition>::To, Locked<M>), LockError<M, E>>
    where
        F: FnOnce(&At<M::Box>, &M::Body) -> Result<(), E>,
    {
        let body = self.body.as_ref().expect("open message missing body");
        match write(&at, body) {
            Ok(()) => {
                let sender = self
                    .sender
                    .take()
                    .expect("open message missing sender before lock");
                let body = self
                    .body
                    .take()
                    .expect("open message missing body before lock");
                let closed = M::close_sender(sender, &at, &body);
                self.armed = false;
                Ok((
                    closed,
                    Locked {
                        at,
                        body,
                        _message: PhantomData,
                        _private: Private,
                    },
                ))
            }
            Err(source) => {
                let sender = self
                    .sender
                    .take()
                    .expect("open message missing sender before failed lock");
                let _ = self
                    .body
                    .take()
                    .expect("open message missing body before failed lock");
                let sender = M::fail_sender(sender);
                self.armed = false;
                Err(LockError { sender, source })
            }
        }
    }
}

impl<M: Message> Drop for Open<M> {
    fn drop(&mut self) {
        assert!(
            !self.armed,
            "open Prototype 1 message '{}' was dropped before being locked",
            M::KIND
        );
    }
}

/// Message locked into its static durable box.
#[must_use = "Locked<M> must be unlocked by the intended receiver or intentionally persisted across process exit"]
pub(crate) struct Locked<M: Message> {
    at: At<M::Box>,
    body: M::Body,
    _message: PhantomData<M>,
    _private: Private,
}

impl<M: Message> Locked<M> {
    pub(crate) fn from_box<E, F>(at: At<M::Box>, read: F) -> Result<Self, UnlockReadError<M, E>>
    where
        F: FnOnce(&At<M::Box>) -> Result<M::Body, E>,
    {
        match read(&at) {
            Ok(body) => Ok(Self {
                at,
                body,
                _message: PhantomData,
                _private: Private,
            }),
            Err(source) => Err(UnlockReadError { at, source }),
        }
    }

    pub(crate) fn unlock(
        self,
        receiver: <<M::Box as MessageBox>::Unlock as Transition>::From,
    ) -> Result<
        (
            <<M::Box as MessageBox>::Unlock as Transition>::To,
            Received<M>,
        ),
        UnlockError<M>,
    > {
        let ready = match M::ready_receiver(receiver, &self.at, &self.body) {
            Ok(ready) => ready,
            Err(source) => {
                return Err(UnlockError {
                    failed: Failed {
                        at: self.at,
                        body: self.body,
                        _message: PhantomData,
                        _private: Private,
                    },
                    source,
                });
            }
        };
        Ok((
            ready,
            Received {
                at: self.at,
                body: self.body,
                _message: PhantomData,
                _private: Private,
            },
        ))
    }
}

impl<M: Message> fmt::Debug for Locked<M>
where
    M::Body: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Locked")
            .field("kind", &M::KIND)
            .field("box", &M::Box::NAME)
            .field("at", &self.at.path())
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

/// Typed failure for a message that reached transport but did not complete receipt.
pub(crate) struct Failed<M: Message> {
    at: At<M::Box>,
    body: M::Body,
    _message: PhantomData<M>,
    _private: Private,
}

impl<M> fmt::Debug for Failed<M>
where
    M: Message,
    M::Body: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Failed")
            .field("kind", &M::KIND)
            .field("box", &M::Box::NAME)
            .field("at", &self.at.path())
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

/// Message that has been consumed by the intended receiver.
#[must_use = "Received<M> is the capability proving a cross-runtime message was consumed"]
pub(crate) struct Received<M: Message> {
    at: At<M::Box>,
    body: M::Body,
    _message: PhantomData<M>,
    _private: Private,
}

impl<M: Message> Received<M> {
    pub(crate) fn at(&self) -> &At<M::Box> {
        &self.at
    }

    pub(crate) fn body(&self) -> &M::Body {
        &self.body
    }
}

impl<M> fmt::Debug for Received<M>
where
    M: Message,
    M::Body: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Received")
            .field("kind", &M::KIND)
            .field("box", &M::Box::NAME)
            .field("at", &self.at.path())
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

/// Failed attempt to read a locked message box from transport.
pub(crate) struct UnlockReadError<M: Message, E> {
    at: At<M::Box>,
    source: E,
}

impl<M: Message, E> UnlockReadError<M, E> {
    pub(crate) fn into_parts(self) -> (At<M::Box>, E) {
        (self.at, self.source)
    }

    pub(crate) fn source(&self) -> &E {
        &self.source
    }
}

impl<M, E> fmt::Debug for UnlockReadError<M, E>
where
    M: Message,
    E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockReadError")
            .field("kind", &M::KIND)
            .field("box", &M::Box::NAME)
            .field("at", &self.at.path())
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<M, E> fmt::Display for UnlockReadError<M, E>
where
    M: Message,
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to read Prototype 1 message '{}' from '{}': {}",
            M::KIND,
            M::Box::NAME,
            self.source
        )
    }
}

impl<M, E> Error for UnlockReadError<M, E>
where
    M: Message + fmt::Debug,
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Failed attempt to unlock a message.
///
/// The failed message state is preserved so the caller cannot silently lose
/// the cross-runtime communication artifact on a wrong-recipient or validation
/// failure.
pub(crate) struct UnlockError<M: Message> {
    failed: Failed<M>,
    source: M::ReceiveError,
}

impl<M: Message> UnlockError<M> {
    pub(crate) fn into_parts(self) -> (Failed<M>, M::ReceiveError) {
        (self.failed, self.source)
    }

    pub(crate) fn source(&self) -> &M::ReceiveError {
        &self.source
    }
}

impl<M> fmt::Debug for UnlockError<M>
where
    M: Message,
    M::Body: fmt::Debug,
    M::ReceiveError: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnlockError")
            .field("kind", &M::KIND)
            .field("failed", &self.failed)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<M> fmt::Display for UnlockError<M>
where
    M: Message,
    M::ReceiveError: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to unlock Prototype 1 message '{}': {}",
            M::KIND,
            self.source
        )
    }
}

impl<M> Error for UnlockError<M>
where
    M: Message + fmt::Debug,
    M::Body: fmt::Debug,
    M::ReceiveError: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Failed attempt to lock an open message.
///
/// The open obligation is returned to the caller so it cannot be lost through a
/// failed write.
pub(crate) struct LockError<M: Message, E> {
    sender: M::SenderFailed,
    source: E,
}

impl<M, E> fmt::Debug for LockError<M, E>
where
    M: Message,
    E: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LockError")
            .field("kind", &M::KIND)
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<M: Message, E> LockError<M, E> {
    pub(crate) fn into_parts(self) -> (M::SenderFailed, E) {
        (self.sender, self.source)
    }

    pub(crate) fn source(&self) -> &E {
        &self.source
    }
}

impl<M, E> fmt::Display for LockError<M, E>
where
    M: Message,
    E: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to lock Prototype 1 message '{}': {}",
            M::KIND,
            self.source
        )
    }
}

impl<M, E> Error for LockError<M, E>
where
    M: Message + fmt::Debug,
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Private;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestMessage;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestLock;

    impl Transition for TestLock {
        type From = ();
        type To = SenderClosed;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestUnlock;

    impl Transition for TestUnlock {
        type From = ();
        type To = ReceiverReady;
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct TestBox;

    impl File for TestBox {
        type Params = PathBuf;

        const NAME: &'static str = "test-box.json";

        fn resolve(params: Self::Params) -> PathBuf {
            params
        }
    }

    impl MessageBox for TestBox {
        type Lock = TestLock;
        type Unlock = TestUnlock;
    }

    impl Message for TestMessage {
        type Box = TestBox;
        type Body = &'static str;
        type SenderFailed = SenderFailed;
        type ReceiveError = std::convert::Infallible;

        const KIND: &'static str = "test";

        fn close_sender(
            _sender: <<Self::Box as MessageBox>::Lock as Transition>::From,
            _at: &At<Self::Box>,
            _body: &Self::Body,
        ) -> <<Self::Box as MessageBox>::Lock as Transition>::To {
            SenderClosed
        }

        fn fail_sender(
            _sender: <<Self::Box as MessageBox>::Lock as Transition>::From,
        ) -> Self::SenderFailed {
            SenderFailed
        }

        fn ready_receiver(
            _receiver: <<Self::Box as MessageBox>::Unlock as Transition>::From,
            _at: &At<Self::Box>,
            _body: &Self::Body,
        ) -> Result<<<Self::Box as MessageBox>::Unlock as Transition>::To, Self::ReceiveError>
        {
            Ok(ReceiverReady)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SenderClosed;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct SenderFailed;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct ReceiverReady;

    #[test]
    fn sender_opens_and_pack_disarms_message() {
        let at = At::<TestBox>::resolve(PathBuf::from("/tmp/test-box.json"));
        let open = Open::<TestMessage>::from_sender((), "body");
        let (sender, locked) = open
            .lock(at, |_, _| Ok::<_, std::convert::Infallible>(()))
            .unwrap();

        assert_eq!(sender, SenderClosed);
        let (_, received) = locked.unlock(()).unwrap();
        assert_eq!(received.body(), &"body");
    }

    #[test]
    fn receiver_consumes_packed_message() {
        let at = At::<TestBox>::resolve(PathBuf::from("/tmp/test-box.json"));
        let locked =
            Locked::<TestMessage>::from_box(at, |_| Ok::<_, std::convert::Infallible>("body"))
                .unwrap();

        let (receiver, received) = locked.unlock(()).unwrap();

        assert_eq!(receiver, ReceiverReady);
        assert_eq!(received.body(), &"body");
    }

    #[test]
    fn sender_can_lock_and_receiver_can_unlock() {
        let at = At::<TestBox>::resolve(PathBuf::from("/tmp/test-box.json"));
        let open = Open::<TestMessage>::from_sender((), "body");
        let (_, locked) = open
            .lock(at, |_, _| Ok::<_, std::convert::Infallible>(()))
            .unwrap();

        let (receiver, received) = locked.unlock(()).unwrap();

        assert_eq!(receiver, ReceiverReady);
        assert_eq!(received.body(), &"body");
    }

    #[test]
    #[should_panic(expected = "open Prototype 1 message 'test' was dropped before being locked")]
    fn dropping_open_message_panics() {
        let _open = Open::<TestMessage>::from_sender((), "body");
    }
}
