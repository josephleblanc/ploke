use super::attribute_processing::extract_attributes;
use super::attribute_processing::extract_docstring;
use super::state::VisitorState;
use super::type_processing::get_or_create_type;
use crate::parser::nodes::ModuleNode;
use crate::parser::nodes::ValueKind;
use crate::parser::nodes::ValueNode;
use crate::parser::nodes::{
    EnumNode, FieldNode, FunctionNode, ImplNode, ImportKind, ImportNode, MacroKind, MacroNode,
    ProcMacroKind, StructNode, TraitNode, TypeAliasNode, TypeDefNode, UnionNode, VariantNode,
};
use crate::parser::relations::*;
use crate::parser::types::*;
use crate::parser::ExtractSpan;

use crate::parser::nodes::ModuleDef;
use ploke_core::{NodeId, TypeId};

use quote::ToTokens;
use syn::spanned::Spanned;
use syn::TypePath;
use syn::{
    visit::{self, Visit},
    ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, ReturnType, Type,
};

pub struct CodeVisitor<'a> {
    state: &'a mut VisitorState,
}

impl<'a> CodeVisitor<'a> {
    pub fn new(state: &'a mut VisitorState) -> Self {
        Self { state }
    }

    // Helper method to extract path segments from a use tree

    fn process_use_tree(&mut self, tree: &syn::UseTree, base_path: &[String]) -> Vec<ImportNode> {
        let mut imports = Vec::new();

        match tree {
            syn::UseTree::Path(path) => {
                let mut new_base = base_path.to_vec();
                new_base.push(path.ident.to_string());

                imports.extend(self.process_use_tree(&path.tree, &new_base));
            }
            syn::UseTree::Name(name) => {
                let mut full_path = base_path.to_vec();
                let use_name = name.ident.to_string();
                let mut is_self_import = false;

                let span = name.extract_span_bytes();

                let checked_name = if use_name == "self" {
                    is_self_import = true;
                    full_path.last().unwrap().clone()
                } else {
                    full_path.push(use_name.clone());
                    use_name
                };
                let import_id = self.add_contains_rel(&checked_name, span);

                imports.push(ImportNode {
                    id: import_id,
                    path: full_path,
                    kind: ImportKind::UseStatement,
                    visible_name: checked_name,
                    original_name: None,
                    is_glob: false,
                    span,
                    is_self_import,
                });
            }
            syn::UseTree::Rename(rename) => {
                let mut full_path = base_path.to_vec();
                let use_rename = rename.ident.to_string();

                let span = rename.extract_span_bytes();
                let import_id = self.add_contains_rel(&use_rename, span);

                full_path.push(use_rename);

                imports.push(ImportNode {
                    id: import_id,
                    path: full_path,
                    kind: ImportKind::UseStatement,
                    visible_name: rename.rename.to_string(),
                    original_name: Some(rename.ident.to_string()),
                    is_glob: false,
                    span,
                    is_self_import: false,
                });
            }
            syn::UseTree::Glob(star) => {
                let span = star.extract_span_bytes();
                let import_id = self.add_contains_rel("*", span);

                let mut full_path = base_path.to_vec();
                full_path.push("*".to_string());
                imports.push(ImportNode {
                    id: import_id,
                    path: base_path.to_vec(),
                    kind: ImportKind::UseStatement,
                    visible_name: "*".to_string(),
                    original_name: None,
                    is_glob: true,
                    span: tree.extract_span_bytes(),
                    is_self_import: false,
                });
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    imports.extend(self.process_use_tree(item, base_path));
                }
            }
        }

