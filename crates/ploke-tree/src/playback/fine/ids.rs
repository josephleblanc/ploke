pub(super) fn fine_candidate_step_id(
    block_height: u64,
    block_hash: &str,
    entry_index: usize,
    candidate_index: usize,
    candidate_set_root: Option<&str>,
    occurrence_id: Option<&str>,
    membership_id: Option<&str>,
) -> String {
    if let Some(membership_id) = membership_id {
        if let Some(candidate_set_root) = candidate_set_root {
            return format!(
                "history:{block_height}:{block_hash}:entry:{entry_index}:candidate-membership:{}:{}",
                step_id_component(candidate_set_root),
                step_id_component(membership_id),
            );
        }
        return format!(
            "history:{block_height}:{block_hash}:entry:{entry_index}:candidate-membership:{membership_id}"
        );
    }
    if let Some(occurrence_id) = occurrence_id {
        return format!(
            "history:{block_height}:{block_hash}:entry:{entry_index}:candidate-occurrence:{occurrence_id}"
        );
    }
    format!("history:{block_height}:{block_hash}:entry:{entry_index}:candidate:{candidate_index}")
}

pub(super) fn fine_selected_step_id(
    block_height: u64,
    block_hash: &str,
    candidate_set_root: Option<&str>,
    occurrence_id: Option<&str>,
    membership_id: Option<&str>,
) -> String {
    if let Some(membership_id) = membership_id {
        if let Some(candidate_set_root) = candidate_set_root {
            return format!(
                "history:{block_height}:{block_hash}:successor-selected:{}:{}",
                step_id_component(candidate_set_root),
                step_id_component(membership_id),
            );
        }
        return format!("history:{block_height}:{block_hash}:successor-selected:{membership_id}");
    }
    if let Some(occurrence_id) = occurrence_id {
        return format!("history:{block_height}:{block_hash}:successor-selected:{occurrence_id}");
    }
    format!("history:{block_height}:{block_hash}:successor-selected")
}

fn step_id_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '%' => encoded.push_str("%25"),
            ':' => encoded.push_str("%3A"),
            _ => encoded.push(ch),
        }
    }
    encoded
}
