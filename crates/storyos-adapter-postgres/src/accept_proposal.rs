#[path = "accept_proposal_read.rs"]
mod read;
#[path = "accept_proposal_write.rs"]
mod write;

use read::read_accept_settlement;
use write::{insert_accept_admission, persist_applied, persist_zero};

use super::*;
use storyos_application::{
    AcceptProposalCommand, AcceptProposalError, AcceptProposalSettlement,
    AcceptProposalSettlementEffect, AcceptProposalStore, AcceptanceRefusalBoundary,
    AcceptanceRefusalReason, ProjectCommandChallengeError, ProjectCommandChallengeUse,
};
use storyos_core::{
    AcceptProposal as CoreAccept, AcceptProposalInvalid, AcceptProposalRefusal,
    AcceptProposalResult, ProposalBundlePolicy, ProposalOperationSelection,
    ProposalSelectionIntent, accept_proposal as classify, classify_proposal_selection,
};

impl AcceptProposalStore for PostgresProjectReader {
    async fn accept_proposal(
        &self,
        command: &AcceptProposalCommand,
    ) -> Result<AcceptProposalSettlement, AcceptProposalError> {
        let mut transaction = self
            .begin_serializable_project_command_transaction(&command.project_scope)
            .await
            .map_err(accept_challenge_error)?;
        let challenge_use = match transaction
            .consume(&command.challenge_binding, &command.nonce_digest)
            .await
        {
            Ok(challenge_use) => challenge_use,
            Err(ProjectCommandChallengeError::InvalidOrExpired) => {
                transaction
                    .rollback()
                    .await
                    .map_err(accept_challenge_error)?;
                let reason = self
                    .retain_acceptance_refusal(
                        command,
                        AcceptanceRefusalReason::InvalidChallenge,
                        AcceptanceRefusalBoundary::Challenge,
                    )
                    .await?;
                return Err(AcceptProposalError::PreAdmissionRefused { reason });
            }
            Err(error) => return Err(accept_challenge_error(error)),
        };
        match challenge_use {
            ProjectCommandChallengeUse::ExactRetrySettled { result_reference } => {
                transaction
                    .rollback()
                    .await
                    .map_err(accept_challenge_error)?;
                read_accept_settlement(self, command, &result_reference).await
            }
            ProjectCommandChallengeUse::ExactRetryInProgress => {
                transaction
                    .rollback()
                    .await
                    .map_err(accept_challenge_error)?;
                Err(AcceptProposalError::BindingConflict)
            }
            ProjectCommandChallengeUse::FirstUse => {
                match persist_accept(&transaction.client, command).await {
                    Ok(settlement) => {
                        transaction.commit().await.map_err(accept_challenge_error)?;
                        Ok(settlement)
                    }
                    Err(error) => {
                        transaction
                            .rollback()
                            .await
                            .map_err(accept_challenge_error)?;
                        let reason = match error {
                            AcceptProposalError::PreAdmissionRefused { reason } => reason,
                            AcceptProposalError::InvalidChallenge => {
                                AcceptanceRefusalReason::InvalidChallenge
                            }
                            other => return Err(other),
                        };
                        let reason = self
                            .retain_acceptance_refusal(
                                command,
                                reason,
                                AcceptanceRefusalBoundary::WriterSession,
                            )
                            .await?;
                        Err(AcceptProposalError::PreAdmissionRefused { reason })
                    }
                }
            }
        }
    }
}

pub(super) struct LoadedProposal {
    current_revision_id: String,
    generation: String,
    closure: String,
    validation_current: bool,
    candidate_text: String,
    chapter_id: String,
    operations: Vec<LoadedOperation>,
    bundle_policy: String,
    receipt_id: Option<String>,
    receipt_result: Option<String>,
    receipt_revision_id: Option<String>,
    receipt_candidate_text: Option<String>,
    current_head_revision_id: Option<String>,
    validated_target_matches_head: bool,
    kind: String,
    chapter_body: String,
    chapter_blocks: Vec<storyos_core::ManuscriptBlock>,
    manuscript_block_id: String,
    inline_from: Option<u32>,
    inline_to: Option<u32>,
    inline_digest: Option<String>,
    inline_base_slice_matches: bool,
}

pub(super) struct LoadedOperation {
    operation_id: String,
    manuscript_block_id: String,
    resolution: String,
    candidate_text: String,
    predecessor_operation_ids: Vec<String>,
}

