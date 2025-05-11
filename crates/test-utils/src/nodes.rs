use ploke_core::ItemKind;
use syn_parser::parser::{nodes::PrimaryNodeId, ParsedCodeGraph};

#[derive(Debug, Clone)]
pub struct TestInfo<'a> {
    args: &'a ParanoidArgs<'a>,
    target_data: &'a ParsedCodeGraph,
    test_pid: PrimaryNodeId,
}

impl<'a> TestInfo<'a> {
    pub fn new(
        args: &'a ParanoidArgs,
        target_data: &'a ParsedCodeGraph,
        test_pid: PrimaryNodeId,
    ) -> Self {
        Self {
            args,
            target_data,
            test_pid,
        }
    }

    pub fn args(&self) -> &ParanoidArgs<'a> {
        self.args
    }

    pub fn target_data(&self) -> &ParsedCodeGraph {
        self.target_data
    }

    pub fn test_pid(&self) -> PrimaryNodeId {
        self.test_pid
    }
}

#[derive(Debug, Clone)]
/// Args for the paranoid helper test functions.
/// Includes all information required to regenerate the NodeId of the target node.
pub struct ParanoidArgs<'a> {
    // parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection - Passed separately now
    /// The name of the test fixture directory (e.g., "fixture_nodes").
    /// Used to construct the absolute path to the fixture crate root.
    pub fixture: &'a str,
    /// The path to the specific source file within the fixture, relative to the fixture root
    /// (e.g., "src/const_static.rs"). Used to find the correct `ParsedCodeGraph` and
    /// as input for `NodeId::generate_synthetic`.
    pub relative_file_path: &'a str,
    /// The expected fully-qualified module path of the *parent* module containing the target item
    /// (e.g., `["crate", "my_module"]`). Used to find the parent `ModuleNodeId` for ID generation.
    pub expected_path: &'a [&'a str],
    /// The identifier (name) of the target item (e.g., "MY_CONST").
    /// Used as input for `NodeId::generate_synthetic`.
    pub ident: &'a str,
    /// The expected `ItemKind` of the target item (e.g., `ItemKind::Const`).
    /// Used to select the correct `PrimaryNodeId` type and for ID generation.
    pub item_kind: ItemKind,
    /// An optional slice of cfg strings expected to be active for the item
    /// (e.g., `Some(&["target_os = \"linux\""])`). Used to calculate the cfg hash
    /// for ID generation. `None` or `Some(&[])` indicates no cfgs.
    pub expected_cfg: Option<&'a [&'a str]>,
}

impl<'a> ParanoidArgs<'a> {
    /// Regenerates the exact uuid::Uuid using the v5 hashing method to check that the node id
    /// correctly matches when using the expected inputs for the typed id node generation.
    /// - Returns a result with the typed PrimaryNodeId matching the input type of `item_kind` provided
    ///   in the `ParanoidArgs`.
    pub fn generate_pid(
        &'a self,
        parsed_graphs: &'a [ParsedCodeGraph],
    ) -> Result<TestInfo<'a>, SynParserError> {
        // 1. Construct the absolute expected file path
        let fixture_root = fixtures_crates_dir().join(self.fixture);
        let target_file_path = fixture_root.join(self.relative_file_path);

        // 2. Find the specific ParsedCodeGraph for the target file
        let target_data = parsed_graphs
            .iter()
            .find(|data| data.file_path == target_file_path)
            .unwrap_or_else(|| {
                panic!(
                    "ParsedCodeGraph for '{}' not found in results",
                    target_file_path.display()
                )
            });
        let graph = &target_data.graph;
        let exp_path_string = self
            .expected_path
            .iter()
            .copied()
            .map(|s| s.to_string())
            .collect_vec();

        let cfgs_bytes_option: Option<Vec<u8>> = self
            .expected_cfg
            .filter(|cfgs_slice| !cfgs_slice.is_empty()) // Only proceed if there are actual CFG strings
            .and_then(|cfgs_slice| calculate_cfg_hash_bytes(&strs_to_strings(cfgs_slice))); // Results in None if expected_cfg is None, or if cfgs_slice is empty, or if calculate_cfg_hash_bytes returns None.

        let actual_parent_scope_id_for_id_gen = match graph
            .find_module_by_path_checked(&exp_path_string)
        {
            Ok(parent_module) => Some(parent_module.id.base_tid()),
            Err(_) => {
                graph.find_module_by_file_path_checked(path::Path::new(self.relative_file_path))?;
                None
            }
        };

        // let actual_parent_scope_id_for_id_gen = Some(parent_module.id.base_tid());
        let actual_cfg_bytes_for_id_gen = cfgs_bytes_option.as_deref();

        // New structured logging:
        if log::log_enabled!(target: LOG_TEST_ID_REGEN, log::Level::Debug) {
            // Check if specific log is enabled
            log::debug!(target: LOG_TEST_ID_REGEN, "ParanoidArgs::generate_pid");
            log::debug!(target: LOG_TEST_ID_REGEN,
                "  Inputs for {} ({:?}):\n    crate_namespace: {}\n    file_path: {:?}\n    relative_path: {:?}\n    item_name: {}\n    item_kind: {:?}\n    parent_scope_id: {:?}\n    cfg_bytes: {:?}",
                self.ident,
                self.item_kind, // Use self.item_kind from ParanoidArgs
                target_data.crate_namespace,
                &target_file_path,
                &exp_path_string, // This is the 'relative_path' for the item's ID context
                self.ident,
                self.item_kind, // Use self.item_kind from ParanoidArgs
                actual_parent_scope_id_for_id_gen,
                actual_cfg_bytes_for_id_gen
            );
        }

        let generated_id = NodeId::generate_synthetic(
            target_data.crate_namespace,
            &target_file_path,
            &exp_path_string,
            self.ident,
            self.item_kind,                    // Use self.item_kind from ParanoidArgs
            actual_parent_scope_id_for_id_gen, // Use the determined parent scope ID
            actual_cfg_bytes_for_id_gen,       // Use the determined CFG bytes
        );

        let pid = match self.item_kind {
            ItemKind::Function => FunctionNodeId::new_test(generated_id).into(),
            ItemKind::Struct => StructNodeId::new_test(generated_id).into(),
            ItemKind::Enum => EnumNodeId::new_test(generated_id).into(),
            ItemKind::Union => UnionNodeId::new_test(generated_id).into(),
            ItemKind::TypeAlias => TypeAliasNodeId::new_test(generated_id).into(),
            ItemKind::Trait => TraitNodeId::new_test(generated_id).into(),
            ItemKind::Impl => ImplNodeId::new_test(generated_id).into(),
            ItemKind::Module => ModuleNodeId::new_test(generated_id).into(),
            ItemKind::Const => ConstNodeId::new_test(generated_id).into(),
            ItemKind::Static => StaticNodeId::new_test(generated_id).into(),
            ItemKind::Macro => MacroNodeId::new_test(generated_id).into(),
            ItemKind::Import => ImportNodeId::new_test(generated_id).into(),
            // TODO: Decide what to do about handling ExternCrate. We kind of do want everything to
            // have a NodeId of some kind, and this will do for now, but we also want to
            // distinguish between an ExternCrate statement and something else... probably.
            ItemKind::ExternCrate => ImportNodeId::new_test(generated_id).into(),
            _ => {
                panic!("You can't use this test helper on Secondary/Assoc nodes, at least not yet.")
            }
        };

        let test_info = TestInfo {
            args: self,
            target_data,
            test_pid: pid,
        };
        Ok(test_info)
    }
}
