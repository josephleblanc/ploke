//! Core operator graph.
//!
//! The graph keeps the three Prototype 1 mappings visible together:
//!
//! - artifacts related by applied patch operations
//! - runtimes hydrated from artifacts
//! - operations performed by runtimes over target artifacts
//!
//! Records contribute evidence to this graph. They do not become the graph.

pub use std::collections::HashSet as Set;
use std::fmt;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    artifacts: Set<Artifact>,
    runtimes: Set<Runtime>,
    operations: Set<Operation>,
    relations: Set<Relation>,
    evidence: Set<Evidence>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn artifacts(&self) -> &Set<Artifact> {
        &self.artifacts
    }

    pub fn runtimes(&self) -> &Set<Runtime> {
        &self.runtimes
    }

    pub fn operations(&self) -> &Set<Operation> {
        &self.operations
    }

    pub fn relations(&self) -> &Set<Relation> {
        &self.relations
    }

    pub fn evidence(&self) -> &Set<Evidence> {
        &self.evidence
    }

    pub fn insert_artifact(&mut self, artifact: Artifact) -> bool {
        self.artifacts.insert(artifact)
    }

    pub fn insert_runtime(&mut self, runtime: Runtime) -> bool {
        self.runtimes.insert(runtime)
    }

    pub fn insert_operation(&mut self, operation: Operation) -> bool {
        self.operations.insert(operation)
    }

    pub fn insert_relation(&mut self, relation: Relation) -> bool {
        self.relations.insert(relation)
    }

    pub fn insert_evidence(&mut self, evidence: Evidence) -> bool {
        self.evidence.insert(evidence)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Artifact {
    pub id: ArtifactId,
}

impl Artifact {
    pub fn new(id: ArtifactId) -> Self {
        Self { id }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Runtime {
    pub id: RuntimeId,
}

impl Runtime {
    pub fn new(id: RuntimeId) -> Self {
        Self { id }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Operation {
    pub id: OperationId,
    pub generator: RuntimeId,
    pub target: ArtifactId,
}

impl Operation {
    pub fn new(id: OperationId, generator: RuntimeId, target: ArtifactId) -> Self {
        Self {
            id,
            generator,
            target,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Relation {
    pub id: RelationId,
    pub kind: RelationKind,
}

impl Relation {
    pub fn new(id: RelationId, kind: RelationKind) -> Self {
        Self { id, kind }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RelationKind {
    ArtifactDerived {
        base: ArtifactId,
        operation: OperationId,
        derived: ArtifactId,
    },
    RuntimeHydrated {
        artifact: ArtifactId,
        runtime: RuntimeId,
    },
    OperationProduced {
        operation: OperationId,
        artifact: ArtifactId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Evidence {
    pub id: EvidenceId,
    pub subject: Subject,
}

impl Evidence {
    pub fn new(id: EvidenceId, subject: Subject) -> Self {
        Self { id, subject }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Subject {
    Artifact(ArtifactId),
    Runtime(RuntimeId),
    Operation(OperationId),
    Relation(RelationId),
}

id_type!(ArtifactId);
id_type!(RuntimeId);
id_type!(OperationId);
id_type!(RelationId);
id_type!(EvidenceId);