impl LoadedProposal {
    pub(super) fn accepted_body(
        &self,
        selected_ids: &[String],
    ) -> Result<String, AcceptProposalError> {
        if self.kind != "inline_edit" {
            return Ok(self.composed_block_body(selected_ids));
        }
        let (Some(from), Some(to)) = (self.inline_from, self.inline_to) else {
            return Err(AcceptProposalError::Unavailable(Box::new(
                std::io::Error::other("inline Acceptance needs exact Anchors"),
            )));
        };
        storyos_core::splice_utf16_range(&self.chapter_body, from, to, &self.candidate_text)
            .map_err(|error| {
                AcceptProposalError::Unavailable(Box::new(std::io::Error::other(format!(
                    "{error:?}"
                ))))
            })
    }

    fn composed_block_body(&self, selected_ids: &[String]) -> String {
        let selected: std::collections::BTreeSet<&str> =
            selected_ids.iter().map(String::as_str).collect();
        if self.chapter_blocks.is_empty() {
            return self
                .operations
                .iter()
                .find(|operation| selected.contains(operation.operation_id.as_str()))
                .map(|operation| operation.candidate_text.clone())
                .unwrap_or_else(|| self.candidate_text.clone());
        }
        let mut blocks = self.chapter_blocks.clone();
        for block in &mut blocks {
            if let Some(operation) = self.operations.iter().find(|operation| {
                selected.contains(operation.operation_id.as_str())
                    && operation.manuscript_block_id == block.manuscript_block_id
            }) {
                block.text = operation.candidate_text.clone();
            }
        }
        crate::manuscript_block::persist_canonical_bytes(&blocks)
    }

    fn inline_slice_matches(&self) -> bool {
        if self.kind != "inline_edit" {
            return true;
        }
        let (Some(from), Some(to), Some(digest)) = (
            self.inline_from,
            self.inline_to,
            self.inline_digest.as_deref(),
        ) else {
            return false;
        };
        let block_text = crate::manuscript_block::blocks_from_stored_payload(
            &self.chapter_body,
            std::slice::from_ref(&self.manuscript_block_id),
        )
        .into_iter()
        .next()
        .map(|block| block.text)
        .unwrap_or_else(|| self.chapter_body.clone());
        let units: Vec<u16> = block_text.encode_utf16().collect();
        let (Ok(start), Ok(end)) = (usize::try_from(from), usize::try_from(to)) else {
            return false;
        };
        let Some(slice) = units
            .get(start..end)
            .and_then(|range| String::from_utf16(range).ok())
        else {
            return false;
        };
        storyos_core::proposal_anchor_base_slice_digest(
            &self.manuscript_block_id,
            "paragraph",
            1,
            storyos_core::PROSEMIRROR_TOKEN_UTF16_V1,
            from,
            to,
            &slice,
        ) == digest
    }
}

async fn persist_accept(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
) -> Result<AcceptProposalSettlement, AcceptProposalError> {
    let Some(loaded) = load_proposal(client, command).await? else {
        return Err(AcceptProposalError::MissingProject);
    };
    if let Some(row) = client
        .query_opt(
            "SELECT reason FROM storyos.acceptance_refusals WHERE owner_user_id = $1::text::uuid
         AND project_id = $2::text::uuid AND idempotency_key = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.challenge_binding.idempotency_key,
            ],
        )
        .await
        .map_err(accept_database_error)?
    {
        let reason = crate::acceptance_refusal::parse_reason(row.get::<_, &str>(0))
            .map_err(accept_parse_error)?;
        return Err(AcceptProposalError::PreAdmissionRefused { reason });
    }
    insert_accept_admission(client, command, &loaded.chapter_id).await?;
    let selection = classify_proposal_selection(
        &command.selected_operation_ids,
        &loaded
            .operations
            .iter()
            .map(|operation| ProposalOperationSelection {
                operation_id: operation.operation_id.clone(),
                resolution: operation.resolution.clone(),
                predecessor_operation_ids: operation.predecessor_operation_ids.clone(),
            })
            .collect::<Vec<_>>(),
        ProposalBundlePolicy::from_stored(&loaded.bundle_policy),
        ProposalSelectionIntent::Accept,
    );
    let classified = classify(&CoreAccept {
        scope_matches: true,
        admission_valid: true,
        proposal_revision_current: loaded.current_revision_id == command.proposal_revision_id,
        retention_retained: true,
        generation_ready: loaded.generation == "ready",
        closure_open: loaded.closure == "open",
        validation_current: loaded.validation_current,
        validation_receipt_valid: loaded.receipt_result.as_deref() == Some("valid")
            && loaded.receipt_id.as_deref() == Some(command.validation_receipt_id.as_str()),
        validation_receipt_matches_revision: loaded.receipt_revision_id.as_deref()
            == Some(command.proposal_revision_id.as_str()),
        selected_operation_pending: selection.all_selected_pending,
        selection_duplicate_free: selection.duplicate_free,
        required_dependencies_met: selection.required_dependencies_met,
        bundle_closure_complete: selection.bundle_closure_complete,
        expected_target_matches_head: loaded.validated_target_matches_head
            && loaded.inline_base_slice_matches
            && loaded.current_head_revision_id.as_deref()
                == Some(command.expected_authoritative_revision_id.as_str()),
        candidate_unaltered: loaded.receipt_candidate_text.as_deref()
            == Some(loaded.candidate_text.as_str()),
    });
    match classified {
        AcceptProposalResult::Applied => persist_applied(client, command, &loaded).await,
        AcceptProposalResult::Invalid { reason } => {
            persist_zero(
                client,
                command,
                "invalid",
                invalid_reason(&reason),
                AcceptProposalSettlementEffect::Invalid { reason },
            )
            .await
        }
        AcceptProposalResult::Conflicted { reason } => {
            persist_zero(
                client,
                command,
                "conflicted",
                "changed_head",
                AcceptProposalSettlementEffect::Conflicted { reason },
            )
            .await
        }
        AcceptProposalResult::Refused { reason } => {
            persist_zero(
                client,
                command,
                "refused",
                refuse_reason(&reason),
                AcceptProposalSettlementEffect::Refused { reason },
            )
            .await
        }
    }
}

