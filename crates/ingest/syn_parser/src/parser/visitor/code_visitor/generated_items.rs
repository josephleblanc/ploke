use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Generics, Ident, ItemEnum, ItemFn, ItemImpl, ItemMacro, ItemStruct, LitStr,
    ReturnType, Token, Type, Visibility, braced, bracketed, parenthesized,
};

use super::*;

struct OpaqueFuture {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    generics: Generics,
    actual: Type,
}

impl Parse for OpaqueFuture {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let vis: Visibility = input.parse()?;
        input.parse::<Token![type]>()?;
        let name: Ident = input.parse()?;
        let generics = if input.peek(Token![<]) {
            input.parse()?
        } else {
            Generics::default()
        };
        input.parse::<Token![=]>()?;
        let actual: Type = input.parse()?;
        input.parse::<Token![;]>()?;
        if !input.is_empty() {
            return Err(input.error("unsupported opaque_future! trailing tokens"));
        }

        Ok(Self {
            attrs,
            vis,
            name,
            generics,
            actual,
        })
    }
}

struct TopLevelRouteFn {
    name: Ident,
    method: Ident,
}

impl Parse for TopLevelRouteFn {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let method: Ident = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("unsupported top-level route macro trailing tokens"));
        }

        Ok(Self { name, method })
    }
}

struct BodyFromImpl {
    ty: Type,
}

impl Parse for BodyFromImpl {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ty: Type = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("unsupported body_from_impl! trailing tokens"));
        }

        Ok(Self { ty })
    }
}

struct AllTheTuples {
    name: Ident,
}

impl Parse for AllTheTuples {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("unsupported all_the_tuples! trailing tokens"));
        }

        Ok(Self { name })
    }
}

struct ImplService {
    params: Vec<Ident>,
}

impl Parse for ImplService {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut params = Vec::new();
        while !input.is_empty() {
            params.push(input.parse()?);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(Self { params })
    }
}

struct DefineRejection {
    attrs: Vec<Attribute>,
    vis: Visibility,
    status: Ident,
    body: LitStr,
    name: Ident,
    has_error: bool,
}

impl Parse for DefineRejection {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let status = parse_ident_attr(input, "status")?;
        let body = parse_lit_attr(input, "body")?;
        let attrs = Attribute::parse_outer(input)?;
        let vis: Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name: Ident = input.parse()?;
        let has_error = if input.peek(syn::token::Paren) {
            let content;
            parenthesized!(content in input);
            let marker: Ident = content.parse()?;
            if marker != "Error" || !content.is_empty() {
                return Err(content.error("unsupported define_rejection! tuple payload"));
            }
            true
        } else {
            false
        };
        input.parse::<Token![;]>()?;
        if !input.is_empty() {
            return Err(input.error("unsupported define_rejection! trailing tokens"));
        }

        Ok(Self {
            attrs,
            vis,
            status,
            body,
            name,
            has_error,
        })
    }
}

struct CompositeRejection {
    attrs: Vec<Attribute>,
    vis: Visibility,
    name: Ident,
    variants: Vec<Ident>,
}

impl Parse for CompositeRejection {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let attrs = Attribute::parse_outer(input)?;
        let vis: Visibility = input.parse()?;
        input.parse::<Token![enum]>()?;
        let name: Ident = input.parse()?;

        let content;
        braced!(content in input);
        let mut variants = Vec::new();
        while !content.is_empty() {
            variants.push(content.parse()?);
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        if variants.is_empty() {
            return Err(content.error("unsupported composite_rejection! empty enum"));
        }
        if !input.is_empty() {
            return Err(input.error("unsupported composite_rejection! trailing tokens"));
        }

        Ok(Self {
            attrs,
            vis,
            name,
            variants,
        })
    }
}

#[derive(Clone, Copy)]
enum MiddlewareService {
    FromFn,
    MapRequest,
    MapResponse,
}

