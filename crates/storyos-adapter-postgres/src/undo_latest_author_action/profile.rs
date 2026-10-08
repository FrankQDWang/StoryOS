//! The `UndoCompensation` settlement profile: the records of one Author Undo, which the
//! compensation adapter of the Forward command kind writes (ADR 0044).

use storyos_application::{
    ManuscriptBlock, ProjectCommandEnvelope, ProjectCommandError, ProjectScope, UndoApplied,
};
use storyos_core::UndoLatestAuthorActionApplied;
use tokio_postgres::Client;

use super::replay::{UndoReplay, frontier_as_of_receipt};
use crate::accept_proposal::{AcceptanceCompensation, LoadedAcceptance, reversal_blocks};
use crate::author_edit::{ObservedProseFrontier, ProseCompensation};
use crate::author_edit_proposal::{ObservedProposalFrontier, ProposalEditCompensation};
use crate::close_editor_flow_draft::{DraftCompensation, ObservedDraftClose};
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    AppliedVariant, AuthoritativeRevision, CommandSpec, LockedProject, ReceiptHeads,
    RevisionSequences, SelectsRecords, SettlementProfile, unavailable,
};
use crate::proposal_decision_compensation::ObservedProposalDecision;
use crate::reopen_rejected_operations::{ObservedOperationReopening, ReopenRejectedCompensation};
use crate::reopen_withdrawn_proposal::ReopenWithdrawnCompensation;
use crate::replan_proposal::ReplanCompensation;
use crate::set_current_chapter::{CurrentChapterCompensation, ObservedCurrentChapterFrontier};
use crate::structural_authority_settlement::{
    CurrentChapterSequences, ObservedStructureFrontier, StructureCompensation,
    StructureTransitionSequences,
};
use crate::undo_compensation::{
    CompensationAction, CompensationAdapter, ForwardCommand, UndoDisposition, UndoRequest,
    allocate_compensation_action,
};
use crate::undo_draft_retry::{load_source_reopen, persist_source_reopen};
use crate::undo_frontier::ObservedFrontier;
use crate::withdraw_proposal::{AuthorWithdrawalCompensation, ObservedAuthorWithdrawal};

/// The applied plan of one Author Undo: its compensation family and the Forward evidence.
pub(crate) struct UndoPlan {
    family: UndoFamily,
    source_sequence: u64,
    /// The Refused Edit Draft that the source superseded, which the Compensation reopens.
    pub(super) source_reopen: Option<ObservedDraftClose>,
    /// The current Project Activity position.
    project_activity_position: u64,
}

/// The compensation family of one applied Author Undo and its Forward evidence.
enum UndoFamily {
    Prose(ObservedProseFrontier),
    Acceptance(LoadedAcceptance),
    Structure(ObservedStructureFrontier),
    CurrentChapter(ObservedCurrentChapterFrontier),
    ProposalEdit(ObservedProposalFrontier),
    Replan(ObservedProposalDecision),
    ReopenRejectedOperations(ObservedOperationReopening),
    ReopenWithdrawnProposal(ObservedProposalDecision),
    AuthorWithdrawal(ObservedAuthorWithdrawal),
    Draft(ObservedDraftClose),
    /// The Forward Author Action of an Undo Acceptance that requires a Reversal Proposal.
    Reversal {
        loaded: LoadedAcceptance,
        blocks: Vec<ManuscriptBlock>,
    },
}

/// The record set that one Author Undo allocates.
pub(crate) enum UndoSelector {
    Revision,
    Structure,
    CurrentChapter,
    Action,
}

/// The scope sequences of one Author Undo.
pub(crate) enum UndoSequences {
    Revision(RevisionSequences),
    Structure(StructureTransitionSequences),
    CurrentChapter(CurrentChapterSequences),
    Action(CompensationAction),
}

/// The applied writes of one Author Undo.
pub(crate) struct UndoWrite<E> {
    pub(crate) effect: E,
    pub(crate) request: UndoRequest,
    pub(crate) plan: UndoPlan,
}

