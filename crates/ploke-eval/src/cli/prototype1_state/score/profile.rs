use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ScoreProfile {
    pub(crate) id: String,
    pub(crate) components: Vec<ScoreProfileComponent>,
}

impl ScoreProfile {
    pub(crate) fn operational() -> Self {
        Self {
            id: PROCEDURE_ID.to_string(),
            components: vec![Self::component(OPERATIONAL_COMPONENT_ID, PROCEDURE_ID)],
        }
    }

    pub(crate) fn operational_with_protocol() -> Self {
        Self {
            id: OPERATIONAL_PROTOCOL_PROFILE_ID.to_string(),
            components: vec![
                Self::component(OPERATIONAL_COMPONENT_ID, PROCEDURE_ID),
                Self::component(PROTOCOL_COMPONENT_ID, PROTOCOL_COMPONENT_PROCEDURE_ID),
            ],
        }
    }

    pub(super) fn includes_protocol(&self) -> bool {
        self.includes_component(PROTOCOL_COMPONENT_ID)
    }

    pub(super) fn includes_operational(&self) -> bool {
        self.includes_component(OPERATIONAL_COMPONENT_ID)
    }

    pub(super) fn includes_component(&self, component_id: &str) -> bool {
        self.components
            .iter()
            .any(|component| component.component_id == component_id)
    }

    fn component(component_id: &str, procedure_id: &str) -> ScoreProfileComponent {
        ScoreProfileComponent {
            component_id: component_id.to_string(),
            procedure_id: procedure_id.to_string(),
        }
    }
}
