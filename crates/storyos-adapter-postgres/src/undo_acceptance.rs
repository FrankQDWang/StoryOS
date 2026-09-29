//! Undo of an accepted Proposal: compensate, reopen, derive, or require reversal.

use storyos_application::{
    UndoLatestAuthorActionCommand, UndoLatestAuthorActionError, UndoLatestAuthorActionSettlement,
    UndoLatestAuthorActionSettlementEffect,
};
use storyos_core::ManuscriptBlock;
use uuid::Uuid;

use crate::author_edit::sha256_hex;

pub(super) struct LoadedAcceptance {
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

enum LinkMode<'a> {
    Reopen,
    Derived,
    Reversal { blocks: &'a [ManuscriptBlock] },
}

pub(super) struct AcceptanceRetry {
    pub outcome: String,
    pub proposal_id: Option<String>,
    pub proposal_revision_id: Option<String>,
    pub source_sequence: u64,
    pub project_activity_position: u64,
}

impl LoadedAcceptance {
    pub(super) fn pending(
        sequence: u64,
        chapter_id: String,
        resulting_revision_id: String,
        prior_revision_id: String,
        prior_payload: Option<String>,
        current_head_revision_id: String,
    ) -> Self {
        let prior_payload_present = prior_payload.is_some();
        Self {
            sequence,
            chapter_id,
            resulting_revision_id,
            prior_revision_id,
            prior_payload: prior_payload.unwrap_or_default(),
            current_head_revision_id,
            prior_evidence_usable: false,
            prior_payload_present,
            facts: None,
        }
    }
}

pub(super) async fn enrich(
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

pub(super) async fn link_after_compensation(
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

pub(super) async fn persist_reversal(
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

pub(super) async fn record_unavailable(
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

pub(super) async fn read_retry(
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

async fn evidence_usable(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    loaded: &LoadedAcceptance,
    facts: &AcceptanceFacts,
    current_envelope: Option<String>,
    prior_envelope: Option<String>,
    current_payload: Option<String>,
) -> Result<bool, UndoLatestAuthorActionError> {
    // A stored empty Chapter payload is usable prior evidence.
    if !loaded.prior_payload_present || facts.block_ids.is_empty() {
        return Ok(false);
    }
    let Some(current_payload) = current_payload else {
        return Ok(false);
    };
    if !digest_agrees(&loaded.prior_payload, prior_envelope.as_deref()) {
        return Ok(false);
    }
    if loaded.current_head_revision_id == loaded.resulting_revision_id {
        let digest = sha256_hex(current_payload.as_bytes());
        return Ok(current_envelope.as_deref() == Some(digest.as_str()));
    }
    if !digest_agrees(&current_payload, current_envelope.as_deref()) {
        return Ok(false);
    }
    let prior_blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &loaded.prior_revision_id,
        &loaded.prior_payload,
    )
    .await
    .map_err(database_error)?;
    let current_blocks = crate::manuscript_block::load_revision_blocks(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &loaded.chapter_id,
        &loaded.current_head_revision_id,
        &current_payload,
    )
    .await
    .map_err(database_error)?;
    Ok(facts.block_ids.iter().all(|block_id| {
        prior_blocks
            .iter()
            .any(|block| &block.manuscript_block_id == block_id)
            && current_blocks
                .iter()
                .any(|block| &block.manuscript_block_id == block_id)
    }))
}

async fn reservation_blocked(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    block_ids: &[String],
) -> Result<bool, UndoLatestAuthorActionError> {
    if block_ids.is_empty() {
        return Ok(true);
    }
    let blocked = client
        .query_one(
            "SELECT EXISTS (
               SELECT 1 FROM storyos.proposal_operations
                WHERE owner_user_id = $1::text::uuid
                  AND project_id = $2::text::uuid
                  AND manuscript_block_id = ANY($3::text[]::uuid[])
                  AND reservation_state = 'unresolved')",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &block_ids,
            ],
        )
        .await
        .map_err(database_error)?
        .get(0);
    Ok(blocked)
}

async fn place_proposal(
    client: &tokio_postgres::Client,
    command: &UndoLatestAuthorActionCommand,
    facts: &AcceptanceFacts,
    base_revision_id: &str,
    mode: LinkMode<'_>,
) -> Result<(String, String), UndoLatestAuthorActionError> {
    let owner = command.project_scope.owner_user_id.as_ref();
    let project = command.project_scope.project_id.as_ref();
    let revision_id = Uuid::now_v7().to_string();
    let (proposal_id, parent, kind_override, candidate_override) = match &mode {
        LinkMode::Reopen => (
            facts.proposal_id.clone(),
            Some(facts.proposal_revision_id.as_str()),
            None,
            None,
        ),
        LinkMode::Derived => (Uuid::now_v7().to_string(), None, None, None),
        LinkMode::Reversal { blocks } => {
            let text = blocks
                .iter()
                .find(|block| block.manuscript_block_id == facts.manuscript_block_id)
                .map(|block| block.text.clone())
                .ok_or(UndoLatestAuthorActionError::BindingConflict)?;
            (
                Uuid::now_v7().to_string(),
                None,
                Some("reversal"),
                Some(text),
            )
        }
    };
    if !matches!(mode, LinkMode::Reopen) {
        let inserted = client
            .execute(
                "INSERT INTO storyos.proposals
                   (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                    source_run_id, source_decision_id, bundle_policy, source_draft_id,
                    source_draft_revision_id, source_draft_payload_digest)
                 SELECT owner_user_id, project_id, $4::text::uuid, COALESCE($5, kind),
                        chapter_id, manuscript_block_id, source_run_id, source_decision_id,
                        bundle_policy, source_draft_id, source_draft_revision_id,
                        source_draft_payload_digest
                   FROM storyos.proposals
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid",
                &[
                    &owner,
                    &project,
                    &facts.proposal_id,
                    &proposal_id,
                    &kind_override,
                ],
            )
            .await
            .map_err(database_error)?;
        if inserted != 1 {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        }
    }
    let candidate = candidate_override
        .clone()
        .unwrap_or_else(|| facts.candidate_text.clone());
    let inserted = client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id, parent_revision_id,
                candidate_blocks)
             SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                    'valid', 'open', COALESCE($5, source.candidate_text), $6::text::uuid,
                    $7::text::uuid, CASE WHEN $5 IS NULL THEN source.candidate_blocks ELSE NULL END
               FROM storyos.proposal_revisions AS source
              WHERE source.owner_user_id = $1::text::uuid
                AND source.project_id = $2::text::uuid
                AND source.proposal_id = $8::text::uuid
                AND source.revision_id = $9::text::uuid",
            &[
                &owner,
                &project,
                &proposal_id,
                &revision_id,
                &candidate_override,
                &base_revision_id,
                &parent,
                &facts.proposal_id,
                &facts.proposal_revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    if inserted != 1 {
        return Err(UndoLatestAuthorActionError::BindingConflict);
    }
    if matches!(mode, LinkMode::Reopen) {
        let updated = client
            .execute(
                "UPDATE storyos.proposal_heads
                    SET current_revision_id = $4::text::uuid
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND current_revision_id = $5::text::uuid",
                &[
                    &owner,
                    &project,
                    &proposal_id,
                    &revision_id,
                    &facts.proposal_revision_id,
                ],
            )
            .await
            .map_err(database_error)?;
        if updated != 1 {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        }
        let updated = client
            .execute(
                "UPDATE storyos.proposal_operations
                    SET resolution = 'pending', reservation_state = 'unresolved'
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($4::text[]::uuid[])
                    AND resolution = 'applied'",
                &[
                    &owner,
                    &project,
                    &proposal_id,
                    &facts.selected_operation_ids,
                ],
            )
            .await
            .map_err(database_error)?;
        if updated != facts.selected_operation_ids.len() as u64 {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        }
    } else {
        client
            .execute(
                "INSERT INTO storyos.proposal_heads
                   (owner_user_id, project_id, proposal_id, current_revision_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
                &[&owner, &project, &proposal_id, &revision_id],
            )
            .await
            .map_err(database_error)?;
        let inserted = client
            .execute(
                "INSERT INTO storyos.proposal_operations
                   (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                    resolution, reservation_state, candidate_text, predecessor_operation_ids,
                    candidate_blocks)
                 SELECT owner_user_id, project_id, $4::text::uuid, operation_id, manuscript_block_id,
                        'pending', 'unresolved', candidate_text, predecessor_operation_ids,
                        candidate_blocks
                   FROM storyos.proposal_operations
                  WHERE owner_user_id = $1::text::uuid
                    AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid
                    AND operation_id = ANY($5::text[]::uuid[])",
                &[
                    &owner,
                    &project,
                    &facts.proposal_id,
                    &proposal_id,
                    &facts.selected_operation_ids,
                ],
            )
            .await
            .map_err(database_error)?;
        if inserted != facts.selected_operation_ids.len() as u64 {
            return Err(UndoLatestAuthorActionError::BindingConflict);
        }
        if let LinkMode::Reversal { blocks } = &mode {
            for block in blocks.iter().filter(|block| {
                facts
                    .block_ids
                    .iter()
                    .any(|block_id| block_id == &block.manuscript_block_id)
            }) {
                client
                    .execute(
                        "UPDATE storyos.proposal_operations
                            SET candidate_text = $5, candidate_blocks = NULL
                          WHERE owner_user_id = $1::text::uuid
                            AND project_id = $2::text::uuid
                            AND proposal_id = $3::text::uuid
                            AND manuscript_block_id = $4::text::uuid",
                        &[
                            &owner,
                            &project,
                            &proposal_id,
                            &block.manuscript_block_id,
                            &block.text,
                        ],
                    )
                    .await
                    .map_err(database_error)?;
            }
        } else {
            client
                .execute(
                    "INSERT INTO storyos.proposal_anchors
                       (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                        manuscript_block_id, base_authoritative_revision_id, manuscript_schema_version,
                        coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest)
                     SELECT owner_user_id, project_id, $4::text::uuid, operation_id, anchor_order,
                            manuscript_block_id, $5::text::uuid, manuscript_schema_version,
                            coordinate_profile, range_from, range_to, boundary_profile, base_slice_digest
                       FROM storyos.proposal_anchors
                      WHERE owner_user_id = $1::text::uuid
                        AND project_id = $2::text::uuid
                        AND proposal_id = $3::text::uuid
                        AND operation_id = ANY($6::text[]::uuid[])",
                    &[
                        &owner,
                        &project,
                        &facts.proposal_id,
                        &proposal_id,
                        &base_revision_id,
                        &facts.selected_operation_ids,
                    ],
                )
                .await
                .map_err(database_error)?;
        }
    }
    client
        .execute(
            "INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, 'valid', $6::text::uuid, $7::text::uuid, $8, 'unresolved')",
            &[
                &owner,
                &project,
                &Uuid::now_v7().to_string(),
                &proposal_id,
                &revision_id,
                &base_revision_id,
                &facts.manuscript_block_id,
                &candidate,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok((proposal_id, revision_id))
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

fn digest_agrees(payload: &str, envelope: Option<&str>) -> bool {
    match envelope {
        Some(digest) => sha256_hex(payload.as_bytes()) == digest,
        None => true,
    }
}

fn database_error(error: tokio_postgres::Error) -> UndoLatestAuthorActionError {
    UndoLatestAuthorActionError::Unavailable(Box::new(error))
}

fn parse_u64(value: String) -> Result<u64, UndoLatestAuthorActionError> {
    value
        .parse()
        .map_err(|error| UndoLatestAuthorActionError::Unavailable(Box::new(error)))
}
