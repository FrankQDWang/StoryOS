use std::convert::Infallible;

use storyos_application::{
    CompleteReadyPartialProposalInput, CompleteReadyPartialProposalSettlement,
    ContinueProposalGenerationInput, ContinueProposalGenerationSettlement, EditorSessionId,
    ProjectCommandEnvelope, ProjectCommandError, ProjectScope, ProposalGenerationCompleted,
    ProposalGenerationStarted,
};
use storyos_core::{
    CompleteReadyPartialProposal, CompleteReadyPartialProposalRefusal, ContinueProposalGeneration,
    ContinueProposalGenerationRefusal, ProposalGenerationConflict, TransitionOutcome,
    complete_ready_partial_proposal, continue_proposal_generation, hex_sha256,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActionOnly, Admission, AppliedResult, Classification, CommandIsolation, CommandSpec,
    EditorAdmission, LockedProject, MissingAdmission, ProfileSequences, ProjectCommand,
    ProjectResponse, RateLimitedChallenge, ReceiptHeads, ReplayEffect, settle_project_command,
    unavailable,
};

#[path = "proposal_generation_decision_write.rs"]
mod write;

impl PostgresProjectReader {
    /// Settles one author completion of a ready-partial Proposal Generation.
    pub async fn complete_ready_partial_proposal(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &CompleteReadyPartialProposalInput,
    ) -> Result<CompleteReadyPartialProposalSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }

    /// Settles one author continuation of a Proposal Generation.
    pub async fn continue_proposal_generation(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ContinueProposalGenerationInput,
    ) -> Result<ContinueProposalGenerationSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The current Proposal Revision, Proposal Generation, and AgentRun of one Proposal.
pub(crate) struct LoadedGeneration {
    revision_id: String,
    generation_state: String,
    validation: String,
    closure: String,
    operation_resolution: String,
    candidate_text: String,
    generation_id: String,
    last_seq: u64,
    source_run_id: String,
    run_status: String,
    chapter_id: String,
    head_revision_id: Option<String>,
    run_agent_id: String,
    conversation_id: String,
    memory_settings_revision: String,
    grant_id: String,
    binding_revision: String,
    author_message: String,
}

/// Reads the Proposal Generation transition record of one Receipt for an exact retry.
const TRANSITION_REPLAY: ReplayEffect = ReplayEffect::Query(
    "SELECT jsonb_build_object(
              'transition_id', transition.transition_id::text,
              'prior_generation_id', transition.prior_generation_id::text,
              'resulting_generation_id', transition.resulting_generation_id::text,
              'prior_generation_state', transition.prior_generation_state,
              'prior_run_id', transition.prior_run_id::text,
              'resulting_run_id', transition.resulting_run_id::text,
              'preserved_validation', transition.preserved_validation,
              'preserved_closure', transition.preserved_closure,
              'preserved_operation_resolution', transition.preserved_operation_resolution)::text
       FROM storyos.proposal_generation_transitions AS transition
      WHERE transition.owner_user_id = $1::text::uuid
        AND transition.project_id = $2::text::uuid
        AND transition.receipt_id = $3::text::uuid",
);

