use std::collections::BTreeMap;

use syn::visit::{self, Visit};

use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{CallBodyOwnerId, ExecutableBodyId, ExecutableBodyKind, StructNode, StructNodeId},
        nodes::{OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId, TypeAliasNodeId},
        relations::TypeRelation,
        types::TypeNode,
    },
};

use super::super::{
    CallRelationResolver, LocalTypeResolution, MAX_IMPORT_CHAIN_DEPTH,
    path::{ParameterCallResolution, ParameterCallTarget},
};
use super::unparen_expr;

impl CallRelationResolver<'_> {
    pub(in crate::resolve::call_resolution) fn resolve_self_field_callable_call(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        let Some(field_name) = direct_self_field_name(path) else {
            return Ok(None);
        };

        if let Some(closure_id) =
            self.resolve_self_field_closure_call(owner, field_name, type_relations)?
        {
            return Ok(Some(ParameterCallResolution::Exact(
                ParameterCallTarget::Closure(closure_id),
            )));
        }

        let Some(struct_node) = self.self_struct_node(owner, type_relations)? else {
            return Ok(None);
        };
        if let Some(resolution) = self.struct_field_function_initializer_resolution(
            struct_node,
            field_name,
            type_relations,
        )? {
            return Ok(Some(resolution));
        }
        self.struct_field_parameter_initializer_resolution(struct_node, field_name, type_relations)
    }

    fn resolve_self_field_closure_call(
        &self,
        owner: CallBodyOwnerId,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ExecutableBodyId>, SynParserError> {
        let Some(field_type) = self.self_field_type(owner, field_name, type_relations)? else {
            return Ok(None);
        };
        if !self.field_type_is_callable_function(field_type, type_relations)? {
            return Ok(None);
        }

        let Some(struct_node) = self.self_struct_node(owner, type_relations)? else {
            return Ok(None);
        };
        self.unique_struct_field_closure_initializer(struct_node, field_name, type_relations)
    }

    fn field_type_is_callable_function(
        &self,
        field_type: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        if matches!(self.type_node(field_type)?, TypeNode::Function(_)) {
            return Ok(true);
        }

        let Ok(source) = OrdinaryTypeSourceId::try_from(field_type) else {
            return Ok(false);
        };
        let mut targets = ordinary_targets_for_source(source, type_relations);
        targets.sort_unstable();
        targets.dedup();

        for target in targets {
            if self.type_target_is_function_alias(target, type_relations, 0)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn type_target_is_function_alias(
        &self,
        target: OrdinaryTypeTargetId,
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<bool, SynParserError> {
        let Ok(alias_id) = TypeAliasNodeId::try_from(target) else {
            return Ok(false);
        };
        if depth >= MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded type alias chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {alias_id}"
            )));
        }

        let alias_node = self.graph.get_type_alias_checked(alias_id)?;
        if matches!(self.type_node(alias_node.type_id)?, TypeNode::Function(_)) {
            return Ok(true);
        }

        let Ok(source) = OrdinaryTypeSourceId::try_from(alias_node.type_id) else {
            return Ok(false);
        };
        for next in ordinary_targets_for_source(source, type_relations) {
            if self.type_target_is_function_alias(next, type_relations, depth + 1)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn struct_field_parameter_initializer_resolution(
        &self,
        struct_node: &StructNode,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        let mut targets = Vec::new();
        let mut matched = false;

        for function in self.graph.functions() {
            let owner = CallBodyOwnerId::Function(function.id);
            let inits = struct_field_parameter_initializer_paths(
                function.body.as_deref(),
                &function.name,
                &struct_node.name,
                field_name,
            )?;
            for init in inits {
                if !self.struct_initializer_path_matches(
                    owner,
                    &init.struct_path,
                    struct_node,
                    type_relations,
                )? {
                    continue;
                }
                matched = true;
                let Some(resolution) = self.resolve_parameter_value_call_for_callable_field(
                    owner,
                    &init.parameter_path,
                    field_name,
                    type_relations,
                )?
                else {
                    return Ok(None);
                };
                push_parameter_targets(&mut targets, resolution);
            }
        }

        for impl_node in self.graph.impls() {
            for method in &impl_node.methods {
                let owner = CallBodyOwnerId::Method(method.id);
                let inits = struct_field_parameter_initializer_paths(
                    method.body.as_deref(),
                    &method.name,
                    &struct_node.name,
                    field_name,
                )?;
                for init in inits {
                    if !self.struct_initializer_path_matches(
                        owner,
                        &init.struct_path,
                        struct_node,
                        type_relations,
                    )? {
                        continue;
                    }
                    matched = true;
                    let Some(resolution) = self.resolve_parameter_value_call_for_callable_field(
                        owner,
                        &init.parameter_path,
                        field_name,
                        type_relations,
                    )?
                    else {
                        return Ok(None);
                    };
                    push_parameter_targets(&mut targets, resolution);
                }
            }
        }

        if !matched {
            return Ok(None);
        }

        targets.sort_unstable();
        targets.dedup();
        Ok(match targets.as_slice() {
            [target] => Some(ParameterCallResolution::Exact(*target)),
            [_, _, ..] => Some(ParameterCallResolution::Ambiguous(targets)),
            [] => None,
        })
    }

    fn struct_field_function_initializer_resolution(
        &self,
        struct_node: &StructNode,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        let mut targets = Vec::new();
        let mut matched = false;
        let mut blocked = false;

        for function in self.graph.functions() {
            let owner = CallBodyOwnerId::Function(function.id);
            let owner_cfg_gated = !function.cfgs.is_empty();
            let inits = struct_field_function_initializer_paths(
                function.body.as_deref(),
                &function.name,
                &struct_node.name,
                field_name,
            )?;
            for path in inits.blocked {
                if self.struct_initializer_path_matches(
                    owner,
                    &path,
                    struct_node,
                    type_relations,
                )? {
                    blocked = true;
                }
            }
            for init in inits.direct {
                if !self.struct_initializer_path_matches(
                    owner,
                    &init.struct_path,
                    struct_node,
                    type_relations,
                )? {
                    continue;
                }
                matched = true;
                let init_cfg_gated = init.cfg_gated || owner_cfg_gated;
                match self.resolve_dynamic_path(owner, &init.function_path)? {
                    super::DynamicPathResolution::Resolved(target) => {
                        targets.push(ParameterCallTarget::Function(target));
                    }
                    super::DynamicPathResolution::Unresolved
                    | super::DynamicPathResolution::Unsupported
                        if init_cfg_gated => {}
                    _ => return Ok(None),
                }
            }
        }

        for impl_node in self.graph.impls() {
            for method in &impl_node.methods {
                let owner = CallBodyOwnerId::Method(method.id);
                let owner_cfg_gated = !method.cfgs.is_empty();
                let inits = struct_field_function_initializer_paths(
                    method.body.as_deref(),
                    &method.name,
                    &struct_node.name,
                    field_name,
                )?;
                for path in inits.blocked {
                    if self.struct_initializer_path_matches(
                        owner,
                        &path,
                        struct_node,
                        type_relations,
                    )? {
                        blocked = true;
                    }
                }
                for init in inits.direct {
                    if !self.struct_initializer_path_matches(
                        owner,
                        &init.struct_path,
                        struct_node,
                        type_relations,
                    )? {
                        continue;
                    }
                    matched = true;
                    let init_cfg_gated = init.cfg_gated || owner_cfg_gated;
                    match self.resolve_dynamic_path(owner, &init.function_path)? {
                        super::DynamicPathResolution::Resolved(target) => {
                            targets.push(ParameterCallTarget::Function(target));
                        }
                        super::DynamicPathResolution::Unresolved
                        | super::DynamicPathResolution::Unsupported
                            if init_cfg_gated => {}
                        _ => return Ok(None),
                    }
                }
            }
        }

        if !matched {
            return Ok(None);
        }
        if blocked {
            return Ok(None);
        }

        targets.sort_unstable();
        targets.dedup();
        Ok(match targets.as_slice() {
            [target] => Some(ParameterCallResolution::Exact(*target)),
            [_, _, ..] => Some(ParameterCallResolution::Ambiguous(targets)),
            [] => None,
        })
    }

    fn self_struct_node(
        &self,
        owner: CallBodyOwnerId,
        type_relations: &[TypeRelation],
    ) -> Result<Option<&StructNode>, SynParserError> {
        let CallBodyOwnerId::Method(method_id) = owner else {
            return Ok(None);
        };
        let Some(impl_id) = self.impl_for_owner_method(method_id)? else {
            return Ok(None);
        };
        let Some(impl_node) = self.maybe_impl_node(impl_id) else {
            return Ok(None);
        };
        let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
            return Ok(None);
        };
        let Ok(struct_id) = StructNodeId::try_from(self_target) else {
            return Ok(None);
        };
        self.graph.get_struct_checked(struct_id).map(Some)
    }

    fn unique_struct_field_closure_initializer(
        &self,
        struct_node: &StructNode,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ExecutableBodyId>, SynParserError> {
        let mut candidates = Vec::new();

        for function in self.graph.functions() {
            if self.body_initializes_struct_field_with_closure(
                CallBodyOwnerId::Function(function.id),
                function.body.as_deref(),
                &function.name,
                struct_node,
                field_name,
                type_relations,
            )? && let Some(closure_id) =
                self.unique_closure_body_for_owner(CallBodyOwnerId::Function(function.id))?
            {
                candidates.push(closure_id);
            }
        }

        for impl_node in self.graph.impls() {
            for method in &impl_node.methods {
                if self.body_initializes_struct_field_with_closure(
                    CallBodyOwnerId::Method(method.id),
                    method.body.as_deref(),
                    &method.name,
                    struct_node,
                    field_name,
                    type_relations,
                )? && let Some(closure_id) =
                    self.unique_closure_body_for_owner(CallBodyOwnerId::Method(method.id))?
                {
                    candidates.push(closure_id);
                }
            }
        }

        candidates.sort_unstable();
        candidates.dedup();
        Ok(match candidates.as_slice() {
            [closure_id] => Some(*closure_id),
            _ => None,
        })
    }

    fn unique_closure_body_for_owner(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Option<ExecutableBodyId>, SynParserError> {
        let closures = self
            .graph
            .executable_bodies()
            .iter()
            .filter(|body| body.parent == owner && body.kind == ExecutableBodyKind::Closure)
            .map(|body| body.id)
            .collect::<Vec<_>>();

        match closures.as_slice() {
            [closure_id] => Ok(Some(*closure_id)),
            [] => Err(SynParserError::InternalState(format!(
                "{owner} initializes a struct field with a closure but no closure executable body was recorded"
            ))),
            _ => Ok(None),
        }
    }

    fn body_initializes_struct_field_with_closure(
        &self,
        owner: CallBodyOwnerId,
        body: Option<&str>,
        owner_name: &str,
        struct_node: &StructNode,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let paths = struct_field_closure_initializer_paths(
            body,
            owner_name,
            &struct_node.name,
            field_name,
        )?;
        let mut matches = 0;
        for path in paths {
            if self.struct_initializer_path_matches(owner, &path, struct_node, type_relations)? {
                matches += 1;
            }
        }
        Ok(matches == 1)
    }

    fn struct_initializer_path_matches(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        struct_node: &StructNode,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        if matches!(path, [segment] if segment == "Self") {
            let CallBodyOwnerId::Method(method_id) = owner else {
                return Ok(false);
            };
            let Some(impl_id) = self.impl_for_owner_method(method_id)? else {
                return Ok(false);
            };
            let Some(impl_node) = self.maybe_impl_node(impl_id) else {
                return Ok(false);
            };
            return Ok(
                self.impl_self_target(impl_node, type_relations)? == Some(struct_node.id.into())
            );
        }

        Ok(matches!(
            self.resolve_local_type_path(owner, path)?,
            LocalTypeResolution::Resolved(target) if target == struct_node.id.into()
        ))
    }
}

fn direct_self_field_name(path: &[String]) -> Option<&str> {
    match path {
        [root, field] if root == "self" => Some(field),
        _ => None,
    }
}

fn ordinary_targets_for_source(
    source: OrdinaryTypeSourceId,
    type_relations: &[TypeRelation],
) -> Vec<OrdinaryTypeTargetId> {
    type_relations
        .iter()
        .filter_map(|relation| match relation {
            TypeRelation::Ordinary {
                source: relation_source,
                target,
            } if *relation_source == source => Some(*target),
            _ => None,
        })
        .collect()
}

fn struct_field_closure_initializer_paths(
    body: Option<&str>,
    owner_name: &str,
    struct_name: &str,
    field_name: &str,
) -> Result<Vec<Vec<String>>, SynParserError> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    let block = syn::parse_str::<syn::Block>(body).map_err(|err| {
        SynParserError::InternalState(format!(
            "failed to parse stored body for self-field closure proof in {owner_name}: {err}"
        ))
    })?;
    let mut visitor = StructFieldClosureVisitor {
        struct_name,
        field_name,
        paths: Vec::new(),
    };
    visitor.visit_block(&block);
    Ok(visitor.paths)
}

fn struct_field_parameter_initializer_paths(
    body: Option<&str>,
    owner_name: &str,
    struct_name: &str,
    field_name: &str,
) -> Result<Vec<FieldParameterInitializer>, SynParserError> {
    let Some(body) = body else {
        return Ok(Vec::new());
    };
    let block = syn::parse_str::<syn::Block>(body).map_err(|err| {
        SynParserError::InternalState(format!(
            "failed to parse stored body for self-field parameter proof in {owner_name}: {err}"
        ))
    })?;
    let mut visitor = StructFieldParameterVisitor {
        struct_name,
        field_name,
        initializers: Vec::new(),
    };
    visitor.visit_block(&block);
    Ok(visitor.initializers)
}

struct StructFieldClosureVisitor<'a> {
    struct_name: &'a str,
    field_name: &'a str,
    paths: Vec<Vec<String>>,
}