impl<'a> CodeVisitor<'a> {
    pub(super) fn record_generated_macro(&mut self, item: &ItemMacro) {
        if item.mac.path.is_ident("opaque_future") {
            let Ok(input) = syn::parse2::<OpaqueFuture>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_opaque_future(item, input);
            return;
        }

        if item.mac.path.is_ident("top_level_handler_fn") {
            let Ok(input) = syn::parse2::<TopLevelRouteFn>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_top_level_handler_fn(item, input);
            return;
        }

        if item.mac.path.is_ident("top_level_service_fn") {
            let Ok(input) = syn::parse2::<TopLevelRouteFn>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_top_level_service_fn(item, input);
            return;
        }

        if item.mac.path.is_ident("body_from_impl") {
            let Ok(input) = syn::parse2::<BodyFromImpl>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_body_from_impl(item, input);
            return;
        }

        if item.mac.path.is_ident("define_rejection")
            && self.current_module_is_one_of(&[
                &["crate", "extract", "rejection"],
                &["crate", "extract", "multipart"],
                &["crate", "extract", "ws", "rejection"],
            ])
        {
            let Ok(input) = syn::parse2::<DefineRejection>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_define_rejection(item, input);
            return;
        }

        if item.mac.path.is_ident("composite_rejection")
            && self.current_module_is_one_of(&[
                &["crate", "extract", "rejection"],
                &["crate", "extract", "multipart"],
                &["crate", "extract", "ws", "rejection"],
            ])
        {
            let Ok(input) = syn::parse2::<CompositeRejection>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_composite_rejection(item, input);
            return;
        }

        if item.mac.path.is_ident("all_the_tuples") {
            let Ok(input) = syn::parse2::<AllTheTuples>(item.mac.tokens.clone()) else {
                return;
            };
            if input.name == "impl_handler" {
                self.record_impl_handler_tuples(item);
                return;
            }
            if input.name == "impl_service"
                && self.current_module_is(&["crate", "middleware", "from_fn"])
            {
                self.record_middleware_impl_service_tuples(item, MiddlewareService::FromFn);
                return;
            }
            if input.name == "impl_service"
                && self.current_module_is(&["crate", "middleware", "map_request"])
            {
                self.record_middleware_impl_service_tuples(item, MiddlewareService::MapRequest);
                return;
            }
            if input.name == "impl_from_request"
                && self.current_module_is(&["crate", "extract", "tuple"])
            {
                self.record_tuple_impl_from_request_tuples(item);
            }
            return;
        }

        if item.mac.path.is_ident("impl_service")
            && self.current_module_is(&["crate", "error_handling"])
        {
            let Ok(input) = syn::parse2::<ImplService>(item.mac.tokens.clone()) else {
                return;
            };
            self.record_error_handling_impl_service(item, input);
            return;
        }

        if item.mac.path.is_ident("impl_service")
            && self.current_module_is(&["crate", "middleware", "map_response"])
        {
            let Ok(input) = syn::parse2::<ImplService>(item.mac.tokens.clone()) else {
                return;
            };
            let Some(item_impl) =
                middleware_impl_service_item(MiddlewareService::MapResponse, input.params.len())
            else {
                return;
            };
            self.record_generated_impls(item, [item_impl]);
        }
    }

    fn record_opaque_future(&mut self, item: &ItemMacro, input: OpaqueFuture) {
        let Some(items) = opaque_future_items(&input) else {
            return;
        };
        let span = item.extract_span_bytes();
        let item_cfgs = extract_cfg_strings(&item.attrs);
        let effective_cfgs = self
            .state
            .current_scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect::<Vec<_>>();
        let cfg_bytes = calculate_cfg_hash_bytes(&effective_cfgs);

        let Some((struct_any, parent_mod)) = self.register_new_node_id(
            &items.struct_item.ident.to_string(),
            ItemKind::Struct,
            cfg_bytes.as_deref(),
        ) else {
            return;
        };
        self.debug_new_id(&items.struct_item.ident.to_string(), struct_any);

        let struct_id: StructNodeId = struct_any
            .try_into()
            .expect("opaque_future! struct should use StructNodeId");
        self.push_primary_scope(
            &items.struct_item.ident.to_string(),
            struct_id.into(),
            &effective_cfgs,
        );
        let fields = self.opaque_future_fields(&items.struct_item);
        let generic_params = self.state.process_generics(&items.struct_item.generics);
        let where_predicates = self
            .state
            .process_where_predicates(&items.struct_item.generics);
        self.pop_primary_scope(&items.struct_item.ident.to_string());

        let struct_node = StructNode {
            id: struct_id,
            name: items.struct_item.ident.to_string(),
            span,
            visibility: self.state.convert_visibility(&items.struct_item.vis),
            fields,
            generic_params,
            where_predicates,
            attributes: extract_attributes(&items.struct_item.attrs),
            docstring: extract_docstring(&items.struct_item.attrs),
            tracking_hash: Some(
                self.state
                    .generate_tracking_hash(&items.struct_item.to_token_stream()),
            ),
            cfgs: item_cfgs.clone(),
        };

        for field in &struct_node.fields {
            self.state
                .code_graph
                .relations
                .push(SyntacticRelation::StructField {
                    source: struct_id,
                    target: field.field_id(),
                });
        }
        self.state
            .code_graph
            .defined_types
            .push(TypeDefNode::Struct(struct_node));
        self.state
            .code_graph
            .relations
            .push(SyntacticRelation::Contains {
                source: parent_mod,
                target: PrimaryNodeId::from(struct_id),
            });

        self.record_generated_impl(&items.impl_item, span, item_cfgs, effective_cfgs);
    }