        imports
    }

    #[cfg(feature = "verbose_debug")]
    fn debug_mod_stack(&mut self) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            let modules: Vec<(NodeId, String)> = self
                .state
                .code_graph
                .modules
                .iter()
                .map(|m| (m.id, m.name.clone()))
                .collect();

            let depth = self.state.code_graph.modules.len();
            //     .iter()
            //     .enumerate()
            //     .find(|(_i, m)| m.id == current_mod.id)
            //     .map(|(i, _m)| i)
            //     .unwrap();
            (1..depth).for_each(|_| print!("{: <3}", ""));
            println!("│");
            (1..depth).for_each(|_| print!("{: <3}", ""));
            print!("└");
            print!("{:─<3}", "");

            println!(" current_mod.name: {:?}", current_mod.name);
            print!("{: <3}", "");
            (0..depth).for_each(|_| print!("{: <3}", ""));
            println!("│   id: {}", current_mod.id);
            print!("{: <3}", "");
            (0..depth).for_each(|_| print!("{: <3}", ""));
            print!("│   items: ",);
            if let Some(item) = current_mod.items().as_ref() {
                item.iter().for_each(|i| print!("|{}|", i))
            }
            print!("{: <3}", "");
            (0..depth).for_each(|_| print!("{: <3}", ""));
            println!("│   self.state.code_graph.modules names: {:?}", modules);
        }
    }
    #[cfg(feature = "verbose_debug")]
    fn debug_mod_stack_push(&mut self, name: String, node_id: NodeId) {
        let depth = self.state.code_graph.modules.len();
        if let Some(current_mod) = self
            .state
            .code_graph
            .modules
            .iter()
            .find(|m| m.items().is_some_and(|items| items.contains(&node_id)))
        {
            (1..depth).for_each(|_| print!("{: <3}", ""));
            println!("│");
            (1..depth).for_each(|_| print!("{: <3}", ""));
            print!("└");
            print!("{:─<3}", "");

            print!(
                " current mod: \"{}\" -> pushing name {} (id: {}) to items: now items = ",
                current_mod.name, name, node_id,
            );
            if let Some(item) = current_mod.items().as_ref() {
                item.iter().for_each(|i| print!("|{}|", i))
            }
            println!();
        } else {
            panic!(
                "Could not find containing module for node with name {}, id {}",
                name, node_id
            );
        }
    }
    #[cfg(feature = "verbose_debug")]
    fn debug_new_id(&mut self, name: &str, node_id: NodeId) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            let depth = self.state.code_graph.modules.len();

            (1..depth).for_each(|_| print!("{: <3}", ""));
            println!("│");
            (1..depth).for_each(|_| print!("{: <3}", ""));
            print!("└");
            print!("{:─<3}", "");

            println!(
                " in {} ++ new id created name: \"{}\" (id: {:?})",
                current_mod.name, name, node_id
            );
        }
    }
    #[cfg(feature = "verbose_debug")]
    fn log_push(&self, name: &str, stack: &[String]) {
        println!("{:+^10} (+) pushed {} <- {:?}", "", name, stack.last());
        println!("{:+^13} {} now: {:?}", "", name, stack);
    }

    #[cfg(feature = "verbose_debug")]
    fn log_pop(&self, name: &str, popped: Option<String>, stack: &[String]) {
        println!("{:+^10} (-) popped {} -> {:?}", "", name, popped);
        println!("{:+^13} {} now: {:?}", "", name, stack);
    }
    /// Generates a new `NodeId::Synthetic` for the item being visited,
    /// ensuring it's immediately linked to the current module via a `Contains` relation.
    /// Requires the item's name and span for UUID generation.
    fn add_contains_rel(&mut self, item_name: &str, item_span: (usize, usize)) -> NodeId {
        // 1. Generate the Synthetic NodeId using context from state
        //    Requires crate_namespace, current_file_path, current_module_path, item_name, item_span
        let node_id = NodeId::generate_synthetic(
            self.state.crate_namespace,      // Provided by VisitorState::new
            &self.state.current_file_path,   // Provided by VisitorState::new
            &self.state.current_module_path, // Tracked during visitation
            item_name,                       // Passed as argument
            item_span,                       // Passed as argument
        );

        // 2. Add the Contains relation using the new ID and GraphId wrapper
        // Find the parent module based on the *current path*, not just the last pushed module.
        let parent_module_id = self
            .state
            .code_graph
            .modules
            .iter()
            .find(|m| m.path == self.state.current_module_path) // Find module matching current path
            .map(|m| m.id);

        if let Some(parent_id) = parent_module_id {
            if let Some(_parent_mod) = self
                .state
                .code_graph
                .modules
                .iter_mut()
                .find(|m| m.id == parent_id)
            {
                if let Some(parent_mod) = self
                    .state
                    .code_graph
                    .modules
                    .iter_mut()
                    .find(|m| m.id == parent_id)
                {
                    // if let ModuleDef::Inline { items, .. } = &mut parent_mod.module_def {
                    //     items.push(node_id);
                    // }
                    // match for both Inline and FileBased, since either can contain the target
                    // items.
                    match &mut parent_mod.module_def {
                        ModuleDef::Inline { items, .. } => items.push(node_id),
                        ModuleDef::FileBased { items, .. } => items.push(node_id),
                        ModuleDef::Declaration { .. } => (),
                    }
                }
            }

            // Create the relation using GraphId wrappers
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(parent_id), // Use the correctly found parent ID
                target: GraphId::Node(node_id),   // Wrap new item ID
                kind: RelationKind::Contains,
            });
            #[cfg(feature = "verbose_debug")]
            {
                // Find parent name again for logging (or pass it down)
                let parent_name = self
                    .state
                    .code_graph
                    .modules
                    .iter()
                    .find(|m| m.id == parent_id)
                    .map(|m| m.name.as_str())
                    .unwrap_or("<unknown>");
                println!("REL_CREATE: Contains relation created for source: {} -> target: {},\n\tsource_id: {}  target_id: {}",
                      parent_name, item_name, parent_id, node_id
                  );
            }

            // Keep the debug hook, assuming debug_mod_stack_push is updated
            // to handle the NodeId enum (e.g., using its Display impl).
            #[cfg(feature = "verbose_debug")]
            self.debug_mod_stack_push(item_name.to_owned(), node_id);
        } else {
            // This case should ideally not happen after the root module is created in analyze_file_phase2,
            // but log a warning just in case.
            eprintln!(
                "Warning: Attempted to add contains relation for item '{}', but no current module found in VisitorState.",
                item_name
            );
        }

        // 3. Return the newly generated NodeId enum
        node_id
    }
}

