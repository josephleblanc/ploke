use cozo::{DataValue, Num};
use uuid::Uuid;

use crate::{
    DbError,
    database::{to_string, to_string_list, to_uuid},
};

use super::{
    CallCalleeEvidenceRow, CallContextRow, CallPathEdge, CallReceiver, CallRelationKind,
    CallResolutionKind, CallResolutionRow, CallSiteKind, CallSiteRow, CallStatusKind,
    CallTargetKind, CallTargetRow, FuturePollFieldProducerFlow, LocalBindingEdgeRow,
    LocalBindingRelationKind, LocalBindingRow, ReturnedCallBinding, ReturnedCallBindingFlow,
    ReturnedCallProducer, ReturnedCallSite, ReturnedCallSource, ReturnedFutureExecutionFlow,
    ReturnedFutureFlow, ReturnedFutureSite, SelfFieldAssignmentFlow, SelfFieldParameterFlow,
};

pub(super) fn decode_site(row: &[DataValue]) -> Result<CallSiteRow, DbError> {
    let site = CallSiteRow {
        id: to_uuid(&row[0])?,
        owner_id: to_uuid(&row[1])?,
        kind: CallSiteKind::from_str(&to_string(&row[2])?)?,
        span: span_pair(&row[3])?,
        cfgs: to_string_list(&row[4])?,
        unsafe_block: required_bool(&row[5])?,
        path: optional_string_list(&row[6])?,
        method: optional_string(&row[7])?,
        macro_name: optional_string(&row[8])?,
        receiver: CallReceiver::from_parts(&row[9], &row[10])?,
        arg_count: optional_index(&row[11])?,
        generic_arg_count: optional_index(&row[12])?,
    };
    validate_call_site_shape(&site)?;
    Ok(site)
}

fn validate_call_site_shape(site: &CallSiteRow) -> Result<(), DbError> {
    let valid = match site.kind {
        CallSiteKind::Path => {
            non_empty_path(site.path.as_deref())
                && site.method.is_none()
                && site.macro_name.is_none()
                && site.receiver.is_none()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_some()
        }
        CallSiteKind::Method => {
            site.path.is_none()
                && non_empty_string(site.method.as_deref())
                && site.macro_name.is_none()
                && site.receiver.is_some()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_some()
        }
        CallSiteKind::Dynamic => {
            optional_non_empty_path(site.path.as_deref())
                && site.method.is_none()
                && site.macro_name.is_none()
                && site.receiver.is_none()
                && site.arg_count.is_some()
                && site.generic_arg_count.is_none()
        }
        CallSiteKind::Macro => {
            site.path.is_none()
                && site.method.is_none()
                && non_empty_string(site.macro_name.as_deref())
                && site.receiver.is_none()
                && site.arg_count.is_none()
                && site.generic_arg_count.is_none()
        }
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed {:?} call_site {}",
            site.kind, site.id
        )))
    }
}

fn non_empty_path(path: Option<&[String]>) -> bool {
    path.is_some_and(|path| !path.is_empty() && path.iter().all(|segment| !segment.is_empty()))
}

fn optional_non_empty_path(path: Option<&[String]>) -> bool {
    path.is_none_or(|path| !path.is_empty() && path.iter().all(|segment| !segment.is_empty()))
}

fn non_empty_string(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.is_empty())
}

fn required_bool(value: &DataValue) -> Result<bool, DbError> {
    match value {
        DataValue::Bool(value) => Ok(*value),
        other => Err(DbError::Cozo(format!(
            "expected call graph bool, found {other:?}"
        ))),
    }
}

pub(super) fn decode_target(row: &[DataValue]) -> Result<CallTargetRow, DbError> {
    Ok(CallTargetRow {
        site_id: to_uuid(&row[0])?,
        target_id: to_uuid(&row[1])?,
        relation: CallRelationKind::from_str(&to_string(&row[2])?)?,
        source_kind: CallSiteKind::from_str(&to_string(&row[3])?)?,
        target_kind: CallTargetKind::from_str(&to_string(&row[4])?)?,
    })
}

