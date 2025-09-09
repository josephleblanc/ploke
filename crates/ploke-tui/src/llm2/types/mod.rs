use super::*;

pub(super) mod model_types;
pub(super) mod enums;
pub(super) mod newtypes;
pub(super) mod params;
pub(super) mod meta;


// --- common types ---
/// Architecture details of a model, including input/output modalities and tokenizer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Architecture {
    /// Input modalities supported by this model (text, image, audio, video).
    pub input_modalities: Vec<InputModality>,
    pub modality: Modality,
    pub output_modalities: Vec<OutputModality>,
    pub tokenizer: Tokenizer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruct_type: Option<InstructType>,
}
