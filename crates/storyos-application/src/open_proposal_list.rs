use std::future::Future;

use storyos_contracts::{
    OPEN_PROPOSAL_LIST_MAX_ENTRIES, OpenProposalListCompleteness, OpenProposalSource,
};

use crate::{ProjectReadError, ProjectScope};

/// One open Proposal: closure `open` and at least one pending Operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenProposalListItem {
    pub proposal_id: String,
    pub revision_id: String,
    pub kind: String,
    pub source: OpenProposalSource,
}

/// The open Proposals of one Chapter, in Proposal identity order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenProposalList {
    pub items: Vec<OpenProposalListItem>,
    pub completeness: OpenProposalListCompleteness,
}

/// Reads the open Proposals of one Chapter under an already authenticated exact Project Scope.
pub trait OpenProposalListReader: Sync {
    /// Gives at most `limit` items in Proposal identity order, with the getProposal visibility filter.
    /// Gives `None` when the Chapter is not visible in the Project Scope.
    fn read_open_proposals(
        &self,
        scope: &ProjectScope,
        chapter_id: &str,
        limit: usize,
    ) -> impl Future<Output = Result<Option<Vec<OpenProposalListItem>>, ProjectReadError>> + Send;
}

pub async fn list_open_proposals(
    reader: &impl OpenProposalListReader,
    scope: &ProjectScope,
    chapter_id: &str,
) -> Result<Option<OpenProposalList>, ProjectReadError> {
    let Some(mut items) = reader
        .read_open_proposals(scope, chapter_id, OPEN_PROPOSAL_LIST_MAX_ENTRIES + 1)
        .await?
    else {
        return Ok(None);
    };
    let completeness = if items.len() > OPEN_PROPOSAL_LIST_MAX_ENTRIES {
        items.truncate(OPEN_PROPOSAL_LIST_MAX_ENTRIES);
        OpenProposalListCompleteness::Truncated
    } else {
        OpenProposalListCompleteness::Complete
    };
    Ok(Some(OpenProposalList {
        items,
        completeness,
    }))
}