pub(super) fn decode_resolution(row: &[DataValue]) -> Result<CallResolutionRow, DbError> {
    let status = CallResolutionRow {
        site_id: to_uuid(&row[0])?,
        site_kind: CallSiteKind::from_str(&to_string(&row[1])?)?,
        status: CallStatusKind::from_str(&to_string(&row[2])?)?,
        resolution: optional_resolution(&row[3])?,
    };
    validate_resolution_shape(&status)?;
    Ok(status)
}

pub(super) fn decode_callee_evidence(row: &[DataValue]) -> Result<CallCalleeEvidenceRow, DbError> {
    let evidence = CallCalleeEvidenceRow {
        site_id: to_uuid(&row[0])?,
        site_kind: CallSiteKind::from_str(&to_string(&row[1])?)?,
        callee_kind: to_string(&row[2])?,
        callee_path: to_string_list(&row[3])?,
        closure_id: optional_uuid(&row[4])?,
    };
    validate_callee_evidence_shape(&evidence)?;
    Ok(evidence)
}

fn validate_callee_evidence_shape(evidence: &CallCalleeEvidenceRow) -> Result<(), DbError> {
    if !evidence.callee_kind.is_empty()
        && !evidence.callee_path.is_empty()
        && evidence
            .callee_path
            .iter()
            .all(|segment| !segment.is_empty())
    {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed call_callee_evidence row for site {}",
            evidence.site_id
        )))
    }
}

pub(super) fn decode_local_binding(row: &[DataValue]) -> Result<LocalBindingRow, DbError> {
    let binding = LocalBindingRow {
        id: to_uuid(&row[0])?,
        owner_id: to_uuid(&row[1])?,
        owner_kind: to_string(&row[2])?,
        kind: to_string(&row[3])?,
        name: to_string(&row[4])?,
        span: span_pair(&row[5])?,
        cfgs: to_string_list(&row[6])?,
        source_kind: to_string(&row[7])?,
        source_id: optional_uuid(&row[8])?,
        source_call_kind: optional_string(&row[9])?,
        source_path: optional_string_list(&row[10])?,
        callee_kind: optional_string(&row[11])?,
        callee_path: optional_string_list(&row[12])?,
    };
    validate_local_binding_shape(&binding)?;
    Ok(binding)
}

pub(super) fn decode_local_binding_edge(row: &[DataValue]) -> Result<LocalBindingEdgeRow, DbError> {
    let edge = LocalBindingEdgeRow {
        source_id: to_uuid(&row[0])?,
        target_id: to_uuid(&row[1])?,
        relation: LocalBindingRelationKind::from_str(&to_string(&row[2])?)?,
        source_kind: to_string(&row[3])?,
        target_kind: to_string(&row[4])?,
    };
    validate_local_binding_edge_shape(&edge)?;
    Ok(edge)
}

pub(super) fn decode_returned_call_binding_flow(
    row: &[DataValue],
) -> Result<ReturnedCallBindingFlow, DbError> {
    let flow = ReturnedCallBindingFlow {
        caller_id: to_uuid(&row[0])?,
        dynamic: ReturnedCallSite {
            id: to_uuid(&row[1])?,
            span: span_pair(&row[2])?,
            path: to_string_list(&row[3])?,
            target_id: to_uuid(&row[4])?,
            relation: CallRelationKind::from_str(&to_string(&row[5])?)?,
            target_kind: CallTargetKind::from_str(&to_string(&row[6])?)?,
        },
        producer: ReturnedCallProducer {
            id: to_uuid(&row[9])?,
            site_id: to_uuid(&row[7])?,
            span: span_pair(&row[8])?,
            path: to_string_list(&row[3])?,
        },
        binding: ReturnedCallBinding {
            id: to_uuid(&row[10])?,
            source: ReturnedCallSource {
                id: to_uuid(&row[11])?,
                relation: LocalBindingRelationKind::from_str(&to_string(&row[12])?)?,
                kind: to_string(&row[13])?,
            },
        },
    };
    validate_returned_call_binding_flow(&flow)?;
    Ok(flow)
}