impl UndoPlan {
    /// The plan of an applied Core outcome. `None` makes a required Reversal unavailable.
    pub(super) async fn new(
        client: &Client,
        request: &UndoRequest,
        applied: UndoLatestAuthorActionApplied,
        observed: Option<ObservedFrontier>,
    ) -> Result<Option<Self>, ProjectCommandError> {
        let (source_sequence, family) = match (applied, observed) {
            (
                UndoLatestAuthorActionApplied::ReversalRequired { source_sequence },
                Some(ObservedFrontier::Acceptance(loaded)),
            ) => match reversal_blocks(client, request, &loaded).await? {
                Some(blocks) => (source_sequence, UndoFamily::Reversal { loaded, blocks }),
                None => return Ok(None),
            },
            (UndoLatestAuthorActionApplied::ReversalRequired { .. }, _)
            | (UndoLatestAuthorActionApplied::Compensated { .. }, None) => {
                return Err(ProjectCommandError::BindingConflict);
            }
            (UndoLatestAuthorActionApplied::Compensated { source_sequence }, Some(observed)) => {
                (source_sequence, UndoFamily::from_frontier(observed)?)
            }
        };
        let source_reopen = match &family {
            UndoFamily::Draft(_) => None,
            UndoFamily::Prose(_)
            | UndoFamily::Acceptance(_)
            | UndoFamily::Structure(_)
            | UndoFamily::CurrentChapter(_)
            | UndoFamily::ProposalEdit(_)
            | UndoFamily::Replan(_)
            | UndoFamily::ReopenRejectedOperations(_)
            | UndoFamily::ReopenWithdrawnProposal(_)
            | UndoFamily::AuthorWithdrawal(_)
            | UndoFamily::Reversal { .. } => load_source_reopen(client, request).await?,
        };
        Ok(Some(Self {
            family,
            source_sequence,
            source_reopen,
            project_activity_position: current_activity_position(client, &request.project_scope)
                .await?,
        }))
    }

    pub(super) fn variant(&self) -> AppliedVariant {
        match &self.family {
            UndoFamily::Reversal { .. } => {
                AppliedVariant::Forward(ForwardCommand::UndoReversalRequired)
            }
            UndoFamily::Draft(_) => AppliedVariant::Compensation(DraftCompensation::RESULT_KIND),
            UndoFamily::Prose(_)
            | UndoFamily::Acceptance(_)
            | UndoFamily::Structure(_)
            | UndoFamily::CurrentChapter(_)
            | UndoFamily::ProposalEdit(_)
            | UndoFamily::Replan(_)
            | UndoFamily::ReopenRejectedOperations(_)
            | UndoFamily::ReopenWithdrawnProposal(_)
            | UndoFamily::AuthorWithdrawal(_) => {
                AppliedVariant::Compensation(ProseCompensation::RESULT_KIND)
            }
        }
    }

    /// The head arrays of the Undo Receipt. The profile gives the resulting head of a Revision.
    pub(super) fn heads(&self, expected: &str) -> ReceiptHeads {
        let prior = match &self.family {
            UndoFamily::Prose(frontier) => frontier.current_head_revision_id.clone(),
            UndoFamily::Acceptance(loaded) | UndoFamily::Reversal { loaded, .. } => {
                loaded.current_head_revision_id.clone()
            }
            UndoFamily::Structure(_)
            | UndoFamily::CurrentChapter(_)
            | UndoFamily::ProposalEdit(_)
            | UndoFamily::Replan(_)
            | UndoFamily::ReopenRejectedOperations(_)
            | UndoFamily::ReopenWithdrawnProposal(_)
            | UndoFamily::AuthorWithdrawal(_)
            | UndoFamily::Draft(_) => expected.to_owned(),
        };
        ReceiptHeads {
            expected: vec![expected.to_owned()],
            prior: vec![prior.clone()],
            resulting: vec![prior],
        }
    }

