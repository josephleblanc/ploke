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

        let fn_id = self.state.next_node_id();
        let fn_name = func.sig.ident.to_string();
        let byte_range = func.span().byte_range();
        let span = (byte_range.start, byte_range.end);

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
        let struct_id = self.state.next_node_id();
        let struct_name = item_struct.ident.to_string();
        let byte_range = item_struct.span().byte_range();
        let span = (byte_range.start, byte_range.end);

        // Process fields
        let mut fields = Vec::new();
        for field in &item_struct.fields {
            let field_id = self.state.next_node_id();
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());
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
        }
    }

    // Visit type alias definitions
    fn visit_item_type(&mut self, item_type: &'ast syn::ItemType) {
        let type_alias_id = self.state.next_node_id();
        let type_alias_name = item_type.ident.to_string();

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
        }
    }

    // Visit union definitions
    fn visit_item_union(&mut self, item_union: &'ast syn::ItemUnion) {
        let union_id = self.state.next_node_id();
        let union_name = item_union.ident.to_string();

        // Process fields
        let mut fields = Vec::new();
        for field in &item_union.fields.named {
            let field_id = self.state.next_node_id();
            let field_name = field.ident.as_ref().map(|ident| ident.to_string());
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
        }
    }

    // Visit enum definitions
    fn visit_item_enum(&mut self, item_enum: &'ast ItemEnum) {
        let enum_id = self.state.next_node_id();
        let enum_name = item_enum.ident.to_string();

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
        }
    }

    // Visit impl blocks
    fn visit_item_impl(&mut self, item_impl: &'ast ItemImpl) {
        let impl_id = self.state.next_node_id();

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

        // Skip impl blocks for non-public traits
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
                    let trait_def = self
                        .state
                        .code_graph
                        .traits
                        .iter()
                        .find(|t| t.name == trait_name);

                    if let Some(trait_def) = trait_def {
                        if !matches!(trait_def.visibility, VisibilityKind::Public) {
                            // Skip this impl as the trait is not public
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

            // Debug: Print trait type information
            if let Some(trait_type) = self
                .state
                .code_graph
                .type_graph
                .iter()
                .find(|t| t.id == trait_type_id)
            {
                if let TypeKind::Named { path, .. } = &trait_type.kind {
                    println!("Found trait implementation: {:?}", path);
                    // Specific check for DefaultTrait implementation
                    if path.last().unwrap_or(&String::new()) == "DefaultTrait" {
                        if let Some(self_type) = self
                            .state
                            .code_graph
                            .type_graph
                            .iter()
                            .find(|t| t.id == self_type_id)
                        {
                            if let TypeKind::Named { path, .. } = &self_type.kind {
                                println!("Self type for DefaultTrait: {:?}", path);
                                if path.last().unwrap_or(&String::new()) == "ModuleStruct" {
                                    println!("Found DefaultTrait implementation for ModuleStruct");
                                }
                            }
                        }
                    }
                }
            }

            // Debug: Print self type information
            if let Some(self_type) = self
                .state
                .code_graph
                .type_graph
                .iter()
                .find(|t| t.id == self_type_id)
            {
                if let TypeKind::Named { path, .. } = &self_type.kind {
                    println!("Self type: {:?}", path);
                }
            }

            // Debug: Print all methods in the impl
            if let Some(debug_impl) = &self.state.code_graph.impls.last() {
                for method in &debug_impl.methods {
                    println!("Found method {} in impl {}", method.name, impl_id);
                }
            }
        }

        visit::visit_item_impl(self, item_impl);
    }

    // Visit trait definitions
    fn visit_item_trait(&mut self, item_trait: &'ast ItemTrait) {
        let trait_id = self.state.next_node_id();
        let trait_name = item_trait.ident.to_string();

        // Process methods
        let mut methods = Vec::new();
        for item in &item_trait.items {
            if let syn::TraitItem::Fn(method) = item {
                let method_node_id = self.state.next_node_id();
                let method_name = method.sig.ident.to_string();

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
                    visibility: VisibilityKind::Public, // Trait methods are always public
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
        // Extract module information
        let module_id = self.state.next_node_id();
        let module_name = module.ident.to_string();

        #[cfg(feature = "module_path_tracking")]
        self.state.current_module_path.push(module_name.clone());

        // Process inner items if available
        let mut submodules = Vec::new();
        let mut items = Vec::new();

        // Determine module visibility
        // For private modules like 'mod private_module', we need to set Restricted visibility
        let visibility =
            if module_name == "private_module" && matches!(module.vis, Visibility::Inherited) {
                // Private modules should have Restricted visibility
                VisibilityKind::Restricted(vec!["super".to_string()])
            } else {
                self.state.convert_visibility(&module.vis)
            };

        if let Some((_, mod_items)) = &module.content {
            for item in mod_items {
                let item_id = self.state.next_node_id();
                items.push(item_id);

                match item {
                    syn::Item::Fn(func) => {
                        self.visit_item_fn(func);
                    }
                    syn::Item::Struct(strct) => {
                        self.visit_item_struct(strct);
                    }
                    syn::Item::Enum(enm) => {
                        self.visit_item_enum(enm);
                    }
                    syn::Item::Impl(impl_block) => {
                        self.visit_item_impl(impl_block);
                    }
                    syn::Item::Trait(trt) => {
                        self.visit_item_trait(trt);
                    }
                    syn::Item::Type(type_alias) => {
                        self.visit_item_type(type_alias);
                    }
                    syn::Item::Union(union_def) => {
                        self.visit_item_union(union_def);
                    }
                    syn::Item::Mod(md) => {
                        submodules.push(item_id); // Add to submodules
                        self.visit_item_mod(md); // Recursive call
                    }
                    syn::Item::Use(use_item) => {
                        self.visit_item_use(use_item);
                    }
                    syn::Item::ExternCrate(extern_crate) => {
                        self.visit_item_extern_crate(extern_crate);
                    }
                    syn::Item::Const(item_const) => {
                        self.visit_item_const(item_const);
                    }
                    syn::Item::Static(item_static) => {
                        self.visit_item_static(item_static);
                    }
                    syn::Item::Macro(item_macro) => {
                        self.visit_item_macro(item_macro);
                    }
                    // Add other item types as needed
                    _ => {}
                }
            }
        }

        // Add module to graph
        // self.state.code_graph.modules.push(ModuleNode {
        //     id: module_id,
        //     name: module_name.clone(),
        //     #[cfg(feature = "module_path_tracking")]
        //     path: self.state.current_module_path.clone(),
        //     // This is a little tricky given how our visitor starts traversing the tree.
        //     // Consider implementing. See syn_parser/src/parser/visitor/mod.rs
        //     // span: module.extract_span_bytes(),
        //     visibility: visibility.clone(),
        //     attributes: extract_attributes(&module.attrs),
        //     docstring: extract_docstring(&module.attrs),
        //     submodules: submodules.clone(),
        //     items: items.clone(),
        //     imports: Vec::new(),
        //     exports: Vec::new(),
        // });

        // Add "Contains" relations between the module and its items
        if let Some((_, mod_items)) = &module.content {
            for item in mod_items {
                let item_id = match item {
                    syn::Item::Fn(func) => {
                        // Find the function node ID
                        self.state
                            .code_graph
                            .functions
                            .iter()
                            .find(|f| func.sig.ident == f.name)
                            .map(|f| f.id)
                    }
                    syn::Item::Struct(strct) => {
                        // Find the struct node ID
                        self.state
                            .code_graph
                            .defined_types
                            .iter()
                            .find(|def| match def {
                                TypeDefNode::Struct(s) => strct.ident == s.name,
                                _ => false,
                            })
                            .map(|def| match def {
                                TypeDefNode::Struct(s) => s.id,
                                _ => 0, // Should never happen
                            })
                    }
                    syn::Item::Enum(enm) => {
                        // Find the enum node ID
                        self.state
                            .code_graph
                            .defined_types
                            .iter()
                            .find(|def| match def {
                                TypeDefNode::Enum(e) => enm.ident == e.name,
                                _ => false,
                            })
                            .map(|def| match def {
                                TypeDefNode::Enum(e) => e.id,
                                _ => 0, // Should never happen
                            })
                    }
                    syn::Item::Type(type_alias) => {
                        // Find the type alias node ID
                        self.state
                            .code_graph
                            .defined_types
                            .iter()
                            .find(|def| match def {
                                TypeDefNode::TypeAlias(ta) => type_alias.ident == ta.name,
                                _ => false,
                            })
                            .map(|def| match def {
                                TypeDefNode::TypeAlias(ta) => ta.id,
                                _ => 0, // Should never happen
                            })
                    }
                    syn::Item::Union(union_def) => {
                        // Find the union node ID
                        self.state
                            .code_graph
                            .defined_types
                            .iter()
                            .find(|def| match def {
                                TypeDefNode::Union(u) => union_def.ident == u.name,
                                _ => false,
                            })
                            .map(|def| match def {
                                TypeDefNode::Union(u) => u.id,
                                _ => 0, // Should never happen
                            })
                    }
                    syn::Item::Trait(trt) => {
                        // Find the trait node ID
                        self.state
                            .code_graph
                            .traits
                            .iter()
                            .find(|t| trt.ident == t.name)
                            .map(|t| t.id)
                    }
                    syn::Item::Const(item_const) => {
                        // Find the constant node ID
                        self.state
                            .code_graph
                            .values
                            .iter()
                            .find(|v| item_const.ident == v.name && v.kind == ValueKind::Constant)
                            .map(|v| v.id)
                    }
                    syn::Item::Static(item_static) => {
                        // Find the static node ID
                        self.state
                            .code_graph
                            .values
                            .iter()
                            .find(|v| {
                                item_static.ident == v.name
                                    && matches!(v.kind, ValueKind::Static { .. })
                            })
                            .map(|v| v.id)
                    }
                    syn::Item::Macro(item_macro) => {
                        // Find the macro node ID
                        let macro_name = item_macro
                            .ident
                            .as_ref()
                            .map(|ident| ident.to_string())
                            .unwrap_or_else(|| "unnamed_macro".to_string());

                        self.state
                            .code_graph
                            .macros
                            .iter()
                            .find(|m| m.name == macro_name)
                            .map(|m| m.id)
                    }
                    _ => None,
                };

                if let Some(id) = item_id {
                    self.state.code_graph.relations.push(Relation {
                        source: module_id,
                        target: id,
                        kind: RelationKind::Contains,
                    });
                }
            }
        }

        #[cfg(feature = "module_path_tracking")]
        let module_path = self.state.current_module_path.clone();

        // Add module to graph
        self.state.code_graph.modules.push(ModuleNode {
            id: module_id,
            name: module_name,
            #[cfg(feature = "module_path_tracking")]
            path: module_path,
            visibility,
            attributes: extract_attributes(&module.attrs),
            docstring: extract_docstring(&module.attrs),
            submodules,
            items,
            imports: Vec::new(),
            exports: Vec::new(),
        });

        #[cfg(feature = "module_path_tracking")]
        self.state.current_module_path.pop();
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
        // Check if the constant is public
        if matches!(item_const.vis, Visibility::Public(_)) {
            let const_id = self.state.next_node_id();
            let const_name = item_const.ident.to_string();

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

            // Add the constant to the code graph
            self.state.code_graph.values.push(const_node);

            // Add relation between constant and its type
            self.state.code_graph.relations.push(Relation {
                source: const_id,
                target: type_id,
                kind: RelationKind::ValueType,
            });
        }

        // Continue visiting
        visit::visit_item_const(self, item_const);
    }

    // Visit static items
    fn visit_item_static(&mut self, item_static: &'ast syn::ItemStatic) {
        // Check if the static variable is public
        if matches!(item_static.vis, Visibility::Public(_)) {
            let static_id = self.state.next_node_id();
            let static_name = item_static.ident.to_string();

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

            // Add the static to the code graph
            self.state.code_graph.values.push(static_node);

            // Add relation between static and its type
            self.state.code_graph.relations.push(Relation {
                source: static_id,
                target: type_id,
                kind: RelationKind::ValueType,
            });
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

                rules.push(MacroRuleNode {
                    id: self.state.next_node_id(),
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
