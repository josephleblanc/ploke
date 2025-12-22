use ploke_core::ArcStr;

use crate::ModelId;
use crate::llm::ProviderKey;

#[derive(Debug, Clone)]
pub enum OverlayAction {
    CloseOverlay(OverlayKind),
    RequestModelEndpoints { model_id: ModelId },
    SelectModel {
        model_id: ModelId,
        provider: Option<ProviderKey>,
    },
    SelectEmbeddingModel {
        model_id: ModelId,
        provider: Option<ArcStr>,
    },
    ApproveSelectedProposal,
    DenySelectedProposal,
    OpenSelectedProposalInEditor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    Approvals,
    ContextBrowser,
    EmbeddingBrowser,
    ModelBrowser,
}
