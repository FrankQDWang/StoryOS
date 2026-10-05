use std::convert::Infallible;

use storyos_application::{
    CloseEditorFlowDraftInput, CloseEditorFlowDraftSettlement, DraftCloseObservation, DraftClosed,
    ProjectCommandEnvelope, ProjectCommandError,
};
use storyos_core::{
    CloseEditorFlowDraftConflict, CloseEditorFlowDraftRefusal, DraftCloseSource, ProjectLifecycle,
    canonical_json, close_editor_flow_draft, hex_sha256,
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
    /// Settles one author Discard of a Refused Edit Draft.
    pub async fn close_editor_flow_draft(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &CloseEditorFlowDraftInput,
    ) -> Result<CloseEditorFlowDraftSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The Receipt references of one Discard outcome.
fn receipt_refs(draft_id: &str, event_id: Option<&String>) -> ReceiptRefs {
    ReceiptRefs {
        draft_artifact_refs: vec![draft_id.to_owned()],
        artifact_lifecycle_event_refs: event_id.into_iter().cloned().collect(),
        ..ReceiptRefs::default()
    }
}

/// The Receipt payload fields of one Discard outcome, without `reason`.
fn observation_fields(
    observation: &DraftCloseObservation,
    event_id: Option<&String>,
) -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::from_iter([
        (
            "draft_revision_id".to_owned(),
            observation.draft_revision_id.clone().into(),
        ),
        (
            "payload_digest".to_owned(),
            observation.payload_digest.clone().into(),
        ),
        (
            "observed_closure".to_owned(),
            observation.observed_closure.clone().into(),
        ),
        ("event_id".to_owned(), event_id.cloned().into()),
    ])
}

