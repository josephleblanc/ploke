#![cfg(feature = "uuid_ids")]

use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use syn_parser::discovery::run_discovery_phase;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::relations::{GraphId, RelationKind};
use syn_parser::parser::types::TypeNode;
use syn_parser::parser::visitor::ParsedCodeGraph;
use syn_parser::parser::{analyze_files_parallel, nodes::*};