async fn load_proposal(
    client: &tokio_postgres::Client,
    command: &AcceptProposalCommand,
) -> Result<Option<LoadedProposal>, AcceptProposalError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.closure,
                    revision.candidate_text, proposal.chapter_id::text,
                    receipt.validation_receipt_id::text, receipt.result,
                    receipt.proposal_revision_id::text, receipt.candidate_text,
                    chapter_head.current_revision_id::text,
                    revision.validation = 'valid' AND NOT EXISTS (SELECT 1 FROM storyos.proposal_validation_conditions AS condition
                      WHERE (condition.owner_user_id, condition.project_id, condition.proposal_id,
                             condition.proposal_revision_id) =
                            (revision.owner_user_id, revision.project_id, revision.proposal_id, revision.revision_id)),
                    COALESCE(receipt.base_authoritative_revision_id = chapter_head.current_revision_id
                      AND revision.base_authoritative_revision_id = chapter_head.current_revision_id, false),
                    proposal.kind, convert_from(payload.canonical_bytes, 'UTF8'),
                    proposal.manuscript_block_id::text, anchor.range_from, anchor.range_to,
                    anchor.base_slice_digest, proposal.bundle_policy
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               LEFT JOIN LATERAL (
                 SELECT operation.operation_id
                   FROM storyos.proposal_operations AS operation
                  WHERE (operation.owner_user_id, operation.project_id, operation.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                  ORDER BY operation.operation_id
                  LIMIT 1
               ) AS primary_operation ON true
               LEFT JOIN storyos.validation_receipts AS receipt
                 ON (receipt.owner_user_id, receipt.project_id, receipt.validation_receipt_id) =
                    (proposal.owner_user_id, proposal.project_id, $4::text::uuid)
               LEFT JOIN storyos.authoritative_heads AS chapter_head
                 ON (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id)
               LEFT JOIN storyos.authoritative_revisions AS chapter_revision
                 ON (chapter_revision.owner_user_id, chapter_revision.project_id,
                     chapter_revision.manuscript_object_id, chapter_revision.revision_id) =
                    (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id, chapter_head.current_revision_id)
               LEFT JOIN storyos.authoritative_payloads AS payload
                 ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                    (chapter_revision.owner_user_id, chapter_revision.project_id,
                     chapter_revision.payload_id)
               LEFT JOIN storyos.proposal_anchors AS anchor
                 ON (anchor.owner_user_id, anchor.project_id, anchor.proposal_id,
                     anchor.operation_id, anchor.anchor_order) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id,
                     primary_operation.operation_id, 1)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.proposal_id = $3::text::uuid",
            &[
                &command.project_scope.owner_user_id.as_ref(),
                &command.project_scope.project_id.as_ref(),
                &command.proposal_id,
                &command.validation_receipt_id,
            ],
        )
        .await
        .map_err(accept_database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let chapter_id: String = row.get(4);
    let chapter_body: String = row.get::<_, Option<String>>(13).unwrap_or_default();
    let current_head_revision_id: Option<String> = row.get(9);
    let operations = load_operations(
        client,
        command.project_scope.owner_user_id.as_ref(),
        command.project_scope.project_id.as_ref(),
        &command.proposal_id,
    )
    .await?;
    let chapter_blocks = match current_head_revision_id.as_deref() {
        Some(revision_id) => crate::manuscript_block::load_revision_blocks(
            client,
            command.project_scope.owner_user_id.as_ref(),
            command.project_scope.project_id.as_ref(),
            &chapter_id,
            revision_id,
            &chapter_body,
        )
        .await
        .map_err(accept_database_error)?,
        None => Vec::new(),
    };
    let mut loaded = LoadedProposal {
        current_revision_id: row.get(0),
        generation: row.get(1),
        closure: row.get(2),
        candidate_text: row.get(3),
        chapter_id,
        operations,
        bundle_policy: row.get(18),
        receipt_id: row.get(5),
        receipt_result: row.get(6),
        receipt_revision_id: row.get(7),
        receipt_candidate_text: row.get(8),
        current_head_revision_id,
        validation_current: row.get(10),
        validated_target_matches_head: row.get(11),
        kind: row.get(12),
        chapter_body,
        chapter_blocks,
        manuscript_block_id: row.get::<_, Option<String>>(14).unwrap_or_default(),
        inline_from: row
            .get::<_, Option<i32>>(15)
            .and_then(|value| u32::try_from(value).ok()),
        inline_to: row
            .get::<_, Option<i32>>(16)
            .and_then(|value| u32::try_from(value).ok()),
        inline_digest: row.get(17),
        inline_base_slice_matches: true,
    };
    loaded.inline_base_slice_matches = loaded.inline_slice_matches();
    Ok(Some(loaded))
}

