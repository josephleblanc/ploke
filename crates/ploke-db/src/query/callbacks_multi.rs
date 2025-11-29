use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{atomic::AtomicUsize, Arc},
};

use cozo::{CallbackOp, NamedRows};
use crossbeam_channel::{Receiver, RecvError, SendError, Sender};
use tracing_subscriber::registry::Data;

use crate::{Database, DbError, NodeType};

pub struct CallbackManager {
    s: Sender<Result<Call, DbError>>,
    db_arc: Arc<Database>,
    update_counter: Arc<AtomicUsize>,
    unregister_code: u32,
    max_calls: Option<AtomicUsize>,
    embeddings: Receiver<Call>,
    shutdown: Receiver<()>,
}

#[derive(Debug, Clone)]
pub struct Callback {
    cb: CallbackOp,
    header: NamedRows,
    rows: NamedRows,
}

impl Callback {
    pub fn new(val: (CallbackOp, NamedRows, NamedRows)) -> Self {
        Self {
            cb: val.0,
            header: val.1,
            rows: val.2,
        }
    }
}

type UnregisterCode = u32;

type Call = (CallbackOp, NamedRows, NamedRows);
type CallHelper = (
    CallbackManager,
    Receiver<Result<Call, DbError>>,
    UnregisterCode,
    Sender<()>,
);

impl CallbackManager {
    pub fn new_bounded(db: Arc<Database>, n: usize) -> Result<CallHelper, DbError> {
        let (s, r) = crossbeam_channel::bounded(n);
        let vec_relation = db.active_embedding_set.relation_name();
        let (unreg_code, db_rx) = db.register_callback(vec_relation.as_ref(), Some(n));
        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded(1);

        let callback_manager = Self {
            s,
            db_arc: Arc::clone(&db),
            update_counter: Arc::new(AtomicUsize::new(0)),
            unregister_code: unreg_code,
            embeddings: db_rx,
            max_calls: None,
            shutdown: shutdown_rx,
        };

        Ok((callback_manager, r, unreg_code, shutdown_tx))
    }

    pub fn new_unbounded(db: Arc<Database>) -> Result<(CallbackManager, Receiver<Result<Call, DbError>>), DbError> {
        let (s, r) = crossbeam_channel::unbounded();
        let vec_relation = db.active_embedding_set.relation_name();
        let (unreg_code, db_rx) = db.register_callback(vec_relation.as_ref(), None);

        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded(1);

        let callback_manager = Self {
            s,
            db_arc: Arc::clone(&db),
            update_counter: Arc::new(AtomicUsize::new(0)),
            unregister_code: unreg_code,
            embeddings: db_rx,
            max_calls: None,
            shutdown: shutdown_rx,
        };

        Ok((callback_manager, r))
    }

    pub fn clone_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.update_counter)
    }

    pub fn run(self) -> Result<(), DbError> {
        loop {
            let msg_res = crossbeam_channel::select! {
                recv(self.embeddings) -> msg => msg,
                recv(self.shutdown) -> msg => {tracing::info!("{:=<}SHUTODWN RECEIVED: CALLBACK{:=<}", "", ""); break;},
                default(std::time::Duration::from_millis(100)) => continue,
            };

            match msg_res {
                Ok(call) => {
                    let result = self.s.send(Ok(call));
                    if let Err(e) = result {
                        // Consumer disconnected.
                        log_send(e);
                    }
                }
                Err(e) => {
                    // A producer disconnected (db was dropped), log and exit.
                    log_err(e);
                    break;
                }
            }
            if self.shutdown.try_recv().is_ok() {
                tracing::info!("{:=<}SHUTODWN RECEIVED: CALLBACK{:=<}", "", "");
                break;
            }
        }
        Ok(())
    }

    pub fn init_max(&mut self, max: AtomicUsize) -> Result<(), DbError> {
        if self.max_calls.is_some() {
            return Err(DbError::CallbackSetCheck);
        }
        self.max_calls.replace(max);
        Ok(())
    }
}

impl Drop for CallbackManager {
    fn drop(&mut self) {
        let vec_rel_name = self.db_arc.active_embedding_set.relation_name().as_ref();
        tracing::info!(
            "Unregistering callback for relation {} with code {}",
            vec_rel_name,
            self.unregister_code
        );
        tracing::debug!(
            "Unregistering callback for relation {} | unregistered? {}",
            vec_rel_name,
            self.db_arc
                .unregister_callback(self.unregister_code)
                .then(|| {
                    tracing::error!("Failed to unregister callback for {:?}", vec_rel_name);
                })
                .is_some()
        );
    }
}

pub fn log_send(e: SendError<Result<Call, DbError>>) {
    tracing::error!("{: >28}{}", "[log_send] in callbacks.rs | ", e)
}
pub fn log_err(e: RecvError) -> DbError {
    tracing::error!("{: >28}{}", "[log_err] in callbacks.rs | ", e);
    DbError::CrossBeamSend(e.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::atomic::{AtomicUsize, Ordering},
        thread,
        time::Duration,
    };

    use cozo::NamedRows;
    use crossbeam_channel::Receiver;
    use tracing::Level;

    use super::*;
    use crate::{Database, DbError, NodeType};

    #[cfg(feature = "multi_embedding_db")]
    use ploke_test_utils::init_test_tracing_with_target;
    #[cfg(feature = "multi_embedding_db")]
    use ploke_test_utils::setup_db_full_multi_embedding;

    struct MockDb;

    impl MockDb {
        fn register_callback(
            &self,
            _rel: &str,
            _cap: Option<usize>,
        ) -> (u32, Receiver<(cozo::CallbackOp, NamedRows, NamedRows)>) {
            let (s, r) = crossbeam_channel::unbounded();
            (0, r)
        }
    }

    #[test]
    fn test_new_bounded() {
        let db = Database::init_with_schema().unwrap();
        let result = CallbackManager::new_bounded(Arc::new(db), 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_new_unbounded() {
        let db = Database::init_with_schema().unwrap();
        let result = CallbackManager::new_unbounded(Arc::new(db));
        assert!(result.is_ok());
    }

    #[test]
    fn test_clone_counter() {
        let db = Database::init_with_schema().unwrap();
        let (manager, _) = CallbackManager::new_unbounded(Arc::new(db)).unwrap();
        let counter = manager.clone_counter();
        assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn test_init_max() {
        let db = Database::init_with_schema().unwrap();
        let (mut manager, _) = CallbackManager::new_unbounded(Arc::new(db)).unwrap();
        let max = AtomicUsize::new(10);
        let result = manager.init_max(max);
        assert!(result.is_ok());

        let max = AtomicUsize::new(10);
        let result = manager.init_max(max);
        assert!(result.is_err());
        assert_eq!(result.err(), Some(DbError::CallbackSetCheck));
    }
}