fn validate_returned_call_binding_flow(flow: &ReturnedCallBindingFlow) -> Result<(), DbError> {
    let target_is_callable = matches!(
        (flow.dynamic.relation, flow.dynamic.target_kind),
        (CallRelationKind::DynamicFunction, CallTargetKind::Function)
            | (CallRelationKind::DynamicClosure, CallTargetKind::Closure)
    );
    let source_is_binding_edge = matches!(
        flow.binding.source.relation,
        LocalBindingRelationKind::BindingSourceClosure
            | LocalBindingRelationKind::BindingSourceCallResult
    );

    if target_is_callable
        && source_is_binding_edge
        && flow.dynamic.path == flow.producer.path
        && !flow.dynamic.path.is_empty()
        && !flow.binding.source.kind.is_empty()
    {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed returned call binding flow for dynamic call {}",
            flow.dynamic.id
        )))
    }
}

pub(super) fn decode_returned_future_flow(
    row: &[DataValue],
) -> Result<ReturnedFutureFlow, DbError> {
    let future = ReturnedFutureSite {
        id: to_uuid(&row[6])?,
        span: span_pair(&row[7])?,
        path: to_string_list(&row[8])?,
        callee_kind: to_string(&row[9])?,
    };
    let flow = ReturnedFutureFlow {
        caller_id: to_uuid(&row[0])?,
        producer: ReturnedCallProducer {
            site_id: to_uuid(&row[1])?,
            span: span_pair(&row[2])?,
            path: to_string_list(&row[3])?,
            id: to_uuid(&row[4])?,
        },
        binding: ReturnedCallBinding {
            id: to_uuid(&row[5])?,
            source: ReturnedCallSource {
                id: future.id,
                relation: LocalBindingRelationKind::from_str(&to_string(&row[10])?)?,
                kind: to_string(&row[11])?,
            },
        },
        future,
    };
    validate_returned_future_flow(&flow)?;
    Ok(flow)
}

fn validate_returned_future_flow(flow: &ReturnedFutureFlow) -> Result<(), DbError> {
    let valid = flow.binding.source.relation == LocalBindingRelationKind::BindingSourceCallResult
        && flow.binding.source.kind == "Dynamic"
        && flow.binding.source.id == flow.future.id
        && flow.future.callee_kind == "ReturnedPathCall"
        && !flow.future.path.is_empty()
        && !flow.producer.path.is_empty();

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed returned future flow for producer call {}",
            flow.producer.site_id
        )))
    }
}

pub(super) fn decode_returned_future_execution_flow(
    row: &[DataValue],
) -> Result<ReturnedFutureExecutionFlow, DbError> {
    let flow = ReturnedFutureExecutionFlow {
        caller_id: to_uuid(&row[0])?,
        producer: ReturnedCallProducer {
            site_id: to_uuid(&row[1])?,
            span: span_pair(&row[2])?,
            path: to_string_list(&row[3])?,
            id: to_uuid(&row[4])?,
        },
        producer_binding: ReturnedCallBinding {
            id: to_uuid(&row[5])?,
            source: ReturnedCallSource {
                id: to_uuid(&row[6])?,
                relation: LocalBindingRelationKind::from_str(&to_string(&row[10])?)?,
                kind: to_string(&row[11])?,
            },
        },
        future: ReturnedFutureSite {
            id: to_uuid(&row[6])?,
            span: span_pair(&row[7])?,
            path: to_string_list(&row[8])?,
            callee_kind: to_string(&row[9])?,
        },
        maker: ReturnedCallProducer {
            site_id: to_uuid(&row[12])?,
            span: span_pair(&row[13])?,
            path: to_string_list(&row[8])?,
            id: to_uuid(&row[14])?,
        },
        callable_binding: ReturnedCallBinding {
            id: to_uuid(&row[15])?,
            source: ReturnedCallSource {
                id: to_uuid(&row[16])?,
                relation: LocalBindingRelationKind::from_str(&to_string(&row[17])?)?,
                kind: to_string(&row[18])?,
            },
        },
        body_edge: CallPathEdge {
            caller_id: to_uuid(&row[16])?,
            call_site_id: to_uuid(&row[19])?,
            span: span_pair(&row[20])?,
            callee_id: to_uuid(&row[22])?,
            relation: CallRelationKind::from_str(&to_string(&row[23])?)?,
            source_kind: CallSiteKind::from_str(&to_string(&row[24])?)?,
            target_kind: CallTargetKind::from_str(&to_string(&row[25])?)?,
        },
    };
    validate_returned_future_execution_flow(&flow)?;
    Ok(flow)
}

