//! The Acceptance Compensation of Author Undo (ADR 0044). It compensates through the
//! `AuthoritativeRevision` profile and reopens or derives the Proposal. Otherwise it requires a
//! Reversal Proposal or reports the source unavailable.

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::AuthorUndoFrontierKind;
use tokio_postgres::Client;
use uuid::Uuid;

use crate::author_edit::{
    ObservedProseFrontier, compensate_revision, decode_revision_compensation,
    load_revision_evidence,
};
use crate::undo_compensation::{CompensationAdapter, CompensationReplay};

mod proposal_placement;
use proposal_placement::{LinkMode, evidence_usable, place_proposal, reservation_blocked};

/// Compensates one applied Acceptance and links its Proposal again.
pub(crate) struct AcceptanceCompensation;

impl CompensationAdapter for AcceptanceCompensation {
    type Forward = ();
    type Evidence = LoadedAcceptance;

    async fn load(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        (): (),
        sequence: u64,
    ) -> Result<Option<LoadedAcceptance>, UndoLatestAuthorActionError> {
        let Some(row) = load_revision_evidence(client, command, sequence).await? else {
            return Ok(None);
        };
        let (Some(chapter_id), Some(resulting_revision_id), Some(prior_revision_id)) =
            (row.get(/*idx*/ 0), row.get(/*idx*/ 1), row.get(/*idx*/ 2))
        else {
            return Ok(None);
        };
        let Some(current_head_revision_id) = row.get(/*idx*/ 4) else {
            return Ok(None);
        };
        let prior_payload: Option<String> = row.get(/*idx*/ 3);
        let loaded = LoadedAcceptance {
            sequence,
            chapter_id,
            resulting_revision_id,
            prior_revision_id,
            prior_payload_present: prior_payload.is_some(),
            prior_payload: prior_payload.unwrap_or_default(),
            current_head_revision_id,
            prior_evidence_usable: false,
            facts: None,
        };
        enrich(client, command, loaded).await.map(Some)
    }

    fn frontier_kind(evidence: &LoadedAcceptance) -> AuthorUndoFrontierKind {
        AuthorUndoFrontierKind::ReversibleAcceptance {
            resulting_revision_id: evidence.resulting_revision_id.clone(),
            prior_evidence_usable: evidence.prior_evidence_usable,
        }
    }

    async fn compensate(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        evidence: &LoadedAcceptance,
        source_sequence: u64,
    ) -> Result<UndoLatestAuthorActionSettlement, UndoLatestAuthorActionError> {
        let prose = ObservedProseFrontier {
            sequence: evidence.sequence,
            chapter_id: evidence.chapter_id.clone(),
            resulting_revision_id: evidence.resulting_revision_id.clone(),
            prior_revision_id: evidence.prior_revision_id.clone(),
            prior_payload: evidence.prior_payload.clone(),
            current_head_revision_id: evidence.current_head_revision_id.clone(),
        };
        let mut settlement = compensate_revision(client, command, &prose, source_sequence).await?;
        let UndoLatestAuthorActionSettlementEffect::Compensated {
            authoritative_commit_id,
            revision_id,
            proposal_id,
            proposal_revision_id,
            ..
        } = &mut settlement.effect
        else {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        };
        (*proposal_id, *proposal_revision_id) = link_after_compensation(
            client,
            command,
            evidence,
            authoritative_commit_id,
            revision_id,
            settlement.project_activity_position,
        )
        .await?;
        Ok(settlement)
    }

    async fn decode(
        client: &Client,
        command: &UndoLatestAuthorActionCommand,
        replay: &CompensationReplay,
    ) -> Result<UndoLatestAuthorActionSettlementEffect, UndoLatestAuthorActionError> {
        decode_revision_compensation(client, command, replay).await
    }
}

pub(crate) struct LoadedAcceptance {
    pub sequence: u64,
    pub chapter_id: String,
    pub resulting_revision_id: String,
    pub prior_revision_id: String,
    pub prior_payload: String,
    pub current_head_revision_id: String,
    pub prior_evidence_usable: bool,
    prior_payload_present: bool,
    facts: Option<AcceptanceFacts>,
}