async fn load_operations(
    client: &tokio_postgres::Client,
    owner_user_id: &str,
    project_id: &str,
    proposal_id: &str,
) -> Result<Vec<LoadedOperation>, AcceptProposalError> {
    let rows = client
        .query(
            "SELECT operation_id::text, manuscript_block_id::text, resolution, candidate_text,
                    COALESCE(predecessor_operation_ids::text[], '{}')
               FROM storyos.proposal_operations
              WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                AND proposal_id = $3::text::uuid
              ORDER BY operation_id",
            &[&owner_user_id, &project_id, &proposal_id],
        )
        .await
        .map_err(accept_database_error)?;
    Ok(rows
        .into_iter()
        .map(|row| LoadedOperation {
            operation_id: row.get(0),
            manuscript_block_id: row.get(1),
            resolution: row.get(2),
            candidate_text: row.get(3),
            predecessor_operation_ids: row.get(4),
        })
        .collect())
}

pub(super) fn invalid_reason(reason: &AcceptProposalInvalid) -> &'static str {
    match reason {
        AcceptProposalInvalid::InvalidValidation => "invalid_validation",
        AcceptProposalInvalid::AlteredCandidate => "altered_candidate",
    }
}

pub(super) fn refuse_reason(reason: &AcceptProposalRefusal) -> &'static str {
    match reason {
        AcceptProposalRefusal::WrongScope => "wrong_scope",
        AcceptProposalRefusal::WrongAdmission => "wrong_admission",
        AcceptProposalRefusal::StaleProposalRevision => "stale_proposal_revision",
        AcceptProposalRefusal::NotEligible => "not_eligible",
        AcceptProposalRefusal::OperationNotPending => "operation_not_pending",
        AcceptProposalRefusal::DuplicateIdentities => "duplicate_identities",
        AcceptProposalRefusal::MissingRequiredDependencies => "missing_required_dependencies",
        AcceptProposalRefusal::IncompleteBundleClosure => "incomplete_bundle_closure",
    }
}

pub(super) fn accept_challenge_error(error: ProjectCommandChallengeError) -> AcceptProposalError {
    match error {
        ProjectCommandChallengeError::BindingConflict => AcceptProposalError::BindingConflict,
        ProjectCommandChallengeError::InvalidOrExpired
        | ProjectCommandChallengeError::RateLimited { .. } => AcceptProposalError::InvalidChallenge,
        ProjectCommandChallengeError::Unavailable(source) => {
            AcceptProposalError::Unavailable(source)
        }
    }
}

pub(super) fn accept_database_error(error: tokio_postgres::Error) -> AcceptProposalError {
    AcceptProposalError::Unavailable(Box::new(error))
}

pub(super) fn accept_parse_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> AcceptProposalError {
    AcceptProposalError::Unavailable(Box::new(error))
}
