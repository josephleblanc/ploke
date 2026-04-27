//! Parent role state for Prototype 1.

use std::marker::PhantomData;
use std::path::Path;

use crate::{
    cli::prototype1_state::{
        backend::WorkspaceBackend,
        identity::{ParentIdentity, parent_identity_path},
    },
    intervention::{Prototype1NodeRecord, load_node_record, prototype1_node_record_path},
    spec::{
        PrepareError, Prototype1ParentError, Prototype1ParentIdentityContext,
        Prototype1ParentNodeContext,
    },
};

/// Parent role before its artifact, identity, and scheduler facts agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unchecked {}

/// Parent role after its artifact, identity, and scheduler facts agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checked {}

/// Runtime role carrier for a Parent in a known verification state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Parent<S> {
    identity: ParentIdentity,
    node: Prototype1NodeRecord,
    _state: PhantomData<S>,
}

/// Inputs needed to check whether an unchecked Parent role is valid here.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Check<'a> {
    pub campaign_id: &'a str,
    pub active_root: &'a Path,
    pub selected_instance: Option<&'a str>,
}

impl Parent<Unchecked> {
    pub(crate) fn load(
        manifest_path: &Path,
        identity: ParentIdentity,
    ) -> Result<Self, PrepareError> {
        let node = load_node_record(manifest_path, &identity.node_id)?;
        Ok(Self {
            identity,
            node,
            _state: PhantomData,
        })
    }

    pub(crate) fn check<B: WorkspaceBackend>(
        self,
        backend: &B,
        manifest_path: &Path,
        check: Check<'_>,
    ) -> Result<Parent<Checked>, PrepareError> {
        backend
            .validate_parent_checkout(check.active_root, &self.identity)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_parent_checkout",
                detail: source.to_string(),
            })?;

        let identity = identity_context(check.active_root, &self.identity);
        let node = node_context(manifest_path, &self.node);

        if identity.generation != node.generation {
            return Err(Prototype1ParentError::GenerationMismatch { identity, node }.into());
        }
        if identity.branch_id != node.branch_id {
            return Err(Prototype1ParentError::BranchMismatch { identity, node }.into());
        }
        if let Some(selected_instance) = check.selected_instance {
            if selected_instance != node.instance_id {
                return Err(Prototype1ParentError::SelectionMismatch {
                    campaign_id: check.campaign_id.to_string(),
                    selected_instance: selected_instance.to_string(),
                    parent: node,
                }
                .into());
            }
        }

        Ok(Parent {
            identity: self.identity,
            node: self.node,
            _state: PhantomData,
        })
    }
}

impl Parent<Checked> {
    pub(crate) fn identity(&self) -> &ParentIdentity {
        &self.identity
    }

    pub(crate) fn node(&self) -> &Prototype1NodeRecord {
        &self.node
    }
}

fn identity_context(
    active_root: &Path,
    identity: &ParentIdentity,
) -> Prototype1ParentIdentityContext {
    Prototype1ParentIdentityContext {
        path: parent_identity_path(active_root),
        node_id: identity.node_id.clone(),
        generation: identity.generation,
        branch_id: identity.branch_id.clone(),
    }
}

fn node_context(manifest_path: &Path, node: &Prototype1NodeRecord) -> Prototype1ParentNodeContext {
    Prototype1ParentNodeContext {
        path: prototype1_node_record_path(manifest_path, &node.node_id),
        node_id: node.node_id.clone(),
        generation: node.generation,
        branch_id: node.branch_id.clone(),
        instance_id: node.instance_id.clone(),
    }
}
