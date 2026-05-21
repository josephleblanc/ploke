use ploke_llm::ProviderSlug;
use ploke_llm::request::ChatCompReqCore;
use ploke_llm::router_only::openrouter::{DataCollection, MaxPrice, ProviderPreferences, SortBy};
use ploke_llm::types::params::LLMParameters;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::loop_graph::ArtifactId;

use super::surface;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PolicyOrigin {
    Explicit,
    #[default]
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Effective<T> {
    pub(crate) value: T,
    pub(crate) origin: PolicyOrigin,
}

impl<T> Effective<T> {
    fn new(value: T, origin: PolicyOrigin) -> Self {
        Self { value, origin }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResponseFormat {
    None,
    JsonObject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProviderPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) order: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) allow_fallbacks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) require_parameters: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) data_collection: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) zdr: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) enforce_distillable_text: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) only: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) ignore: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) quantizations: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_price: Option<CanonicalMaxPrice>,
}

impl ProviderPolicy {
    fn from_prefs(value: &ProviderPreferences) -> Self {
        Self {
            order: value
                .order
                .as_ref()
                .map(|items| items.iter().map(|item| item.as_str().to_string()).collect()),
            allow_fallbacks: value.allow_fallbacks,
            require_parameters: value.require_parameters,
            data_collection: value.data_collection.map(|item| match item {
                DataCollection::Allow => "allow".to_string(),
                DataCollection::Deny => "deny".to_string(),
            }),
            zdr: value.zdr,
            enforce_distillable_text: canonical_provider_slugs(
                value.enforce_distillable_text.as_ref(),
            ),
            only: canonical_provider_slugs(value.only.as_ref()),
            ignore: canonical_provider_slugs(value.ignore.as_ref()),
            quantizations: value.quantizations.as_ref().map(|items| {
                let mut values = items
                    .iter()
                    .map(|item| item.as_str().to_string())
                    .collect::<Vec<_>>();
                values.sort();
                values
            }),
            sort: value.sort.map(|item| match item {
                SortBy::Price => "price".to_string(),
                SortBy::Throughput => "throughput".to_string(),
                SortBy::Latency => "latency".to_string(),
            }),
            max_price: value.max_price.map(CanonicalMaxPrice::from),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub(crate) struct CanonicalMaxPrice {
    pub(crate) prompt_tokens: Option<f64>,
    pub(crate) completion_tokens: Option<f64>,
    pub(crate) request: Option<f64>,
}

impl Eq for CanonicalMaxPrice {}

impl From<MaxPrice> for CanonicalMaxPrice {
    fn from(value: MaxPrice) -> Self {
        Self {
            prompt_tokens: value.prompt_tokens,
            completion_tokens: value.completion_tokens,
            request: value.request,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct ParameterPolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<Effective<u32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) seed: Option<Effective<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_p: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_k: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) frequency_penalty: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) presence_penalty: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) repetition_penalty: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) logit_bias: Option<Effective<Vec<(i32, String)>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_logprobs: Option<Effective<i32>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_p: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) top_a: Option<Effective<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) verbosity: Option<Effective<String>>,
}