struct AcceptanceFacts {
    acceptance_receipt_id: String,
    proposal_id: String,
    proposal_revision_id: String,
    candidate_text: String,
    manuscript_block_id: String,
    lineage_drifted: bool,
    selected_operation_ids: Vec<String>,
    block_ids: Vec<String>,
}

pub(crate) struct AcceptanceRetry {
    pub outcome: String,
    pub proposal_id: Option<String>,
    pub proposal_revision_id: Option<String>,
    pub source_sequence: u64,
    pub project_activity_position: u64,
}

async fn enrich(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    mut loaded: LoadedAcceptance,
) -> Result<LoadedAcceptance, UndoLatestAuthorActionError> {
    let Some(row) = client
        .query_opt(
            "SELECT acceptance.acceptance_receipt_id::text,
                    acceptance.proposal_id::text,
                    acceptance.proposal_revision_id::text,
                    revision.candidate_text,
                    proposal.manuscript_block_id::text,
                    (head.current_revision_id IS DISTINCT FROM acceptance.proposal_revision_id
                      OR current_revision.closure IS DISTINCT FROM 'open'),
                    COALESCE((SELECT array_agg(selected.id::text)
                                FROM unnest(acceptance.selected_operation_ids) AS selected(id)),
                             ARRAY[]::text[]),
                    COALESCE((SELECT array_agg(operation.manuscript_block_id::text)
                                FROM storyos.proposal_operations AS operation
                               WHERE operation.owner_user_id = acceptance.owner_user_id
                                 AND operation.project_id = acceptance.project_id
                                 AND operation.proposal_id = acceptance.proposal_id
                                 AND operation.operation_id = ANY (acceptance.selected_operation_ids)),
                             ARRAY[]::text[]),
                    current_envelope.payload_digest,
                    convert_from(current_payload.canonical_bytes, 'UTF8'),
                    prior_envelope.payload_digest
               FROM storyos.author_action_entries AS action
               JOIN storyos.domain_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.receipt_id) =
                    (action.owner_user_id, action.project_id, action.receipt_id)
               JOIN storyos.acceptance_receipts AS acceptance
                 ON (acceptance.owner_user_id, acceptance.project_id, acceptance.acceptance_receipt_id) =
                    (receipt.owner_user_id, receipt.project_id, receipt.receipt_id)
               JOIN storyos.proposals AS proposal
                 ON (proposal.owner_user_id, proposal.project_id, proposal.proposal_id) =
                    (acceptance.owner_user_id, acceptance.project_id, acceptance.proposal_id)
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id) =
                    (acceptance.owner_user_id, acceptance.project_id, acceptance.proposal_id,
                     acceptance.proposal_revision_id)
               JOIN storyos.proposal_revisions AS current_revision
                 ON (current_revision.owner_user_id, current_revision.project_id,
                     current_revision.proposal_id, current_revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id, head.current_revision_id)
               LEFT JOIN storyos.authoritative_revision_envelopes AS current_envelope
                 ON (current_envelope.owner_user_id, current_envelope.project_id,
                     current_envelope.manuscript_object_id, current_envelope.revision_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id,
                     $5::text::uuid)
               LEFT JOIN storyos.authoritative_revision_envelopes AS prior_envelope
                 ON (prior_envelope.owner_user_id, prior_envelope.project_id,
                     prior_envelope.manuscript_object_id, prior_envelope.revision_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id,
                     $4::text::uuid)
               LEFT JOIN storyos.authoritative_revisions AS current_authoritative
                 ON (current_authoritative.owner_user_id, current_authoritative.project_id,
                     current_authoritative.manuscript_object_id, current_authoritative.revision_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id, $5::text::uuid)
               LEFT JOIN storyos.authoritative_payloads AS current_payload
                 ON (current_payload.owner_user_id, current_payload.project_id, current_payload.payload_id) =
                    (current_authoritative.owner_user_id, current_authoritative.project_id,
                     current_authoritative.payload_id)
              WHERE action.owner_user_id = $1::text::uuid
                AND action.project_id = $2::text::uuid
                AND action.author_action_sequence = $3::text::numeric
                AND action.disposition = 'forward'
                AND receipt.command_kind = 'acceptProposal'",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &loaded.sequence.to_string(),
                &loaded.prior_revision_id,
                &loaded.current_head_revision_id,
            ],
        )
        .await
        .map_err(database_error)?
    else {
        return Ok(loaded);
    };
    let facts = AcceptanceFacts {
        acceptance_receipt_id: row.get(0),
        proposal_id: row.get(1),
        proposal_revision_id: row.get(2),
        candidate_text: row.get(3),
        manuscript_block_id: row.get(4),
        lineage_drifted: row.get(5),
        selected_operation_ids: row.get(6),
        block_ids: row.get(7),
    };
    let current_envelope = row.get::<_, Option<String>>(8);
    let current_payload = row.get::<_, Option<String>>(9);
    let prior_envelope = row.get::<_, Option<String>>(10);
    loaded.prior_evidence_usable = evidence_usable(
        client,
        command,
        &loaded,
        &facts,
        current_envelope,
        prior_envelope,
        current_payload,
    )
    .await?;
    loaded.facts = Some(facts);
    Ok(loaded)
}

