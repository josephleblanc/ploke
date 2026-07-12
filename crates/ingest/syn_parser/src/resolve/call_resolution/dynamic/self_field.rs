use syn::visit::{self, Visit};

use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{CallBodyOwnerId, ExecutableBodyId, ExecutableBodyKind, StructNode, StructNodeId},
        relations::TypeRelation,
        types::TypeNode,
    },
};

use super::super::{
    CallRelationResolver, LocalTypeResolution,
    path::{ParameterCallResolution, ParameterCallTarget},
};
use super::unparen_expr;

impl CallRelationResolver<'_> {
    pub(super) fn resolve_self_field_callable_call(
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
        if !matches!(self.type_node(field_type)?, TypeNode::Function(_)) {
            return Ok(None);
        }

        let Some(struct_node) = self.self_struct_node(owner, type_relations)? else {
            return Ok(None);
        };
        self.unique_struct_field_closure_initializer(struct_node, field_name, type_relations)
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
                let Some(resolution) =
                    self.resolve_parameter_value_call(owner, &init.parameter_path, type_relations)?
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
                    let Some(resolution) = self.resolve_parameter_value_call(
                        owner,
                        &init.parameter_path,
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

    fn visit_expr_closure(&mut self, _expr: &'ast syn::ExprClosure) {}
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

fn callable_parameter_path(expr: &syn::Expr) -> Option<Vec<String>> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            single_segment_path(path_segments(&path.path))
        }
        syn::Expr::Call(call) => boxed_call_parameter_path(call),
        _ => None,
    }
}

fn boxed_call_parameter_path(call: &syn::ExprCall) -> Option<Vec<String>> {
    let syn::Expr::Path(func) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if func.qself.is_some() || !is_box_new(&path_segments(&func.path)) {
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

fn single_segment_path(path: Vec<String>) -> Option<Vec<String>> {
    matches!(path.as_slice(), [_]).then_some(path)
}

fn is_box_new(path: &[String]) -> bool {
    matches!(path, [box_, new] if box_ == "Box" && new == "new")
        || matches!(
            path,
            [root, boxed, box_, new]
                if (root == "std" || root == "alloc") && boxed == "boxed" && box_ == "Box" && new == "new"
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