    fn record_top_level_handler_fn(&mut self, item: &ItemMacro, input: TopLevelRouteFn) {
        let Some(mut function) = top_level_handler_fn_item(&input) else {
            return;
        };
        function.attrs.extend(item.attrs.clone());
        syn::visit::Visit::visit_item_fn(self, &function);
    }

    fn record_top_level_service_fn(&mut self, item: &ItemMacro, input: TopLevelRouteFn) {
        let Some(mut function) = top_level_service_fn_item(&input) else {
            return;
        };
        function.attrs.extend(item.attrs.clone());
        syn::visit::Visit::visit_item_fn(self, &function);
    }

    fn record_body_from_impl(&mut self, item: &ItemMacro, input: BodyFromImpl) {
        let Some(item_impl) = body_from_impl_item(&input) else {
            return;
        };
        self.record_generated_impls(item, [item_impl]);
    }

    fn record_define_rejection(&mut self, item: &ItemMacro, input: DefineRejection) {
        let Some(mut struct_item) = define_rejection_struct_item(&input) else {
            return;
        };
        let Some(items) = define_rejection_items(&input) else {
            return;
        };
        struct_item.attrs.extend(item.attrs.clone());
        syn::visit::Visit::visit_item_struct(self, &struct_item);
        self.record_generated_impls(item, items);
    }

    fn record_composite_rejection(&mut self, item: &ItemMacro, input: CompositeRejection) {
        let Some((mut enum_item, impl_item)) = composite_rejection_items(&input) else {
            return;
        };
        enum_item.attrs.extend(item.attrs.clone());
        syn::visit::Visit::visit_item_enum(self, &enum_item);
        self.record_generated_impls(item, [impl_item]);
    }

    fn record_impl_handler_tuples(&mut self, item: &ItemMacro) {
        self.record_generated_impls(item, (1..=16).filter_map(handler_impl_item));
    }

    fn record_error_handling_impl_service(&mut self, item: &ItemMacro, input: ImplService) {
        let Some(item_impl) = error_handling_impl_service_item(&input) else {
            return;
        };
        self.record_generated_impls(item, [item_impl]);
    }

    fn record_middleware_impl_service_tuples(&mut self, item: &ItemMacro, kind: MiddlewareService) {
        self.record_generated_impls(
            item,
            (1..=16).filter_map(|arity| middleware_impl_service_item(kind, arity)),
        );
    }

    fn record_tuple_impl_from_request_tuples(&mut self, item: &ItemMacro) {
        self.record_generated_impls(item, (1..=16).flat_map(tuple_impl_from_request_items));
    }

    pub(super) fn generated_impl_macro_methods(
        &mut self,
        item: &syn::ImplItemMacro,
        effective_cfgs: &[String],
    ) -> Vec<MethodNode> {
        if !self.current_module_is(&["crate", "routing", "method_routing"]) {
            return Vec::new();
        }

        let Ok(input) = syn::parse2::<TopLevelRouteFn>(item.mac.tokens.clone()) else {
            return Vec::new();
        };
        let item_impl = if item.mac.path.is_ident("chained_handler_fn") {
            chained_handler_fn_impl_item(&input)
        } else if item.mac.path.is_ident("chained_service_fn") {
            chained_service_fn_impl_item(&input)
        } else {
            None
        };

        let Some(item_impl) = item_impl else {
            return Vec::new();
        };
        self.generated_impl_methods(&item_impl, effective_cfgs)
    }

