use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{OpenBlockProposal, SECOND_PROSE_CHANGE_TEXT, open_block_proposal};
use uuid::Uuid;

pub(crate) async fn open_selected_prose_change(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
    author_message: &str,
) -> Result<Option<String>, CompleteAgentRunError> {
    let targets = select_open_targets(client, claim, chapter_id).await?;
    let selected = if author_message.starts_with("Revise these passages") {
        targets.unreserved
    } else {
        targets.unreserved.into_iter().take(1).collect()
    };
    let Some(first) = selected.first() else {
        let classification = open_block_proposal(&OpenBlockProposal {
            scope_matches: true,
            target_block_present: targets.conflicting.is_some(),
            expected_base_revision_id: targets
                .conflicting
                .as_ref()
                .map(|target| target.revision_id.clone())
                .unwrap_or_default(),
            current_base_revision_id: targets
                .conflicting
                .as_ref()
                .map(|target| target.revision_id.clone()),
            conflicting_reservation: targets.conflicting.is_some(),
        });
        let _ = classification.validation_receipt_result();
        return Ok(None);
    };
    let classification = open_block_proposal(&OpenBlockProposal {
        scope_matches: true,
        target_block_present: true,
        expected_base_revision_id: first.revision_id.clone(),
        current_base_revision_id: Some(first.revision_id.clone()),
        conflicting_reservation: false,
    });
    let Some(validation_result) = classification.validation_receipt_result() else {
        return Ok(None);
    };
    persist_applied_proposal(
        client,
        claim,
        &PersistAppliedProposal {
            chapter_id,
            decision_id,
            candidate_text,
            author_message,
            targets: &selected,
            validation_result,
        },
    )
    .await
}

struct PersistAppliedProposal<'a> {
    chapter_id: &'a str,
    decision_id: &'a str,
    candidate_text: &'a str,
    author_message: &'a str,
    targets: &'a [OpenTarget],
    validation_result: &'a str,
}

pub(crate) struct OpenTarget {
    pub block_id: String,
    pub revision_id: String,
}

pub(crate) struct OpenTargets {
    pub unreserved: Vec<OpenTarget>,
    pub conflicting: Option<OpenTarget>,
}

pub(crate) async fn select_open_targets(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
) -> Result<OpenTargets, CompleteAgentRunError> {
    let rows = client
        .query(
            "SELECT member.manuscript_block_id::text, member.revision_id::text,
                    reservation.proposal_id IS NOT NULL
               FROM storyos.authoritative_heads AS head
               JOIN storyos.manuscript_revision_members AS member
                 ON (member.owner_user_id, member.project_id, member.manuscript_object_id,
                     member.revision_id) =
                    (head.owner_user_id, head.project_id, head.manuscript_object_id,
                     head.current_revision_id)
               LEFT JOIN storyos.proposal_operations AS reservation
                 ON (reservation.owner_user_id, reservation.project_id,
                     reservation.manuscript_block_id) =
                    (member.owner_user_id, member.project_id, member.manuscript_block_id)
                AND reservation.reservation_state = 'unresolved'
              WHERE head.owner_user_id = $1::text::uuid
                AND head.project_id = $2::text::uuid
                AND head.manuscript_object_id = $3::text::uuid
              ORDER BY member.block_order",
            &[
                &claim.project_scope.owner_user_id.as_ref(),
                &claim.project_scope.project_id.as_ref(),
                &chapter_id,
            ],
        )
        .await
        .map_err(database_error)?;
    let mut unreserved = Vec::new();
    let mut first_live = None;
    for row in rows {
        let target = OpenTarget {
            block_id: row.get(0),
            revision_id: row.get(1),
        };
        if first_live.is_none() {
            first_live = Some(OpenTarget {
                block_id: target.block_id.clone(),
                revision_id: target.revision_id.clone(),
            });
        }
        if !row.get::<_, bool>(2) {
            unreserved.push(target);
        }
    }
    Ok(OpenTargets {
        unreserved,
        conflicting: first_live,
    })
}

async fn persist_applied_proposal(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    persist: &PersistAppliedProposal<'_>,
) -> Result<Option<String>, CompleteAgentRunError> {
    let Some(first) = persist.targets.first() else {
        return Ok(None);
    };
    let proposal_id = Uuid::now_v7().to_string();
    let proposal_revision_id = Uuid::now_v7().to_string();
    let validation_receipt_id = Uuid::now_v7().to_string();
    let owner = claim.project_scope.owner_user_id.as_ref();
    let project = claim.project_scope.project_id.as_ref();
    let bundle_policy = if persist.author_message.contains("as a bundle") {
        "atomic"
    } else {
        "none"
    };
    let ordered = bundle_policy == "atomic" || persist.author_message.contains("in order");
    client
        .execute(
            "INSERT INTO storyos.proposals
               (owner_user_id, project_id, proposal_id, kind, chapter_id, manuscript_block_id,
                source_run_id, source_decision_id, bundle_policy)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'block_edit',
                     $4::text::uuid, $5::text::uuid, $6::text::uuid, $7::text::uuid, $8)",
            &[
                &owner,
                &project,
                &proposal_id,
                &persist.chapter_id,
                &first.block_id,
                &claim.run_id,
                &persist.decision_id,
                &bundle_policy,
            ],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_revisions
               (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                closure, candidate_text, base_authoritative_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid, 'ready',
                     'valid', 'open', $5, $6::text::uuid)",
            &[
                &owner,
                &project,
                &proposal_id,
                &proposal_revision_id,
                &persist.candidate_text,
                &first.revision_id,
            ],
        )
        .await
        .map_err(database_error)?;
    client
        .execute(
            "INSERT INTO storyos.proposal_heads
               (owner_user_id, project_id, proposal_id, current_revision_id)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)",
            &[&owner, &project, &proposal_id, &proposal_revision_id],
        )
        .await
        .map_err(database_error)?;
    let mut predecessor_ids: Vec<String> = Vec::new();
    for (index, target) in persist.targets.iter().enumerate() {
        let operation_id = Uuid::now_v7().to_string();
        let operation_candidate = if index == 0 {
            persist.candidate_text
        } else {
            SECOND_PROSE_CHANGE_TEXT
        };
        let predecessors: Vec<&str> = if ordered {
            predecessor_ids.iter().map(String::as_str).collect()
        } else {
            Vec::new()
        };
        client
            .execute(
                "INSERT INTO storyos.proposal_operations
                   (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                    resolution, reservation_state, candidate_text, predecessor_operation_ids)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, 'pending', 'unresolved', $6, $7::text[]::uuid[])",
                &[
                    &owner,
                    &project,
                    &proposal_id,
                    &operation_id,
                    &target.block_id,
                    &operation_candidate,
                    &predecessors,
                ],
            )
            .await
            .map_err(database_error)?;
        predecessor_ids.push(operation_id);
    }
    client
        .execute(
            "INSERT INTO storyos.validation_receipts
               (owner_user_id, project_id, validation_receipt_id, proposal_id,
                proposal_revision_id, result, base_authoritative_revision_id,
                manuscript_block_id, candidate_text, reservation_state)
             VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                     $5::text::uuid, $6, $7::text::uuid, $8::text::uuid, $9, 'unresolved')",
            &[
                &owner,
                &project,
                &validation_receipt_id,
                &proposal_id,
                &proposal_revision_id,
                &persist.validation_result,
                &first.revision_id,
                &first.block_id,
                &persist.candidate_text,
            ],
        )
        .await
        .map_err(database_error)?;
    Ok(Some(proposal_id))
}

fn database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