fn validate_returned_future_execution_flow(
    flow: &ReturnedFutureExecutionFlow,
) -> Result<(), DbError> {
    let valid = flow.producer_binding.source.relation
        == LocalBindingRelationKind::BindingSourceCallResult
        && flow.producer_binding.source.kind == "Dynamic"
        && flow.producer_binding.source.id == flow.future.id
        && flow.future.callee_kind == "ReturnedPathCall"
        && flow.maker.path == flow.future.path
        && !flow.maker.path.is_empty()
        && flow.callable_binding.source.relation == LocalBindingRelationKind::BindingSourceClosure
        && flow.callable_binding.source.kind == "Closure"
        && flow.body_edge.caller_id == flow.callable_binding.source.id;

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed returned future execution flow for producer call {}",
            flow.producer.site_id
        )))
    }
}

pub(super) fn decode_self_field_parameter_flow(
    row: &[DataValue],
) -> Result<SelfFieldParameterFlow, DbError> {
    let flow = SelfFieldParameterFlow {
        site: decode_site(&row[0..13])?,
        status: decode_resolution(&row[13..17])?,
        constructor_id: to_uuid(&row[17])?,
        return_binding: decode_local_binding(&row[18..31])?,
        field_binding: decode_local_binding(&row[31..44])?,
        parameter_binding: decode_local_binding(&row[44..57])?,
    };
    validate_self_field_parameter_flow(&flow)?;
    Ok(flow)
}

fn validate_self_field_parameter_flow(flow: &SelfFieldParameterFlow) -> Result<(), DbError> {
    let site_path = flow.site.path.as_deref().unwrap_or_default();
    let field_path = flow
        .field_binding
        .source_path
        .as_deref()
        .unwrap_or_default();
    let parameter_path = flow
        .field_binding
        .callee_path
        .as_deref()
        .unwrap_or_default();

    let valid = flow.site.kind == CallSiteKind::Dynamic
        && site_path.len() == 2
        && site_path.first().is_some_and(|segment| segment == "self")
        && field_path == &site_path[1..]
        && flow.status.site_id == flow.site.id
        && flow.status.site_kind == CallSiteKind::Dynamic
        && flow.status.status != CallStatusKind::Resolved
        && flow.status.resolution.is_none()
        && flow.return_binding.owner_id == flow.constructor_id
        && flow.return_binding.kind == "ReturnExpression"
        && flow.return_binding.source_kind == "Constructed"
        && flow.field_binding.owner_id == flow.constructor_id
        && flow.field_binding.kind == "FieldProjection"
        && flow.field_binding.source_kind == "FieldProjection"
        && flow.field_binding.source_id == Some(flow.return_binding.id)
        && flow.field_binding.callee_kind.as_deref() == Some("Path")
        && flow.parameter_binding.owner_id == flow.constructor_id
        && flow.parameter_binding.kind == "ParameterBinding"
        && flow.parameter_binding.source_kind == "Parameter"
        && parameter_path.len() == 1
        && parameter_path[0] == flow.parameter_binding.name;

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed self-field parameter flow for call site {}",
            flow.site.id
        )))
    }
}

pub(super) fn decode_self_field_assignment_flow(
    row: &[DataValue],
) -> Result<SelfFieldAssignmentFlow, DbError> {
    let flow = SelfFieldAssignmentFlow {
        site: decode_site(&row[0..13])?,
        status: decode_resolution(&row[13..17])?,
        owner_type: to_string(&row[17])?,
        setter_id: to_uuid(&row[18])?,
        assignment_binding: decode_local_binding(&row[19..32])?,
        parameter_binding: decode_local_binding(&row[32..45])?,
        source_edge: decode_local_binding_edge(&row[45..50])?,
    };
    validate_self_field_assignment_flow(&flow)?;
    Ok(flow)
}

