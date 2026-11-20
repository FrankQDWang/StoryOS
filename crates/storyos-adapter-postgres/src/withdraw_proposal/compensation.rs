//! The author withdrawal Compensation of Author Undo (ADR 0076).

use storyos_application::{ProjectCommandError, ProjectScope, UndoRecords};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::author_edit_proposal::{decode_proposal_compensation, proposal_receipt_payload};
use crate::command_replay::ReplayFault;
use crate::command_sequence::{ActionDisposition, insert_author_action, unavailable};
use crate::undo_compensation::{
    CompensationAction, CompensationAdapter, CompensationReplay, UndoRequest,
    allocate_compensation_action,
};
use crate::undo_frontier::proposal_head_restores;
use crate::undo_latest_author_action::undo_database_error;

/// Reopens a Proposal that the author withdrew. A later producer Withdrawal keeps it withdrawn
/// (ADR 0069).
pub(crate) struct AuthorWithdrawalCompensation;

impl CompensationAdapter for AuthorWithdrawalCompensation {
    type Forward = ();
    type Evidence = ObservedAuthorWithdrawal;
    type Sequences = CompensationAction;

    async fn load(
        client: &Client,
        command: &UndoRequest,
        _forward: (),
        sequence: u64,
    ) -> Result<Option<ObservedAuthorWithdrawal>, ProjectCommandError> {
        load_author_withdrawal(client, command, sequence).await
    }

    fn frontier_kind(_evidence: &ObservedAuthorWithdrawal) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleStructureTransition
    }

    async fn allocate(
        client: &Client,
        scope: &ProjectScope,
    ) -> Result<CompensationAction, ProjectCommandError> {
        allocate_compensation_action(client, scope).await
    }

    fn receipt_payload(
        evidence: &ObservedAuthorWithdrawal,
        project_activity_position: u64,
    ) -> serde_json::Map<String, serde_json::Value> {
        proposal_receipt_payload(
            &evidence.compensation_revision_id,
            &evidence.current_revision_id,
            project_activity_position,
        )
    }

    async fn compensate(
        client: &Client,
        command: &UndoRequest,
        evidence: &ObservedAuthorWithdrawal,
        sequences: CompensationAction,
        source_sequence: u64,
    ) -> Result<UndoRecords, ProjectCommandError> {
        persist_withdrawal_compensation(client, command, evidence, sequences, source_sequence).await
    }

    /// The Activity position is the one that the Receipt payload records (issue 1031).
    fn decode(replay: &CompensationReplay) -> Result<UndoRecords, ReplayFault> {
        let proposal_revision_id = match replay.receipt_payload.get("proposal_revision_id") {
            Some(serde_json::Value::String(text)) => text.clone(),
            _ => {
                return Err(ReplayFault::Unavailable(
                    "a withdrawal Compensation has no Proposal Revision".into(),
                ));
            }
        };
        decode_proposal_compensation(replay, Some(proposal_revision_id))
    }
}

pub(crate) struct ObservedAuthorWithdrawal {
    pub sequence: u64,
    /// The Proposal Revision that the Compensation appends.
    pub compensation_revision_id: String,
    pub chapter_id: String,
    pub current_revision_id: String,
    pub generation: String,
    pub candidate_text: String,
    pub base_authoritative_revision_id: String,
    /// `withdrawn` when a producer Withdrawal still keeps the head withdrawn, else `open`.
    pub closure: &'static str,
}

