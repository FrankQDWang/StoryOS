use std::future::Future;

use crate::{ProjectReadError, ProjectScope};

/// One inspectable current Block Proposal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BlockProposalRecord {
    pub project_scope: ProjectScope,
    pub proposal_id: String,
    pub kind: String,
    pub revision_id: String,
    pub generation: String,
    pub validation: String,
    pub condition_refs: Vec<String>,
    pub closure: String,
    pub operation_id: String,
    pub operation_resolution: String,
    pub chapter_id: String,
    pub manuscript_block_id: String,
    pub base_authoritative_revision_id: String,
    pub reservation_state: String,
    pub candidate_text: String,
    pub source_run_id: String,
    pub source_decision_id: String,
    pub validation_receipt_id: Option<String>,
    pub validation_receipt_result: Option<String>,
}

/// Reads one current Block Proposal under an already authenticated exact Project Scope.
pub trait ProposalReader: Sync {
    fn read_proposal(
        &self,
        scope: &ProjectScope,
        proposal_id: &str,
    ) -> impl Future<Output = Result<Option<BlockProposalRecord>, ProjectReadError>> + Send;
}

pub async fn open_proposal(
    reader: &impl ProposalReader,
    scope: &ProjectScope,
    proposal_id: &str,
) -> Result<Option<BlockProposalRecord>, ProjectReadError> {
    match reader.read_proposal(scope, proposal_id).await? {
        Some(record) if &record.project_scope == scope && record.proposal_id == proposal_id => {
            Ok(Some(record))
        }
        Some(_) | None => Ok(None),
    }
}
