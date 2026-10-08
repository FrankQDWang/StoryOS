//! The selection of an Author Undo compensation by the Forward command kind (ADR 0044).

use std::future::Future;

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;

/// The compensation adapter of one Forward command family (ADR 0044).
///
/// Author Undo selects the adapter from the Forward command kind. The adapter loads the Forward
/// evidence under the Project lock and writes the Compensation in the Undo transaction. On an
/// exact retry, it decodes the effect of the settled Compensation. It never runs transaction
/// control.
pub(crate) trait CompensationAdapter {
    /// The Forward command kinds of the family.
    type Forward: Copy + Send;
    /// The Forward evidence that one Compensation reverses.
    type Evidence: Send + Sync;

    /// Loads the evidence of one Forward Author Action. `None` makes the action a Barrier.
    fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        forward: Self::Forward,
        sequence: u64,
    ) -> impl Future<Output = Result<Option<Self::Evidence>, UndoLatestAuthorActionError>> + Send;

    fn frontier_kind(evidence: &Self::Evidence) -> AuthorUndoFrontierKind;

    fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &Self::Evidence,
        source_sequence: u64,
    ) -> impl Future<Output = Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError>> + Send;

    fn decode(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> impl Future<
        Output = Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError>,
    > + Send;
}

/// The records of a settled Compensation that an exact retry of Author Undo reads.
pub(crate) struct CompensationReplay {
    pub(crate) source_sequence: u64,
    pub(crate) author_action_sequence: u64,
    pub(crate) authoritative_commit_id: Option<String>,
    pub(crate) snapshot_id: Option<String>,
    /// The JSON text of the result payload of the Undo Receipt.
    pub(crate) result_payload: String,
    /// The Proposal Revision that a Proposal Compensation appended to the source head.
    pub(crate) restored_proposal_revision_id: Option<String>,
    pub(crate) author_undo_frontier_sequence: Option<u64>,
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
