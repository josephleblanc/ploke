use super::attribute_processing::extract_attributes;
use super::attribute_processing::extract_docstring;
use super::state::VisitorState;
use super::type_processing::get_or_create_type;
use crate::parser::nodes::*;
use crate::parser::relations::*;
use crate::parser::types::*;
use crate::parser::ExtractSpan;

use quote::ToTokens;
use syn::spanned::Spanned;
use syn::TypePath;
use syn::{
    visit::{self, Visit},
    ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemTrait, ReturnType, Type, Visibility,
};

pub struct CodeVisitor<'a> {
    state: &'a mut VisitorState,
}

impl<'a> CodeVisitor<'a> {
    pub fn new(state: &'a mut VisitorState) -> Self {
        Self { state }
    }

    // Helper method to extract path segments from a use tree
    fn extract_use_path(use_tree: &syn::UseTree, path_segments: &mut Vec<String>) {
        match use_tree {
            syn::UseTree::Path(path) => {
                path_segments.push(path.ident.to_string());
                CodeVisitor::extract_use_path(&path.tree, path_segments);
            }
            syn::UseTree::Name(name) => {
                path_segments.push(name.ident.to_string());
            }
            syn::UseTree::Rename(rename) => {
                path_segments.push(format!("{} as {}", rename.ident, rename.rename));
            }
            syn::UseTree::Glob(_) => {
                path_segments.push("*".to_string());
            }
            syn::UseTree::Group(group) => {
                for tree in &group.items {
                    let mut new_path = path_segments.clone();
                    CodeVisitor::extract_use_path(tree, &mut new_path);
                }
            }
        }
    }

    #[cfg(feature = "use_statement_tracking")]
    fn process_use_tree(tree: &syn::UseTree, base_path: &[String]) -> Vec<UseStatement> {
        let mut statements = Vec::new();

        match tree {
            syn::UseTree::Path(path) => {
                let mut new_base = base_path.to_vec();
                new_base.push(path.ident.to_string());
                statements.extend(Self::process_use_tree(&path.tree, &new_base));
            }
            syn::UseTree::Name(name) => {
                let mut path = base_path.to_vec();
                path.push(name.ident.to_string());
                statements.push(UseStatement {
                    path,
                    visible_name: name.ident.to_string(),
                    original_name: None,
                    is_glob: false,
                    span: name.extract_span_bytes(),
                });
            }
            syn::UseTree::Rename(rename) => {
                let mut path = base_path.to_vec();
                path.push(rename.ident.to_string());
                statements.push(UseStatement {
                    path,
                    visible_name: rename.rename.to_string(),
                    original_name: Some(rename.ident.to_string()),
                    is_glob: false,
                    span: rename.extract_span_bytes(),
                });
            }
            syn::UseTree::Glob(_) => {
                let mut path = base_path.to_vec();
                path.push("*".to_string());
                statements.push(UseStatement {
                    path,
                    visible_name: "*".to_string(),
                    original_name: None,
                    is_glob: true,
                    span: tree.extract_span_bytes(),
                });
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    statements.extend(Self::process_use_tree(item, base_path));
                }
            }
        }