impl<'ast> Visit<'ast> for StructFieldClosureVisitor<'_> {
    fn visit_expr_struct(&mut self, expr: &'ast syn::ExprStruct) {
        if struct_path_matches(&expr.path, self.struct_name) {
            for field in &expr.fields {
                if member_matches(&field.member, self.field_name)
                    && matches!(unparen_expr(&field.expr), syn::Expr::Closure(_))
                {
                    self.paths.push(path_segments(&expr.path));
                }
            }
        }
        visit::visit_expr_struct(self, expr);
    }

    fn visit_expr_closure(&mut self, _expr: &'ast syn::ExprClosure) {}
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldParameterInitializer {
    struct_path: Vec<String>,
    parameter_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldFunctionInitializer {
    struct_path: Vec<String>,
    function_path: Vec<String>,
    cfg_gated: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FieldFunctionInitializers {
    direct: Vec<FieldFunctionInitializer>,
    blocked: Vec<Vec<String>>,
}

struct StructFieldParameterVisitor<'a> {
    struct_name: &'a str,
    field_name: &'a str,
    initializers: Vec<FieldParameterInitializer>,
}

impl<'ast> Visit<'ast> for StructFieldParameterVisitor<'_> {
    fn visit_expr_struct(&mut self, expr: &'ast syn::ExprStruct) {
        if struct_path_matches(&expr.path, self.struct_name) {
            for field in &expr.fields {
                if member_matches(&field.member, self.field_name)
                    && let Some(parameter_path) = callable_parameter_path(&field.expr)
                {
                    self.initializers.push(FieldParameterInitializer {
                        struct_path: path_segments(&expr.path),
                        parameter_path,
                    });
                }
            }
        }
        visit::visit_expr_struct(self, expr);
    }

    fn visit_expr_assign(&mut self, expr: &'ast syn::ExprAssign) {
        if self_field_path_matches(expr.left.as_ref(), self.field_name)
            && let Some(parameter_path) = callable_parameter_path(expr.right.as_ref())
        {
            self.initializers.push(FieldParameterInitializer {
                struct_path: vec!["Self".to_string()],
                parameter_path,
            });
        }
        visit::visit_expr_assign(self, expr);
    }

    fn visit_expr_closure(&mut self, _expr: &'ast syn::ExprClosure) {}
}

fn struct_field_function_initializer_paths(
    body: Option<&str>,
    owner_name: &str,
    struct_name: &str,
    field_name: &str,
) -> Result<FieldFunctionInitializers, SynParserError> {
    let Some(body) = body else {
        return Ok(FieldFunctionInitializers::default());
    };
    let block = syn::parse_str::<syn::Block>(body).map_err(|err| {
        SynParserError::InternalState(format!(
            "failed to parse stored body for self-field function proof in {owner_name}: {err}"
        ))
    })?;
    let mut visitor = StructFieldFunctionVisitor {
        struct_name,
        field_name,
        initializers: FieldFunctionInitializers::default(),
        cfg_depth: 0,
        aliases: vec![BTreeMap::new()],
    };
    visitor.visit_block(&block);
    Ok(visitor.initializers)
}

struct StructFieldFunctionVisitor<'a> {
    struct_name: &'a str,
    field_name: &'a str,
    initializers: FieldFunctionInitializers,
    cfg_depth: usize,
    aliases: Vec<BTreeMap<String, FieldFunctionAlias>>,
}

impl<'ast> Visit<'ast> for StructFieldFunctionVisitor<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.aliases.push(BTreeMap::new());
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.aliases.pop();
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some((name, function_path)) = local_function_alias(local)
            && let Some(scope) = self.aliases.last_mut()
        {
            scope.insert(
                name,
                FieldFunctionAlias {
                    function_path,
                    cfg_gated: self.cfg_depth > 0,
                },
            );
        }
        visit::visit_local(self, local);
    }

    fn visit_expr_struct(&mut self, expr: &'ast syn::ExprStruct) {
        if struct_path_matches(&expr.path, self.struct_name) {
            for field in &expr.fields {
                if !member_matches(&field.member, self.field_name) {
                    continue;
                }
                match self.callable_function_initializer(field) {
                    FunctionInitializer::Direct {
                        function_path,
                        cfg_gated,
                    } => {
                        self.initializers.direct.push(FieldFunctionInitializer {
                            struct_path: path_segments(&expr.path),
                            function_path,
                            cfg_gated,
                        });
                    }
                    FunctionInitializer::Blocked => {
                        self.initializers.blocked.push(path_segments(&expr.path));
                    }
                    FunctionInitializer::Other => {}
                }
            }
        }
        visit::visit_expr_struct(self, expr);
    }

    fn visit_expr_block(&mut self, expr: &'ast syn::ExprBlock) {
        let is_cfg_gated = expr.attrs.iter().any(is_cfg_attr);
        if is_cfg_gated {
            self.cfg_depth += 1;
        }
        visit::visit_expr_block(self, expr);
        if is_cfg_gated {
            self.cfg_depth -= 1;
        }
    }

    fn visit_expr_closure(&mut self, _expr: &'ast syn::ExprClosure) {}
}