    fn record_generated_impls(
        &mut self,
        item: &ItemMacro,
        impls: impl IntoIterator<Item = ItemImpl>,
    ) {
        let span = item.extract_span_bytes();
        let item_cfgs = extract_cfg_strings(&item.attrs);
        let effective_cfgs = self
            .state
            .current_scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect::<Vec<_>>();

        for item_impl in impls {
            self.record_generated_impl(&item_impl, span, item_cfgs.clone(), effective_cfgs.clone());
        }
    }

    fn current_module_is(&self, path: &[&str]) -> bool {
        self.state
            .current_module_path
            .iter()
            .map(String::as_str)
            .eq(path.iter().copied())
    }

    fn current_module_is_one_of(&self, paths: &[&[&str]]) -> bool {
        paths.iter().any(|path| self.current_module_is(path))
    }

    fn opaque_future_fields(&mut self, item: &ItemStruct) -> Vec<FieldNode> {
        item.fields
            .iter()
            .zip(u8::MIN..u8::MAX)
            .map(|(field, i)| {
                let mut name = field.ident.as_ref().map(|ident| ident.to_string());
                let key = name.get_or_insert_default();
                key.extend(
                    "unnamed_field"
                        .chars()
                        .chain(item.ident.to_string().chars()),
                );
                key.push(i.into());
                let field_cfgs = extract_cfg_strings(&field.attrs);
                let effective_cfgs = self
                    .state
                    .current_scope_cfgs
                    .iter()
                    .cloned()
                    .chain(field_cfgs.iter().cloned())
                    .collect::<Vec<_>>();
                let cfg_bytes = calculate_cfg_hash_bytes(&effective_cfgs);
                let any_id = self.state.generate_synthetic_node_id(
                    key,
                    ItemKind::Field,
                    cfg_bytes.as_deref(),
                );
                self.debug_new_id(key, any_id);
                let field_id: FieldNodeId = any_id
                    .try_into()
                    .expect("opaque_future! field should use FieldNodeId");
                FieldNode {
                    id: field_id,
                    name,
                    type_id: get_or_create_type(self.state, &field.ty),
                    visibility: self.state.convert_visibility(&field.vis),
                    attributes: extract_attributes(&field.attrs),
                    cfgs: field_cfgs,
                }
            })
            .collect()
    }

    fn record_generated_impl(
        &mut self,
        item: &ItemImpl,
        span: (usize, usize),
        item_cfgs: Vec<String>,
        effective_cfgs: Vec<String>,
    ) {
        let name = name_impl(item, span);
        let cfg_bytes = calculate_cfg_hash_bytes(&effective_cfgs);
        let Some((impl_any, parent_mod)) =
            self.register_new_node_id(&name, ItemKind::Impl, cfg_bytes.as_deref())
        else {
            return;
        };
        self.debug_new_id(&name, impl_any);

        let impl_id: ImplNodeId = impl_any
            .try_into()
            .expect("generated impl should use ImplNodeId");
        self.push_primary_scope(&name, impl_id.into(), &effective_cfgs);
        let self_type = get_or_create_type(self.state, &item.self_ty);
        let trait_type = item.trait_.as_ref().map(|(_, path, _)| {
            let ty = Type::Path(TypePath {
                qself: None,
                path: path.clone(),
            });
            get_or_create_trait_type(self.state, &ty)
        });
        let methods = self.generated_impl_methods(item, &effective_cfgs);
        let generic_params = self.state.process_generics(&item.generics);
        let where_predicates = self.state.process_where_predicates(&item.generics);
        self.pop_primary_scope(&name);

        let impl_node = ImplNode {
            id: impl_id,
            span,
            self_type,
            trait_type,
            methods,
            generic_params,
            where_predicates,
            cfgs: item_cfgs,
        };

        for method in &impl_node.methods {
            self.state
                .code_graph
                .relations
                .push(SyntacticRelation::ImplAssociatedItem {
                    source: impl_id,
                    target: AssociatedItemNodeId::from(method.method_id()),
                });
        }
        self.state.code_graph.impls.push(impl_node);
        self.state
            .code_graph
            .relations
            .push(SyntacticRelation::Contains {
                source: parent_mod,
                target: PrimaryNodeId::from(impl_id),
            });
    }