fn validate_self_field_assignment_flow(flow: &SelfFieldAssignmentFlow) -> Result<(), DbError> {
    let site_path = flow.site.path.as_deref().unwrap_or_default();
    let site_field_path = match flow.site.kind {
        CallSiteKind::Dynamic if site_path.len() == 2 && site_path[0] == "self" => &site_path[1..],
        CallSiteKind::Path if site_path.len() == 1 => site_path,
        _ => &[],
    };
    let assignment_path = flow
        .assignment_binding
        .source_path
        .as_deref()
        .unwrap_or_default();
    let parameter_path = flow
        .assignment_binding
        .callee_path
        .as_deref()
        .unwrap_or_default();

    let valid = !site_field_path.is_empty()
        && assignment_path == site_field_path
        && flow.status.site_id == flow.site.id
        && flow.status.site_kind == flow.site.kind
        && flow.status.status != CallStatusKind::Resolved
        && flow.status.resolution.is_none()
        && !flow.owner_type.is_empty()
        && flow.assignment_binding.owner_id == flow.setter_id
        && flow.assignment_binding.kind == "FieldAssignment"
        && flow.assignment_binding.source_kind == "SelfFieldAssignment"
        && flow.assignment_binding.source_id.is_none()
        && flow.assignment_binding.callee_kind.as_deref() == Some("Path")
        && flow.parameter_binding.owner_id == flow.setter_id
        && flow.parameter_binding.kind == "ParameterBinding"
        && flow.parameter_binding.source_kind == "Parameter"
        && parameter_path.len() == 1
        && parameter_path[0] == flow.parameter_binding.name
        && flow.source_edge.source_id == flow.assignment_binding.id
        && flow.source_edge.target_id == flow.parameter_binding.id
        && flow.source_edge.relation == LocalBindingRelationKind::BindingSourceParameter
        && flow.source_edge.source_kind == "LocalBinding"
        && flow.source_edge.target_kind == "LocalBinding";

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed self-field assignment flow for call site {}",
            flow.site.id
        )))
    }
}

pub(super) fn decode_future_poll_field_producer_flow(
    row: &[DataValue],
) -> Result<FuturePollFieldProducerFlow, DbError> {
    let flow = FuturePollFieldProducerFlow {
        site: decode_site(&row[0..13])?,
        status: decode_resolution(&row[13..17])?,
        poll_owner_type: to_string(&row[17])?,
        producer_id: to_uuid(&row[18])?,
        return_binding: decode_local_binding(&row[19..32])?,
        field_binding: decode_local_binding(&row[32..45])?,
        source_status: decode_resolution(&row[45..49])?,
        source_site: decode_site(&row[49..62])?,
        source_edge: decode_local_binding_edge(&row[62..67])?,
    };
    validate_future_poll_field_producer_flow(&flow)?;
    Ok(flow)
}

fn validate_future_poll_field_producer_flow(
    flow: &FuturePollFieldProducerFlow,
) -> Result<(), DbError> {
    let Some(CallReceiver::MethodResultField { field_path, .. }) = flow.site.receiver.as_ref()
    else {
        return Err(DbError::Cozo(format!(
            "future poll field producer flow {} missing method-result field receiver",
            flow.site.id
        )));
    };

    let field_name = format!("return.{}", field_path.join("."));
    let return_path = flow
        .return_binding
        .source_path
        .as_deref()
        .unwrap_or_default();

    let valid = flow.site.kind == CallSiteKind::Method
        && flow.site.method.as_deref() == Some("poll")
        && flow.status.site_id == flow.site.id
        && flow.status.site_kind == CallSiteKind::Method
        && flow.status.status != CallStatusKind::Resolved
        && flow.status.resolution.is_none()
        && !flow.poll_owner_type.is_empty()
        && return_path
            .last()
            .is_some_and(|segment| segment == &flow.poll_owner_type)
        && flow.return_binding.owner_id == flow.producer_id
        && flow.return_binding.kind == "ReturnExpression"
        && flow.return_binding.name == "return"
        && flow.return_binding.source_kind == "Constructed"
        && flow.field_binding.owner_id == flow.producer_id
        && flow.field_binding.kind == "LetBinding"
        && flow.field_binding.name == field_name
        && flow.field_binding.source_id == Some(flow.source_site.id)
        && flow.field_binding.source_call_kind.as_deref() == Some(flow.source_site.kind.as_str())
        && flow.field_binding.source_kind == "PathCallResult"
        && flow.source_status.site_id == flow.source_site.id
        && flow.source_status.site_kind == flow.source_site.kind
        && flow.source_site.owner_id == flow.producer_id
        && flow.source_edge.source_id == flow.field_binding.id
        && flow.source_edge.target_id == flow.source_site.id
        && flow.source_edge.relation == LocalBindingRelationKind::BindingSourceCallResult
        && flow.source_edge.source_kind == "LocalBinding"
        && flow.source_edge.target_kind == flow.source_site.kind.as_str();

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed future poll field producer flow for call site {}",
            flow.site.id
        )))
    }
}