impl StructFieldFunctionVisitor<'_> {
    fn callable_function_initializer(&self, field: &syn::FieldValue) -> FunctionInitializer {
        let Some(function_path) = callable_function_path(&field.expr) else {
            return FunctionInitializer::Other;
        };
        if field.colon_token.is_some() {
            return FunctionInitializer::Direct {
                function_path,
                cfg_gated: self.cfg_depth > 0,
            };
        }

        let [name] = function_path.as_slice() else {
            return FunctionInitializer::Blocked;
        };
        let Some(alias) = self.function_alias(name) else {
            return FunctionInitializer::Blocked;
        };
        FunctionInitializer::Direct {
            function_path: alias.function_path.clone(),
            cfg_gated: self.cfg_depth > 0 || alias.cfg_gated,
        }
    }

    fn function_alias(&self, name: &str) -> Option<&FieldFunctionAlias> {
        self.aliases.iter().rev().find_map(|scope| scope.get(name))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldFunctionAlias {
    function_path: Vec<String>,
    cfg_gated: bool,
}

enum FunctionInitializer {
    Direct {
        function_path: Vec<String>,
        cfg_gated: bool,
    },
    Blocked,
    Other,
}

fn is_cfg_attr(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("cfg")
}

fn struct_path_matches(path: &syn::Path, struct_name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == struct_name || segment.ident == "Self")
}

fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn member_matches(member: &syn::Member, field_name: &str) -> bool {
    match member {
        syn::Member::Named(ident) => ident == field_name,
        syn::Member::Unnamed(index) => index.index.to_string() == field_name,
    }
}

fn self_field_path_matches(expr: &syn::Expr, field_name: &str) -> bool {
    let syn::Expr::Field(field) = unparen_expr(expr) else {
        return false;
    };
    if !member_matches(&field.member, field_name) {
        return false;
    }
    matches!(
        unparen_expr(field.base.as_ref()),
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self")
    )
}

fn callable_parameter_path(expr: &syn::Expr) -> Option<Vec<String>> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            single_segment_path(path_segments(&path.path))
        }
        syn::Expr::Call(call) => {
            boxed_call_parameter_path(call).or_else(|| option_some_parameter_path(call))
        }
        _ => None,
    }
}

fn callable_function_path(expr: &syn::Expr) -> Option<Vec<String>> {
    let syn::Expr::Path(path) = unparen_expr(expr) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    single_segment_path(path_segments(&path.path))
}

fn local_function_alias(local: &syn::Local) -> Option<(String, Vec<String>)> {
    let syn::Pat::Ident(pat) = &local.pat else {
        return None;
    };
    if pat.by_ref.is_some() || pat.mutability.is_some() || pat.subpat.is_some() {
        return None;
    }
    let init = local.init.as_ref()?;
    let function_path = callable_function_path(&init.expr)?;
    Some((pat.ident.to_string(), function_path))
}