async fn link_after_compensation(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    loaded: &LoadedAcceptance,
    authoritative_commit_id: &str,
    base_revision_id: &str,
    activity_position: u64,
) -> Result<(Option<String>, Option<String>), UndoLatestAuthorActionError> {
    let Some(facts) = loaded.facts.as_ref() else {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    };
    let linked = if reservation_blocked(client, command, &facts.block_ids).await? {
        None
    } else {
        let mode = if facts.lineage_drifted {
            LinkMode::Derived
        } else {
            LinkMode::Reopen
        };
        Some(place_proposal(client, command, facts, base_revision_id, mode).await?)
    };
    insert_child(
        client,
        command,
        facts,
        ChildReceipt {
            source_sequence: loaded.sequence,
            outcome: "compensated",
            commit_id: Some(authoritative_commit_id),
            proposal_id: linked.as_ref().map(|(proposal_id, _)| proposal_id.as_str()),
            proposal_revision_id: linked.as_ref().map(|(_, revision_id)| revision_id.as_str()),
            reason: None,
            activity_position,
        },
    )
    .await?;
    Ok(match linked {
        Some((proposal_id, revision_id)) => (Some(proposal_id), Some(revision_id)),
        None => (None, None),
    })
}

pub(crate) async fn persist_reversal(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    loaded: &LoadedAcceptance,
    source_sequence: u64,
) -> Result<Option<UndoLatestAuthorActionSettlement>, UndoLatestAuthorActionError> {
    let Some(facts) = loaded.facts.as_ref() else {
        return Ok(None);
    };
    if reservation_blocked(client, command, &facts.block_ids).await? {
        return Ok(None);
    }
    let blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &loaded.prior_revision_id,
        &loaded.prior_payload,
    )
    .await
    .map_err(database_error)?;
    if facts.block_ids.iter().any(|block_id| {
        blocks
            .iter()
            .all(|block| &block.manuscript_block_id != block_id)
    }) {
        return Ok(None);
    }
    let (proposal_id, proposal_revision_id) = place_proposal(
        client,
        command,
        facts,
        &loaded.current_head_revision_id,
        LinkMode::Reversal { blocks: &blocks },
    )
    .await?;
    let counter_row = client
        .query_one(
            "UPDATE storyos.scope_counters
                SET author_action_sequence = author_action_sequence + 1
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
          RETURNING author_action_sequence::text, project_activity_position::text",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(database_error)?;
    let author_action_sequence = parse_u64(counter_row.get(0))?;
    let project_activity_position = parse_u64(counter_row.get(1))?;
    // The relation trigger accepts one forward action and zero commits only for authoritative_applied.
    let receipt_created_at = crate::undo_latest_author_action::insert_undo_receipt(
        client,
        command,
        "authoritative_applied",
        "{}",
        &loaded.current_head_revision_id,
        &loaded.current_head_revision_id,
        crate::undo_latest_author_action::UndoReceiptAuthority::None,
    )
    .await?;
    client
        .execute(
            "INSERT INTO storyos.author_action_entries
               (owner_user_id, project_id, author_action_sequence, disposition,
                receipt_id, receipt_result_kind)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, 'forward',
                     $4::text::uuid, 'authoritative_applied')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &author_action_sequence.to_string(),
                &command.ids.receipt_id,
            ],
        )
        .await
        .map_err(database_error)?;
    insert_child(
        client,
        command,
        facts,
        ChildReceipt {
            source_sequence,
            outcome: "reversal_required",
            commit_id: None,
            proposal_id: Some(proposal_id.as_str()),
            proposal_revision_id: Some(proposal_revision_id.as_str()),
            reason: None,
            activity_position: project_activity_position,
        },
    )
    .await?;
    let response_project =
        crate::undo_latest_author_action::settle_idempotency(client, command).await?;
    Ok(Some(UndoLatestAuthorActionSettlement {
        source_reopen_event: None,
        ids: command.ids.clone(),
        effect: UndoLatestAuthorActionSettlementEffect::ReversalRequired {
            source_sequence,
            author_action_sequence,
            proposal_id,
            proposal_revision_id,
        },
        receipt_created_at,
        project_activity_position,
        response_project,
    }))
}