impl ProjectCommand for CloseEditorFlowDraftInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "closeEditorFlowDraft",
        applied_result: AppliedResult::command("draft_closure_changed"),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidWriter,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: ReplayEffect::Query(
            "SELECT jsonb_build_object(
                      'event_id', event.event_id::text,
                      'draft_id', event.draft_id::text,
                      'author_action_sequence', event.author_action_sequence::text)::text
               FROM storyos.draft_close_events AS event
              WHERE event.owner_user_id = $1::text::uuid
                AND event.project_id = $2::text::uuid
                AND event.receipt_id = $3::text::uuid",
        ),
    };
    type Profile = ActionOnly;
    type Response = NoResponse;
    type ZeroEffect = DraftCloseObservation;
    /// The identity of the new close event.
    type Applied = String;
    type Plan = DraftCloseObservation;
    type Effect = DraftClosed;
    type NoEffect = Infallible;
    type Conflict = CloseEditorFlowDraftConflict;
    type Refusal = CloseEditorFlowDraftRefusal;

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
        let row = client
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
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.draft_id,
                ],
            )
            .await
            .map_err(unavailable)?
            .ok_or(ProjectCommandError::MissingProject)?;
        let observation = DraftCloseObservation {
            draft_revision_id: row.get(/*idx*/ 0),
            payload_digest: row.get(/*idx*/ 1),
            observed_closure: row.get(/*idx*/ 2),
        };
        let retention: String = row.get(/*idx*/ 3);
        if retention == "retained" {
            let payload: serde_json::Value = serde_json::from_str(
                &row.get::<_, Option<String>>(/*idx*/ 4)
                    .ok_or(ProjectCommandError::BindingConflict)?,
            )
            .map_err(unavailable)?;
            if hex_sha256(canonical_json(&payload).as_bytes()) != observation.payload_digest {
                return Err(ProjectCommandError::BindingConflict);
            }
        }
        let reopen_event_id: Option<String> = row.get(/*idx*/ 5);
        let outcome = close_editor_flow_draft(
            &DraftCloseSource {
                revision: &self.source_current_draft_revision_id,
                digest: &self.source_draft_payload_digest,
                reopen_event_id: self.source_reopen_event_id.as_deref(),
            },
            &DraftCloseSource {
                revision: &observation.draft_revision_id,
                digest: &observation.payload_digest,
                reopen_event_id: reopen_event_id.as_deref(),
            },
            &observation.observed_closure,
            &retention,
        );
        Ok(Classification {
            zero_receipt: ZeroReceipt::Observed {
                fields: observation_fields(&observation, /*event_id*/ None),
                refs: receipt_refs(&self.draft_id, /*event_id*/ None),
                effect: observation.clone(),
            },
            outcome: outcome.map_applied(|()| (Uuid::now_v7().to_string(), observation)),
            admission: Admission::ExplicitEditorCommand(EditorAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                chapter_object_id: None,
                expected_authoritative_revision_id: None,
                target_refs: Vec::new(),
                writer: EditorWriter::ClientGeneration(self.writer_generation),
            }),
            heads: ReceiptHeads::default(),
        })
    }

    fn applied_receipt_payload(&self, event_id: &String, plan: &DraftCloseObservation) -> String {
        let mut fields = observation_fields(plan, Some(event_id));
        fields.insert("reason".to_owned(), "abandoned".into());
        serde_json::Value::Object(fields).to_string()
    }

    fn applied_receipt_refs(
        &self,
        event_id: &String,
        _plan: &DraftCloseObservation,
    ) -> ReceiptRefs {
        receipt_refs(&self.draft_id, Some(event_id))
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        ActionSequence(sequence): &ActionSequence,
        observation: DraftCloseObservation,
        event_id: String,
    ) -> Result<DraftClosed, ProjectCommandError> {
        // The Draft close event needs the canonical text of the Draft identity.
        if Uuid::parse_str(&self.draft_id)
            .map_err(unavailable)?
            .to_string()
            != self.draft_id
        {
            return Err(ProjectCommandError::BindingConflict);
        }
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.draft_close_events
                   (owner_user_id, project_id, event_id, draft_id, revision_id, payload_digest,
                    receipt_id, author_action_sequence, created_at)
                 SELECT $1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                        $5::text::uuid, $6, receipt.receipt_id, $8::text::numeric,
                        receipt.created_at
                   FROM storyos.domain_receipts AS receipt
                  WHERE receipt.owner_user_id = $1::text::uuid
                    AND receipt.project_id = $2::text::uuid
                    AND receipt.receipt_id = $7::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &event_id,
                    &self.draft_id,
                    &observation.draft_revision_id,
                    &observation.payload_digest,
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
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.draft_id,
                    &event_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        Ok(DraftClosed {
            observation,
            event_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<DraftClosed, ReplayFault> {
        let observation = self
            .decode_zero_authority_effect(replay)?
            .ok_or(ReplayFault::BindingConflict)?;
        let damaged = || ReplayFault::Unavailable("the Draft close event does not match".into());
        let event_id = replay
            .receipt_text("event_id")
            .map(str::to_owned)
            .ok_or_else(damaged)?;
        if replay.receipt_text("reason") != Some("abandoned")
            || replay.effect_text("event_id").as_ref() != Some(&event_id)
            || replay.effect_text("draft_id").as_ref() != Some(&self.draft_id)
            || replay.effect_text("author_action_sequence") != replay.author_action_sequence
        {
            return Err(damaged());
        }
        Ok(DraftClosed {
            observation,
            event_id,
        })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<DraftCloseObservation>, ReplayFault> {
        if !replay.fence_digest_matches
            || !replay.admission_matches
            || replay.draft_artifact_refs != [self.draft_id.as_str()]
        {
            return Err(ReplayFault::BindingConflict);
        }
        let text = |key: &str| {
            replay.receipt_text(key).map(str::to_owned).ok_or_else(|| {
                ReplayFault::Unavailable(format!("the Draft Discard Receipt has no {key}").into())
            })
        };
        Ok(Some(DraftCloseObservation {
            draft_revision_id: text("draft_revision_id")?,
            payload_digest: text("payload_digest")?,
            observed_closure: text("observed_closure")?,
        }))
    }
}
