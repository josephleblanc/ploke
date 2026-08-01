use std::{marker::PhantomData, path::PathBuf};

use ploke_db::Database;

pub(super) struct EvalDb<State = NotChecked> {
    db: Option<Database>,
    path: Option<PathBuf>,
    _state: PhantomData<State>,
}

pub(super) struct NotChecked;
pub(super) struct Loaded;
pub(super) struct NotConfigured;

impl EvalDb<NotChecked> {
    fn load_from_path(self, path: PathBuf) -> EvalDb<Loaded> {
        // let db;
        todo!()
    }
}
