use std::{
    collections::{hash_map::Entry, HashMap},
    sync::{atomic::AtomicUsize, Arc},
};

use cozo::{CallbackOp, NamedRows};
use crossbeam_channel::{Receiver, RecvError, SendError, Sender};

use crate::{Database, DbError, NodeType};

pub struct CallbackManager {
    s: Sender<Result<Call, DbError>>,
    update_counter: Arc<AtomicUsize>,
    unregister_codes: HashMap<NodeType, u32>,
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

impl CallbackManager {
    pub fn new_bounded(
        db: &Database,
        n: usize,
    ) -> Result<(CallbackManager, Receiver<Result<Call, DbError>>), DbError> {
        let (s, r) = crossbeam_channel::bounded(n);
        let mut unregister_codes = HashMap::new();
        let mut sx: HashMap<NodeType, Receiver<Call>> = HashMap::new();
        for ty in NodeType::primary_nodes() {
            let (unreg_code, r) = db.register_callback(ty.relation_str(), Some(n));
            unregister_codes.insert(ty, unreg_code);
            sx.insert(ty, r);
        }

        let callback_manager = Self {
            s,
            update_counter: Arc::new(AtomicUsize::new(0)),
            unregister_codes,
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
        };

        Ok((callback_manager, r))
    }

    pub fn new_unbounded(
        db: &Database,
    ) -> Result<(CallbackManager, Receiver<Result<Call, DbError>>), DbError> {
        let (s, r) = crossbeam_channel::unbounded();
        let mut unregister_codes = HashMap::new();
        let mut sx: HashMap<NodeType, Receiver<Call>> = HashMap::new();
        for ty in NodeType::primary_nodes() {
            let (unreg_code, r) = db.register_callback(ty.relation_str(), None);
            unregister_codes.insert(ty, unreg_code);
            sx.insert(ty, r);
        }

        let callback_manager = Self {
            s,
            update_counter: Arc::new(AtomicUsize::new(0)),
            unregister_codes,
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
        };

        Ok((callback_manager, r))
    }

    pub fn clone_counter(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.update_counter)
    }

    pub async fn run(&self) -> Result<(), DbError> {
        loop {
            let result = crossbeam_channel::select! {
                recv(self.functions) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.consts) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.enums) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.macros) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.modules) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.statics) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.structs) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.traits) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.type_alias) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                recv(self.unions) -> msg => {
                    self.s.send(msg.map_err(log_err)).map_err(log_send)
            },
                    };
        }
    }

    pub fn init_max(&mut self, max: AtomicUsize) -> Result<(), DbError> {
        if self.max_calls.is_some() {
            return Err(DbError::CallbackSetCheck);
        }
        self.max_calls.replace(max);
        Ok(())
    }
}

pub fn log_send(e: SendError<Result<Call, DbError>>) {
    tracing::warn!("{}", e)
}
pub fn log_err(e: RecvError) -> DbError {
    tracing::warn!("{}", e);
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
