pub struct HintRenderer;

impl HintRenderer {
    pub fn render_field_hint(
        &self,
        available_fields: &[&str],
        is_public_struct_access: bool,
    ) -> String {
        let public = if is_public_struct_access { "public " } else { "" };

        match available_fields {
            [] => "No fields available.".to_string(),
            [field] => format!("Only available {public}field is \"{field}\"."),
            _ => {
                let use_storage_wording =
                    available_fields.iter().any(|field| field.contains("storage"));

                if use_storage_wording {
                    const NUM_OF_FIELDS_TO_DISPLAY: usize = 4;
                    let display_fields = available_fields
                        .iter()
                        .map(|field| format!("storage.{field}"))
                        .take(NUM_OF_FIELDS_TO_DISPLAY)
                        .collect::<Vec<_>>()
                        .join(", ");

                    format!("Available storage fields are {display_fields}.")
                } else {
                    const NUM_OF_FIELDS_TO_DISPLAY: usize = 4;
                    let display_fields = available_fields
                        .iter()
                        .take(NUM_OF_FIELDS_TO_DISPLAY)
                        .map(|field| format!("\"{field}\""))
                        .collect::<Vec<_>>()
                        .join(", ");

                    format!("Available {public}fields are {display_fields}.")
                }
            }
        }
    }
}

pub fn exercise_fixture() -> (String, String) {
    let renderer = HintRenderer;
    let struct_hint =
        renderer.render_field_hint(&["alpha", "beta", "gamma", "delta", "epsilon"], true);
    let storage_hint =
        renderer.render_field_hint(&["storage.alpha", "storage.beta", "storage.gamma"], false);

    (struct_hint, storage_hint)
}