async fn load_author_withdrawal(
    client: &Client,
    command: &UndoRequest,
    sequence: u64,
) -> Result<Option<ObservedAuthorWithdrawal>, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT proposal.chapter_id::text,
                    head.current_revision_id::text,
                    revision.generation,
                    revision.candidate_text,
                    revision.base_authoritative_revision_id::text,
                    withdrawal.proposal_id::text,
                    withdrawal.proposal_revision_id::text
               FROM storyos.author_action_entries AS action
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id)
               JOIN storyos.proposal_withdrawals AS withdrawal
                 ON (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.withdrawal_receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                AND withdrawal.author_action_sequence = action.author_action_sequence
                AND withdrawal.withdrawal_reason = 'author_withdrew'
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
                AND revision.closure = 'withdrawn'
               JOIN storyos.proposals AS proposal
                 ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                    (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id)
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric
                AND action.disposition = 'forward'
                AND receipt.command_kind = 'withdrawProposal'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &sequence.to_string(),
            ],
        )
        .await
        .map_err(undo_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id: Option<String> = row.get(/*idx*/ 0);
    let Some(chapter_id) = chapter_id else {
        return Ok(None);
    };
    let current_revision_id: String = row.get(/*idx*/ 1);
    let proposal_id: String = row.get(/*idx*/ 5);
    let withdrawn_revision_id: String = row.get(/*idx*/ 6);
    // Only later Proposal Compensations can explain a head that moved after the withdrawal
    // (ADR 0068).
    if current_revision_id != withdrawn_revision_id
        && !proposal_head_restores(
            client,
            command,
            &proposal_id,
            &current_revision_id,
            &withdrawn_revision_id,
        )
        .await?
    {
        return Ok(None);
    }
    let producer_withdrew = head_withdrawn_by_producer(
        client,
        &command.project_scope,
        &proposal_id,
        &current_revision_id,
    )
    .await?;
    Ok(Some(ObservedAuthorWithdrawal {
        sequence,
        compensation_revision_id: Uuid::now_v7().to_string(),
        chapter_id,
        current_revision_id,
        generation: row.get(/*idx*/ 2),
        candidate_text: row.get(/*idx*/ 3),
        base_authoritative_revision_id: row.get(/*idx*/ 4),
        closure: if producer_withdrew {
            "withdrawn"
        } else {
            "open"
        },
    }))
}

async fn persist_withdrawal_compensation(
    client: &Client,
    command: &UndoRequest,
    frontier: &ObservedAuthorWithdrawal,
    sequences: CompensationAction,
    source_sequence: u64,
) -> Result<UndoRecords, ProjectCommandError> {
    let proposal_revision_id = frontier.compensation_revision_id.clone();
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id)
             SELECT withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id,
                    $4::text::uuid, $5, 'pending', $9, $6, $7::text::uuid,
                    $8::text::uuid
               FROM storyos.proposal_withdrawals AS withdrawal
              WHERE withdrawal.owner_user_id = $1::text::uuid
                AND withdrawal.project_id = $2::text::uuid
                AND withdrawal.author_action_sequence = $3::text::numeric
                AND withdrawal.withdrawal_reason = 'author_withdrew'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &source_sequence.to_string(),
                &proposal_revision_id,
                &frontier.generation,
                &frontier.candidate_text,
                &frontier.base_authoritative_revision_id,
                &frontier.current_revision_id,
                &frontier.closure,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if inserted != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    let head_updates = client
        .execute(
            "UPDATE storyos.proposal_heads AS head
                SET current_revision_id = $4::text::uuid
               FROM storyos.proposal_withdrawals AS withdrawal
              WHERE head.owner_user_id = withdrawal.owner_user_id
                AND head.project_id = withdrawal.project_id
                AND head.proposal_id = withdrawal.proposal_id
                AND head.current_revision_id = $5::text::uuid
                AND withdrawal.owner_user_id = $1::text::uuid
                AND withdrawal.project_id = $2::text::uuid
                AND withdrawal.author_action_sequence = $3::text::numeric",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &source_sequence.to_string(),
                &proposal_revision_id,
                &frontier.current_revision_id,
            ],
        )
        .await
        .map_err(undo_database_error)?;
    if head_updates != 1 {
        return Err(ProjectCommandError::BindingConflict);
    }
    insert_author_action(
        client,
        &command.project_scope,
        sequences.author_action_sequence,
        &ActionDisposition::Compensation { source_sequence },
        None,
        &command.ids.receipt_id,
    )
    .await?;
    Ok(UndoRecords::Proposal {
        proposal_revision_id: Some(proposal_revision_id),
        project_activity_position: sequences.project_activity_position,
    })
}

