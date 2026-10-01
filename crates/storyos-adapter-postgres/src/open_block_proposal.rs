use storyos_application::{ClaimedAgentRun, CompleteAgentRunError};
use storyos_core::{OpenBlockProposal, open_block_proposal};
use uuid::Uuid;

use crate::admitted_proposal_target::{load_admitted_targets, load_current_target};

pub(crate) struct ProseOpening {
    pub proposal_id: Option<String>,
    pub locations: Option<Vec<storyos_contracts::ProseChangeLocationInspect>>,
}

pub(crate) async fn open_selected_prose_change(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
    author_message: &str,
    produced: Option<&[storyos_core::ProseChangeCandidate]>,
) -> Result<ProseOpening, CompleteAgentRunError> {
    let targets = load_admitted_targets(client, claim, chapter_id).await?;
    let mut opening = ProseOpening {
        proposal_id: None,
        locations: produced.map(|_| Vec::new()),
    };
    for chapter in targets.chunk_by(|left, right| left.chapter_id == right.chapter_id) {
        let result = open_chapter(
            client,
            claim,
            &chapter[0].chapter_id,
            decision_id,
            candidate_text,
            author_message,
            produced,
            chapter.iter().collect(),
        )
        .await?;
        if opening.proposal_id.is_none() {
            opening.proposal_id = result.proposal_id;
        }
        if let (Some(locations), Some(changes)) = (&mut opening.locations, result.locations) {
            locations.extend(changes);
        }
    }
    Ok(opening)
}

#[allow(clippy::too_many_arguments)]
async fn open_chapter(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    chapter_id: &str,
    decision_id: &str,
    candidate_text: &str,
    author_message: &str,
    produced: Option<&[storyos_core::ProseChangeCandidate]>,
    targets: Vec<&crate::admitted_proposal_target::AdmittedTarget>,
) -> Result<ProseOpening, CompleteAgentRunError> {
    use storyos_contracts::{ProseChangeLocationInspect, ProseChangeLocationOutcome};
    let multiple = produced.is_some();
    let selected: Vec<_> = if multiple {
        targets
    } else {
        targets.into_iter().take(1).collect()
    };
    let candidates = produced.unwrap_or_default();
    let candidate_by_block: std::collections::BTreeMap<_, _> = candidates
        .iter()
        .map(|candidate| (candidate.manuscript_block_id.as_str(), candidate))
        .collect();
    let mut locations = Vec::new();
    let mut validation_result = "invalid";
    for target in &selected {
        let candidate = candidate_by_block.get(target.block_id.as_str());
        let current = load_current_target(client, claim, chapter_id, &target.block_id).await?;
        let result = open_block_proposal(&OpenBlockProposal {
            scope_matches: true,
            target_block_present: current.revision_id.is_some(),
            expected_base_revision_id: target.revision_id.clone(),
            current_base_revision_id: current.revision_id,
            conflicting_reservation: current.reserved,
        });
        if let Some(receipt_result) = result.validation_receipt_result() {
            validation_result = receipt_result;
        }
        let reason = match result {
            storyos_core::OpenBlockProposalResult::Applied => "eligible",
            storyos_core::OpenBlockProposalResult::Refused {
                reason: storyos_core::OpenBlockProposalRefusal::WrongScope,
            } => "wrong_scope",
            storyos_core::OpenBlockProposalResult::Refused {
                reason: storyos_core::OpenBlockProposalRefusal::UnavailableTarget,
            } => "unavailable_target",
            storyos_core::OpenBlockProposalResult::Conflicted {
                reason: storyos_core::OpenBlockProposalConflict::ChangedHead,
            } => "changed_head",
            storyos_core::OpenBlockProposalResult::Conflicted {
                reason: storyos_core::OpenBlockProposalConflict::ConflictingReservation,
            } => "conflicting_reservation",
        };
        locations.push(ProseChangeLocationInspect {
            chapter_id: chapter_id.to_owned(),
            manuscript_block_id: target.block_id.clone(),
            base_authoritative_revision_id: target.revision_id.clone(),
            candidate_text: if multiple {
                candidate
                    .expect("validated producer target")
                    .candidate_text
                    .clone()
            } else {
                candidate_text.to_owned()
            },
            explanation: candidate
                .map(|value| value.explanation.clone())
                .unwrap_or_default(),
            current: None,
            outcome: ProseChangeLocationOutcome::Refused {
                reason: reason.to_owned(),
            },
        });
    }
    let grouped = author_message.contains("as a bundle") || author_message.contains("in order");
    if grouped && locations.iter().any(|location| !matches!(&location.outcome, ProseChangeLocationOutcome::Refused { reason } if reason == "eligible")) {
        for location in &mut locations {
            if matches!(&location.outcome, ProseChangeLocationOutcome::Refused { reason } if reason == "eligible") {
                location.outcome = ProseChangeLocationOutcome::Refused { reason: "group_precondition_failed".to_owned() };
            }
        }
    }
    let eligible_ids: std::collections::BTreeSet<_> = locations.iter().filter_map(|location|
        matches!(&location.outcome, ProseChangeLocationOutcome::Refused { reason } if reason == "eligible")
            .then_some(location.manuscript_block_id.as_str())).collect();
    let eligible: Vec<_> = selected
        .into_iter()
        .filter(|target| eligible_ids.contains(target.block_id.as_str()))
        .collect();
    let persisted = if eligible.is_empty() {
        None
    } else {
        Some(
            persist_applied_proposal(
                client,
                claim,
                &PersistAppliedProposal {
                    chapter_id,
                    decision_id,
                    candidate_text: if multiple {
                        candidate_by_block
                            .get(eligible[0].block_id.as_str())
                            .expect("validated producer target")
                            .candidate_text
                            .as_str()
                    } else {
                        candidate_text
                    },
                    author_message,
                    targets: &eligible,
                    validation_result,
                    candidates: produced,
                },
            )
            .await?,
        )
    };
    if let Some(persisted) = &persisted {
        let operations: std::collections::BTreeMap<_, _> = persisted
            .operations
            .iter()
            .map(|(block, operation)| (block.as_str(), operation))
            .collect();
        for location in &mut locations {
            if let Some(operation_id) = operations.get(location.manuscript_block_id.as_str()) {
                location.outcome = ProseChangeLocationOutcome::Opened {
                    proposal_id: persisted.proposal_id.clone(),
                    operation_id: (*operation_id).clone(),
                    revision_id: persisted.revision_id.clone(),
                    validation_receipt_id: persisted.validation_receipt_id.clone(),
                };
            }
        }
    }
    Ok(ProseOpening {
        proposal_id: persisted.map(|value| value.proposal_id),
        locations: multiple.then_some(locations),
    })
}

