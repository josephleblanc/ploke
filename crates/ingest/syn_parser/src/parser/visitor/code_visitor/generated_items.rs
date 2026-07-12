use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{
    Attribute, Generics, Ident, ItemFn, ItemImpl, ItemMacro, ItemStruct, ReturnType, Token, Type,
    Visibility,
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

        if item.mac.path.is_ident("all_the_tuples") {
            let Ok(input) = syn::parse2::<AllTheTuples>(item.mac.tokens.clone()) else {
                return;
            };
            if input.name == "impl_handler" {
                self.record_impl_handler_tuples(item);
            }
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
        let span = item.extract_span_bytes();
        let item_cfgs = extract_cfg_strings(&item.attrs);
        let effective_cfgs = self
            .state
            .current_scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect::<Vec<_>>();

        self.record_generated_impl(&item_impl, span, item_cfgs, effective_cfgs);
    }

    fn record_impl_handler_tuples(&mut self, item: &ItemMacro) {
        let span = item.extract_span_bytes();
        let item_cfgs = extract_cfg_strings(&item.attrs);
        let effective_cfgs = self
            .state
            .current_scope_cfgs
            .iter()
            .cloned()
            .chain(item_cfgs.iter().cloned())
            .collect::<Vec<_>>();

        for arity in 1..=16 {
            let Some(item_impl) = handler_impl_item(arity) else {
                continue;
            };
            self.record_generated_impl(&item_impl, span, item_cfgs.clone(), effective_cfgs.clone());
        }
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

fn parse_item<T: syn::parse::Parse>(tokens: TokenStream) -> Option<T> {
    syn::parse2(tokens).ok()
}
