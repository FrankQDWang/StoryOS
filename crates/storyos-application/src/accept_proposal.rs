use storyos_core::{AcceptProposalConflict, AcceptProposalInvalid, AcceptProposalRefusal};

use crate::{EditorSessionId, Project, ProjectCommandSettlement, RevisionApplied};

/// The typed input of one Acceptance of selected pending Proposal Operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptProposalInput {
    pub editor_session_id: EditorSessionId,
    pub proposal_id: String,
    pub proposal_revision_id: String,
    pub validation_receipt_id: String,
    pub selected_operation_ids: Vec<String>,
    pub expected_authoritative_revision_id: String,
}

/// The settled Acceptance. The zero-authority effect holds the condition references of the
/// Receipt.
pub type AcceptProposalSettlement = ProjectCommandSettlement<
    RevisionApplied<()>,
    AcceptProposalInvalid,
    AcceptProposalConflict,
    AcceptProposalRefusal,
    Project,
    Vec<String>,
>;