    pub(super) fn receipt_payload(&self) -> serde_json::Map<String, serde_json::Value> {
        let position = self.project_activity_position;
        match &self.family {
            UndoFamily::ProposalEdit(evidence) => {
                ProposalEditCompensation::receipt_payload(evidence, position)
            }
            UndoFamily::Replan(evidence) => ReplanCompensation::receipt_payload(evidence, position),
            UndoFamily::ReopenRejectedOperations(evidence) => {
                ReopenRejectedCompensation::receipt_payload(evidence, position)
            }
            UndoFamily::ReopenWithdrawnProposal(evidence) => {
                ReopenWithdrawnCompensation::receipt_payload(evidence, position)
            }
            UndoFamily::AuthorWithdrawal(evidence) => {
                AuthorWithdrawalCompensation::receipt_payload(evidence, position)
            }
            UndoFamily::Draft(evidence) => DraftCompensation::receipt_payload(evidence, position),
            UndoFamily::Prose(_)
            | UndoFamily::Acceptance(_)
            | UndoFamily::Structure(_)
            | UndoFamily::CurrentChapter(_)
            | UndoFamily::Reversal { .. } => serde_json::Map::new(),
        }
    }

    pub(super) fn receipt_draft(&self) -> Option<(String, String)> {
        match &self.family {
            UndoFamily::Draft(evidence) => DraftCompensation::receipt_draft(evidence),
            UndoFamily::Prose(_)
            | UndoFamily::Acceptance(_)
            | UndoFamily::Structure(_)
            | UndoFamily::CurrentChapter(_)
            | UndoFamily::ProposalEdit(_)
            | UndoFamily::Replan(_)
            | UndoFamily::ReopenRejectedOperations(_)
            | UndoFamily::ReopenWithdrawnProposal(_)
            | UndoFamily::AuthorWithdrawal(_)
            | UndoFamily::Reversal { .. } => None,
        }
    }
}

impl UndoFamily {
    fn from_frontier(observed: ObservedFrontier) -> Result<Self, ProjectCommandError> {
        Ok(match observed {
            ObservedFrontier::Prose(frontier) => Self::Prose(frontier),
            ObservedFrontier::Acceptance(loaded) => Self::Acceptance(loaded),
            ObservedFrontier::Structure(frontier) => Self::Structure(frontier),
            ObservedFrontier::CurrentChapter(frontier) => Self::CurrentChapter(frontier),
            ObservedFrontier::Proposal(frontier) => Self::ProposalEdit(frontier),
            ObservedFrontier::Replan(decision) => Self::Replan(decision),
            ObservedFrontier::ReopenRejectedOperations(reopening) => {
                Self::ReopenRejectedOperations(reopening)
            }
            ObservedFrontier::ReopenWithdrawnProposal(decision) => {
                Self::ReopenWithdrawnProposal(decision)
            }
            ObservedFrontier::AuthorWithdrawal(withdrawal) => Self::AuthorWithdrawal(withdrawal),
            ObservedFrontier::DraftClose(close) => Self::Draft(close),
            ObservedFrontier::Barrier { .. } => return Err(ProjectCommandError::BindingConflict),
        })
    }
}

impl SelectsRecords<UndoCompensation> for UndoPlan {
    fn selector(&self) -> UndoSelector {
        match &self.family {
            UndoFamily::Prose(_) | UndoFamily::Acceptance(_) => UndoSelector::Revision,
            UndoFamily::Structure(_) => UndoSelector::Structure,
            UndoFamily::CurrentChapter(_) => UndoSelector::CurrentChapter,
            UndoFamily::ProposalEdit(_)
            | UndoFamily::Replan(_)
            | UndoFamily::ReopenRejectedOperations(_)
            | UndoFamily::ReopenWithdrawnProposal(_)
            | UndoFamily::AuthorWithdrawal(_)
            | UndoFamily::Draft(_)
            | UndoFamily::Reversal { .. } => UndoSelector::Action,
        }
    }
}

/// The records of one Author Undo. Each family writes them through its compensation adapter.
pub(crate) struct UndoCompensation;