        statements
    }

    fn debug_mod_stack(&mut self) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            let modules: Vec<(usize, String)> = self
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
            println!("│   id: {:?}", current_mod.id);
            print!("{: <3}", "");
            (0..depth).for_each(|_| print!("{: <3}", ""));
            println!("│   items: {:?}", current_mod.items);
            print!("{: <3}", "");
            (0..depth).for_each(|_| print!("{: <3}", ""));
            println!("│   submodules: {:?}", current_mod.submodules);
            print!("{: <3}", "");
            (0..depth).for_each(|_| print!("{: <3}", ""));
            println!("│   self.state.code_graph.modules names: {:?}", modules);
        }
    }
    fn debug_mod_stack_push(&mut self, name: String, node_id: NodeId) {
        #[cfg(feature = "verbose_debug")]
        {
            if let Some(current_mod) = self.state.code_graph.modules.last() {
                let depth = self.state.code_graph.modules.len();

                (1..depth).for_each(|_| print!("{: <3}", ""));
                println!("│");
                (1..depth).for_each(|_| print!("{: <3}", ""));
                print!("└");
                print!("{:─<3}", "");

                println!(
                    " {} -> pushing name {} (id: {}) to items: now items = {:?}",
                    current_mod.name, name, node_id, current_mod.items
                );
            }
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
                " in {} ++ new id created name: \"{}\" (id: {})",
                current_mod.name, name, node_id
            );
        }
    }
    #[cfg(feature = "verbose_debug")]
    fn debug_submodule(&mut self, name: &str, node_id: NodeId) {
        if let Some(current_mod) = self.state.code_graph.modules.last() {
            let depth = self.state.code_graph.modules.len();

            (1..depth).for_each(|_| print!("{: <3}", ""));
            println!("│");
            (1..depth).for_each(|_| print!("{: <3}", ""));
            print!("└");
            print!("{:─<3}", "");

            println!(
                " {} -> pushing new submodule to submodules : \"{}\" (id: {})",
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

    fn add_contains_rel(&mut self, node_name: Option<&str>) -> NodeId {
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

        if is_proc_macro {
            let macro_id = self.state.next_node_id();
            let macro_name = func.sig.ident.to_string();

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
                rules: Vec::new(), // Procedural macros don't have declarative rules
                attributes,
                docstring,
                body,
            };

            // Add the macro to the code graph
            self.state.code_graph.macros.push(macro_node);
        }

        let fn_name = func.sig.ident.to_string();
        let byte_range = func.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let fn_id = self.add_contains_rel(Some(&fn_name));
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&fn_name, fn_id);
        // Register function with current module

        // Process function parameters
        let mut parameters = Vec::new();
        for arg in &func.sig.inputs {
            if let Some(param) = self.state.process_fn_arg(arg) {
                // Add relation between function and parameter
                self.state.code_graph.relations.push(Relation {
                    source: fn_id,
                    target: param.id,
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
                    source: fn_id,
                    target: type_id,
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
        });

        // Continue visiting the function body
        visit::visit_item_fn(self, func);
    }

    // Visit struct definitions
    fn visit_item_struct(&mut self, item_struct: &'ast ItemStruct) {
        let struct_name = item_struct.ident.to_string();
        let struct_id = self.add_contains_rel(Some(&struct_name));

        #[cfg(feature = "module_path_tracking")]
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&struct_name, struct_id);
        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        // Process fields
        let mut fields = Vec::new();
        for field in &item_struct.fields {
            let field_id = self.state.next_node_id();
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());
            #[cfg(feature = "verbose_debug")]
            self.debug_new_id(
                &field_name
                    .clone()
                    .unwrap_or("unnamed_struct_field".to_string()),
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

            // Add relation between struct and field
            self.state.code_graph.relations.push(Relation {
                source: struct_id,
                target: field_id,
                kind: RelationKind::StructField,
            });

            fields.push(field_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_struct.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_struct.attrs);
        let attributes = extract_attributes(&item_struct.attrs);

        // Store struct info only if public
        if matches!(item_struct.vis, Visibility::Public(_)) {
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
                }));

            visit::visit_item_struct(self, item_struct);
        } else {
            #[cfg(feature = "visibility_resolution")]
            {
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
                    }));
                visit::visit_item_struct(self, item_struct);
            }
        }
    }

    // Visit type alias definitions
    fn visit_item_type(&mut self, item_type: &'ast syn::ItemType) {
        let type_alias_name = item_type.ident.to_string();
        let type_alias_id = self.add_contains_rel(Some(&type_alias_name));
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&type_alias_name, type_alias_id);

        // Process the aliased type
        let type_id = get_or_create_type(self.state, &item_type.ty);

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_type.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_type.attrs);
        let attributes = extract_attributes(&item_type.attrs);

        // Store type alias info only if public
        if matches!(item_type.vis, Visibility::Public(_)) {
            self.state
                .code_graph
                .defined_types
                .push(TypeDefNode::TypeAlias(TypeAliasNode {
                    id: type_alias_id,
                    name: type_alias_name,
                    span: item_type.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&item_type.vis),
                    type_id,
                    generic_params,
                    attributes,
                    docstring,
                }));

            visit::visit_item_type(self, item_type);
        } else {
            #[cfg(feature = "visibility_resolution")]
            {
                self.state
                    .code_graph
                    .defined_types
                    .push(TypeDefNode::TypeAlias(TypeAliasNode {
                        id: type_alias_id,
                        name: type_alias_name,
                        span: item_type.extract_span_bytes(),
                        visibility: self.state.convert_visibility(&item_type.vis),
                        type_id,
                        generic_params,
                        attributes,
                        docstring,
                    }));

                visit::visit_item_type(self, item_type);
            }
        }
    }

    // Visit union definitions
    fn visit_item_union(&mut self, item_union: &'ast syn::ItemUnion) {
        let union_name = item_union.ident.to_string();
        let union_id = self.add_contains_rel(Some(&union_name));
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&union_name, union_id);

        // Process fields
        let mut fields = Vec::new();
        for field in &item_union.fields.named {
            let field_id = self.state.next_node_id();
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());
            #[cfg(feature = "verbose_debug")]
            self.debug_new_id(
                &field_name.clone().unwrap_or("unnamed_union".to_string()),
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
                source: union_id,
                target: field_id,
                kind: RelationKind::StructField, // Reuse StructField relation for union fields
            });

            fields.push(field_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_union.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_union.attrs);
        let attributes = extract_attributes(&item_union.attrs);

        // Store union info only if public
        if matches!(item_union.vis, Visibility::Public(_)) {
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
                }));

            visit::visit_item_union(self, item_union);
        } else {
            #[cfg(feature = "visibility_resolution")]
            {
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
                    }));

                visit::visit_item_union(self, item_union);
            }
        }
    }

    // Visit enum definitions
    fn visit_item_enum(&mut self, item_enum: &'ast ItemEnum) {
        let enum_name = item_enum.ident.to_string();
        let enum_id = self.add_contains_rel(Some(&enum_name));

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&enum_name, enum_id);

        // Process variants
        let mut variants = Vec::new();
        for variant in &item_enum.variants {
            let variant_id = self.state.next_node_id();
            let variant_name = variant.ident.to_string();

            // Process fields of the variant
            let mut fields = Vec::new();
            match &variant.fields {
                syn::Fields::Named(fields_named) => {
                    for field in &fields_named.named {
                        let field_id = self.state.next_node_id();
                        let field_name = field.ident.as_ref().map(|ident| ident.to_string());
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
                        };

                        fields.push(field_node);
                    }
                }
                syn::Fields::Unnamed(fields_unnamed) => {
                    for field in fields_unnamed.unnamed.iter() {
                        let field_id = self.state.next_node_id();
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
                source: enum_id,
                target: variant_id,
                kind: RelationKind::EnumVariant,
            });

            variants.push(variant_node);
        }

        // Process generic parameters
        let generic_params = self.state.process_generics(&item_enum.generics);

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_enum.attrs);
        let attributes = extract_attributes(&item_enum.attrs);

        // Store enum info only if public
        if matches!(item_enum.vis, Visibility::Public(_)) {
            self.state
                .code_graph
                .defined_types
                .push(TypeDefNode::Enum(EnumNode {
                    id: enum_id,
                    name: enum_name,
                    span: item_enum.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&item_enum.vis),
                    variants,
                    generic_params,
                    attributes,
                    docstring,
                }));

            visit::visit_item_enum(self, item_enum);
        } else {
            #[cfg(feature = "visibility_resolution")]
            {
                self.state
                    .code_graph
                    .defined_types
                    .push(TypeDefNode::Enum(EnumNode {
                        id: enum_id,
                        name: enum_name,
                        span: item_enum.extract_span_bytes(),
                        visibility: self.state.convert_visibility(&item_enum.vis),
                        variants,
                        generic_params,
                        attributes,
                        docstring,
                    }));

                visit::visit_item_enum(self, item_enum);
            }
        }
    }

    // Visit impl blocks
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        // TODO: add name here if/when I implement visibility for impl
        let impl_id = self.add_contains_rel(None);

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id("unnamed_impl", impl_id);

        // Process self type
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
        // Handle trait visibility filtering
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
                    let trait_def = self
                        .state
                        .code_graph
                        .traits
                        .iter()
                        .chain(&self.state.code_graph.private_traits)
                        .find(|t| t.name == trait_name);

                    if let Some(trait_def) = trait_def {
                        // Only skip private traits when visibility resolution is DISABLED
                        if !cfg!(feature = "visibility_resolution")
                            && !matches!(trait_def.visibility, VisibilityKind::Public)
                        {
                            return;
                        }
                    } else {
                        // Trait definition not found, skip this impl
                        return;
                    }
                }
            }
        }
        // Process methods
        let mut methods = Vec::new();
        for item in &item_impl.items {
            if let syn::ImplItem::Fn(method) = item {
                let method_node_id = self.state.next_node_id();
                let method_name = method.sig.ident.to_string();
                #[cfg(feature = "verbose_debug")]
                self.debug_new_id(&method_name, method_node_id);

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // Add relation between method and parameter
                        self.state.code_graph.relations.push(Relation {
                            source: method_node_id,
                            target: param.id,
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
                            source: method_node_id,
                            target: type_id,
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

                // Extract method body as a string
                let body = Some(method.block.to_token_stream().to_string());

                // Store method info
                let method_node = FunctionNode {
                    id: method_node_id,
                    name: method_name,
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&method.vis),
                    parameters,
                    return_type,
                    generic_params,
                    attributes,
                    docstring,
                    body,
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
            source: impl_id,
            target: self_type_id,
            kind: relation_kind,
        });
        if let Some(trait_type_id) = trait_type_id {
            self.state.code_graph.relations.push(Relation {
                source: impl_id,
                target: trait_type_id,
                kind: RelationKind::ImplementsTrait,
            });
        }

        visit::visit_item_impl(self, item_impl);
    }

    // Visit trait definitions
    fn visit_item_trait(&mut self, item_trait: &'ast ItemTrait) {
        let trait_name = item_trait.ident.to_string();
        let trait_id = self.add_contains_rel(Some(&trait_name));
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&trait_name, trait_id);

        // Process methods
        let mut methods = Vec::new();
        for item in &item_trait.items {
            if let syn::TraitItem::Fn(method) = item {
                let method_node_id = self.state.next_node_id();
                let method_name = method.sig.ident.to_string();
                #[cfg(feature = "verbose_debug")]
                self.debug_new_id(&method_name, method_node_id);

                // Process method parameters
                let mut parameters = Vec::new();
                for arg in &method.sig.inputs {
                    if let Some(param) = self.state.process_fn_arg(arg) {
                        // Add relation between method and parameter
                        self.state.code_graph.relations.push(Relation {
                            source: method_node_id,
                            target: param.id,
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
                            source: method_node_id,
                            target: type_id,
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
                    visibility: if cfg!(feature = "visibility_resolution") {
                        // For trait methods, visibility matches the trait's visibility
                        self.state.convert_visibility(&item_trait.vis)
                    } else {
                        // Old behavior - assume public
                        VisibilityKind::Public
                    },
                    parameters,
                    return_type,
                    generic_params,
                    attributes,
                    docstring,
                    body,
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
            .map(|bound| {
                let ty = Type::TraitObject(syn::TypeTraitObject {
                    dyn_token: None,
                    bounds: syn::punctuated::Punctuated::from_iter(vec![bound.clone()]),
                });
                get_or_create_type(self.state, &ty)
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
        };
        self.state.code_graph.traits.push(trait_node);
        // }

        // Add relation for super traits
        for super_trait_id in &super_traits {
            self.state.code_graph.relations.push(Relation {
                source: trait_id,
                target: *super_trait_id,
                kind: RelationKind::Inherits,
            });
        }

        visit::visit_item_trait(self, item_trait);
    }

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
        // Create an import node
        let import_id = self.state.next_node_id();
        self.debug_mod_stack_push("NO NAME".to_string(), import_id);

        // Process the use path
        let mut path_segments = Vec::new();
        let current_path = &use_item.tree;

        // Extract path segments from the use tree
        CodeVisitor::extract_use_path(current_path, &mut path_segments);

        // Create relations for the used types
        if !path_segments.is_empty() {
            // Create a synthetic type for the imported item
            let type_id = self.state.next_type_id();
            self.state.code_graph.type_graph.push(TypeNode {
                id: type_id,
                kind: TypeKind::Named {
                    path: path_segments.clone(),
                    is_fully_qualified: false,
                },
                related_types: Vec::new(),
            });

            // Add a Uses relation
            self.state.code_graph.relations.push(Relation {
                source: import_id,
                target: type_id,
                kind: RelationKind::Uses,
            });
        }
        #[cfg(feature = "use_statement_tracking")]
        {
            let base_segments = if use_item.leading_colon.is_some() {
                vec!["".to_string()] // Represents leading ::
            } else {
                Vec::new()
            };

            let statements = Self::process_use_tree(&use_item.tree, &base_segments);
            for mut stmt in statements {
                stmt.span = use_item.extract_span_bytes();
                self.state.code_graph.use_statements.push(stmt);
            }
        }
        // Continue visiting
        visit::visit_item_use(self, use_item);
    }

    // Visit extern crate statements
    fn visit_item_extern_crate(&mut self, extern_crate: &'ast syn::ItemExternCrate) {
        // Create an import node for extern crate
        let import_id = self.state.next_node_id();

        // Get the crate name
        let crate_name = extern_crate.ident.to_string();
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&crate_name, import_id);

        // Create a synthetic type for the extern crate
        let type_id = self.state.next_type_id();
        self.state.code_graph.type_graph.push(TypeNode {
            id: type_id,
            kind: TypeKind::Named {
                path: vec![crate_name.clone()],
                is_fully_qualified: false,
            },
            related_types: Vec::new(),
        });

        // Add a Uses relation
        self.state.code_graph.relations.push(Relation {
            source: import_id,
            target: type_id,
            kind: RelationKind::Uses,
        });

        // Continue visiting
        visit::visit_item_extern_crate(self, extern_crate);
    }

    // Visit constant items
    fn visit_item_const(&mut self, item_const: &'ast syn::ItemConst) {
        let const_id = self.state.next_node_id();
        let const_name = item_const.ident.to_string();

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
        };

        // Check if the constant is public
        if matches!(item_const.vis, Visibility::Public(_)) {
            // Add the constant to the code graph
            self.state.code_graph.values.push(const_node);

            // Add relation between constant and its type
            self.state.code_graph.relations.push(Relation {
                source: const_id,
                target: type_id,
                kind: RelationKind::ValueType,
            });
        } else {
            #[cfg(feature = "visibility_resolution")]
            {
                // Add the constant to the code graph
                self.state.code_graph.values.push(const_node);

                // Add relation between constant and its type
                self.state.code_graph.relations.push(Relation {
                    source: const_id,
                    target: type_id,
                    kind: RelationKind::ValueType,
                });
            }
        }

        // Continue visiting
        visit::visit_item_const(self, item_const);
    }

    // Visit static items
    fn visit_item_static(&mut self, item_static: &'ast syn::ItemStatic) {
        let static_id = self.state.next_node_id();
        let static_name = item_static.ident.to_string();
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
        };

        // Check if the static variable is public
        if matches!(item_static.vis, Visibility::Public(_)) {
            // Add the static to the code graph
            self.state.code_graph.values.push(static_node);

            // Add relation between static and its type
            self.state.code_graph.relations.push(Relation {
                source: static_id,
                target: type_id,
                kind: RelationKind::ValueType,
            });
        } else {
            #[cfg(feature = "visibility_resolution")]
            {
                // Add the static to the code graph
                self.state.code_graph.values.push(static_node);

                // Add relation between static and its type
                self.state.code_graph.relations.push(Relation {
                    source: static_id,
                    target: type_id,
                    kind: RelationKind::ValueType,
                });
            }
        }

        // Continue visiting
        visit::visit_item_static(self, item_static);
    }

    // Visit macro definitions (macro_rules!)
    fn visit_item_macro(&mut self, item_macro: &'ast syn::ItemMacro) {
        // Only process macros with #[macro_export]
        if !item_macro
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("macro_export"))
        {
            return;
        }

        let macro_id = self.state.next_node_id();

        // Get the macro name
        let macro_name = item_macro
            .ident
            .as_ref()
            .map(|ident| ident.to_string())
            .unwrap_or_else(|| "unnamed_macro".to_string());
        #[cfg(feature = "verbose_debug")]
        self.debug_new_id(&macro_name, macro_id);

        // Extract the macro body
        let body = Some(item_macro.mac.tokens.to_string());

        // Extract doc comments and other attributes
        let docstring = extract_docstring(&item_macro.attrs);
        let attributes = extract_attributes(&item_macro.attrs);

        // Parse macro rules (simplified approach)
        let mut rules = Vec::new();
        let tokens_str = item_macro.mac.tokens.to_string();

        // Very basic parsing of macro rules - in a real implementation,
        // you would want to use a more sophisticated approach
        // However, for our purposes this will do fine for the MVP
        for rule in tokens_str.split(";") {
            if rule.trim().is_empty() {
                continue;
            }

            // Try to split the rule into pattern and expansion
            if let Some(idx) = rule.find("=>") {
                let pattern = rule[..idx].trim().to_string();
                let expansion = rule[(idx + 2)..].trim().to_string();
                let rule_id = self.state.next_node_id();
                #[cfg(feature = "verbose_debug")]
                self.debug_new_id("Macro `=>` item", macro_id);

                rules.push(MacroRuleNode {
                    id: rule_id,
                    pattern,
                    expansion,
                });
            }
        }

        // Create the macro node
        let macro_node = MacroNode {
            id: macro_id,
            name: macro_name,
            visibility: VisibilityKind::Public, // Macros with #[macro_export] are public
            kind: MacroKind::DeclarativeMacro,
            rules,
            attributes,
            docstring,
            body,
        };

        // Add the macro to the code graph
        self.state.code_graph.macros.push(macro_node);
    }

    // Visit macro invocations
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // Create a node ID for this macro invocation
        let invocation_id = self.state.next_node_id();

        #[cfg(feature = "verbose_debug")]
        self.debug_new_id("unnamed macro invocation (Whaa?)", invocation_id);

        // Get the macro name
        let macro_path = mac.path.to_token_stream().to_string();

        // Find if this macro is defined in our code graph
        let defined_macro = self
            .state
            .code_graph
            .macros
            .iter()
            .find(|m| m.name == macro_path.split("::").last().unwrap_or(&macro_path));

        if let Some(defined_macro) = defined_macro {
            // Add a relation between the invocation and the macro definition
            self.state.code_graph.relations.push(Relation {
                source: invocation_id,
                target: defined_macro.id,
                kind: RelationKind::MacroUse,
            });
        }

        // Continue visiting
        visit::visit_macro(self, mac);
    }
}