struct PersistedProposal {
    proposal_id: String,
    revision_id: String,
    validation_receipt_id: String,
    operations: Vec<(String, String)>,
}

struct PersistAppliedProposal<'a> {
    chapter_id: &'a str,
    decision_id: &'a str,
    candidate_text: &'a str,
    author_message: &'a str,
    targets: &'a [&'a crate::admitted_proposal_target::AdmittedTarget],
    validation_result: &'a str,
    candidates: Option<&'a [storyos_core::ProseChangeCandidate]>,
}

async fn persist_applied_proposal(
    client: &tokio_postgres::Client,
    claim: &ClaimedAgentRun,
    persist: &PersistAppliedProposal<'_>,
) -> Result<PersistedProposal, CompleteAgentRunError> {
    let first = &persist.targets[0];
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
    let mut operations = Vec::new();
    let candidates: std::collections::BTreeMap<_, _> = persist
        .candidates
        .unwrap_or_default()
        .iter()
        .map(|candidate| {
            (
                candidate.manuscript_block_id.as_str(),
                candidate.candidate_text.as_str(),
            )
        })
        .collect();
    for target in persist.targets {
        let operation_id = Uuid::now_v7().to_string();
        let operation_candidate = match persist.candidates {
            Some(_) => *candidates
                .get(target.block_id.as_str())
                .expect("validated producer target"),
            None => persist.candidate_text,
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
        operations.push((target.block_id.clone(), operation_id.clone()));
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
    Ok(PersistedProposal {
        proposal_id,
        revision_id: proposal_revision_id,
        validation_receipt_id,
        operations,
    })
}

fn database_error(error: tokio_postgres::Error) -> CompleteAgentRunError {
    CompleteAgentRunError::Unavailable(Box::new(error))
}