fn boxed_call_parameter_path(call: &syn::ExprCall) -> Option<Vec<String>> {
    let syn::Expr::Path(func) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if func.qself.is_some() || !super::is_box_new_path(&func.path) {
        return None;
    }
    let mut args = call.args.iter();
    let arg = args.next()?;
    if args.next().is_some() {
        return None;
    }
    let syn::Expr::Path(path) = unparen_expr(arg) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    single_segment_path(path_segments(&path.path))
}

fn option_some_parameter_path(call: &syn::ExprCall) -> Option<Vec<String>> {
    let syn::Expr::Path(func) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if func.qself.is_some() || !is_option_some(&path_segments(&func.path)) {
        return None;
    }
    let mut args = call.args.iter();
    let arg = args.next()?;
    if args.next().is_some() {
        return None;
    }
    callable_parameter_path(arg)
}

fn single_segment_path(path: Vec<String>) -> Option<Vec<String>> {
    matches!(path.as_slice(), [_]).then_some(path)
}

fn is_option_some(path: &[String]) -> bool {
    matches!(path, [variant] if variant == "Some")
        || matches!(path, [option, variant] if option == "Option" && variant == "Some")
        || matches!(
            path,
            [root, option_mod, option, variant]
                if (root == "std" || root == "core")
                    && option_mod == "option"
                    && option == "Option"
                    && variant == "Some"
        )
}

fn push_parameter_targets(
    targets: &mut Vec<ParameterCallTarget>,
    resolution: ParameterCallResolution,
) {
    match resolution {
        ParameterCallResolution::Exact(target) => targets.push(target),
        ParameterCallResolution::Ambiguous(candidates) => targets.extend(candidates),
    }
}
