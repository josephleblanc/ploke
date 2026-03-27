pub mod logging;

#[cfg(feature = "mod_tree_cfg")]
#[allow(unused_imports)]
pub(crate) use logging::LOG_TARGET_PATH_CFGS;

pub use logging::{
    AccLogCtx, LOG_TARGET_MOD_TREE_BUILD, LOG_TARGET_NODE_ID, LOG_TARGET_RELS, LOG_TARGET_VIS,
    LogDataStructure, LogStyle, LogStyleBool, LogStyleDebug,
};

#[cfg(test)]
pub(crate) mod test_setup;
