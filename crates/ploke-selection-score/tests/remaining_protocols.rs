use ploke_selection_score::{
    papers::{agentcl, trace},
    protocols::{harness1, hll, iteris, mcp_persona, vesta},
};

#[test]
fn trace_probability_and_bce_follow_binary_risk_formulas() {
    assert_eq!(trace::unsafe_probability(0.0), Some(0.5));

    let loss = trace::bce_loss(0.8, true).expect("valid probability should score");
    assert!((loss - -0.8_f64.ln()).abs() < 1e-12);
    assert!(trace::unsafe_probability(f64::NAN).is_none());
    assert!(trace::bce_loss(1.2, true).is_none());
}

#[test]
fn agentcl_gain_metrics_are_directional_differences() {
    let plasticity = agentcl::plasticity_gain(0.7, 0.4).expect("finite scores should differ");
    let stability = agentcl::stability_gain(0.8, 0.7).expect("finite scores should differ");
    let generalization =
        agentcl::generalization_gain(0.6, 0.5).expect("finite scores should differ");

    assert!((plasticity - 0.3).abs() < 1e-12);
    assert!((stability - 0.1).abs() < 1e-12);
    assert!((generalization - 0.1).abs() < 1e-12);
    assert!(agentcl::plasticity_gain(f64::NAN, 0.4).is_none());
}

#[test]
fn vesta_selects_metric_direction_and_keeps_earliest_tie() {
    assert_eq!(
        vesta::choose_model(vesta::Metric::Aic, &[3.0, 1.0, 1.0]),
        Some(1)
    );
    assert_eq!(
        vesta::choose_model(vesta::Metric::Jsd, &[3.0, 1.0, 2.0]),
        Some(1)
    );
    assert_eq!(
        vesta::choose_model(vesta::Metric::ElpdLoo, &[3.0, 4.0, 4.0]),
        Some(1)
    );
    assert!(vesta::choose_model(vesta::Metric::Aic, &[]).is_none());
}

#[test]
fn harness1_predicates_preserve_state_authority_boundaries() {
    assert!(harness1::keep_doc(true, true));
    assert!(!harness1::keep_doc(true, false));
    assert!(harness1::should_stop(true, false));
    assert!(harness1::should_stop(false, true));
    assert!(!harness1::should_stop(false, false));
}

#[test]
fn hll_success_and_pass_rate_require_all_success_predicates() {
    let result = [
        hll::hll_success(true, true, true),
        hll::hll_success(true, false, true),
        hll::hll_success(true, true, false),
    ];

    assert_eq!(result, [true, false, false]);
    assert_eq!(hll::pass_rate(&result), Some(1.0 / 3.0));
    assert!(hll::pass_rate(&[]).is_none());
}

#[test]
fn mcp_persona_success_and_rate_require_execution_persona_and_fidelity() {
    let result = [
        mcp_persona::task_success(true, true, true),
        mcp_persona::task_success(true, true, false),
        mcp_persona::task_success(false, true, true),
    ];

    assert_eq!(result, [true, false, false]);
    assert_eq!(mcp_persona::success_rate(&result), Some(1.0 / 3.0));
    assert!(mcp_persona::success_rate(&[]).is_none());
}

#[test]
fn iteris_accepts_only_source_supported_outputs_with_human_verification() {
    assert!(iteris::accept(Some(iteris::OutputKind::PhaseDiagram), true));
    assert!(iteris::accept(
        Some(iteris::OutputKind::QrcpCounterexample),
        true
    ));
    assert!(iteris::accept(
        Some(iteris::OutputKind::VerifiedProof),
        true
    ));
    assert!(!iteris::accept(
        Some(iteris::OutputKind::VerifiedProof),
        false
    ));
    assert!(!iteris::accept(None, true));
}
