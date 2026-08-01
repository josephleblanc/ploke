use ploke_selection_score::papers::planning;

#[test]
fn valid_partition_rejects_negative_or_overallocated_costs() {
    assert!(planning::valid_partition(&[1.0, 2.0], 3.0));
    assert!(!planning::valid_partition(&[1.0, 2.1], 3.0));
    assert!(!planning::valid_partition(&[-1.0, 1.0], 3.0));
    assert!(!planning::valid_partition(&[1.0], -3.0));
}

#[test]
fn maximum_single_change_takes_sup_over_finite_transition_deltas() {
    let mscf = planning::maximum_single_change(&[-2.0, 0.5, 1.25, 1.0])
        .expect("finite transition deltas should score");

    assert_eq!(mscf, 1.25);
    assert!(planning::maximum_single_change(&[]).is_none());
    assert!(planning::maximum_single_change(&[f64::INFINITY]).is_none());
}

#[test]
fn remaining_after_saturation_subtracts_finite_allocations() {
    let next = planning::remaining_after_saturation(&[5.0, 3.0], &[1.5, 3.0])
        .expect("valid saturated allocation should update remainders");

    assert_eq!(next, vec![3.5, 0.0]);
}

#[test]
fn remaining_after_saturation_keeps_infinite_remainders_sticky() {
    let next = planning::remaining_after_saturation(&[f64::INFINITY, 3.0], &[10.0, 1.0])
        .expect("finite allocation under infinite remainder should be sticky");

    assert_eq!(next, vec![f64::INFINITY, 2.0]);
}

#[test]
fn remaining_after_saturation_rejects_invalid_allocations() {
    assert!(planning::remaining_after_saturation(&[2.0], &[3.0]).is_none());
    assert!(planning::remaining_after_saturation(&[2.0], &[-1.0]).is_none());
    assert!(planning::remaining_after_saturation(&[f64::NAN], &[1.0]).is_none());
    assert!(planning::remaining_after_saturation(&[f64::INFINITY], &[f64::INFINITY]).is_none());
}

#[test]
fn valid_saturated_allocation_delegates_to_remainder_update() {
    assert!(planning::valid_saturated_allocation(&[2.0], &[1.0]));
    assert!(!planning::valid_saturated_allocation(&[2.0], &[3.0]));
}
