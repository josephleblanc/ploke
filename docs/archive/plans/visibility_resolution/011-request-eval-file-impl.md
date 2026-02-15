# A fully articulated question about implementing file resolution and integrating it for full visibility resolution.
1. The module system architecture you want (hierarchical/flat)
  - I'm not sure exactly what you mean by this. Determine which (hierarchical/flat) best describes the current implementation, if applicable. If not applicable, please clarify this point.
2. The granularity of tracking needed (per-file/module/workspace)
  - This is part of what I would like you to evaluate. With your access to so many of the methods used in `use` path resolution, module tracking resolution, and overall visibility resolution, you may be able to suggest what is required from file tracking to complement our current visibility implementation and extend it to handle:
    2.1. All of the user's files in all of their project's sub-directories.
    2.2. All of the user's optionally chosen dependencies (note however that options.rs is currently a stub)
  - The goal is currently to provide all data required to determine whether a given node in the `CodeGraph` is valid in some target context. Ultimately the target context will likely by the byte location in the target user's file. However, we are also considering parsing the target file in which code generation is requested in order to generate the query, but this approach seems wasteful. 
    - We have not yet determined how the byte location provided by the Span of the parsed file will be used to query the database. 
    - File tracking may or may not play an important role in this regard, though I am unsure about this.
    - You may provide guidance on this point.
3. The performance constraints (in-memory/cached/on-demand)
  - Let's not worry about this too much for now. We don't want to pre-optimize, but we also don't to make stupid design decisions that are needlessly expensive

I have provided the requested files. Additionally, here are some of the functions from `code_visitor.rs`:

```rust

pub struct CodeVisitor<'a> {
    state: &'a mut VisitorState,
}

impl<'a> CodeVisitor<'a> {

  // Other methods..

    /// Returns a newly generated NodeId while also adding a new Contains relation from the current
    /// module (souce) to the node (target) whose NodeId is being generated.
    /// - Follows the pattern of generating a NodeId only at the time the Relation is added to
    ///     CodeVisitor, making orphaned nodes difficult to add by mistake.
    fn add_contains_rel(&mut self, node_name: Option<&str>) -> NodeId {
        // TODO: Consider changing the return type to Result<NodeId> depending on the type of node
        // being traversed. Add generic type instaed of the optional node name and find a clean way
        // to handle adding nodes without names, perhaps by implementing a trait like UnnamedNode
        // for them.
        let node_id = self.state.next_node_id(); // Generate ID here

        if let Some(current_mod) = self.state.code_graph.modules.last_mut() {
            #[cfg(feature = "visibility_resolution")]
            current_mod.items.push(node_id);

            self.state.code_graph.relations.push(Relation {
                source: current_mod.id,
                target: node_id,
                kind: RelationKind::Contains,
            });

            #[cfg(feature = "verbose_debug")]
            if let Some(name) = node_name {
                self.debug_mod_stack_push(name.to_owned(), node_id);
            }
        }

        node_id // Return the new ID
    }
}

impl<'a, 'ast> Visit<'ast> for CodeVisitor<'a> {
    // other visit_item_* methods..

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        #[cfg(feature = "verbose_debug")]
        self.debug_mod_stack();

        let module_name = module.ident.to_string();
        let module_id = self.add_contains_rel(Some(&module_name));

        #[cfg(feature = "module_path_tracking")]
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&module_name, module_id);

        // Save current path before entering module
        #[cfg(feature = "module_path_tracking")]
        let parent_path = self.state.current_module_path.clone();

        // Update path for nested module visitation
        #[cfg(feature = "module_path_tracking")]
        self.state.current_module_path.push(module_name.clone());

        // Process module contents
        let mut submodules = Vec::new();
        let mut items = Vec::new();

        if let Some((_, mod_items)) = &module.content {
            for item in mod_items {
                let item_id = self.state.next_node_id();
                self.debug_mod_stack_push("NO NAME".to_string(), item_id);
                items.push(item_id);

                // Store item-module relationship immediately

                #[cfg(not(feature = "visibility_resolution"))]
                self.state.code_graph.relations.push(Relation {
                    source: module_id,
                    target: item_id,
                    kind: RelationKind::Contains,
                });
                #[cfg(feature = "verbose_debug")]
                if matches!(item, syn::Item::Mod(_)) {
                    submodules.push(item_id);
                    self.debug_submodule("No name Maybe ok?", item_id);
                }
            }
        }

        // Create module node with proper path tracking
        // Create module node with proper hierarchy tracking
        let module_node = ModuleNode {
            id: module_id,
            name: module_name.clone(),
            #[cfg(feature = "module_path_tracking")]
            path: self.state.current_module_path.clone(),
            visibility: self.state.convert_visibility(&module.vis),
            attributes: extract_attributes(&module.attrs),
            docstring: extract_docstring(&module.attrs),
            submodules,
            items,
            imports: Vec::new(),
            exports: Vec::new(),
        };

        // Restore parent path after processing module
        #[cfg(feature = "module_path_tracking")]
        {
            self.state.current_module_path = parent_path;
        }

        // WARNING: experimenting with this
        self.state.current_module.push(module_node.name.clone());
        #[cfg(feature = "verbose_debug")]
        {
            self.log_push("current module", &self.state.current_module);
            println!(
                "{:+^13} self.state.current_module now: {:?}",
                "", self.state.current_module
            );
        }
        self.state
            .current_module_path
            .push(module_node.name.clone());
        #[cfg(feature = "verbose_debug")]
        {
            println!(
                "{:+^10} (+) pushed self.state.current_module_path <- {:?}",
                "",
                module_node.name.clone()
            );
            println!(
                "{:+^13} self.state.current_module_path now: {:?}",
                "", self.state.current_module_path
            );
        }

        self.state.code_graph.modules.push(module_node);
        // continue visiting.
        visit::visit_item_mod(self, module);

        // WARNING: experimenting with this
        let popped = self.state.current_module.pop();
        #[cfg(feature = "verbose_debug")]
        self.log_pop("current_module", popped, &self.state.current_module);

        let popped = self.state.current_module_path.pop();
        #[cfg(feature = "verbose_debug")]
        self.log_pop(
            "current_module_path",
            popped,
            &self.state.current_module_path,
        );
    }

    // other visit_item_* methods..
}
```

I have provided the requested files and done my best to clarify your questions given the current lack of a complete design. Note that the cfg flags "visibility_resolution" and "module_tracking" will help you put together how the current module tracking implementation and visibility resolution works, and that the "use_statement_tracking" cfg flag will help you understand how `use` statements are tied into the current implementation. There may be some gaps in the integration between the different cfg flags, but they are meant to be used together to fully resolve resolution.

Please evaluate what would be required to implement file tracking and integrate it into our module path tracking, visibility resolution, and `use` statement tracking to provide all the data needed for us to query our embedded `cozo` database for the validity of a given code item at a given location in the user's code.