impl SettlementProfile for UndoCompensation {
    type Selector = UndoSelector;
    type Sequences = UndoSequences;
    type Write<E: Send> = UndoWrite<E>;
    type Applied<E: Send> = UndoApplied<E>;

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
        selector: &UndoSelector,
    ) -> Result<UndoSequences, ProjectCommandError> {
        Ok(match selector {
            UndoSelector::Revision => {
                UndoSequences::Revision(AuthoritativeRevision::allocate(client, scope, &()).await?)
            }
            UndoSelector::Structure => {
                UndoSequences::Structure(StructureCompensation::allocate(client, scope).await?)
            }
            UndoSelector::CurrentChapter => UndoSequences::CurrentChapter(
                CurrentChapterCompensation::allocate(client, scope).await?,
            ),
            UndoSelector::Action => {
                UndoSequences::Action(allocate_compensation_action(client, scope).await?)
            }
        })
    }

    fn commit_ids(sequences: &UndoSequences) -> Vec<String> {
        match sequences {
            UndoSequences::Revision(sequences) => AuthoritativeRevision::commit_ids(sequences),
            UndoSequences::Structure(sequences) => StructureCompensation::commit_ids(sequences),
            UndoSequences::CurrentChapter(_) | UndoSequences::Action(_) => Vec::new(),
        }
    }

    fn revision_ids(sequences: &UndoSequences) -> Vec<String> {
        match sequences {
            UndoSequences::Revision(sequences) => AuthoritativeRevision::revision_ids(sequences),
            UndoSequences::Structure(_)
            | UndoSequences::CurrentChapter(_)
            | UndoSequences::Action(_) => Vec::new(),
        }
    }

    async fn persist<E: Send>(
        client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _spec: &CommandSpec,
        _variant: AppliedVariant,
        sequences: UndoSequences,
        write: UndoWrite<E>,
    ) -> Result<UndoApplied<E>, ProjectCommandError> {
        let UndoWrite {
            effect,
            request,
            plan,
        } = write;
        let source = plan.source_sequence;
        let author_action_sequence = match &sequences {
            UndoSequences::Revision(sequences) => sequences.author_action_sequence,
            UndoSequences::Structure(sequences) => sequences.author_action_sequence,
            UndoSequences::CurrentChapter(sequences) => sequences.author_action_sequence,
            UndoSequences::Action(sequences) => sequences.author_action_sequence,
        };
        let request = &request;
        let mismatch = || unavailable("the Undo sequences do not match the compensation family");
        let records = match (&plan.family, sequences) {
            (UndoFamily::Prose(evidence), UndoSequences::Revision(sequences)) => {
                ProseCompensation::compensate(client, request, evidence, sequences, source).await?
            }
            (UndoFamily::Acceptance(evidence), UndoSequences::Revision(sequences)) => {
                AcceptanceCompensation::compensate(client, request, evidence, sequences, source)
                    .await?
            }
            (UndoFamily::Structure(evidence), UndoSequences::Structure(sequences)) => {
                StructureCompensation::compensate(client, request, evidence, sequences, source)
                    .await?
            }
            (UndoFamily::CurrentChapter(evidence), UndoSequences::CurrentChapter(sequences)) => {
                CurrentChapterCompensation::compensate(client, request, evidence, sequences, source)
                    .await?
            }
            (UndoFamily::ProposalEdit(evidence), UndoSequences::Action(sequences)) => {
                ProposalEditCompensation::compensate(client, request, evidence, sequences, source)
                    .await?
            }
            (UndoFamily::Replan(evidence), UndoSequences::Action(sequences)) => {
                ReplanCompensation::compensate(client, request, evidence, sequences, source).await?
            }
            (UndoFamily::ReopenRejectedOperations(evidence), UndoSequences::Action(sequences)) => {
                ReopenRejectedCompensation::compensate(client, request, evidence, sequences, source)
                    .await?
            }
            (UndoFamily::ReopenWithdrawnProposal(evidence), UndoSequences::Action(sequences)) => {
                ReopenWithdrawnCompensation::compensate(
                    client, request, evidence, sequences, source,
                )
                .await?
            }
            (UndoFamily::AuthorWithdrawal(evidence), UndoSequences::Action(sequences)) => {
                AuthorWithdrawalCompensation::compensate(
                    client, request, evidence, sequences, source,
                )
                .await?
            }
            (UndoFamily::Draft(evidence), UndoSequences::Action(sequences)) => {
                DraftCompensation::compensate(client, request, evidence, sequences, source).await?
            }
            (UndoFamily::Reversal { loaded, blocks }, UndoSequences::Action(sequences)) => {
                crate::accept_proposal::persist_reversal(
                    client, request, loaded, blocks, sequences, source,
                )
                .await?
            }
            (_, _) => return Err(mismatch()),
        };
        let source_reopen_event = match &plan.source_reopen {
            Some(source) => {
                Some(persist_source_reopen(client, request, source, author_action_sequence).await?)
            }
            None => None,
        };
        let author_undo_frontier_sequence = match &plan.family {
            UndoFamily::Draft(evidence) => evidence
                .next_frontier
                .as_deref()
                .map(str::parse)
                .transpose()
                .map_err(unavailable)?,
            UndoFamily::Reversal { .. } => None,
            UndoFamily::Prose(_)
            | UndoFamily::Acceptance(_)
            | UndoFamily::Structure(_)
            | UndoFamily::CurrentChapter(_)
            | UndoFamily::ProposalEdit(_)
            | UndoFamily::Replan(_)
            | UndoFamily::ReopenRejectedOperations(_)
            | UndoFamily::ReopenWithdrawnProposal(_)
            | UndoFamily::AuthorWithdrawal(_) => {
                crate::editor_session::current_author_undo_frontier_sequence(
                    client,
                    request.project_scope.owner_user_id.as_ref(),
                    request.project_scope.project_id.as_ref(),
                )
                .await
                .map_err(super::undo_from_session)?
            }
        };
        Ok(UndoApplied {
            effect,
            source_sequence: source,
            author_action_sequence,
            author_undo_frontier_sequence,
            source_reopen_event,
            records,
        })
    }

    fn replay<E: Send>(
        decode: impl FnOnce() -> Result<E, ReplayFault>,
        replay: &CommandReplay,
    ) -> Result<UndoApplied<E>, ReplayFault> {
        let damaged = || ReplayFault::Unavailable("the Author Undo evidence is damaged".into());
        let stored = UndoReplay::parse(replay)?;
        let author_action_sequence = replay
            .author_action_sequence
            .as_deref()
            .ok_or_else(damaged)?
            .parse()
            .map_err(|_| damaged())?;
        let compensation = stored.compensation(replay)?;
        let disposition = stored.disposition()?;
        let expected_action = match disposition {
            Some(_) => "compensation",
            None => "forward",
        };
        if replay.author_action_disposition.as_deref() != Some(expected_action) {
            return Err(damaged());
        }
        let (source_sequence, records, author_undo_frontier_sequence) = match disposition {
            Some(UndoDisposition::Prose) => (
                stored.compensated_source_sequence()?,
                ProseCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::Acceptance) => (
                stored.compensated_source_sequence()?,
                AcceptanceCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::Structure(_)) => (
                stored.compensated_source_sequence()?,
                StructureCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::CurrentChapter) => (
                stored.compensated_source_sequence()?,
                CurrentChapterCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::ProposalEdit) => (
                stored.compensated_source_sequence()?,
                ProposalEditCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::Replan) => (
                stored.compensated_source_sequence()?,
                ReplanCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::ReopenRejectedOperations) => (
                stored.compensated_source_sequence()?,
                ReopenRejectedCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::ReopenWithdrawnProposal) => (
                stored.compensated_source_sequence()?,
                ReopenWithdrawnCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::AuthorWithdrawal) => (
                stored.compensated_source_sequence()?,
                AuthorWithdrawalCompensation::decode(&compensation)?,
                frontier_as_of_receipt(replay)?,
            ),
            Some(UndoDisposition::Draft) => (
                stored.compensated_source_sequence()?,
                DraftCompensation::decode(&compensation)?,
                stored.draft_frontier(replay)?,
            ),
            Some(UndoDisposition::Barrier) => return Err(damaged()),
            None => stored.reversal()?,
        };
        Ok(UndoApplied {
            effect: decode()?,
            source_sequence,
            author_action_sequence,
            author_undo_frontier_sequence,
            source_reopen_event: stored.source_reopen_event()?,
            records,
        })
    }
}

/// The current Project Activity position of `scope`.
async fn current_activity_position(
    client: &Client,
    scope: &ProjectScope,
) -> Result<u64, ProjectCommandError> {
    client
        .query_one(
            "SELECT project_activity_position::text FROM storyos.scope_counters
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
            &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
        )
        .await
        .map_err(unavailable)?
        .get::<_, String>(/*idx*/ 0)
        .parse()
        .map_err(unavailable)
}
