use std::path::Path;

use crate::cli::{
    Prototype1StateWalkControlCommand, Prototype1StateWalkStartCommand,
    Prototype1StateWalkStepCommand,
};

use super::{paths, protocol::WalkStartConfig};

impl Prototype1StateWalkStartCommand {
    pub(crate) fn start_config(self) -> WalkStartConfig {
        WalkStartConfig {
            campaign: self.campaign,
            node_id: self.node_id,
            repo_root: self.repo_root,
            init_parent_identity: self.init_parent_identity,
            identity_branch: self.identity_branch,
            identity_instance: self.identity_instance,
            handoff_invocation: self.handoff_invocation,
            stop_after: self.stop_after,
            successor_selection: self.successor_selection,
            successor_selection_seed: self.successor_selection_seed,
            successor_selection_metrics: self.successor_selection_metrics,
            candidate_generator: self.candidate_generator,
            format: self.format,
        }
    }

    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

impl Prototype1StateWalkStepCommand {
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

impl Prototype1StateWalkControlCommand {
    pub(crate) fn repo_root_ref(&self) -> Option<&Path> {
        self.repo_root.as_deref()
    }
}

pub(crate) fn resolve_socket(
    repo_root: Option<&Path>,
    socket: Option<&Path>,
) -> Result<(std::path::PathBuf, std::path::PathBuf), crate::spec::PrepareError> {
    let repo_root = paths::resolve_repo_root(repo_root)?;
    let socket = paths::socket_path(&repo_root, socket)?;
    Ok((repo_root, socket))
}