fn validate_local_binding_edge_shape(edge: &LocalBindingEdgeRow) -> Result<(), DbError> {
    let valid = match edge.relation {
        LocalBindingRelationKind::OwnerContainsBinding => {
            is_call_owner_kind(&edge.source_kind) && edge.target_kind == "LocalBinding"
        }
        LocalBindingRelationKind::BindingSourceClosure => {
            edge.source_kind == "LocalBinding" && edge.target_kind == "Closure"
        }
        LocalBindingRelationKind::BindingSourceCallResult => {
            edge.source_kind == "LocalBinding" && is_call_site_kind(&edge.target_kind)
        }
        LocalBindingRelationKind::BindingSourceFunction => {
            edge.source_kind == "LocalBinding" && edge.target_kind == "Function"
        }
        LocalBindingRelationKind::BindingSourceLocalItem => {
            edge.source_kind == "LocalBinding" && edge.target_kind == "LocalItem"
        }
        LocalBindingRelationKind::BindingProjectsField => {
            edge.source_kind == "LocalBinding" && edge.target_kind == "LocalBinding"
        }
        LocalBindingRelationKind::BindingSourceParameter => {
            edge.source_kind == "LocalBinding" && edge.target_kind == "LocalBinding"
        }
        LocalBindingRelationKind::BindingAliasesBinding => {
            edge.source_kind == "LocalBinding" && edge.target_kind == "LocalBinding"
        }
        LocalBindingRelationKind::ArgumentSuppliesParameter => {
            is_call_site_kind(&edge.source_kind) && edge.target_kind == "LocalBinding"
        }
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed local_binding_edge {:?}: {} -> {}",
            edge.relation, edge.source_kind, edge.target_kind
        )))
    }
}

