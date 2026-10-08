//! The selection of an Author Undo compensation by the Forward command kind (ADR 0044).

use std::future::Future;

use storyos_application::{
    AuthorCommandAdmissionIds, EditorSessionId, ProjectCommandChallengeBinding,
    ProjectCommandEnvelope, ProjectCommandError, ProjectScope, UndoLatestAuthorActionInput,
    UndoRecords,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

use crate::command_replay::ReplayFault;

/// The request facts of one Author Undo that the compensation adapters read.
pub(crate) struct UndoRequest {
    pub(crate) project_scope: ProjectScope,
    pub(crate) challenge_binding: ProjectCommandChallengeBinding,
    pub(crate) canonical_command_bytes: Vec<u8>,
    pub(crate) ids: AuthorCommandAdmissionIds,
    pub(crate) editor_session_id: EditorSessionId,
    pub(crate) expected_author_undo_frontier_sequence: u64,
    pub(crate) expected_authoritative_revision_id: String,
}

impl UndoRequest {
    pub(crate) fn new(
        envelope: &ProjectCommandEnvelope,
        input: &UndoLatestAuthorActionInput,
    ) -> Self {
        Self {
            project_scope: envelope.project_scope.clone(),
            challenge_binding: envelope.challenge_binding.clone(),
            canonical_command_bytes: envelope.canonical_command_bytes.clone(),
            ids: envelope.ids.clone(),
            editor_session_id: input.editor_session_id.clone(),
            expected_author_undo_frontier_sequence: input.expected_author_undo_frontier_sequence,
            expected_authoritative_revision_id: input.expected_authoritative_revision_id.clone(),
        }
    }
}

/// The compensation adapter of one Forward command family (ADR 0044).
///
/// Author Undo selects the adapter from the Forward command kind. The adapter loads the Forward
/// evidence under the Project lock. After the Undo Receipt, it writes the Compensation records
/// with the sequences that the Author Undo settlement profile allocated. On an exact retry, it
/// decodes the records from the stored evidence. It never runs transaction control.
pub(crate) trait CompensationAdapter {
    /// The Forward command kinds of the family.
    type Forward: Copy + Send;
    /// The Forward evidence that one Compensation reverses.
    type Evidence: Send + Sync;
    /// The scope sequences that one Compensation of the family uses.
    type Sequences: Send + Sync;
    /// The Receipt result kind of a Compensation of the family.
    const RESULT_KIND: &'static str = "authoritative_applied";

    /// Loads the evidence of one Forward Author Action. `None` makes the action a Barrier.
    fn load(
        client: &Client,
        request: &UndoRequest,
        forward: Self::Forward,
        sequence: u64,
    ) -> impl Future<Output = Result<Option<Self::Evidence>, ProjectCommandError>> + Send;

    fn frontier_kind(evidence: &Self::Evidence) -> AuthorUndoFrontierKind;

    /// Allocates the scope sequences of one Compensation before the Undo Receipt.
    fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> impl Future<Output = Result<Self::Sequences, ProjectCommandError>> + Send;

    /// The Authoritative Commit identities that the Undo Receipt binds.
    fn commit_ids(_sequences: &Self::Sequences) -> Vec<String> {
        Vec::new()
    }

    /// The fields of the Undo Receipt payload. `project_activity_position` is the current
    /// Project Activity position.
    fn receipt_payload(
        _evidence: &Self::Evidence,
        _project_activity_position: u64,
    ) -> serde_json::Map<String, serde_json::Value> {
        serde_json::Map::new()
    }

    /// The Draft and its lifecycle event that the Undo Receipt records, when the Compensation
    /// reopens a Draft.
    fn receipt_draft(_evidence: &Self::Evidence) -> Option<(String, String)> {
        None
    }

    /// Writes the Compensation records after the Undo Receipt and gives them.
    fn compensate(
        client: &Client,
        request: &UndoRequest,
        evidence: &Self::Evidence,
        sequences: Self::Sequences,
        source_sequence: u64,
    ) -> impl Future<Output = Result<UndoRecords, ProjectCommandError>> + Send;

    /// Decodes the records of a settled Compensation of the family.
    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault>;
}

/// The scope sequences of a Compensation that writes no Commit, Activity record, or Snapshot.
pub(crate) struct CompensationAction {
    pub(crate) author_action_sequence: u64,
    /// The current Project Activity position, which the Compensation reports.
    pub(crate) project_activity_position: u64,
}

/// Allocates the Author Action Sequence position of one Compensation without a Commit.
pub(crate) async fn allocate_compensation_action(
    client: &Client,
    scope: &ProjectScope,
) -> Result<CompensationAction, ProjectCommandError> {
    let row = client
        .query_one(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = author_action_sequence + 1
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
          RETURNING author_action_sequence::text, project_activity_position::text",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(crate::command_sequence::unavailable)?;
    let sequence = |index: usize| {
        row.get::<_, String>(index)
            .parse()
            .map_err(crate::command_sequence::unavailable)
    };
    Ok(CompensationAction {
        author_action_sequence: sequence(/*index*/ 0)?,
        project_activity_position: sequence(/*index*/ 1)?,
    })
}

/// The stored evidence of a settled Compensation that an exact retry of Author Undo reads.
pub(crate) struct CompensationReplay<'a> {
    /// The canonical Snapshot that the Compensation wrote, with its Activity position.
    pub(crate) snapshot: Option<(String, u64)>,
    pub(crate) authoritative_commit_id: Option<String>,
    /// The Proposal Revision that a Proposal Compensation appended to the source head.
    pub(crate) restored_proposal_revision_id: Option<String>,
    pub(crate) receipt_payload: &'a serde_json::Map<String, serde_json::Value>,
    /// The `payload` JSON of the Draft reopen event of a Draft Compensation.
    pub(crate) draft_event: Option<&'a serde_json::Value>,
    pub(crate) revision: Option<&'a crate::command_replay::ReplayedRevision>,
    /// The `undo_acceptance_receipts` row of an Undo Acceptance.
    pub(crate) acceptance: Option<AcceptanceChildReplay>,
}

/// The stored `undo_acceptance_receipts` row of one Undo Acceptance.
pub(crate) struct AcceptanceChildReplay {
    pub(crate) outcome: String,
    pub(crate) source_sequence: u64,
    pub(crate) proposal_id: Option<String>,
    pub(crate) proposal_revision_id: Option<String>,
}

/// The command kind and applied variant of one Forward Author Action.
#[derive(Clone, Copy)]
pub(crate) enum ForwardCommand {
    AuthorEdit(AuthorEditVariant),
    AcceptProposal,
    Structure(StructureCommand),
    SetCurrentChapter,
    WithdrawProposal,
    CloseEditorFlowDraft,
    ExpandRefusedEditDraftToProposal,
    ReplanProposal,
    ReopenRejectedOperations,
    ReopenWithdrawnProposal,
    RejectProposalOperations,
    CompleteReadyPartialProposal,
    ContinueProposalGeneration,
    /// The Forward Author Action of an Undo Acceptance that requires a Reversal Proposal.
    UndoReversalRequired,
}

#[derive(Clone, Copy)]
pub(crate) enum AuthorEditVariant {
    AuthoritativeApplied,
    ProposalRevised,
}

#[derive(Clone, Copy)]
pub(crate) enum StructureCommand {
    CreateVolume,
    UpdateVolume,
    DeleteVolume,
    CreateChapter,
    UpdateChapter,
    DeleteChapter,
}

/// The Author Undo Disposition of a Forward command kind: one compensation family or a Barrier.
pub(crate) enum UndoDisposition {
    Prose,
    Acceptance,
    ProposalEdit,
    Replan,
    ReopenRejectedOperations,
    ReopenWithdrawnProposal,
    Structure(StructureCommand),
    CurrentChapter,
    AuthorWithdrawal,
    Draft,
    Barrier,
}

impl ForwardCommand {
    /// Reads the Forward command from the command kind and result kind of its Domain Receipt.
    pub(crate) fn from_receipt(command_kind: &str, result_kind: &str) -> Option<Self> {
        Some(match command_kind {
            "applyAuthorEdit" => Self::AuthorEdit(match result_kind {
                "authoritative_applied" => AuthorEditVariant::AuthoritativeApplied,
                "proposal_revised" => AuthorEditVariant::ProposalRevised,
                _ => return None,
            }),
            "acceptProposal" => Self::AcceptProposal,
            "createVolume" => Self::Structure(StructureCommand::CreateVolume),
            "updateVolume" => Self::Structure(StructureCommand::UpdateVolume),
            "deleteVolume" => Self::Structure(StructureCommand::DeleteVolume),
            "createChapter" => Self::Structure(StructureCommand::CreateChapter),
            "updateChapter" => Self::Structure(StructureCommand::UpdateChapter),
            "deleteChapter" => Self::Structure(StructureCommand::DeleteChapter),
            "setCurrentChapter" => Self::SetCurrentChapter,
            "withdrawProposal" => Self::WithdrawProposal,
            "closeEditorFlowDraft" => Self::CloseEditorFlowDraft,
            "expandRefusedEditDraftToProposal" => Self::ExpandRefusedEditDraftToProposal,
            "replanProposal" => Self::ReplanProposal,
            "reopenRejectedOperations" => Self::ReopenRejectedOperations,
            "reopenWithdrawnProposal" => Self::ReopenWithdrawnProposal,
            "rejectProposalOperations" => Self::RejectProposalOperations,
            "completeReadyPartialProposal" => Self::CompleteReadyPartialProposal,
            "continueProposalGeneration" => Self::ContinueProposalGeneration,
            "undoLatestAuthorAction" => Self::UndoReversalRequired,
            _ => return None,
        })
    }

    /// The command kind of the Forward Author Action.
    pub(crate) const fn command_kind(self) -> &'static str {
        match self {
            Self::AuthorEdit(_) => "applyAuthorEdit",
            Self::AcceptProposal => "acceptProposal",
            Self::Structure(StructureCommand::CreateVolume) => "createVolume",
            Self::Structure(StructureCommand::UpdateVolume) => "updateVolume",
            Self::Structure(StructureCommand::DeleteVolume) => "deleteVolume",
            Self::Structure(StructureCommand::CreateChapter) => "createChapter",
            Self::Structure(StructureCommand::UpdateChapter) => "updateChapter",
            Self::Structure(StructureCommand::DeleteChapter) => "deleteChapter",
            Self::SetCurrentChapter => "setCurrentChapter",
            Self::WithdrawProposal => "withdrawProposal",
            Self::CloseEditorFlowDraft => "closeEditorFlowDraft",
            Self::ExpandRefusedEditDraftToProposal => "expandRefusedEditDraftToProposal",
            Self::ReplanProposal => "replanProposal",
            Self::ReopenRejectedOperations => "reopenRejectedOperations",
            Self::ReopenWithdrawnProposal => "reopenWithdrawnProposal",
            Self::RejectProposalOperations => "rejectProposalOperations",
            Self::CompleteReadyPartialProposal => "completeReadyPartialProposal",
            Self::ContinueProposalGeneration => "continueProposalGeneration",
            Self::UndoReversalRequired => "undoLatestAuthorAction",
        }
    }

    /// The Domain Receipt result kind of the Forward Author Action.
    pub(crate) const fn result_kind(self) -> &'static str {
        match self {
            Self::AuthorEdit(AuthorEditVariant::AuthoritativeApplied)
            | Self::AcceptProposal
            | Self::Structure(_)
            | Self::SetCurrentChapter
            | Self::UndoReversalRequired => "authoritative_applied",
            Self::AuthorEdit(AuthorEditVariant::ProposalRevised)
            | Self::ReplanProposal
            | Self::ReopenRejectedOperations
            | Self::ReopenWithdrawnProposal => "proposal_revised",
            Self::WithdrawProposal => "proposal_closure_changed",
            Self::CloseEditorFlowDraft => "draft_closure_changed",
            Self::ExpandRefusedEditDraftToProposal => "proposal_created_from_draft",
            Self::RejectProposalOperations => "proposal_operations_resolved",
            Self::CompleteReadyPartialProposal => "proposal_generation_completed",
            Self::ContinueProposalGeneration => "proposal_generation_started",
        }
    }

    pub(crate) fn disposition(self) -> UndoDisposition {
        match self {
            Self::AuthorEdit(AuthorEditVariant::AuthoritativeApplied) => UndoDisposition::Prose,
            Self::AuthorEdit(AuthorEditVariant::ProposalRevised) => UndoDisposition::ProposalEdit,
            Self::ReplanProposal => UndoDisposition::Replan,
            Self::ReopenRejectedOperations => UndoDisposition::ReopenRejectedOperations,
            Self::ReopenWithdrawnProposal => UndoDisposition::ReopenWithdrawnProposal,
            Self::AcceptProposal => UndoDisposition::Acceptance,
            Self::Structure(command) => UndoDisposition::Structure(command),
            Self::SetCurrentChapter => UndoDisposition::CurrentChapter,
            Self::WithdrawProposal => UndoDisposition::AuthorWithdrawal,
            Self::CloseEditorFlowDraft | Self::ExpandRefusedEditDraftToProposal => {
                UndoDisposition::Draft
            }
            Self::RejectProposalOperations
            | Self::CompleteReadyPartialProposal
            | Self::ContinueProposalGeneration
            | Self::UndoReversalRequired => UndoDisposition::Barrier,
        }
    }
}