#[allow(clippy::needless_lifetimes)]
impl<'a, 'ast> Visit<'ast> for CodeVisitor<'a> {
    // Visit function definitions
    fn visit_item_fn(&mut self, func: &'ast ItemFn) {
        // Check if this function is a procedural macro
        let is_proc_macro = func.attrs.iter().any(|attr| {
            attr.path().is_ident("proc_macro")
                || attr.path().is_ident("proc_macro_derive")
                || attr.path().is_ident("proc_macro_attribute")
        });

        // TODO: Validate Correctness:
        // This if block runs, then so does the following code.
        // Are we processing the proc_macro functions twice?
        // We don't want to deal with the more complex aspects of macros, but do want to note their
        // location, name, and other metadata that might be useful for the RAG to get easy wins.
        if is_proc_macro {
            let macro_name = func.sig.ident.to_string();

            let span = func.extract_span_bytes();

            let macro_id = self.add_contains_rel(&macro_name, span);

            #[cfg(feature = "verbose_debug")]
            self.debug_new_id(&macro_name, macro_id);

            // Determine the kind of procedural macro
            let proc_macro_kind = if func
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("proc_macro_derive"))
            {
                ProcMacroKind::Derive
            } else if func
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("proc_macro_attribute"))
            {
                ProcMacroKind::Attribute
            } else {
                ProcMacroKind::Function
            };

            // Extract doc comments and other attributes
            let docstring = extract_docstring(&func.attrs);
            let attributes = extract_attributes(&func.attrs);

            // Extract function body as a string
            let body = Some(func.block.to_token_stream().to_string());

            // Create the macro node
            let macro_node = MacroNode {
                id: macro_id,
                name: macro_name,
                visibility: self.state.convert_visibility(&func.vis),
                kind: MacroKind::ProcedureMacro {
                    kind: proc_macro_kind,
                },
                attributes,
                docstring,
                body,
                span,
                tracking_hash: Some(self.state.generate_tracking_hash(&func.to_token_stream())),
            };