fn validate_local_binding_shape(binding: &LocalBindingRow) -> Result<(), DbError> {
    let valid = match binding.source_kind.as_str() {
        "Parameter" => {
            binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && binding.source_path.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "Typed" => {
            binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "Constructed" => {
            binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "InitializedPath" => {
            binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "ValueAlias" => {
            binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "FieldProjection" => {
            binding.source_id.is_some()
                && binding.source_call_kind.is_none()
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.as_deref() == Some("Path")
                && non_empty_path(binding.callee_path.as_deref())
        }
        "SelfFieldAssignment" => {
            binding.source_id.is_none()
                && binding.source_call_kind.is_none()
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.as_deref() == Some("Path")
                && non_empty_path(binding.callee_path.as_deref())
        }
        "Closure" | "AsyncClosure" => {
            binding.source_id.is_some()
                && binding.source_call_kind.is_none()
                && binding.source_path.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "LocalFunction" => {
            binding.source_id.is_some()
                && binding.source_call_kind.is_none()
                && binding.source_path.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "PathCallResult" => {
            binding.source_id.is_some()
                && binding.source_call_kind.as_deref() == Some("Path")
                && non_empty_path(binding.source_path.as_deref())
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        }
        "DynamicCallResult" => {
            binding.source_id.is_some()
                && binding.source_call_kind.as_deref() == Some("Dynamic")
                && binding.source_path.is_none()
                && non_empty_string(binding.callee_kind.as_deref())
                && optional_non_empty_path(binding.callee_path.as_deref())
        }
        _ => false,
    };

    let valid_kind = match binding.kind.as_str() {
        "ReturnExpression" => binding.name == "return",
        "ParameterBinding" => {
            non_empty_string(Some(binding.name.as_str())) && binding.source_kind == "Parameter"
        }
        "LetBinding" => {
            non_empty_string(Some(binding.name.as_str()))
                && matches!(
                    binding.source_kind.as_str(),
                    "Typed"
                        | "Constructed"
                        | "InitializedPath"
                        | "ValueAlias"
                        | "Closure"
                        | "AsyncClosure"
                        | "PathCallResult"
                        | "DynamicCallResult"
                )
        }
        "LocalFunctionBinding" => {
            non_empty_string(Some(binding.name.as_str())) && binding.source_kind == "LocalFunction"
        }
        "FieldProjection" => {
            non_empty_string(Some(binding.name.as_str()))
                && binding.source_kind == "FieldProjection"
        }
        "FieldAssignment" => {
            non_empty_string(Some(binding.name.as_str()))
                && binding.source_kind == "SelfFieldAssignment"
        }
        _ => false,
    };

    if valid && valid_kind {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "malformed local_binding {} source kind {:?}",
            binding.id, binding.source_kind
        )))
    }
}

fn is_call_owner_kind(kind: &str) -> bool {
    matches!(
        kind,
        "Function"
            | "Macro"
            | "Method"
            | "Const"
            | "Static"
            | "Closure"
            | "AsyncBlock"
            | "LocalItem"
    )
}

fn is_call_site_kind(kind: &str) -> bool {
    matches!(kind, "Path" | "Method" | "Dynamic" | "Macro")
}

fn validate_resolution_shape(status: &CallResolutionRow) -> Result<(), DbError> {
    let valid = match status.status {
        CallStatusKind::Resolved => status.resolution == Some(CallResolutionKind::LocalExact),
        CallStatusKind::Unresolved
        | CallStatusKind::Ambiguous
        | CallStatusKind::External
        | CallStatusKind::Unsupported => status.resolution.is_none(),
    };

    if valid {
        Ok(())
    } else {
        Err(DbError::Cozo(format!(
            "call_resolution_status resolution_kind {:?} is invalid for {:?} call site {}",
            status.resolution, status.status, status.site_id
        )))
    }
}

pub(super) fn validate_owner_context_targets(row: &CallContextRow) -> Result<(), DbError> {
    if row.status.status == CallStatusKind::Resolved && row.targets.len() != 1 {
        return Err(DbError::Cozo(format!(
            "resolved call site {} expected exactly one target, found {}",
            row.site.id,
            row.targets.len()
        )));
    }
    Ok(())
}

fn optional_string(value: &DataValue) -> Result<Option<String>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_string(other)?)),
    }
}

fn optional_string_list(value: &DataValue) -> Result<Option<Vec<String>>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_string_list(other)?)),
    }
}

fn optional_uuid(value: &DataValue) -> Result<Option<Uuid>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(to_uuid(other)?)),
    }
}

fn optional_index(value: &DataValue) -> Result<Option<u32>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(required_index(other)?)),
    }
}

fn optional_resolution(value: &DataValue) -> Result<Option<CallResolutionKind>, DbError> {
    match value {
        DataValue::Null => Ok(None),
        other => Ok(Some(CallResolutionKind::from_str(&to_string(other)?)?)),
    }
}

fn required_index(value: &DataValue) -> Result<u32, DbError> {
    match value {
        DataValue::Num(Num::Int(value)) => u32::try_from(*value)
            .map_err(|err| DbError::Cozo(format!("call graph integer out of u32 range: {err}"))),
        other => Err(DbError::Cozo(format!(
            "expected call graph integer, found {other:?}"
        ))),
    }
}

fn span_pair(value: &DataValue) -> Result<(u32, u32), DbError> {
    match value {
        DataValue::List(items) if items.len() == 2 => {
            Ok((required_index(&items[0])?, required_index(&items[1])?))
        }
        other => Err(DbError::Cozo(format!(
            "expected call graph span pair, found {other:?}"
        ))),
    }
}
