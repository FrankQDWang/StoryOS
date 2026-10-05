use std::convert::Infallible;

use storyos_application::{
    DraftCloseObservation, DraftExpanded, DraftExpansionObservation,
    ExpandRefusedEditDraftSettlement, ExpandRefusedEditDraftToProposalInput,
    ProjectCommandEnvelope, ProjectCommandError,
};
use storyos_core::{
    DraftCloseSource, ExpandRefusedEditDraftConflict, ExpandRefusedEditDraftRefusal,
    InlineTargetBlock, ManuscriptBlockKind, OpenInlineProposal, PROSEMIRROR_TOKEN_UTF16_V1,
    ProjectLifecycle, canonical_json, expand_refused_edit_draft, hex_sha256,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActionOnly, ActionSequence, Admission, AppliedResult, Classification, CommandIsolation,
    CommandSpec, EditorAdmission, EditorWriter, LockedProject, MissingAdmission, NoResponse,
    ProjectCommand, RateLimitedChallenge, ReceiptHeads, ReceiptRefs, ReplayEffect, ZeroReceipt,
    settle_project_command, unavailable,
};

impl PostgresProjectReader {
    /// Settles one author expansion of a Refused Edit Draft to an inline edit Proposal.
    pub async fn expand_refused_edit_draft(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ExpandRefusedEditDraftToProposalInput,
    ) -> Result<ExpandRefusedEditDraftSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The Receipt payload fields of one expansion outcome, without `reason`.
///
/// The Proposal and close event fields are null until an applied outcome sets them.
fn observation_fields(
    observation: &DraftExpansionObservation,
) -> serde_json::Map<String, serde_json::Value> {
    let draft = &observation.draft;
    serde_json::Map::from_iter([
        (
            "draft_revision_id".to_owned(),
            draft.draft_revision_id.clone().into(),
        ),
        (
            "payload_digest".to_owned(),
            draft.payload_digest.clone().into(),
        ),
        (
            "observed_closure".to_owned(),
            draft.observed_closure.clone().into(),
        ),
        (
            "current_target_revision_id".to_owned(),
            observation.current_target_revision_id.clone().into(),
        ),
        ("event_id".to_owned(), serde_json::Value::Null),
        ("proposal_id".to_owned(), serde_json::Value::Null),
        ("proposal_revision_id".to_owned(), serde_json::Value::Null),
    ])
}

impl ProjectCommand for ExpandRefusedEditDraftToProposalInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "expandRefusedEditDraftToProposal",
        applied_result: AppliedResult::command("proposal_created_from_draft"),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidWriter,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'event_id', event.event_id::text,
                      'draft_id', event.draft_id::text,
                      'author_action_sequence', event.author_action_sequence::text,
                      'close_reason', event.close_reason,
                      'proposal_id', revision.proposal_id::text,
                      'proposal_revision_id', revision.revision_id::text,
                      'candidate_blocks', revision.candidate_blocks::text)::text
               FROM storyos.draft_close_events AS event
               JOIN storyos.domain_receipts AS receipt
                 USING (owner_user_id, project_id, receipt_id)
          LEFT JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id::text,
                     revision.revision_id::text) =
                    (receipt.owner_user_id, receipt.project_id,
                     receipt.result_payload->>'proposal_id',
                     receipt.result_payload->>'proposal_revision_id')
              WHERE event.owner_user_id = $1::text::uuid
                AND event.project_id = $2::text::uuid
                AND event.receipt_id = $3::text::uuid",
        ),
    };
    type Profile = ActionOnly;
    type Response = NoResponse;
    type ZeroEffect = DraftExpansionObservation;
    type Applied = ();
    /// The new Proposal and close event identities and the observation that they settle.
    type Plan = DraftExpanded;
    type Effect = DraftExpanded;
    type NoEffect = Infallible;
    type Conflict = ExpandRefusedEditDraftConflict;
    type Refusal = ExpandRefusedEditDraftRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        match project.lifecycle {
            ProjectLifecycle::Active => {}
            ProjectLifecycle::Archived => return Err(ProjectCommandError::MissingProject),
        }
        let scope = &envelope.project_scope;
        let owner_user_id = scope.owner_user_id.as_ref();
        let project_id = scope.project_id.as_ref();
        let source = client
            .query_opt(
                "SELECT draft.current_revision_id::text, revision.payload_digest, draft.closure,
                        draft.retention_state,
                        CASE WHEN draft.retention_state = 'retained'
                             THEN revision.payload::text END,
                        draft.reopen_event_id::text
                   FROM storyos.draft_artifacts AS draft
                   JOIN storyos.draft_artifact_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id, revision.draft_id,
                         revision.revision_id) =
                        (draft.owner_user_id, draft.project_id, draft.draft_id,
                         draft.current_revision_id)
                  WHERE draft.owner_user_id = $1::text::uuid
                    AND draft.project_id = $2::text::uuid
                    AND draft.draft_id = $3::text::uuid
                    FOR UPDATE OF draft",
                &[&owner_user_id, &project_id, &self.draft_id],
            )
            .await
            .map_err(unavailable)?
            .ok_or(ProjectCommandError::MissingProject)?;
        let draft = DraftCloseObservation {
            draft_revision_id: source.get(/*idx*/ 0),
            payload_digest: source.get(/*idx*/ 1),
            observed_closure: source.get(/*idx*/ 2),
        };
        let retention: String = source.get(/*idx*/ 3);
        let reopen_event_id: Option<String> = source.get(/*idx*/ 5);
        let target = client
            .query_opt(
                "SELECT head.current_revision_id::text,
                        convert_from(payload.canonical_bytes, 'UTF8'),
                        ARRAY(SELECT member.manuscript_block_id::text
                                FROM storyos.manuscript_revision_members AS member
                               WHERE (member.owner_user_id, member.project_id,
                                      member.manuscript_object_id, member.revision_id) =
                                     (head.owner_user_id, head.project_id,
                                      head.manuscript_object_id, head.current_revision_id)
                               ORDER BY member.block_order),
                        EXISTS(SELECT 1 FROM storyos.proposal_operations AS operation
                                WHERE operation.owner_user_id = head.owner_user_id
                                  AND operation.project_id = head.project_id
                                  AND operation.manuscript_block_id = $4::text::uuid
                                  AND operation.reservation_state = 'unresolved')
                   FROM storyos.authoritative_heads AS head
                   JOIN storyos.authoritative_revisions AS revision
                     ON (revision.owner_user_id, revision.project_id,
                         revision.manuscript_object_id, revision.revision_id) =
                        (head.owner_user_id, head.project_id, head.manuscript_object_id,
                         head.current_revision_id)
                   JOIN storyos.authoritative_payloads AS payload
                     ON (payload.owner_user_id, payload.project_id, payload.payload_id) =
                        (revision.owner_user_id, revision.project_id, revision.payload_id)
                  WHERE head.owner_user_id = $1::text::uuid
                    AND head.project_id = $2::text::uuid
                    AND head.manuscript_object_id = $3::text::uuid
                    AND NOT EXISTS(
                      SELECT 1 FROM storyos.chapter_removal_decisions AS removed
                       WHERE (removed.owner_user_id, removed.project_id, removed.chapter_id) =
                             (head.owner_user_id, head.project_id, head.manuscript_object_id))
                    FOR UPDATE OF head",
                &[
                    &owner_user_id,
                    &project_id,
                    &self.chapter_id,
                    &self.target_ref,
                ],
            )
            .await
            .map_err(unavailable)?;
        let current_target_revision_id =
            target.as_ref().map(|row| row.get::<_, String>(/*idx*/ 0));
        let blocks = target
            .as_ref()
            .map(|row| {
                crate::manuscript_block::blocks_from_stored_payload(
                    &row.get::<_, String>(/*idx*/ 1),
                    &row.get::<_, Vec<String>>(/*idx*/ 2),
                )
            })
            .unwrap_or_default();
        let inline = OpenInlineProposal {
            scope_matches: true,
            target_block_present: blocks
                .iter()
                .any(|block| block.manuscript_block_id == self.target_ref),
            expected_base_revision_id: self.expected_target_revision_id.clone(),
            current_base_revision_id: current_target_revision_id.clone(),
            conflicting_reservation: target.is_some_and(|row| row.get(/*idx*/ 3)),
            current_schema_version: 1,
            current_coordinate_profile: PROSEMIRROR_TOKEN_UTF16_V1.to_owned(),
            blocks: blocks
                .into_iter()
                .map(|block| InlineTargetBlock {
                    manuscript_block_id: block.manuscript_block_id,
                    block_kind: match block.block_kind {
                        ManuscriptBlockKind::Paragraph => "paragraph",
                        ManuscriptBlockKind::Heading => "heading",
                    }
                    .to_owned(),
                    text: block.text,
                })
                .collect(),
            anchors: vec![self.anchor.clone()],
        };
        let outcome = expand_refused_edit_draft(
            &DraftCloseSource {
                revision: &self.source_current_draft_revision_id,
                digest: &self.source_draft_payload_digest,
                reopen_event_id: self.source_reopen_event_id.as_deref(),
            },
            &DraftCloseSource {
                revision: &draft.draft_revision_id,
                digest: &draft.payload_digest,
                reopen_event_id: reopen_event_id.as_deref(),
            },
            &draft.observed_closure,
            &retention,
            || {
                let payload: serde_json::Value = serde_json::from_str(
                    &source
                        .get::<_, Option<String>>(/*idx*/ 4)
                        .ok_or_else(|| unavailable("the retained Draft has no payload"))?,
                )
                .map_err(unavailable)?;
                if hex_sha256(canonical_json(&payload).as_bytes()) != draft.payload_digest {
                    return Err(ProjectCommandError::BindingConflict);
                }
                serde_json::from_value(payload).map_err(unavailable)
            },
            &inline,
        )?;
        // The Draft settlement records need the canonical text of the Draft identity.
        if Uuid::parse_str(&self.draft_id)
            .map_err(unavailable)?
            .to_string()
            != self.draft_id
        {
            return Err(ProjectCommandError::BindingConflict);
        }
        let observation = DraftExpansionObservation {
            draft,
            current_target_revision_id,
        };
        let heads = observation
            .current_target_revision_id
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        Ok(Classification {
            zero_receipt: ZeroReceipt::Observed {
                fields: observation_fields(&observation),
                refs: ReceiptRefs {
                    draft_artifact_refs: vec![self.draft_id.clone()],
                    ..ReceiptRefs::default()
                },
                effect: observation.clone(),
            },
            outcome: outcome.map_applied(|candidate_blocks| {
                (
                    (),
                    DraftExpanded {
                        observation,
                        event_id: Uuid::now_v7().to_string(),
                        proposal_id: Uuid::now_v7().to_string(),
                        proposal_revision_id: Uuid::now_v7().to_string(),
                        candidate_blocks,
                    },
                )
            }),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: Some(self.chapter_id.clone()),
                expected_authoritative_revision_id: Some(self.expected_target_revision_id.clone()),
                target_refs: vec![self.target_ref.clone()],
                writer: EditorWriter::ClientGeneration(self.writer_generation),
            }),
            heads: ReceiptHeads {
                expected: vec![self.expected_target_revision_id.clone()],
                prior: heads.clone(),
                resulting: heads,
            },
        })
    }

    fn applied_receipt_payload(&self, _applied: &(), expanded: &DraftExpanded) -> String {
        let mut fields = observation_fields(&expanded.observation);
        fields.insert("reason".to_owned(), "superseded".into());
        fields.insert("event_id".to_owned(), expanded.event_id.clone().into());
        fields.insert(
            "proposal_id".to_owned(),
            expanded.proposal_id.clone().into(),
        );
        fields.insert(
            "proposal_revision_id".to_owned(),
            expanded.proposal_revision_id.clone().into(),
        );
        serde_json::Value::Object(fields).to_string()
    }

    fn applied_receipt_refs(&self, _applied: &(), expanded: &DraftExpanded) -> ReceiptRefs {
        let draft = &expanded.observation.draft;
        ReceiptRefs {
            proposal_revision_ids: vec![expanded.proposal_revision_id.clone()],
            draft_artifact_refs: vec![self.draft_id.clone()],
            artifact_lifecycle_event_refs: vec![expanded.event_id.clone()],
            source_draft_disposition: Some(
                serde_json::json!({
                    "kind": "closed_superseded",
                    "source_draft_kind": "refused_edit",
                    "source_draft_id": self.draft_id,
                    "source_draft_revision_id": draft.draft_revision_id,
                    "source_draft_payload_digest": draft.payload_digest,
                    "prior_closure": "open",
                    "resulting_closure": "closed",
                    "close_reason": "superseded",
                    "closure_event_ref": expanded.event_id,
                })
                .to_string(),
            ),
        }
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        ActionSequence(sequence): &ActionSequence,
        expanded: DraftExpanded,
        _applied: (),
    ) -> Result<DraftExpanded, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let owner_user_id = scope.owner_user_id.as_ref();
        let project_id = scope.project_id.as_ref();
        let draft = &expanded.observation.draft;
        let candidate_text = expanded
            .candidate_blocks
            .iter()
            .map(|block| block.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let candidate_blocks =
            serde_json::to_string(&expanded.candidate_blocks).map_err(unavailable)?;
        let anchor = &self.anchor;
        client
            .execute(
                "WITH proposal AS (
                   INSERT INTO storyos.proposals
                     (owner_user_id, project_id, proposal_id, kind, chapter_id,
                      manuscript_block_id, source_draft_id, source_draft_revision_id,
                      source_draft_payload_digest)
                   VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, 'inline_edit',
                           $6::text::uuid, $7::text::uuid, $8::text::uuid, $9::text::uuid, $10)
                 ), revision AS (
                   INSERT INTO storyos.proposal_revisions
                     (owner_user_id, project_id, proposal_id, revision_id, generation, validation,
                      closure, candidate_text, candidate_blocks, base_authoritative_revision_id)
                   VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                           'ready', 'pending', 'open', $11, $12::text::jsonb, $13::text::uuid)
                 ), head AS (
                   INSERT INTO storyos.proposal_heads
                     (owner_user_id, project_id, proposal_id, current_revision_id)
                   VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid)
                 ), operation AS (
                   INSERT INTO storyos.proposal_operations
                     (owner_user_id, project_id, proposal_id, operation_id, manuscript_block_id,
                      resolution, reservation_state, candidate_text, candidate_blocks)
                   VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $5::text::uuid,
                           $7::text::uuid, 'pending', 'unresolved', $11, $12::text::jsonb)
                 )
                 INSERT INTO storyos.proposal_anchors
                   (owner_user_id, project_id, proposal_id, operation_id, anchor_order,
                    manuscript_block_id, base_authoritative_revision_id,
                    manuscript_schema_version, coordinate_profile, range_from, range_to,
                    boundary_profile, base_slice_digest)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $5::text::uuid, 1,
                         $14::text::uuid, $15::text::uuid, $16::text::integer, $17,
                         $18::text::integer, $19::text::integer, $20, $21)",
                &[
                    &owner_user_id,
                    &project_id,
                    &expanded.proposal_id,
                    &expanded.proposal_revision_id,
                    &Uuid::now_v7().to_string(),
                    &self.chapter_id,
                    &self.target_ref,
                    &self.draft_id,
                    &draft.draft_revision_id,
                    &draft.payload_digest,
                    &candidate_text,
                    &candidate_blocks,
                    &expanded.observation.current_target_revision_id,
                    &anchor.manuscript_block_id,
                    &anchor.base_authoritative_revision_id,
                    &anchor.manuscript_schema_version.to_string(),
                    &anchor.coordinate_profile,
                    &anchor.from.to_string(),
                    &anchor.to.to_string(),
                    &anchor.boundary_profile,
                    &anchor.base_slice_digest,
                ],
            )
            .await
            .map_err(unavailable)?;
        client
            .execute(
                "INSERT INTO storyos.draft_close_events
                   (owner_user_id, project_id, event_id, draft_id, revision_id, payload_digest,
                    receipt_id, receipt_result_kind, author_action_sequence, created_at,
                    close_reason)
                 SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                        $5::text::uuid, $6, receipt.receipt_id, receipt.result_kind,
                        $8::text::numeric, receipt.created_at, 'superseded'
                   FROM storyos.domain_receipts AS receipt
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $7::text::uuid",
                &[
                    &owner_user_id,
                    &project_id,
                    &expanded.event_id,
                    &self.draft_id,
                    &draft.draft_revision_id,
                    &draft.payload_digest,
                    &envelope.ids.receipt_id,
                    &sequence.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        client
            .execute(
                "UPDATE storyos.draft_artifacts
                    SET closure = 'closed', close_event_id = $4::text::uuid,
                        reopen_event_id = NULL
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND draft_id = $3::text::uuid",
                &[
                    &owner_user_id,
                    &project_id,
                    &self.draft_id,
                    &expanded.event_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(expanded)
    }

    fn decode(&self, replay: &CommandReplay) -> Result<DraftExpanded, ReplayFault> {
        let observation = self
            .decode_zero_authority_effect(replay)?
            .ok_or(ReplayFault::BindingConflict)?;
        let damaged =
            || ReplayFault::Unavailable("the Draft expansion effect does not match".into());
        let receipt_text = |key: &str| {
            replay
                .receipt_text(key)
                .map(str::to_owned)
                .ok_or_else(damaged)
        };
        let event_id = receipt_text("event_id")?;
        let proposal_id = receipt_text("proposal_id")?;
        let proposal_revision_id = receipt_text("proposal_revision_id")?;
        replay.require_receipt_text("reason", "superseded")?;
        if replay.effect_text("event_id").as_ref() != Some(&event_id)
            || replay.effect_text("draft_id").as_ref() != Some(&self.draft_id)
            || replay.effect_text("author_action_sequence") != replay.author_action_sequence
            || replay.effect_text("close_reason").as_deref() != Some("superseded")
            || replay.effect_text("proposal_id").as_ref() != Some(&proposal_id)
            || replay.effect_text("proposal_revision_id").as_ref() != Some(&proposal_revision_id)
        {
            return Err(damaged());
        }
        let candidate_blocks =
            serde_json::from_str(&replay.effect_text("candidate_blocks").ok_or_else(damaged)?)
                .map_err(|error| ReplayFault::Unavailable(Box::new(error)))?;
        Ok(DraftExpanded {
            observation,
            event_id,
            proposal_id,
            proposal_revision_id,
            candidate_blocks,
        })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<DraftExpansionObservation>, ReplayFault> {
        if !replay.fence_digest_matches
            || !replay.admission_matches
            || replay.draft_artifact_refs != [self.draft_id.as_str()]
        {
            return Err(ReplayFault::BindingConflict);
        }
        let text = |key: &str| {
            replay.receipt_text(key).map(str::to_owned).ok_or_else(|| {
                ReplayFault::Unavailable(format!("the Draft expansion Receipt has no {key}").into())
            })
        };
        Ok(Some(DraftExpansionObservation {
            draft: DraftCloseObservation {
                draft_revision_id: text("draft_revision_id")?,
                payload_digest: text("payload_digest")?,
                observed_closure: text("observed_closure")?,
            },
            current_target_revision_id: replay
                .receipt_text("current_target_revision_id")
                .map(str::to_owned),
        }))
    }
}