            // Add the macro to the code graph
            self.state.code_graph.macros.push(macro_node);
        }

        let fn_name = func.sig.ident.to_string();
        let byte_range = func.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        let fn_id = self.add_contains_rel(&fn_name, span);
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&fn_name, fn_id);
        // Register function with current module

        // Process function parameters
        let mut parameters = Vec::new();
        for arg in &func.sig.inputs {
            if let Some(param) = self.state.process_fn_arg(arg) {
                // Add relation between function and parameter

                self.state.code_graph.relations.push(Relation {
                    source: GraphId::Node(fn_id),
                    target: GraphId::Type(param.type_id),
                    kind: RelationKind::FunctionParameter,
                });
                parameters.push(param);
            }
        }

        // Extract return type if it exists
        let return_type = match &func.sig.output {
            ReturnType::Default => None,
            ReturnType::Type(_, ty) => {
                let type_id = get_or_create_type(self.state, ty);
                // Add relation between function and return type
                self.state.code_graph.relations.push(Relation {
                    source: GraphId::Node(fn_id),
                    target: GraphId::Type(type_id),
                    kind: RelationKind::FunctionReturn,
                });
                Some(type_id)
            }
        };

        // Process generic parameters
        let generic_params = self.state.process_generics(&func.sig.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&func.attrs);
        let attributes = extract_attributes(&func.attrs);

        // Extract function body as a string
        let body = Some(func.block.to_token_stream().to_string());

        // Store function info
        self.state.code_graph.functions.push(FunctionNode {
            id: fn_id,
            name: fn_name,
            span,
            visibility: self.state.convert_visibility(&func.vis),
            parameters,
            return_type,
            generic_params,
            attributes,
            docstring,
            body,
            tracking_hash: Some(self.state.generate_tracking_hash(&func.to_token_stream())),
        });

        // Continue visiting the function body
        visit::visit_item_fn(self, func);
    }

    // Visit struct definitions
    fn visit_item_struct(&mut self, item_struct: &'ast ItemStruct) {
        let struct_name = item_struct.ident.to_string();

        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        let struct_id = self.add_contains_rel(&struct_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&struct_name, struct_id);

        // Process fields
        let mut fields = Vec::new();
        for field in &item_struct.fields {
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());
            let span = field.extract_span_bytes();

            let field_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or(format!("{} unnamed field", struct_name)),
                span,
            );

            #[cfg(feature = "verbose_debug")]
            self.debug_new_id(
                &field
                    .ident
                    .as_ref()
                    .map(|ident| ident.to_string())
                    .unwrap_or("unnamed_struct_field".to_string()),
                field_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);

            // TODO: Remove Nodes from fields for new version
            // Support as full nodes for now
            let field_node = FieldNode {
                id: field_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
            };

            // Add relation between struct and field
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(struct_id),
                target: GraphId::Node(field_id),
                kind: RelationKind::StructField,
            });

            fields.push(field_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_struct.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_struct.attrs);
        let attributes = extract_attributes(&item_struct.attrs);

        // Store all structs regardless of visibility
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Struct(StructNode {
                id: struct_id,
                name: struct_name,
                span,
                visibility: self.state.convert_visibility(&item_struct.vis),
                fields,
                generic_params,
                attributes,
                docstring,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_struct.to_token_stream()),
                ),
            }));
        visit::visit_item_struct(self, item_struct);
    }

    // Visit type alias definitions
    fn visit_item_type(&mut self, item_type: &'ast syn::ItemType) {
        let type_alias_name = item_type.ident.to_string();

        let span = item_type.extract_span_bytes();
        let type_alias_id = self.add_contains_rel(&type_alias_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&type_alias_name, type_alias_id);

        // Process the aliased type
        let type_id = get_or_create_type(self.state, &item_type.ty);

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_type.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_type.attrs);
        let attributes = extract_attributes(&item_type.attrs);

        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::TypeAlias(TypeAliasNode {
                id: type_alias_id,
                name: type_alias_name,
                span,
                visibility: self.state.convert_visibility(&item_type.vis),
                type_id,
                generic_params,
                attributes,
                docstring,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_type.to_token_stream()),
                ),
            }));

        visit::visit_item_type(self, item_type);
    }

    // Visit union definitions
    fn visit_item_union(&mut self, item_union: &'ast syn::ItemUnion) {
        let union_name = item_union.ident.to_string();

        let span = item_union.extract_span_bytes();
        let union_id = self.add_contains_rel(&union_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&union_name, union_id);

        // Process fields
        let mut fields = Vec::new();
        for field in &item_union.fields.named {
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());

            let field_id = self.state.generate_synthetic_node_id(
                &field_name
                    .clone()
                    .unwrap_or(format!("Unnamed field of {}", union_name)),
                span,
            );
            #[cfg(feature = "verbose_debug")]
            self.debug_new_id(
                &field_name
                    .clone()
                    .unwrap_or_else(|| format!("Unnamed field of {}", union_name.clone())),
                field_id,
            );
            let type_id = get_or_create_type(self.state, &field.ty);

            let field_node = FieldNode {
                id: field_id,
                name: field_name,
                type_id,
                visibility: self.state.convert_visibility(&field.vis),
                attributes: extract_attributes(&field.attrs),
            };

            // Add relation between union and field
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(union_id),
                target: GraphId::Node(field_id),
                kind: RelationKind::StructField, // Reuse StructField relation for union fields
            });

            fields.push(field_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_union.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_union.attrs);
        let attributes = extract_attributes(&item_union.attrs);

        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Union(UnionNode {
                id: union_id,
                name: union_name,
                visibility: self.state.convert_visibility(&item_union.vis),
                fields,
                generic_params,
                attributes,
                docstring,
                span,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_union.to_token_stream()),
                ),
            }));

        visit::visit_item_union(self, item_union);
    }

    // Visit enum definitions
    fn visit_item_enum(&mut self, item_enum: &'ast ItemEnum) {
        let enum_name = item_enum.ident.to_string();

        let span = item_enum.extract_span_bytes();
        let enum_id = self.add_contains_rel(&enum_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&enum_name, enum_id);

        // Process variants
        let mut variants = Vec::new();
        for variant in &item_enum.variants {
            let variant_name = variant.ident.to_string();
            let variant_span = variant.extract_span_bytes();

            let variant_id = self
                .state
                .generate_synthetic_node_id(&variant_name, variant_span);

            // Process fields of the variant
            let mut fields = Vec::new();
            match &variant.fields {
                syn::Fields::Named(fields_named) => {
                    for field in &fields_named.named {
                        let field_span = field.extract_span_bytes();
                        let field_name = field.ident.as_ref().map(|ident| ident.to_string());
                        let field_id = self.state.generate_synthetic_node_id(
                            &field_name
                                .clone()
                                .unwrap_or_else(|| format!("Unnamed field of {}", enum_name)),
                            field_span,
                        );
                        #[cfg(feature = "verbose_debug")]
                        self.debug_new_id(
                            &field_name
                                .clone()
                                .unwrap_or("unnamed_enum_field".to_string()),
                            field_id,
                        );
                        let type_id = get_or_create_type(self.state, &field.ty);

                        let field_node = FieldNode {
                            id: field_id,
                            name: field_name,
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs),
                        }; // Add relation between variant and named field
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(variant_id),
                            target: GraphId::Node(field_id),
                            kind: RelationKind::VariantField,
                        });

                        fields.push(field_node);
                    }
                }
                syn::Fields::Unnamed(fields_unnamed) => {
                    for field in fields_unnamed.unnamed.iter() {
                        let unnamed = "Tuple field".to_string();
                        let field_id = self.state.generate_synthetic_node_id(&unnamed, span);
                        let type_id = get_or_create_type(self.state, &field.ty);
                        #[cfg(feature = "verbose_debug")]
                        self.debug_new_id("unnamed_enum_field", field_id);

                        let field_node = FieldNode {
                            id: field_id,
                            name: None, // Tuple fields don't have names
                            type_id,
                            visibility: self.state.convert_visibility(&field.vis),
                            attributes: extract_attributes(&field.attrs),
                        };
                        // Add relation between variant and unnamed field
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(variant_id),
                            target: GraphId::Node(field_id),
                            kind: RelationKind::VariantField,
                        });

                        fields.push(field_node);
                    }
                }
                syn::Fields::Unit => {
                    // Unit variants don't have fields
                }
            }

            // Extract discriminant if any
            let discriminant = variant
                .discriminant
                .as_ref()
                .map(|(_, expr)| expr.to_token_stream().to_string());

            let variant_node = VariantNode {
                id: variant_id,
                name: variant_name,
                fields,
                discriminant,
                attributes: extract_attributes(&variant.attrs),
            };

            // Add relation between enum and variant
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(enum_id),
                target: GraphId::Node(variant_id),
                kind: RelationKind::EnumVariant,
            });

            variants.push(variant_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_enum.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_enum.attrs);
        let attributes = extract_attributes(&item_enum.attrs);

        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Enum(EnumNode {
                id: enum_id,
                name: enum_name,
                span,
                visibility: self.state.convert_visibility(&item_enum.vis),
                variants,
                generic_params,
                attributes,
                docstring,
                tracking_hash: Some(
                    self.state
                        .generate_tracking_hash(&item_enum.to_token_stream()),
                ),
            }));

        visit::visit_item_enum(self, item_enum);
    }

    // Visit impl blocks
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        let impl_name = name_impl(item_impl);
        let span = item_impl.extract_span_bytes();

        let impl_id = self.add_contains_rel(&impl_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id("unnamed_impl", impl_id);

        // Process self type,
        // // Case 1: Simple struct
        // impl MyStruct {}
        // // item_impl.self_ty = Type::Path for "MyStruct"
        //
        // // Case 2: Generic struct
        // impl<T> MyStruct<T> {}
        // // item_impl.self_ty = Type::Path for "MyStruct<T>"
        //
        // // Case 3: Trait impl
        // impl MyTrait for MyStruct {}
        // // item_impl.self_ty = Type::Path for "MyStruct"
        // // item_impl.trait_ = Some for "MyTrait"
        let self_type_id = get_or_create_type(self.state, &item_impl.self_ty);

        // Process trait type if it's a trait impl
        let trait_type_id = item_impl.trait_.as_ref().map(|(_, path, _)| {
            let ty = Type::Path(TypePath {
                qself: None,
                path: path.clone(),
            });

            // Return TraitId
            get_or_create_type(self.state, &ty)
        });

        // Handle trait visibility filtering when feature is enabled
        if let Some(trait_type_id) = trait_type_id {
            if let Some(trait_type) = self
                .state
                .code_graph
                .type_graph
                .iter()
                .find(|t| t.id == trait_type_id)
            {
                if let TypeKind::Named { path, .. } = &trait_type.kind {
                    let trait_name = path.last().unwrap_or(&String::new()).to_string();

                    // Check both public and private traits
                    // TODO: Add implements trait relation here
                    #[allow(unused_variables, reason = "useful later")]
                    let trait_def = self
                        .state
                        .code_graph
                        .traits
                        .iter()
                        .chain(&self.state.code_graph.private_traits)
                        .find(|t| t.name == trait_name);
                }
            }
        }
        // Process methods
        let mut methods = Vec::new();
        for item in &item_impl.items {
            // NOTE: There are NO other match arms or if-let chains here
            //       to handle syn::ImplItem::Const or syn::ImplItem::Type
            if let syn::ImplItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                let method_span = method.extract_span_bytes();
                let method_node_id = self.add_contains_rel(&method_name, method_span);

                #[cfg(feature = "verbose_debug")]
                self.debug_new_id(&method_name, method_node_id);

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // Add relation between method and parameter
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(param.type_id),
                            kind: RelationKind::FunctionParameter,
                        });
                        parameters.push(param);
                    }
                }

                // Extract return type if it exists
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => {
                        let type_id = get_or_create_type(self.state, ty);
                        // Add relation between method and return type
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(type_id),
                            kind: RelationKind::FunctionReturn,
                        });
                        Some(type_id)
                    }
                };
                self.state.code_graph.relations.push(Relation {
                    source: GraphId::Type(self_type_id), // The struct/enum type
                    target: GraphId::Node(method_node_id),
                    kind: RelationKind::Method,
                });
                // Process generic parameters for methods
                let generic_params = self.state.process_generics(&method.sig.generics);

                // Extract doc comments and other attributes for methods
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes(&method.attrs);

                // Extract method body as a string
                let body = Some(method.block.to_token_stream().to_string());

                // Store method info
                let method_node = FunctionNode {
                    id: method_node_id,
                    name: method_name.clone(),
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&method.vis),
                    parameters,
                    return_type,
                    generic_params,
                    attributes,
                    docstring,
                    body,
                    tracking_hash: Some(
                        self.state
                            .generate_tracking_hash(&method_name.to_token_stream()),
                    ),
                };

                methods.push(method_node);
            }
        }

        // Process generic parameters for impl block
        let generic_params = self.state.process_generics(&item_impl.generics);

        // Store impl info
        let impl_node = ImplNode {
            id: impl_id,
            span: item_impl.extract_span_bytes(),
            self_type: self_type_id,
            trait_type: trait_type_id,
            methods,
            generic_params,
        };
        self.state.code_graph.impls.push(impl_node);

        // Add relation: ImplementsFor or ImplementsTrait
        let relation_kind = if trait_type_id.is_some() {
            RelationKind::ImplementsTrait
        } else {
            RelationKind::ImplementsFor
        };
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(impl_id),
            target: GraphId::Type(self_type_id),
            kind: relation_kind,
        });
        if let Some(trait_type_id) = trait_type_id {
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(impl_id),
                target: GraphId::Type(trait_type_id),
                kind: RelationKind::ImplementsTrait,
            });
        }

        visit::visit_item_impl(self, item_impl);
    }

    // Visit trait definitions
    fn visit_item_trait(&mut self, item_trait: &'ast ItemTrait) {
        let trait_name = item_trait.ident.to_string();
        let span = item_trait.extract_span_bytes();

        let trait_id = self.add_contains_rel(&trait_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&trait_name, trait_id);

        // Process methods
        let mut methods = Vec::new();
        for item in &item_trait.items {
            if let syn::TraitItem::Fn(method) = item {
                let method_name = method.sig.ident.to_string();
                let method_span = method.extract_span_bytes();

                let method_node_id = self
                    .state
                    .generate_synthetic_node_id(&method_name, method_span);

                #[cfg(feature = "verbose_debug")]
                self.debug_new_id(&method_name, method_node_id);

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // Add relation between method and parameter
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(param.type_id),
                            kind: RelationKind::FunctionParameter,
                        });
                        parameters.push(param);
                    }
                }

                // Extract return type if it exists
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => {
                        let type_id = get_or_create_type(self.state, ty);
                        // Add relation between method and return type
                        self.state.code_graph.relations.push(Relation {
                            source: GraphId::Node(method_node_id),
                            target: GraphId::Type(type_id),
                            kind: RelationKind::FunctionReturn,
                        });
                        Some(type_id)
                    }
                };

                // Process generic parameters for methods
                let generic_params = self.state.process_generics(&method.sig.generics);

                // Extract doc comments and other attributes for methods
                let docstring = extract_docstring(&method.attrs);
                let attributes = extract_attributes(&method.attrs);

                // Extract method body if available (trait methods may have default implementations)
                let body = method
                    .default
                    .as_ref()
                    .map(|block| block.to_token_stream().to_string());

                // Store method info
                let method_node = FunctionNode {
                    id: method_node_id,
                    name: method_name,
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&item_trait.vis),
                    parameters,
                    return_type,
                    generic_params,
                    attributes,
                    docstring,
                    body,
                    tracking_hash: Some(
                        self.state
                            .generate_tracking_hash(&method.clone().to_token_stream()),
                    ),
                };
                methods.push(method_node);
            }
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_trait.generics);

        // Process super traits
        let super_traits: Vec<TypeId> = item_trait
            .supertraits
            .iter()
            .filter_map(|bound| {
                // Use filter_map to handle non-trait bounds if necessary
                match bound {
                    syn::TypeParamBound::Trait(trait_bound) => {
                        // Construct a Type::Path from the TraitBound's path
                        // This correctly represents the supertrait type itself.
                        let ty = syn::Type::Path(syn::TypePath {
                            qself: None, // Supertraits typically don't have qself
                            path: trait_bound.path.clone(),
                        });
                        Some(get_or_create_type(self.state, &ty))
                    }
                    syn::TypeParamBound::Lifetime(_) => {
                        // We don't store lifetime bounds as supertrait TypeIds currently
                        None
                    }
                    // Handle other TypeParamBound variants like Verbatim if needed in the future
                    _ => {
                        // Log or handle unexpected bound types if necessary
                        None
                    }
                }
            })
            .collect();

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_trait.attrs);
        let attributes = extract_attributes(&item_trait.attrs);

        // Store trait info
        let trait_node = TraitNode {
            id: trait_id,
            name: trait_name.clone(),
            span: item_trait.extract_span_bytes(),
            visibility: self.state.convert_visibility(&item_trait.vis),
            methods,
            generic_params,
            super_traits: super_traits.clone(),
            attributes,
            docstring,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_trait.to_token_stream()),
            ),
        };
        self.state.code_graph.traits.push(trait_node);
        // }

        // Add relation for super traits
        for super_trait_id in &super_traits {
            self.state.code_graph.relations.push(Relation {
                source: GraphId::Node(trait_id),
                target: GraphId::Type(*super_trait_id),
                kind: RelationKind::Inherits,
            });
        }

        visit::visit_item_trait(self, item_trait);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        let module_name = module.ident.to_string();
        let span = module.extract_span_bytes();

        let module_id = self.add_contains_rel(&module_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_mod_stack();

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&module_name, module_id);

        // Save current path before entering module
        let parent_path = self.state.current_module_path.clone();

        // Update path for nested module visitation
        self.state.current_module_path.push(module_name.clone());

        // Process module contents

        // Create module node with proper path tracking
        // Create module node with proper hierarchy tracking
        let module_def = match &module.content {
            Some(_) => ModuleDef::Inline {
                items: Vec::new(),
                span,
            },
            None => ModuleDef::Declaration {
                declaration_span: span,
                resolved_definition: None, // Resolved during phase 3 resolution
            },
        };
        let module_node = ModuleNode {
            id: module_id,
            name: module_name.clone(),
            path: self.state.current_module_path.clone(),
            visibility: self.state.convert_visibility(&module.vis),
            attributes: extract_attributes(&module.attrs),
            docstring: extract_docstring(&module.attrs),
            imports: Vec::new(),
            exports: Vec::new(),
            span, // Assign the extracted span
            tracking_hash: Some(self.state.generate_tracking_hash(&module.to_token_stream())),
            module_def,
        };

        // Restore parent path after processing module
        self.state.current_module_path = parent_path;

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

        let _popped = self.state.current_module.pop();
        #[cfg(feature = "verbose_debug")]
        self.log_pop("current_module", _popped, &self.state.current_module);

        let _popped = self.state.current_module_path.pop();
        #[cfg(feature = "verbose_debug")]
        self.log_pop(
            "current_module_path",
            _popped,
            &self.state.current_module_path,
        );
    }

    /// Visits `use` statements during AST traversal.
    ///
    /// # Current Limitations
    /// - Does not handle macro-generated `use` statements (MVP exclusion)
    /// - `pub use` re-exports are treated as regular imports
    ///
    /// # Flow
    /// 1. Captures raw path segments and spans
    /// 2. Normalizes `self`/`super` prefixes
    /// 3. Stores statements in `VisitorState` for later resolution
    fn visit_item_use(&mut self, use_item: &'ast syn::ItemUse) {
        // Process the use tree first
        let base_path = if use_item.leading_colon.is_some() {
            vec!["".to_string()] // Absolute path
        } else {
            Vec::new() // Relative path
        };
        let imports = self.process_use_tree(&use_item.tree, &base_path);

        // Get a mutable reference to the graph only once
        let graph = &mut self.state.code_graph;
        let current_module_path = &self.state.current_module_path;

        // Add all imports to the current module
        if let Some(module) = graph
            .modules
            .iter_mut()
            .find(|m| &m.path == current_module_path)
        {
            let module_id = module.id;

            for import in imports {
                // Add module import relation
                graph.relations.push(Relation {
                    source: GraphId::Node(module_id),
                    target: GraphId::Node(import.id),
                    kind: RelationKind::ModuleImports,
                });

                graph.use_statements.push(import.clone());
                // Add to module's imports list
                module.imports.push(import);
            }
        }
        visit::visit_item_use(self, use_item);
    }
    // Continue visiting

    /// Visit extern crate item, e.g. `extern crate serde;`
    /// Note that these may be renamed, e.g. `extern crate serde as MySerde;`
    /// This visit method will:
    ///     - Create an `ImportNode` for the extern crate and add it to current_module.imports
    ///     - Adds a new `Contains` relation with current module (through `add_contains_rel`)
    ///     - `NodeId::Synthetic` created through `add_contains_rel`
    fn visit_item_extern_crate(&mut self, extern_crate: &'ast syn::ItemExternCrate) {
        let crate_name = extern_crate.ident.to_string();
        let span = extern_crate.extract_span_bytes();

        let import_id = self.add_contains_rel(&crate_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&crate_name, import_id);

        let import_node = ImportNode {
            id: import_id,
            span,
            path: vec![crate_name.clone()], // Path is just crate name
            kind: ImportKind::ExternCrate,  // <<< Correct kind
            // Name used by `use` statements
            visible_name: extern_crate
                .rename
                .as_ref()
                .map(|(_, id)| id.to_string())
                .unwrap_or_else(|| crate_name.clone()),
            // Original name, only Some if item is renamed, otherwise None
            original_name: extern_crate.rename.as_ref().map(|_| crate_name.clone()),
            is_glob: false,
            is_self_import: false,
        };
        let module_id = if let Some(module) = self
            .state
            .code_graph
            .modules
            .iter_mut()
            .find(|m| m.items().is_some_and(|items| items.contains(&import_id)))
        {
            module.imports.push(import_node.clone());
            module.id
        } else {
            panic!(
                "Could not find containing module for import_node: {:#?}",
                import_node
            );
        };
        self.state.code_graph.use_statements.push(import_node);
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(module_id),
            target: GraphId::Node(import_id),
            kind: RelationKind::ModuleImports,
        });

        let type_id = {
            // 1. Construct a representative syn::Type for the external crate.
            //    Using just the crate name as the path is simplest.
            let syn_type_path = syn::parse_str::<syn::TypePath>(&crate_name).unwrap_or_else(|_| {
                // Fallback if crate_name isn't a valid path segment (highly unlikely)
                eprintln!(
                    "Warning: Could not parse extern crate name '{}' as a TypePath.",
                    crate_name
                );
                syn::parse_str::<syn::TypePath>("__invalid_extern_crate_name__").unwrap()
            });
            let syn_type = syn::Type::Path(syn_type_path);

            // 2. Use the standard function to get/create the TypeId and register the TypeNode.
            //    This handles caching and ensures the TypeNode is added to type_graph.
            //    The synthetic ID will be based on hashing "crate_name".
            get_or_create_type(self.state, &syn_type)
        };

        // --- Relation Creation ---
        // TODO: Remove this relation
        // I did not really understand that an `extern crate some_crate` statement actually worked
        // on the entire crate namespace itself, and cannot be specific enough to import a type
        // directly. Therefore, the `TypeId` generation doesn't make sense here. `TypeId` is
        // reserved for actual types.
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(import_id),
            target: GraphId::Type(type_id), // type_id is now guaranteed to be registered
            kind: RelationKind::Uses,
        });

        // Continue visiting
        visit::visit_item_extern_crate(self, extern_crate);
    }

    // Visit constant items
    fn visit_item_const(&mut self, item_const: &'ast syn::ItemConst) {
        let const_name = item_const.ident.to_string();
        let span = item_const.extract_span_bytes();
        let const_id = self.add_contains_rel(&const_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&const_name, const_id);

        // Process the type
        let type_id = get_or_create_type(self.state, &item_const.ty);

        // Extract the value expression as a string
        let value = Some(item_const.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_const.attrs);
        let attributes = extract_attributes(&item_const.attrs);

        // Create the constant node
        let const_node = ValueNode {
            id: const_id,
            name: const_name,
            visibility: self.state.convert_visibility(&item_const.vis),
            type_id,
            kind: ValueKind::Constant,
            value,
            attributes,
            docstring,
            span,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_const.to_token_stream()),
            ),
        };

        // Add the constant to the code graph
        self.state.code_graph.values.push(const_node);

        // Add relation between constant and its type
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(const_id),
            target: GraphId::Type(type_id),
            kind: RelationKind::ValueType,
        });

        // Continue visiting
        visit::visit_item_const(self, item_const);
    }

    // Visit static items
    fn visit_item_static(&mut self, item_static: &'ast syn::ItemStatic) {
        let static_name = item_static.ident.to_string();
        let span = item_static.extract_span_bytes();

        let static_id = self.add_contains_rel(&static_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&static_name, static_id);

        // Process the type
        let type_id = get_or_create_type(self.state, &item_static.ty);

        // Extract the value expression as a string
        let value = Some(item_static.expr.to_token_stream().to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_static.attrs);
        let attributes = extract_attributes(&item_static.attrs);

        // Create the static node
        let static_node = ValueNode {
            id: static_id,
            name: static_name,
            visibility: self.state.convert_visibility(&item_static.vis),
            type_id,
            kind: ValueKind::Static {
                is_mutable: matches!(item_static.mutability, syn::StaticMutability::Mut(_)),
            },
            value,
            attributes,
            docstring,
            span,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_static.to_token_stream()),
            ),
        };

        // Add the static to the code graph
        self.state.code_graph.values.push(static_node);

        // Add relation between static and its type
        self.state.code_graph.relations.push(Relation {
            source: GraphId::Node(static_id),
            target: GraphId::Type(type_id),
            kind: RelationKind::ValueType,
        });

        // Continue visiting
        visit::visit_item_static(self, item_static);
    }

    // Visit macro definitions (macro_rules!)
    fn visit_item_macro(&mut self, item_macro: &'ast syn::ItemMacro) {
        let is_exported = item_macro
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("macro_export"));

        // Determine visibility based on #[macro_export]
        let visibility = if is_exported {
            VisibilityKind::Public
        } else {
            // Macros defined without #[macro_export] follow module scoping rules.
            // Represent this as Inherited, meaning visibility depends on the containing module.
            VisibilityKind::Inherited
        };

        // Original suggestion had a check here to potentially skip non-exported macros.
        // Let's remove that skip for now and process all encountered macro_rules! definitions,
        // assigning appropriate visibility. We can filter later if needed.
        // if !is_exported && visibility == VisibilityKind::Inherited {
        //     return; // Reconsider this later if we only want exported/explicitly pub macros
        // }

        let macro_name = item_macro
            .ident
            .as_ref()
            .map(|ident| ident.to_string())
            .unwrap_or_else(|| "unnamed_macro".to_string());

        let span = item_macro.extract_span_bytes();
        let macro_id = self.add_contains_rel(&macro_name, span);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&macro_name, macro_id);

        let body = Some(item_macro.mac.tokens.to_string());
        let docstring = extract_docstring(&item_macro.attrs);
        let attributes = extract_attributes(&item_macro.attrs); // Includes #[macro_export] if present

        let macro_node = MacroNode {
            id: macro_id,
            name: macro_name,
            visibility, // Use the correctly determined visibility
            kind: MacroKind::DeclarativeMacro,
            // rules field removed
            attributes,
            docstring,
            body,
            span,
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&item_macro.to_token_stream()),
            ),
        };

        self.state.code_graph.macros.push(macro_node);

        // Do NOT recurse into the macro body with visit::visit_item_macro
    }
}

/// Helper function to name item_impol in visit_item_impl
fn name_impl(item_impl: &ItemImpl) -> String {
    // Naming:
    // `impl MyStruct { ... }`
    // `self_type_str`: `"MyStruct"`
    // `trait_str`: `None`
    //   `impl_name`:** `"impl MyStruct"`
    //
    // `impl<T: Display> MyStruct<T> { ... }`
    // `self_type_str`: `"MyStruct < T >"` (Note: `to_token_stream` includes spaces around `< >`)
    // `trait_str`: `None`
    //   `impl_name`:** `"impl MyStruct < T >"`
    //
    // `impl MyTrait for MyStruct { ... }`
    // `self_type_str`: `"MyStruct"`
    // `trait_str`: `Some("MyTrait")`
    // `impl_name`: `"impl MyTrait for MyStruct"`
    let self_type_str = item_impl.self_ty.to_token_stream().to_string();
    let trait_str = item_impl
        .trait_
        .as_ref()
        .map(|(_, path, _)| path.to_token_stream().to_string());

    match trait_str {
        Some(t) => format!("impl {} for {}", t, self_type_str),
        None => format!("impl {}", self_type_str),
    }
}