    fn generated_impl_methods(
        &mut self,
        item: &ItemImpl,
        effective_cfgs: &[String],
    ) -> Vec<MethodNode> {
        item.items
            .iter()
            .filter_map(|item| match item {
                syn::ImplItem::Fn(method) => Some(method),
                _ => None,
            })
            .map(|method| {
                let name = method.sig.ident.to_string();
                let item_cfgs = extract_cfg_strings(&method.attrs);
                let cfgs = self
                    .state
                    .current_scope_cfgs
                    .iter()
                    .cloned()
                    .chain(item_cfgs.iter().cloned())
                    .collect::<Vec<_>>();
                let cfg_bytes = calculate_cfg_hash_bytes(&cfgs);
                let any_id = self.state.generate_synthetic_node_id(
                    &name,
                    ItemKind::Method,
                    cfg_bytes.as_deref(),
                );
                self.debug_new_id(&name, any_id);
                let method_id: MethodNodeId = any_id
                    .try_into()
                    .expect("generated impl method should use MethodNodeId");
                self.push_assoc_scope(
                    &name,
                    AssociatedItemNodeId::from(method_id),
                    &self.state.current_scope_cfgs.clone(),
                );
                let parameters = method
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| self.state.process_fn_arg(arg))
                    .collect::<Vec<_>>();
                let return_type = match &method.sig.output {
                    ReturnType::Default => None,
                    ReturnType::Type(_, ty) => Some(get_or_create_type(self.state, ty)),
                };
                let generic_params = self.state.process_generics(&method.sig.generics);
                let where_predicates = self.state.process_where_predicates(&method.sig.generics);
                self.pop_assoc_scope(&name);

                self.record_body_call_sites(
                    CallBodyOwnerId::Method(method_id),
                    &method.block,
                    effective_cfgs,
                    &receiver_param_names(&parameters),
                    &method.sig.inputs,
                );

                MethodNode {
                    id: method_id,
                    name,
                    span: method.extract_span_bytes(),
                    visibility: self.state.convert_visibility(&method.vis),
                    is_unsafe: method.sig.unsafety.is_some(),
                    is_async: method.sig.asyncness.is_some(),
                    parameters,
                    return_type,
                    generic_params,
                    where_predicates,
                    attributes: extract_attributes(&method.attrs),
                    docstring: extract_docstring(&method.attrs),
                    body: Some(method.block.to_token_stream().to_string()),
                    tracking_hash: Some(
                        self.state.generate_tracking_hash(&method.to_token_stream()),
                    ),
                    cfgs: item_cfgs,
                }
            })
            .collect()
    }
}

struct OpaqueFutureItems {
    struct_item: ItemStruct,
    impl_item: ItemImpl,
}

fn opaque_future_items(input: &OpaqueFuture) -> Option<OpaqueFutureItems> {
    let attrs = &input.attrs;
    let vis = &input.vis;
    let name = &input.name;
    let generics = &input.generics;
    let actual = &input.actual;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let struct_item = parse_item(quote! {
        #(#attrs)*
        #vis struct #name #generics {
            #[pin]
            future: #actual,
        }
    })?;
    let impl_item = parse_item(quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            pub(crate) fn new(future: #actual) -> Self {
                Self { future }
            }
        }
    })?;

    Some(OpaqueFutureItems {
        struct_item,
        impl_item,
    })
}