pub(crate) async fn record_unavailable(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    loaded: &LoadedAcceptance,
) -> Result<(), UndoLatestAuthorActionError> {
    let Some(facts) = loaded.facts.as_ref() else {
        return Ok(());
    };
    insert_child(
        client,
        command,
        facts,
        ChildReceipt {
            source_sequence: loaded.sequence,
            outcome: "unavailable",
            commit_id: None,
            proposal_id: None,
            proposal_revision_id: None,
            reason: Some("source_unavailable"),
            activity_position: 0,
        },
    )
    .await
}

pub(crate) async fn read_retry(
    client: &tokio_postgres::Client,
    owner_user_id: &str,
    project_id: &str,
    receipt_id: &str,
) -> Result<Option<AcceptanceRetry>, UndoLatestAuthorActionError> {
    let Some(row) = client
        .query_opt(
            "SELECT outcome, proposal_id::text, proposal_revision_id::text,
                    source_author_action_sequence::text, reported_activity_position::text
               FROM storyos.undo_acceptance_receipts
              WHERE owner_user_id = $1::text::uuid
                AND project_id = $2::text::uuid
                AND author_undo_receipt_id = $3::text::uuid",
            &[&owner_user_id, &project_id, &receipt_id],
        )
        .await
        .map_err(database_error)?
    else {
        return Ok(None);
    };
    Ok(Some(AcceptanceRetry {
        outcome: row.get(0),
        proposal_id: row.get(1),
        proposal_revision_id: row.get(2),
        source_sequence: parse_u64(row.get(3))?,
        project_activity_position: parse_u64(row.get(4))?,
    }))
}

struct ChildReceipt<'a> {
    source_sequence: u64,
    outcome: &'a str,
    commit_id: Option<&'a str>,
    proposal_id: Option<&'a str>,
    proposal_revision_id: Option<&'a str>,
    reason: Option<&'a str>,
    activity_position: u64,
}

async fn insert_child(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    facts: &AcceptanceFacts,
    child: ChildReceipt<'_>,
) -> Result<(), UndoLatestAuthorActionError> {
    client
        .execute(
            "INSERT INTO storyos.undo_acceptance_receipts
               (owner_user_id, project_id, undo_acceptance_receipt_id, author_undo_receipt_id,
                acceptance_receipt_id, source_author_action_sequence, outcome,
                authoritative_commit_id, proposal_id, proposal_revision_id, reason,
                reported_activity_position)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6::text::numeric, $7, $8::text::uuid, $9::text::uuid,
                     $10::text::uuid, $11, $12::text::numeric)",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &Uuid::now_v7().to_string(),
                &command.ids.receipt_id,
                &facts.acceptance_receipt_id,
                &child.source_sequence.to_string(),
                &child.outcome,
                &child.commit_id,
                &child.proposal_id,
                &child.proposal_revision_id,
                &child.reason,
                &child.activity_position.to_string(),
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(())
}

fn database_error(error: tokio_postgres::Error) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn parse_u64(value: String) -> Result<u64, UndoLatestAuthorActionError> {
    value
        .parse()
        .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))
}