impl ProjectCommand for CompleteReadyPartialProposalInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "completeReadyPartialProposal",
        applied_result: AppliedResult::command("proposal_generation_completed"),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: TRANSITION_REPLAY,
    };
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = ();
    type Plan = LoadedGeneration;
    type Effect = ProposalGenerationCompleted;
    type NoEffect = Infallible;
    type Conflict = ProposalGenerationConflict;
    type Refusal = CompleteReadyPartialProposalRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let loaded = load_generation(client, &envelope.project_scope, &self.proposal_id).await?;
        let outcome = complete_ready_partial_proposal(&CompleteReadyPartialProposal {
            revision_current: loaded.revision_id == self.proposal_revision_id,
            closure_open: loaded.closure == "open",
            generation_state: loaded.generation_state.clone(),
            generation_id_matches: loaded.generation_id == self.generation_id,
            candidate_digest_matches: hex_sha256(loaded.candidate_text.as_bytes())
                == self.expected_candidate_digest,
            stream_seq_matches: loaded.last_seq == self.last_applied_stream_seq,
            expected_target_matches_head: loaded.head_revision_id.as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        Ok(editor_classification(
            outcome,
            loaded,
            &self.editor_session_id,
            &self.expected_authoritative_revision_id,
        ))
    }

    fn applied_receipt_payload(&self, _applied: &(), _plan: &LoadedGeneration) -> String {
        r#"{"transition":"generation_completed"}"#.to_owned()
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        sequence: &ProfileSequences<Self>,
        loaded: LoadedGeneration,
        _applied: (),
    ) -> Result<ProposalGenerationCompleted, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let updated = client
            .execute(
                "UPDATE storyos.proposal_revisions
                    SET generation = 'ready'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND revision_id = $4::text::uuid
                    AND generation = 'ready_partial'",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &loaded.revision_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let event_id = Uuid::now_v7().to_string();
        write::insert_transition(
            client,
            envelope,
            sequence.0,
            &self.proposal_id,
            &loaded,
            write::ResultingGeneration {
                event_id: &event_id,
                generation_id: &loaded.generation_id,
                state: "ready",
                run_id: &loaded.source_run_id,
            },
        )
        .await?;
        Ok(ProposalGenerationCompleted {
            generation_id: loaded.generation_id,
            preserved_validation: loaded.validation,
            preserved_closure: loaded.closure,
            preserved_operation_resolution: loaded.operation_resolution,
            generation_event_id: event_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProposalGenerationCompleted, ReplayFault> {
        // The transition record has no nullable stored value, so it has no pre-capture form.
        let field = |key| {
            replay.effect_text(key).ok_or_else(|| {
                ReplayFault::Unavailable("the applied transition record is missing".into())
            })
        };
        Ok(ProposalGenerationCompleted {
            generation_id: field("resulting_generation_id")?,
            preserved_validation: field("preserved_validation")?,
            preserved_closure: field("preserved_closure")?,
            preserved_operation_resolution: field("preserved_operation_resolution")?,
            generation_event_id: field("transition_id")?,
        })
    }
}

impl ProjectCommand for ContinueProposalGenerationInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "continueProposalGeneration",
        applied_result: AppliedResult::command("proposal_generation_started"),
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::InvalidChallenge,
        // The ActionOnly profile writes no Activity record.
        activity_kind: "",
        replay_effect: TRANSITION_REPLAY,
    };
    type Profile = ActionOnly;
    type Response = ProjectResponse;
    type ZeroEffect = ();
    type Applied = ();
    type Plan = LoadedGeneration;
    type Effect = ProposalGenerationStarted;
    type NoEffect = Infallible;
    type Conflict = ProposalGenerationConflict;
    type Refusal = ContinueProposalGenerationRefusal;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let loaded = load_generation(client, scope, &self.proposal_id).await?;
        let pending = client
            .query(
                "SELECT operation_id::text
                   FROM storyos.proposal_operations
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND resolution = 'pending'
                    AND operation_id = ANY($4::text[]::uuid[])",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &self.selected_pending_operation_ids,
                ],
            )
            .await
            .map_err(unavailable)?;
        let outcome = continue_proposal_generation(&ContinueProposalGeneration {
            revision_current: loaded.revision_id == self.proposal_revision_id,
            closure_open: loaded.closure == "open",
            generation_state: loaded.generation_state.clone(),
            expected_generation_state: self.expected_generation_state.clone(),
            generation_id_matches: loaded.generation_id == self.prior_generation_id,
            candidate_digest_matches: hex_sha256(loaded.candidate_text.as_bytes())
                == self.expected_candidate_digest,
            selected_operations_pending: pending.len() == self.selected_pending_operation_ids.len(),
            selection_duplicate_free: self
                .selected_pending_operation_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == self.selected_pending_operation_ids.len(),
            expected_target_matches_head: loaded.head_revision_id.as_deref()
                == Some(self.expected_authoritative_revision_id.as_str()),
        });
        Ok(editor_classification(
            outcome,
            loaded,
            &self.editor_session_id,
            &self.expected_authoritative_revision_id,
        ))
    }

    fn applied_receipt_payload(&self, _applied: &(), _plan: &LoadedGeneration) -> String {
        r#"{"transition":"generation_started"}"#.to_owned()
    }

    async fn apply(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        sequence: &ProfileSequences<Self>,
        loaded: LoadedGeneration,
        _applied: (),
    ) -> Result<ProposalGenerationStarted, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let resulting_run_id = match loaded.run_status.as_str() {
            "completed" | "refused" | "cancelled" => {
                let run_id = Uuid::now_v7().to_string();
                write::insert_successor_run(client, envelope, &loaded, &run_id).await?;
                run_id
            }
            _ => loaded.source_run_id.clone(),
        };
        let new_generation_id = Uuid::now_v7().to_string();
        let event_id = Uuid::now_v7().to_string();
        client
            .execute(
                "INSERT INTO storyos.proposal_generations
                   (owner_user_id, project_id, generation_id, proposal_id,
                    last_applied_stream_seq, run_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         0, $5::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &new_generation_id,
                    &self.proposal_id,
                    &resulting_run_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        client
            .execute(
                "UPDATE storyos.proposal_generation_heads
                    SET generation_id = $4::text::uuid
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &new_generation_id,
                ],
            )
            .await
            .map_err(unavailable)?;
        let updated = client
            .execute(
                "UPDATE storyos.proposal_revisions
                    SET generation = 'generating'
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid
                    AND proposal_id = $3::text::uuid AND revision_id = $4::text::uuid
                    AND generation = $5",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.proposal_id,
                    &loaded.revision_id,
                    &loaded.generation_state,
                ],
            )
            .await
            .map_err(unavailable)?;
        if updated != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        write::insert_transition(
            client,
            envelope,
            sequence.0,
            &self.proposal_id,
            &loaded,
            write::ResultingGeneration {
                event_id: &event_id,
                generation_id: &new_generation_id,
                state: "generating",
                run_id: &resulting_run_id,
            },
        )
        .await?;
        Ok(ProposalGenerationStarted {
            prior_generation_id: loaded.generation_id,
            new_generation_id,
            prior_generation_state: loaded.generation_state,
            prior_run_id: loaded.source_run_id,
            resulting_run_id,
            preserved_validation: loaded.validation,
            preserved_closure: loaded.closure,
            preserved_operation_resolution: loaded.operation_resolution,
            generation_event_id: event_id,
        })
    }

    fn decode(&self, replay: &CommandReplay) -> Result<ProposalGenerationStarted, ReplayFault> {
        // The transition record has no nullable stored value, so it has no pre-capture form.
        let field = |key| {
            replay.effect_text(key).ok_or_else(|| {
                ReplayFault::Unavailable("the applied transition record is missing".into())
            })
        };
        Ok(ProposalGenerationStarted {
            prior_generation_id: field("prior_generation_id")?,
            new_generation_id: field("resulting_generation_id")?,
            prior_generation_state: field("prior_generation_state")?,
            prior_run_id: field("prior_run_id")?,
            resulting_run_id: field("resulting_run_id")?,
            preserved_validation: field("preserved_validation")?,
            preserved_closure: field("preserved_closure")?,
            preserved_operation_resolution: field("preserved_operation_resolution")?,
            generation_event_id: field("transition_id")?,
        })
    }
}

