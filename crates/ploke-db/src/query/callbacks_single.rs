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
    unregister_codes: Arc<HashMap<NodeType, u32>>,
    max_calls: Option<AtomicUsize>,
    functions: Receiver<Call>,
    consts: Receiver<Call>,
    enums: Receiver<Call>,
    macros: Receiver<Call>,
    modules: Receiver<Call>,
    statics: Receiver<Call>,
    structs: Receiver<Call>,
    traits: Receiver<Call>,
    type_alias: Receiver<Call>,
    unions: Receiver<Call>,
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
type Call = (CallbackOp, NamedRows, NamedRows);
type CallHelper = (
    CallbackManager,
    Receiver<Result<Call, DbError>>,
    Arc<HashMap<NodeType, u32>>,
    Sender<()>,
);

impl CallbackManager {
    pub fn new_bounded(db: Arc<Database>, n: usize) -> Result<CallHelper, DbError> {
        let (s, r) = crossbeam_channel::bounded(n);
        let mut unregister_codes = HashMap::new();
        let mut sx: HashMap<NodeType, Receiver<Call>> = HashMap::new();
        for ty in NodeType::primary_nodes() {
            let (unreg_code, r) = db.register_callback(ty.relation_str(), Some(n));
            unregister_codes.insert(ty, unreg_code);
            sx.insert(ty, r);
        }
        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded(1);

        let codes_arc = Arc::new(unregister_codes);

        let callback_manager = Self {
            s,
            db_arc: Arc::clone(&db),
            update_counter: Arc::new(AtomicUsize::new(0)),
            unregister_codes: Arc::clone(&codes_arc),
            functions: get_recvr(NodeType::Function, &mut sx)?,
            consts: get_recvr(NodeType::Const, &mut sx)?,
            enums: get_recvr(NodeType::Enum, &mut sx)?,
            macros: get_recvr(NodeType::Macro, &mut sx)?,
            modules: get_recvr(NodeType::Module, &mut sx)?,
            statics: get_recvr(NodeType::Static, &mut sx)?,
            structs: get_recvr(NodeType::Struct, &mut sx)?,
            traits: get_recvr(NodeType::Trait, &mut sx)?,
            type_alias: get_recvr(NodeType::TypeAlias, &mut sx)?,
            unions: get_recvr(NodeType::Union, &mut sx)?,
            max_calls: None,
            shutdown: shutdown_rx,
        };

        Ok((callback_manager, r, codes_arc, shutdown_tx))
    }

    pub fn new_unbounded(
        db: Arc<Database>,
    ) -> Result<(CallbackManager, Receiver<Result<Call, DbError>>), DbError> {
        let (s, r) = crossbeam_channel::unbounded();
        let mut unregister_codes = HashMap::new();
        let mut sx: HashMap<NodeType, Receiver<Call>> = HashMap::new();
        for ty in NodeType::primary_nodes() {
            let ty_relation_str = ty.relation_str();
            let (unreg_code, r) = db.register_callback(ty.relation_str(), None);
            unregister_codes.insert(ty, unreg_code);
            sx.insert(ty, r);
        }

        let (shutdown_tx, shutdown_rx) = crossbeam_channel::bounded(1);

        let callback_manager = Self {
            s,
            db_arc: Arc::clone(&db),
            update_counter: Arc::new(AtomicUsize::new(0)),
            unregister_codes: Arc::new(unregister_codes),
            functions: get_recvr(NodeType::Function, &mut sx)?,
            consts: get_recvr(NodeType::Const, &mut sx)?,
            enums: get_recvr(NodeType::Enum, &mut sx)?,
            macros: get_recvr(NodeType::Macro, &mut sx)?,
            modules: get_recvr(NodeType::Module, &mut sx)?,
            statics: get_recvr(NodeType::Static, &mut sx)?,
            structs: get_recvr(NodeType::Struct, &mut sx)?,
            traits: get_recvr(NodeType::Trait, &mut sx)?,
            type_alias: get_recvr(NodeType::TypeAlias, &mut sx)?,
            unions: get_recvr(NodeType::Union, &mut sx)?,
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
                recv(self.functions) -> msg => msg,
                recv(self.consts) -> msg => msg,
                recv(self.enums) -> msg => msg,
                recv(self.macros) -> msg => msg,
                recv(self.modules) -> msg => msg,
                recv(self.statics) -> msg => msg,
                recv(self.structs) -> msg => msg,
                recv(self.traits) -> msg => msg,
                recv(self.type_alias) -> msg => msg,
                recv(self.unions) -> msg => msg,
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
        for (node_type, code) in self.unregister_codes.iter() {
            tracing::info!(
                "Unregistering callback for NodeType::{:?} with code {}",
                node_type,
                code
            );
            tracing::debug!(
                "Unregistering callback for NodeType::{:?} | unregistered? {}",
                node_type,
                self.db_arc
                    .unregister_callback(*code)
                    .then(|| {
                        tracing::error!("Failed to unregister callback for {:?}", node_type,);
                    })
                    .is_some()
            );
        }
    }
}

pub fn log_send(e: SendError<Result<Call, DbError>>) {
    tracing::error!("{: >28}{}", "[log_send] in callbacks.rs | ", e)
}
pub fn log_err(e: RecvError) -> DbError {
    tracing::error!("{: >28}{}", "[log_err] in callbacks.rs | ", e);
    DbError::CrossBeamSend(e.to_string())
}

fn get_recvr(
    ty: NodeType,
    receivers: &mut HashMap<NodeType, Receiver<Call>>,
) -> Result<Receiver<(CallbackOp, NamedRows, NamedRows)>, DbError> {
    match receivers.entry(ty) {
        Entry::Occupied(occ) => {
            let (ty, r) = occ.remove_entry();
            Ok(r)
        }
        Entry::Vacant(vac) => Err(DbError::CallbackErr),
    }
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

    use ploke_test_utils::init_test_tracing_with_target;
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
    fn test_get_recvr_success() {
        let mut receivers = HashMap::new();
        let (s, r) = crossbeam_channel::unbounded();
        receivers.insert(NodeType::Function, r);

        let result = get_recvr(NodeType::Function, &mut receivers);
        assert!(result.is_ok());
        assert!(receivers.is_empty());
    }

    #[test]
    fn test_get_recvr_fail() {
        let mut receivers = HashMap::new();
        let result = get_recvr(NodeType::Function, &mut receivers);
        assert!(result.is_err());
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