/// Whether a producer Withdrawal keeps the withdrawn head `head_revision_id` withdrawn (ADR 0069).
/// The walk goes from the head through recorded Proposal Compensations. From a Compensation
/// Revision with a `withdrawn` parent, it goes to the parent. An author Withdrawal that Author
/// Undo compensated keeps nothing withdrawn.
async fn head_withdrawn_by_producer(
    client: &Client,
    scope: &ProjectScope,
    proposal_id: &str,
    head_revision_id: &str,
) -> Result<bool, ProjectCommandError> {
    let rows = client
        .query(
            "WITH RECURSIVE kept(revision_id) AS (
                 SELECT $4::text::uuid
               UNION
                 SELECT step.revision_id
                   FROM kept
             CROSS JOIN LATERAL (
                 SELECT next.revision_id
                   FROM storyos.proposal_revisions AS appended
                   JOIN storyos.proposal_revisions AS parent
                     ON (parent.owner_user_id, parent.project_id, parent.proposal_id,
                         parent.revision_id) =
                        (appended.owner_user_id, appended.project_id, appended.proposal_id,
                         appended.parent_revision_id)
                   JOIN storyos.domain_receipts AS receipt
                     ON (receipt.owner_user_id, receipt.project_id) =
                        (appended.owner_user_id, appended.project_id)
                    AND receipt.result_payload->>'proposal_revision_id' =
                        appended.revision_id::text
                    AND receipt.result_payload->>'source_proposal_revision_id' =
                        appended.parent_revision_id::text
                   JOIN storyos.author_action_entries AS compensation
                     ON (compensation.owner_user_id, compensation.project_id,
                         compensation.receipt_id) =
                        (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
                    AND compensation.disposition = 'compensation'
                   JOIN storyos.author_action_entries AS source
                     ON (source.owner_user_id, source.project_id, source.author_action_sequence) =
                        (compensation.owner_user_id, compensation.project_id,
                         compensation.compensated_source_sequence)
                    AND source.disposition = 'forward'
                   JOIN storyos.domain_receipts AS source_receipt
                     ON (source_receipt.owner_user_id, source_receipt.project_id,
                         source_receipt.receipt_id) =
                        (source.owner_user_id, source.project_id, source.receipt_id)
                    AND source_receipt.command_kind IN ('applyAuthorEdit', 'withdrawProposal')
             CROSS JOIN LATERAL (
                 VALUES (CASE WHEN parent.closure = 'withdrawn'
                              THEN appended.parent_revision_id END)
             ) AS next(revision_id)
                  WHERE (appended.owner_user_id, appended.project_id, appended.proposal_id,
                         appended.revision_id) =
                        ($1::text::uuid, $2::text::uuid, $3::text::uuid, kept.revision_id)
                    AND next.revision_id IS NOT NULL
             ) AS step
             )
             SELECT withdrawal.withdrawal_reason = 'current_producer_withdrew'
               FROM kept
               JOIN storyos.proposal_withdrawals AS withdrawal
                 ON (withdrawal.owner_user_id, withdrawal.project_id, withdrawal.proposal_id,
                     withdrawal.proposal_revision_id) =
                    ($1::text::uuid, $2::text::uuid, $3::text::uuid, kept.revision_id)
              WHERE NOT EXISTS (
                SELECT 1
                  FROM storyos.author_action_entries AS undo
                 WHERE (undo.owner_user_id, undo.project_id, undo.compensated_source_sequence) =
                       (withdrawal.owner_user_id, withdrawal.project_id,
                        withdrawal.author_action_sequence)
                   AND undo.disposition = 'compensation'
              )",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
                &head_revision_id,
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(rows.iter().any(|row| row.get::<_, bool>(/*idx*/ 0)))
}