fn top_level_handler_fn_item(input: &TopLevelRouteFn) -> Option<ItemFn> {
    let name = &input.name;
    let method = &input.method;
    parse_item(quote! {
        pub fn #name<H, T, S>(handler: H) -> MethodRouter<S, Infallible>
        where
            H: Handler<T, S>,
            T: 'static,
            S: Clone + Send + Sync + 'static,
        {
            on(MethodFilter::#method, handler)
        }
    })
}

fn top_level_service_fn_item(input: &TopLevelRouteFn) -> Option<ItemFn> {
    let name = &input.name;
    let method = &input.method;
    parse_item(quote! {
        pub fn #name<T, S>(svc: T) -> MethodRouter<S, T::Error>
        where
            T: Service<Request> + Clone + Send + Sync + 'static,
            T::Response: IntoResponse + 'static,
            T::Future: Send + 'static,
            S: Clone,
        {
            on_service(MethodFilter::#method, svc)
        }
    })
}

fn chained_handler_fn_impl_item(input: &TopLevelRouteFn) -> Option<ItemImpl> {
    let name = &input.name;
    let method = &input.method;
    parse_item(quote! {
        impl Generated {
            #[track_caller]
            pub fn #name<H, T>(self, handler: H) -> Self
            where
                H: Handler<T, S>,
                T: 'static,
                S: Send + Sync + 'static,
            {
                self.on(MethodFilter::#method, handler)
            }
        }
    })
}

fn chained_service_fn_impl_item(input: &TopLevelRouteFn) -> Option<ItemImpl> {
    let name = &input.name;
    let method = &input.method;
    parse_item(quote! {
        impl Generated {
            #[track_caller]
            pub fn #name<T>(self, svc: T) -> Self
            where
                T: Service<Request, Error = E> + Clone + Send + Sync + 'static,
                T::Response: IntoResponse + 'static,
                T::Future: Send + 'static,
            {
                self.on_service(MethodFilter::#method, svc)
            }
        }
    })
}

fn body_from_impl_item(input: &BodyFromImpl) -> Option<ItemImpl> {
    let ty = &input.ty;
    parse_item(quote! {
        impl From<#ty> for Body {
            fn from(buf: #ty) -> Self {
                Self::new(http_body_util::Full::from(buf))
            }
        }
    })
}

fn define_rejection_items(input: &DefineRejection) -> Option<Vec<ItemImpl>> {
    let inherent = define_rejection_inherent_item(input)?;
    let response = define_rejection_response_item(input)?;
    Some(vec![inherent, response])
}

fn define_rejection_struct_item(input: &DefineRejection) -> Option<ItemStruct> {
    let attrs = &input.attrs;
    let vis = &input.vis;
    let name = &input.name;

    if input.has_error {
        parse_item(quote! {
            #(#attrs)*
            #[derive(Debug)]
            #vis struct #name(pub(crate) Error);
        })
    } else {
        parse_item(quote! {
            #(#attrs)*
            #[derive(Debug)]
            #[non_exhaustive]
            #vis struct #name;
        })
    }
}

fn define_rejection_inherent_item(input: &DefineRejection) -> Option<ItemImpl> {
    let name = &input.name;
    let status = &input.status;
    let mut methods = quote! {
        pub fn body_text(&self) -> String {
            self.to_string()
        }

        pub fn status(&self) -> http::StatusCode {
            http::StatusCode::#status
        }
    };
    if input.has_error {
        methods = quote! {
            pub(crate) fn from_err<E>(err: E) -> Self
            where
                E: Into<BoxError>,
            {
                Self(Error::new(err))
            }

            #methods
        };
    }

    parse_item(quote! {
        impl #name {
            #methods
        }
    })
}

fn define_rejection_response_item(input: &DefineRejection) -> Option<ItemImpl> {
    let name = &input.name;
    let body = &input.body;
    let response_body = if input.has_error {
        quote! {
            let body_text = self.body_text();
            axum_core::__log_rejection!(
                rejection_type = #name,
                body_text = body_text,
                status = status,
            );
            (status, body_text).into_response()
        }
    } else {
        quote! {
            axum_core::__log_rejection!(
                rejection_type = #name,
                body_text = #body,
                status = status,
            );
            (status, #body).into_response()
        }
    };

    parse_item(quote! {
        impl IntoResponse for #name {
            fn into_response(self) -> Response {
                let status = self.status();
                #response_body
            }
        }
    })
}

fn composite_rejection_items(input: &CompositeRejection) -> Option<(ItemEnum, ItemImpl)> {
    let attrs = &input.attrs;
    let vis = &input.vis;
    let name = &input.name;
    let variants = &input.variants;

    let enum_item = parse_item(quote! {
        #(#attrs)*
        #[derive(Debug)]
        #[non_exhaustive]
        #vis enum #name {
            #(
                #[allow(missing_docs)]
                #variants(#variants),
            )*
        }
    })?;
    let impl_item = parse_item(quote! {
        impl IntoResponse for #name {
            fn into_response(self) -> Response {
                match self {
                    #(
                        Self::#variants(inner) => inner.into_response(),
                    )*
                }
            }
        }
    })?;

    Some((enum_item, impl_item))
}