/// The editor Admission and the one-head Receipt arrays of a generation decision.
fn editor_classification<C: ProjectCommand<Applied = (), Plan = LoadedGeneration>>(
    outcome: TransitionOutcome<(), C::NoEffect, C::Conflict, C::Refusal>,
    loaded: LoadedGeneration,
    editor_session_id: &EditorSessionId,
    expected_authoritative_revision_id: &str,
) -> Classification<C> {
    let head = vec![expected_authoritative_revision_id.to_owned()];
    Classification {
        admission: Admission::ExplicitEditorCommand(EditorAdmission {
            editor_session_id: editor_session_id.as_ref().to_owned(),
            chapter_object_id: Some(loaded.chapter_id.clone()),
            expected_authoritative_revision_id: Some(expected_authoritative_revision_id.to_owned()),
        }),
        heads: ReceiptHeads {
            expected: head.clone(),
            prior: head.clone(),
            resulting: head,
        },
        outcome: outcome.map_applied(|()| ((), loaded)),
    }
}

async fn load_generation(
    client: &Client,
    scope: &ProjectScope,
    proposal_id: &str,
) -> Result<LoadedGeneration, ProjectCommandError> {
    let row = client
        .query_opt(
            "SELECT head.current_revision_id::text, revision.generation, revision.validation,
                    revision.closure, operation.resolution,
                    revision.candidate_text, generation.generation_id::text,
                    generation.last_applied_stream_seq,
                    COALESCE(generation.run_id, proposal.source_run_id)::text,
                    run.status, proposal.chapter_id::text, chapter_head.current_revision_id::text,
                    run.project_agent_id::text, run.conversation_id::text,
                    run.memory_settings_revision::text, run.grant_id::text,
                    run.project_model_use_binding_revision::text, run.author_message
               FROM storyos.proposals AS proposal
               JOIN storyos.proposal_heads AS head
                 ON (head.owner_user_id, head.project_id, head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_revisions AS revision
                 ON (revision.owner_user_id, revision.project_id, revision.proposal_id,
                     revision.revision_id) =
                    (head.owner_user_id, head.project_id, head.proposal_id,
                     head.current_revision_id)
               JOIN storyos.proposal_generation_heads AS generation_head
                 ON (generation_head.owner_user_id, generation_head.project_id,
                     generation_head.proposal_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
               JOIN storyos.proposal_generations AS generation
                 ON (generation.owner_user_id, generation.project_id, generation.generation_id) =
                    (generation_head.owner_user_id, generation_head.project_id,
                     generation_head.generation_id)
               JOIN LATERAL (
                 SELECT first_operation.resolution
                   FROM storyos.proposal_operations AS first_operation
                  WHERE (first_operation.owner_user_id, first_operation.project_id,
                         first_operation.proposal_id) =
                        (proposal.owner_user_id, proposal.project_id, proposal.proposal_id)
                  ORDER BY first_operation.operation_id
                  LIMIT 1
               ) AS operation ON true
               JOIN storyos.agent_runs AS run
                 ON (run.owner_user_id, run.project_id, run.run_id) =
                    (proposal.owner_user_id, proposal.project_id,
                     COALESCE(generation.run_id, proposal.source_run_id))
               LEFT JOIN storyos.authoritative_heads AS chapter_head
                 ON (chapter_head.owner_user_id, chapter_head.project_id,
                     chapter_head.manuscript_object_id) =
                    (proposal.owner_user_id, proposal.project_id, proposal.chapter_id)
              WHERE proposal.owner_user_id = $1::text::uuid
                AND proposal.project_id = $2::text::uuid
                AND proposal.proposal_id = $3::text::uuid",
            &[
                &scope.owner_user_id.as_ref(),
                &scope.project_id.as_ref(),
                &proposal_id,
            ],
        )
        .await
        .map_err(unavailable)?
        .ok_or(ProjectCommandError::MissingProject)?;
    Ok(LoadedGeneration {
        revision_id: row.get(/*idx*/ 0),
        generation_state: row.get(/*idx*/ 1),
        validation: row.get(/*idx*/ 2),
        closure: row.get(/*idx*/ 3),
        operation_resolution: row.get(/*idx*/ 4),
        candidate_text: row.get(/*idx*/ 5),
        generation_id: row.get(/*idx*/ 6),
        last_seq: u64::try_from(row.get::<_, i64>(/*idx*/ 7)).unwrap_or(0),
        source_run_id: row.get(/*idx*/ 8),
        run_status: row.get(/*idx*/ 9),
        chapter_id: row.get(/*idx*/ 10),
        head_revision_id: row.get(/*idx*/ 11),
        run_agent_id: row.get(/*idx*/ 12),
        conversation_id: row.get(/*idx*/ 13),
        memory_settings_revision: row.get(/*idx*/ 14),
        grant_id: row.get(/*idx*/ 15),
        binding_revision: row.get(/*idx*/ 16),
        author_message: row.get(/*idx*/ 17),
    })
}