impl ParameterPolicy {
    pub(crate) fn from_effective(request: &LLMParameters, defaults: &LLMParameters) -> Self {
        Self {
            max_tokens: choose_value(request.max_tokens, defaults.max_tokens),
            temperature: choose_float(request.temperature, defaults.temperature),
            seed: choose_value(request.seed, defaults.seed),
            top_p: choose_float(request.top_p, defaults.top_p),
            top_k: choose_float(request.top_k, defaults.top_k),
            frequency_penalty: choose_float(request.frequency_penalty, defaults.frequency_penalty),
            presence_penalty: choose_float(request.presence_penalty, defaults.presence_penalty),
            repetition_penalty: choose_float(
                request.repetition_penalty,
                defaults.repetition_penalty,
            ),
            logit_bias: choose_logit_bias(
                request.logit_bias.as_ref(),
                defaults.logit_bias.as_ref(),
            ),
            top_logprobs: choose_value(request.top_logprobs, defaults.top_logprobs),
            min_p: choose_float(request.min_p, defaults.min_p),
            top_a: choose_float(request.top_a, defaults.top_a),
            verbosity: choose_value(
                request
                    .verbosity
                    .map(|value| format!("{value:?}").to_lowercase()),
                defaults
                    .verbosity
                    .map(|value| format!("{value:?}").to_lowercase()),
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectiveBinding {
    pub(crate) summary: String,
    pub(crate) target_metric: String,
    pub(crate) writable_intent: String,
}

impl ObjectiveBinding {
    pub(crate) fn from_objective(objective: &surface::EditObjective) -> Self {
        Self {
            summary: objective.intent().to_string(),
            target_metric: format!("{:?}", objective.spec().target_metric()),
            writable_intent: format!("{:?}", objective.spec().writable_intent()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub(crate) struct ProposalBinding {
    pub(crate) proposal_id: String,
    pub(crate) run_id: String,
}

impl ProposalBinding {
    fn verify_matches(&self, proposal_id: &str, run_id: &str) -> Result<(), String> {
        if self.proposal_id.trim().is_empty() {
            return Err("request-policy receipt proposal_id is empty".to_string());
        }
        if self.run_id.trim().is_empty() {
            return Err("request-policy receipt run_id is empty".to_string());
        }
        if self.proposal_id != proposal_id {
            return Err(format!(
                "request-policy receipt proposal_id '{}' does not match admitted proposal '{}'",
                self.proposal_id, proposal_id
            ));
        }
        if self.run_id != run_id {
            return Err(format!(
                "request-policy receipt run_id '{}' does not match admitted run '{}'",
                self.run_id, run_id
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum PayloadHash {
    Known { value: String },
    Unknown { reason: String },
}

impl PayloadHash {
    pub(crate) fn known(value: impl Into<String>) -> Self {
        Self::Known {
            value: value.into(),
        }
    }

    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }

    fn verify(&self, label: &str) -> Result<(), String> {
        match self {
            Self::Known { value } => {
                if value.trim().is_empty() {
                    Err(format!("request-policy receipt {label} is empty"))
                } else {
                    Ok(())
                }
            }
            Self::Unknown { reason } => {
                if reason.trim().is_empty() {
                    Err(format!(
                        "request-policy receipt {label} unknown reason is empty"
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub(crate) schema_version: u32,
    pub(crate) base_artifact_id: ArtifactId,
    pub(crate) objective: ObjectiveBinding,
    #[serde(default)]
    pub(crate) proposal: ProposalBinding,
    pub(crate) router: String,
    pub(crate) model: Effective<String>,
    pub(crate) response_format: Effective<ResponseFormat>,
    pub(crate) stop: Effective<Vec<String>>,
    pub(crate) stream: Effective<bool>,
    pub(crate) parameters: ParameterPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) provider: Option<ProviderPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) request_payload_hash: Option<PayloadHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) response_payload_hash: Option<PayloadHash>,
    pub(crate) client_policy_hash: String,
}

impl Receipt {
    pub(crate) fn openrouter(
        base_artifact_id: ArtifactId,
        objective: &surface::EditObjective,
        request: &ChatCompReqCore,
        defaults: &ChatCompReqCore,
        request_params: &LLMParameters,
        default_params: &LLMParameters,
        provider: Option<&ProviderPreferences>,
    ) -> Self {
        Self::for_router(
            "openrouter",
            base_artifact_id,
            objective,
            request,
            defaults,
            request_params,
            default_params,
            provider,
        )
    }

    pub(crate) fn google(
        base_artifact_id: ArtifactId,
        objective: &surface::EditObjective,
        request: &ChatCompReqCore,
        defaults: &ChatCompReqCore,
        request_params: &LLMParameters,
        default_params: &LLMParameters,
    ) -> Self {
        Self::for_router(
            "google",
            base_artifact_id,
            objective,
            request,
            defaults,
            request_params,
            default_params,
            None,
        )
    }

    fn for_router(
        router: impl Into<String>,
        base_artifact_id: ArtifactId,
        objective: &surface::EditObjective,
        request: &ChatCompReqCore,
        defaults: &ChatCompReqCore,
        request_params: &LLMParameters,
        default_params: &LLMParameters,
        provider: Option<&ProviderPreferences>,
    ) -> Self {
        let receipt = Self {
            schema_version: 1,
            base_artifact_id,
            objective: ObjectiveBinding::from_objective(objective),
            proposal: ProposalBinding::default(),
            router: router.into(),
            model: Effective::new(
                request.model.to_string(),
                if request.model == defaults.model {
                    PolicyOrigin::Default
                } else {
                    PolicyOrigin::Explicit
                },
            ),
            response_format: Effective::new(
                if request.response_format.is_some() {
                    ResponseFormat::JsonObject
                } else {
                    ResponseFormat::None
                },
                if request.response_format.is_some() {
                    PolicyOrigin::Explicit
                } else {
                    PolicyOrigin::Default
                },
            ),
            stop: Effective::new(
                request
                    .stop
                    .clone()
                    .or_else(|| defaults.stop.clone())
                    .unwrap_or_default(),
                if request.stop.is_some() {
                    PolicyOrigin::Explicit
                } else {
                    PolicyOrigin::Default
                },
            ),
            stream: Effective::new(
                request.stream.or(defaults.stream).unwrap_or(false),
                if request.stream.is_some() {
                    PolicyOrigin::Explicit
                } else {
                    PolicyOrigin::Default
                },
            ),
            parameters: ParameterPolicy::from_effective(request_params, default_params),
            provider: provider.map(ProviderPolicy::from_prefs),
            request_payload_hash: None,
            response_payload_hash: None,
            client_policy_hash: String::new(),
        };
        let client_policy_hash = receipt.compute_client_policy_hash();
        Self {
            client_policy_hash,
            ..receipt
        }
    }

    pub(crate) fn bind_proposal(
        mut self,
        proposal_id: impl Into<String>,
        run_id: impl Into<String>,
    ) -> Self {
        self.proposal = ProposalBinding {
            proposal_id: proposal_id.into(),
            run_id: run_id.into(),
        };
        self
    }

    pub(crate) fn with_request_payload_hash(mut self, request_payload_hash: PayloadHash) -> Self {
        self.request_payload_hash = Some(request_payload_hash);
        self
    }

    pub(crate) fn with_response_payload_hash(mut self, response_payload_hash: PayloadHash) -> Self {
        self.response_payload_hash = Some(response_payload_hash);
        self
    }

    pub(crate) fn verify_admission_binding(
        &self,
        proposal_id: &str,
        run_id: &str,
    ) -> Result<(), String> {
        self.verify_current_client_policy_shape()?;
        self.proposal.verify_matches(proposal_id, run_id)?;
        verify_payload_hash(self.request_payload_hash.as_ref(), "request_payload_hash")?;
        verify_payload_hash(self.response_payload_hash.as_ref(), "response_payload_hash")?;
        Ok(())
    }

    /// Verifies the currently modeled client-side request-policy shape.
    ///
    /// This is not proof that a live outbound request receipt was captured; it
    /// only validates the policy fields currently represented by `Receipt`.
    pub(crate) fn verify_current_client_policy_shape(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err(format!(
                "request-policy receipt has unsupported schema_version {}",
                self.schema_version
            ));
        }
        if self.router.trim().is_empty() {
            return Err("request-policy receipt router is empty".to_string());
        }
        if self.model.value.trim().is_empty() {
            return Err("request-policy receipt model is empty".to_string());
        }
        if self.objective.summary.trim().is_empty() {
            return Err("request-policy receipt objective summary is empty".to_string());
        }
        if self.client_policy_hash.is_empty() {
            return Err("request-policy receipt client_policy_hash is empty".to_string());
        }
        let expected = self.compute_client_policy_hash();
        if self.client_policy_hash != expected {
            return Err(
                "request-policy receipt client_policy_hash does not match canonical policy"
                    .to_string(),
            );
        }
        Ok(())
    }

    pub(crate) fn base_artifact_id(&self) -> &ArtifactId {
        &self.base_artifact_id
    }

    fn compute_client_policy_hash(&self) -> String {
        let preimage = PolicyPreimage {
            router: &self.router,
            model: &self.model,
            response_format: &self.response_format,
            stop: &self.stop,
            stream: &self.stream,
            parameters: &self.parameters,
            provider: &self.provider,
        };
        let bytes =
            serde_json::to_vec(&preimage).expect("request-policy preimage should serialize");
        format!("{:x}", Sha256::digest(bytes))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProposalProducer {
    #[default]
    NonRouter,
    Router {
        request_policy: Receipt,
    },
}

impl ProposalProducer {
    pub(crate) fn verify_complete(
        &self,
        base_artifact_id: &ArtifactId,
        proposal_id: &str,
        run_id: &str,
    ) -> Result<(), String> {
        match self {
            Self::NonRouter => Ok(()),
            Self::Router { request_policy } => {
                request_policy.verify_admission_binding(proposal_id, run_id)?;
                if request_policy.base_artifact_id() != base_artifact_id {
                    return Err(format!(
                        "request-policy receipt base_artifact_id '{}' does not match checked base Artifact '{}'",
                        request_policy.base_artifact_id(),
                        base_artifact_id
                    ));
                }
                Ok(())
            }
        }
    }
}

#[derive(Serialize)]
struct PolicyPreimage<'a> {
    router: &'a str,
    model: &'a Effective<String>,
    response_format: &'a Effective<ResponseFormat>,
    stop: &'a Effective<Vec<String>>,
    stream: &'a Effective<bool>,
    parameters: &'a ParameterPolicy,
    provider: &'a Option<ProviderPolicy>,
}

fn canonical_provider_slugs<S>(values: Option<&S>) -> Option<Vec<String>>
where
    for<'a> &'a S: IntoIterator<Item = &'a ProviderSlug>,
{
    values.map(|items| {
        let mut values = items
            .into_iter()
            .map(|item| item.as_str().to_string())
            .collect::<Vec<_>>();
        values.sort();
        values
    })
}

fn verify_payload_hash(payload_hash: Option<&PayloadHash>, label: &str) -> Result<(), String> {
    let payload_hash = payload_hash.ok_or_else(|| {
        format!(
            "request-policy receipt {label} is unset; set known/unknown explicitly before admission"
        )
    })?;
    payload_hash.verify(label)
}

fn choose_value<T>(explicit: Option<T>, default: Option<T>) -> Option<Effective<T>> {
    explicit
        .map(|value| Effective::new(value, PolicyOrigin::Explicit))
        .or_else(|| default.map(|value| Effective::new(value, PolicyOrigin::Default)))
}

fn choose_float(explicit: Option<f32>, default: Option<f32>) -> Option<Effective<String>> {
    choose_value(explicit, default)
        .map(|value| Effective::new(format!("{:.9}", value.value), value.origin))
}

fn choose_logit_bias(
    explicit: Option<&std::collections::BTreeMap<i32, f32>>,
    default: Option<&std::collections::BTreeMap<i32, f32>>,
) -> Option<Effective<Vec<(i32, String)>>> {
    let (value, origin) = if let Some(value) = explicit {
        (value, PolicyOrigin::Explicit)
    } else {
        (default?, PolicyOrigin::Default)
    };
    Some(Effective::new(
        value
            .iter()
            .map(|(token, bias)| (*token, format!("{bias:.9}")))
            .collect(),
        origin,
    ))
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ploke_llm::router_only::openrouter::DataCollection;
    use ploke_llm::{ModelId, ProviderSlug};

    use super::*;
    use crate::cli::prototype1_state::edit_surface::{diagnosis, surface as edit_surface};
    use crate::cli::prototype1_state::history::EvidenceRef;

    fn objective() -> edit_surface::EditObjective {
        edit_surface::EditObjective::new(
            edit_surface::ObjectiveSpec::new(
                "reduce invalid candidates",
                edit_surface::ObjectiveKind::ReduceKnownFailure {
                    limiter: diagnosis::Limiter::InvalidCandidateGeneration,
                    failure_kind: diagnosis::FailureKind::SemanticEditResolution,
                },
                edit_surface::TargetMetric::InvalidEditSurfaceCandidates,
                edit_surface::WritableIntent::SemanticResolution,
            ),
            [EvidenceRef::new("history:diagnosis:test")],
            [EvidenceRef::new("history:context:test")],
        )
    }

    #[test]
    fn request_policy_receipt_hash_is_stable_for_equivalent_effective_provider_policy() {
        let defaults = ChatCompReqCore::default()
            .with_model(ModelId::from_str("moonshotai/kimi-k2").expect("model"));
        let request = defaults.clone().with_json_response().with_streaming(true);
        let params = LLMParameters {
            max_tokens: Some(512),
            top_p: Some(0.95),
            ..LLMParameters::default()
        };
        let default_params = LLMParameters {
            temperature: Some(0.2),
            ..LLMParameters::default()
        };
        let provider_a = ProviderPreferences::default()
            .with_data_collection(DataCollection::Deny)
            .with_only([ProviderSlug::new("anthropic"), ProviderSlug::new("openai")])
            .with_ignore([ProviderSlug::new("deepseek"), ProviderSlug::new("mistral")]);
        let provider_b = ProviderPreferences::default()
            .with_ignore([ProviderSlug::new("mistral"), ProviderSlug::new("deepseek")])
            .with_data_collection(DataCollection::Deny)
            .with_only([ProviderSlug::new("openai"), ProviderSlug::new("anthropic")]);

        let left = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &params,
            &default_params,
            Some(&provider_a),
        );
        let right = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &params,
            &default_params,
            Some(&provider_b),
        );

        assert_eq!(left.client_policy_hash, right.client_policy_hash);
    }

    #[test]
    fn request_policy_receipt_hash_changes_when_effective_policy_changes() {
        let defaults = ChatCompReqCore::default();
        let request = defaults.clone().with_streaming(true);
        let mut params = LLMParameters::default();
        params.top_p = Some(0.95);
        let mut changed = params.clone();
        changed.top_p = Some(0.5);

        let left = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &params,
            &LLMParameters::default(),
            None,
        );
        let right = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &changed,
            &LLMParameters::default(),
            None,
        );

        assert_ne!(left.client_policy_hash, right.client_policy_hash);
    }

    #[test]
    fn google_receipt_uses_google_router_without_openrouter_provider_policy() {
        let defaults = ChatCompReqCore::default();
        let request = defaults
            .clone()
            .with_model(ModelId::from_str("google/gemini-2.5-flash").expect("model"));

        let receipt = Receipt::google(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &LLMParameters::default(),
            &LLMParameters::default(),
        );

        assert_eq!(receipt.router, "google");
        assert!(receipt.provider.is_none());
        receipt
            .verify_current_client_policy_shape()
            .expect("google receipt should verify current policy shape");
    }

    #[test]
    fn request_policy_receipt_hash_changes_when_router_changes() {
        let defaults = ChatCompReqCore::default();
        let request = defaults
            .clone()
            .with_model(ModelId::from_str("google/gemini-2.5-flash").expect("model"));

        let openrouter = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &LLMParameters::default(),
            &LLMParameters::default(),
            None,
        );
        let google = Receipt::google(
            ArtifactId::new("artifact:base"),
            &objective(),
            &request,
            &defaults,
            &LLMParameters::default(),
            &LLMParameters::default(),
        );

        assert_ne!(openrouter.client_policy_hash, google.client_policy_hash);
    }

    #[test]
    fn router_receipt_rejects_missing_proposal_binding_for_admission() {
        let receipt = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &ChatCompReqCore::default(),
            &ChatCompReqCore::default(),
            &LLMParameters::default(),
            &LLMParameters::default(),
            None,
        );

        let err = receipt
            .verify_admission_binding("proposal-1", "run-1")
            .expect_err("missing proposal binding must reject");

        assert!(err.contains("proposal_id is empty"));
    }

    #[test]
    fn router_receipt_rejects_unset_payload_hashes_for_admission() {
        let receipt = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &ChatCompReqCore::default(),
            &ChatCompReqCore::default(),
            &LLMParameters::default(),
            &LLMParameters::default(),
            None,
        )
        .bind_proposal("proposal-live", "run-live");

        let err = receipt
            .verify_admission_binding("proposal-live", "run-live")
            .expect_err("unset payload hashes must reject");

        assert!(err.contains("request_payload_hash is unset"));
    }

    #[test]
    fn router_receipt_accepts_bound_proposal_with_explicit_unknown_payload_hashes() {
        let receipt = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &ChatCompReqCore::default(),
            &ChatCompReqCore::default(),
            &LLMParameters::default(),
            &LLMParameters::default(),
            None,
        )
        .bind_proposal("proposal-live", "run-live")
        .with_request_payload_hash(PayloadHash::unknown("request payload not exposed"))
        .with_response_payload_hash(PayloadHash::unknown("response payload not exposed"));

        receipt
            .verify_admission_binding("proposal-live", "run-live")
            .expect("explicit unknown payload hashes are admissible when binding matches");
    }

    #[test]
    fn router_receipt_rejects_mismatched_proposal_binding_for_admission() {
        let receipt = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &ChatCompReqCore::default(),
            &ChatCompReqCore::default(),
            &LLMParameters::default(),
            &LLMParameters::default(),
            None,
        )
        .bind_proposal("proposal-live", "run-live")
        .with_request_payload_hash(PayloadHash::unknown("live request payload not captured"))
        .with_response_payload_hash(PayloadHash::unknown("live response payload not captured"));

        let err = receipt
            .verify_admission_binding("proposal-other", "run-live")
            .expect_err("mismatched proposal binding must reject");

        assert!(err.contains("does not match admitted proposal"));
    }

    #[test]
    fn router_receipt_rejects_mismatched_run_binding_for_admission() {
        let receipt = Receipt::openrouter(
            ArtifactId::new("artifact:base"),
            &objective(),
            &ChatCompReqCore::default(),
            &ChatCompReqCore::default(),
            &LLMParameters::default(),
            &LLMParameters::default(),
            None,
        )
        .bind_proposal("proposal-live", "run-live")
        .with_request_payload_hash(PayloadHash::unknown("live request payload not captured"))
        .with_response_payload_hash(PayloadHash::unknown("live response payload not captured"));

        let err = receipt
            .verify_admission_binding("proposal-live", "run-other")
            .expect_err("mismatched run binding must reject");

        assert!(err.contains("does not match admitted run"));
    }

    #[test]
    fn router_proposal_producer_rejects_mismatched_base_artifact_id() {
        let producer = ProposalProducer::Router {
            request_policy: Receipt::openrouter(
                ArtifactId::new("artifact:base"),
                &objective(),
                &ChatCompReqCore::default(),
                &ChatCompReqCore::default(),
                &LLMParameters::default(),
                &LLMParameters::default(),
                None,
            )
            .bind_proposal("proposal-live", "run-live")
            .with_request_payload_hash(PayloadHash::unknown("live request payload not captured"))
            .with_response_payload_hash(PayloadHash::unknown("live response payload not captured")),
        };

        let err = producer
            .verify_complete(
                &ArtifactId::new("artifact:other"),
                "proposal-live",
                "run-live",
            )
            .expect_err("mismatched base artifact id must reject");

        assert!(err.contains("base_artifact_id"));
        assert!(err.contains("artifact:base"));
        assert!(err.contains("artifact:other"));
    }
}