fn handler_impl_item(arity: usize) -> Option<ItemImpl> {
    if !(1..=16).contains(&arity) {
        return None;
    }

    let params = (1..=arity)
        .map(|index| format_ident!("T{}", index))
        .collect::<Vec<_>>();
    let parts = &params[..params.len() - 1];
    let last = params.last()?;

    parse_item(quote! {
        impl<F, Fut, S, Res, M, #(#parts,)* #last> Handler<(M, #(#parts,)* #last,), S> for F
        where
            F: FnOnce(#(#parts,)* #last,) -> Fut + Clone + Send + Sync + 'static,
            Fut: Future<Output = Res> + Send,
            S: Send + Sync + 'static,
            Res: IntoResponse,
            #( #parts: FromRequestParts<S> + Send, )*
            #last: FromRequest<S, M> + Send,
        {
            type Future = Pin<Box<dyn Future<Output = Response> + Send>>;

            fn call(self, req: Request, state: S) -> Self::Future {
                let (mut parts, body) = req.into_parts();
                Box::pin(async move {
                    #(
                        let #parts = #parts::from_request_parts(&mut parts, &state).await;
                    )*

                    let req = Request::from_parts(parts, body);

                    let #last = #last::from_request(req, &state).await;

                    self(#(#parts,)* #last,).await.into_response()
                })
            }
        }
    })
}

fn error_handling_impl_service_item(input: &ImplService) -> Option<ItemImpl> {
    let params = &input.params;
    if params.is_empty() {
        return None;
    }

    parse_item(quote! {
        impl<S, F, B, Res, Fut, #(#params,)*> Service<Request<B>>
            for HandleError<S, F, (#(#params,)*)>
        where
            S: Service<Request<B>> + Clone + Send + 'static,
            S::Response: IntoResponse + Send,
            S::Error: Send,
            S::Future: Send,
            F: FnOnce(#(#params),*, S::Error) -> Fut + Clone + Send + 'static,
            Fut: Future<Output = Res> + Send,
            Res: IntoResponse,
            #( #params: FromRequestParts<()> + Send, )*
            B: Send + 'static,
        {
            fn call(&mut self, req: Request<B>) -> Self::Future {
                Box::pin(async move {
                    #(
                        let #params = #params::from_request_parts(&mut parts, &()).await;
                    )*
                })
            }
        }
    })
}

fn middleware_impl_service_item(kind: MiddlewareService, arity: usize) -> Option<ItemImpl> {
    match kind {
        MiddlewareService::FromFn | MiddlewareService::MapRequest => {
            if !(1..=16).contains(&arity) {
                return None;
            }
            let params = (1..=arity)
                .map(|index| format_ident!("T{}", index))
                .collect::<Vec<_>>();
            let parts = &params[..params.len() - 1];
            let last = params.last()?;

            match kind {
                MiddlewareService::FromFn => parse_item(quote! {
                    impl<F, S, I, #(#parts,)* #last> Service<Request>
                        for FromFn<F, S, I, (#(#parts,)* #last,)>
                    {
                        fn call(&mut self, req: Request) -> Self::Future {
                            let not_ready_inner = self.inner.clone();
                            let ready_inner = std::mem::replace(&mut self.inner, not_ready_inner);
                            let mut f = self.f.clone();
                            let state = self.state.clone();
                            let (mut parts, body) = req.into_parts();
                            let future = Box::pin(async move {
                                #(
                                    let #parts = #parts::from_request_parts(&mut parts, &state).await;
                                )*
                                let req = Request::from_parts(parts, body);
                                let #last = #last::from_request(req, &state).await;
                                let inner = BoxCloneSyncService::new(MapIntoResponse::new(ready_inner));
                                let next = Next { inner };
                                f(#(#parts,)* #last, next).await.into_response()
                            });
                            ResponseFuture { inner: future }
                        }
                    }
                }),
                MiddlewareService::MapRequest => parse_item(quote! {
                    impl<F, S, I, B, #(#parts,)* #last> Service<Request<B>>
                        for MapRequest<F, S, I, (#(#parts,)* #last,)>
                    {
                        fn call(&mut self, req: Request<B>) -> Self::Future {
                            std::mem::replace(&mut self.inner, not_ready_inner);
                        }
                    }
                }),
                MiddlewareService::MapResponse => None,
            }
        }
        MiddlewareService::MapResponse => {
            if arity > 16 {
                return None;
            }
            let params = (1..=arity)
                .map(|index| format_ident!("T{}", index))
                .collect::<Vec<_>>();

            parse_item(quote! {
                impl<F, Fut, S, I, B, ResBody #(, #params)*> Service<Request<B>>
                    for MapResponse<F, S, I, (#(#params,)*)>
                {
                    fn call(&mut self, req: Request<B>) -> Self::Future {
                        std::mem::replace(&mut self.inner, not_ready_inner);
                    }
                }
            })
        }
    }
}

fn tuple_impl_from_request_items(arity: usize) -> Vec<ItemImpl> {
    if !(1..=16).contains(&arity) {
        return Vec::new();
    }

    let params = (1..=arity)
        .map(|index| format_ident!("T{}", index))
        .collect::<Vec<_>>();
    let parts = &params[..params.len() - 1];
    let last = match params.last() {
        Some(last) => last,
        None => return Vec::new(),
    };
    let mut items = Vec::new();

    if let Some(item) = parse_item(quote! {
        impl<S, #(#parts,)* #last> FromRequestParts<S> for (#(#parts,)* #last,)
        where
            #( #parts: FromRequestParts<S> + Send, )*
            #last: FromRequestParts<S> + Send,
            S: Send + Sync,
        {
            type Rejection = Response;

            async fn from_request_parts(
                parts: &mut Parts,
                state: &S,
            ) -> Result<Self, Self::Rejection> {
                #(
                    let #parts = #parts::from_request_parts(parts, state)
                        .await;
                )*
                let #last = #last::from_request_parts(parts, state)
                    .await;

                Ok((#(#parts,)* #last,))
            }
        }
    }) {
        items.push(item);
    }

    if let Some(item) = parse_item(quote! {
        impl<S, #(#parts,)* #last> FromRequest<S> for (#(#parts,)* #last,)
        where
            #( #parts: FromRequestParts<S> + Send, )*
            #last: FromRequest<S> + Send,
            S: Send + Sync,
        {
            type Rejection = Response;

            fn from_request(
                req: Request,
                state: &S,
            ) -> impl Future<Output = Result<Self, Self::Rejection>> {
                let (mut parts, body) = req.into_parts();

                async move {
                    #(
                        let #parts = #parts::from_request_parts(&mut parts, state)
                            .await;
                    )*

                    let req = Request::from_parts(parts, body);

                    let #last = #last::from_request(req, state)
                        .await;

                    Ok((#(#parts,)* #last,))
                }
            }
        }
    }) {
        items.push(item);
    }

    items
}

fn parse_item<T: syn::parse::Parse>(tokens: TokenStream) -> Option<T> {
    syn::parse2(tokens).ok()
}

fn parse_ident_attr(input: ParseStream<'_>, expected: &str) -> syn::Result<Ident> {
    input.parse::<Token![#]>()?;
    let content;
    bracketed!(content in input);
    let key: Ident = content.parse()?;
    if key != expected {
        return Err(content.error(format!("expected {expected:?} attribute")));
    }
    content.parse::<Token![=]>()?;
    let value: Ident = content.parse()?;
    if !content.is_empty() {
        return Err(content.error(format!("unsupported {expected:?} attribute")));
    }
    Ok(value)
}

fn parse_lit_attr(input: ParseStream<'_>, expected: &str) -> syn::Result<LitStr> {
    input.parse::<Token![#]>()?;
    let content;
    bracketed!(content in input);
    let key: Ident = content.parse()?;
    if key != expected {
        return Err(content.error(format!("expected {expected:?} attribute")));
    }
    content.parse::<Token![=]>()?;
    let value: LitStr = content.parse()?;
    if !content.is_empty() {
        return Err(content.error(format!("unsupported {expected:?} attribute")));
    }
    Ok(value)
}
