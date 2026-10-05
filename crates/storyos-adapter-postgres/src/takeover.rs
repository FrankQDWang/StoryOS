use std::convert::Infallible;

use storyos_application::{
    ProjectCommandEnvelope, ProjectCommandError, TakeOverProjectWriterInput,
    TakeOverProjectWriterSettlement, WriterTakeover,
};
use storyos_core::{
    CurrentWriter, TakeOverProjectWriter as CoreTakeOverProjectWriter,
    TakeOverProjectWriterNoEffect, take_over_project_writer,
};
use tokio_postgres::Client;
use uuid::Uuid;

use crate::PostgresProjectReader;
use crate::command_replay::{CommandReplay, ReplayFault};
use crate::command_sequence::{
    ActivityOnly, ActivitySequences, ActivityWrite, Admission, AppliedResult, Classification,
    CommandIsolation, CommandSpec, LockedProject, MissingAdmission, NoResponse, ProjectCommand,
    RateLimitedChallenge, ReceiptHeads, ReplayEffect, TakeoverAdmission, ZeroAuthorityRows,
    ZeroAuthorityWrite, ZeroOutcome, settle_project_command, unavailable,
};

impl PostgresProjectReader {
    /// Settles one writer takeover as a zero-authority outcome with its writer effect rows.
    pub async fn take_over_project_writer(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &TakeOverProjectWriterInput,
    ) -> Result<TakeOverProjectWriterSettlement, ProjectCommandError> {
        settle_project_command(self, envelope, input).await
    }
}

/// The current writer and the Current Chapter head that a takeover moves away from.
struct TakeoverFacts {
    writer_generation: Option<u64>,
    writer_session_id: Option<String>,
    current_chapter_id: Option<String>,
    current_head: Option<String>,
}

async fn load_takeover_facts(
    client: &Client,
    envelope: &ProjectCommandEnvelope,
) -> Result<TakeoverFacts, ProjectCommandError> {
    let row = client
        .query_one(
            "SELECT writer.writer_generation::text, writer.current_editor_session_id::text,
                    project.current_chapter_id::text, head.current_revision_id::text
               FROM storyos.projects AS project
          LEFT JOIN LATERAL (
                    SELECT current.writer_generation, current.current_editor_session_id
                      FROM storyos.project_writer_generations AS current
                     WHERE (current.owner_user_id, current.project_id) =
                           (project.owner_user_id, project.project_id)
                     ORDER BY current.writer_generation DESC
                     LIMIT 1
               ) AS writer ON true
          LEFT JOIN storyos.authoritative_heads AS head
                 ON (head.owner_user_id, head.project_id, head.manuscript_object_id) =
                    (project.owner_user_id, project.project_id, project.current_chapter_id)
              WHERE project.owner_user_id = $1::text::uuid
                AND project.project_id = $2::text::uuid",
            &[
                &envelope.project_scope.owner_user_id.as_ref(),
                &envelope.project_scope.project_id.as_ref(),
            ],
        )
        .await
        .map_err(unavailable)?;
    Ok(TakeoverFacts {
        writer_generation: row
            .get::<_, Option<String>>(/*idx*/ 0)
            .map(|generation| generation.parse())
            .transpose()
            .map_err(unavailable)?,
        writer_session_id: row.get(/*idx*/ 1),
        current_chapter_id: row.get(/*idx*/ 2),
        current_head: row.get(/*idx*/ 3),
    })
}

impl ProjectCommand for TakeOverProjectWriterInput {
    const SPEC: CommandSpec = CommandSpec {
        kind: "takeOverProjectWriter",
        applied_result: AppliedResult::AUTHORITATIVE_APPLIED,
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::BindingConflict,
        rate_limited: RateLimitedChallenge::Unavailable,
        activity_kind: "writer_takeover_applied",
        replay_effect: ReplayEffect::NoQuery,
    };
    type Profile = ActivityOnly;
    type Response = NoResponse;
    type ZeroEffect = WriterTakeover;
    type Applied = Infallible;
    type Plan = ();
    type Effect = Infallible;
    type NoEffect = TakeOverProjectWriterNoEffect;
    type Conflict = Infallible;
    type Refusal = Infallible;

    async fn classify(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<Classification<Self>, ProjectCommandError> {
        let facts = load_takeover_facts(client, envelope).await?;
        let outcome = take_over_project_writer(&CoreTakeOverProjectWriter {
            observed_writer_generation: self.observed_writer_generation,
            current_writer: facts
                .writer_generation
                .map(|writer_generation| CurrentWriter {
                    writer_generation,
                    is_requesting_session: facts.writer_session_id.as_deref()
                        == Some(self.editor_session_id.as_ref()),
                }),
            lifecycle: project.lifecycle,
            current_chapter_has_head: facts.current_head.is_some(),
        })
        .map_err(|_| ProjectCommandError::BindingConflict)?;
        let head = facts.current_head.unwrap_or_default();
        Ok(Classification {
            outcome: outcome.map_applied(|applied| match applied {}),
            admission: Admission::WriterTakeover(TakeoverAdmission {
                editor_session_id: self.editor_session_id.as_ref().to_owned(),
                observed_writer_generation: self.observed_writer_generation,
                editor_contract_revision: self.editor_contract_revision.clone(),
            }),
            heads: ReceiptHeads {
                expected: vec![head.clone()],
                prior: vec![head.clone()],
                resulting: vec![head],
            },
        })
    }

    async fn apply(
        &self,
        _client: &Client,
        _envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        _sequences: &ActivitySequences,
        _plan: (),
        applied: Infallible,
    ) -> Result<ActivityWrite<Infallible>, ProjectCommandError> {
        match applied {}
    }

    fn decode(&self, _replay: &CommandReplay) -> Result<Infallible, ReplayFault> {
        Err(ReplayFault::BindingConflict)
    }

    fn zero_authority_rows(&self, outcome: &ZeroOutcome<'_, Self>) -> ZeroAuthorityRows {
        match outcome {
            ZeroOutcome::NoEffect(TakeOverProjectWriterNoEffect::WriterTakeoverApplied) => {
                ZeroAuthorityRows::EffectWithActivity
            }
            ZeroOutcome::Conflicted(reason) => match **reason {},
            ZeroOutcome::Refused(reason) => match **reason {},
        }
    }

    async fn write_zero_authority_activity(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        _project: &LockedProject,
        activity: &ActivitySequences,
    ) -> Result<ZeroAuthorityWrite<WriterTakeover>, ProjectCommandError> {
        let scope = &envelope.project_scope;
        let facts = load_takeover_facts(client, envelope).await?;
        let (
            Some(prior_writer_generation),
            Some(prior_editor_session_id),
            Some(current_chapter_id),
            Some(current_head),
        ) = (
            facts.writer_generation,
            facts.writer_session_id,
            facts.current_chapter_id,
            facts.current_head,
        )
        else {
            return Err(ProjectCommandError::BindingConflict);
        };
        let resulting_writer_generation = prior_writer_generation
            .checked_add(/*rhs*/ 1)
            .ok_or(ProjectCommandError::BindingConflict)?;
        let generation_inserts = client
            .execute(
                "INSERT INTO storyos.project_writer_generations
                   (owner_user_id, project_id, writer_generation, current_editor_session_id)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::numeric, $4::text::uuid)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &resulting_writer_generation.to_string(),
                    &self.editor_session_id.as_ref(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if generation_inserts != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let base_updates = client
            .execute(
                "UPDATE storyos.editor_session_base_snapshots AS snapshot
                    SET snapshot_id = $4::text::uuid,
                        chapter_object_id = $5::text::uuid,
                        authoritative_revision_id = $6::text::uuid,
                        project_activity_position = $7::text::numeric,
                        created_at = clock_timestamp()
                   FROM storyos.project_writer_generations AS writer
                  WHERE snapshot.owner_user_id = $1::text::uuid
                    AND snapshot.project_id = $2::text::uuid
                    AND snapshot.editor_session_id = $3::text::uuid
                    AND (writer.owner_user_id, writer.project_id,
                         writer.current_editor_session_id, writer.writer_generation) =
                        (snapshot.owner_user_id, snapshot.project_id,
                         snapshot.editor_session_id, $8::text::numeric)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.editor_session_id.as_ref(),
                    &Uuid::now_v7().to_string(),
                    &current_chapter_id,
                    &current_head,
                    &activity.project_activity_position.to_string(),
                    &resulting_writer_generation.to_string(),
                ],
            )
            .await
            .map_err(unavailable)?;
        if base_updates != 1 {
            return Err(ProjectCommandError::BindingConflict);
        }
        let snapshot_id = Uuid::now_v7().to_string();
        crate::snapshot::persist_canonical_snapshot(
            client,
            scope,
            &snapshot_id,
            activity.project_activity_position,
        )
        .await
        .map_err(unavailable)?;
        let effect = WriterTakeover {
            prior_editor_session_id,
            prior_writer_generation,
            resulting_editor_session_id: self.editor_session_id.as_ref().to_owned(),
            resulting_writer_generation,
            resulting_snapshot_id: snapshot_id,
            resulting_snapshot_activity_position: activity.project_activity_position,
            resulting_head: current_head,
        };
        Ok(ZeroAuthorityWrite {
            activity: serde_json::json!({
                "kind": "takeover_applied",
                "prior_editor_session_id": effect.prior_editor_session_id,
                "prior_writer_generation": self.observed_writer_generation.to_string(),
                "resulting_editor_session_id": effect.resulting_editor_session_id,
                "resulting_writer_generation": effect.resulting_writer_generation.to_string(),
                "resulting_snapshot_id": effect.resulting_snapshot_id,
                "resulting_snapshot_activity_position":
                    effect.resulting_snapshot_activity_position.to_string(),
                "resulting_heads": [effect.resulting_head],
            }),
            effect,
        })
    }

    fn decode_zero_authority_effect(
        &self,
        replay: &CommandReplay,
    ) -> Result<Option<WriterTakeover>, ReplayFault> {
        if !replay.fence_digest_matches {
            return Err(ReplayFault::BindingConflict);
        }
        let text = |key: &str| {
            replay.activity_optional_text(key).ok_or_else(|| {
                ReplayFault::Unavailable(format!("the takeover Activity has no {key}").into())
            })
        };
        let number = |key: &str| -> Result<u64, ReplayFault> {
            text(key)?
                .parse()
                .map_err(|error| ReplayFault::Unavailable(Box::new(error)))
        };
        let [resulting_head] =
            <[String; 1]>::try_from(replay.resulting_heads.clone()).map_err(|_| {
                ReplayFault::Unavailable("the takeover Receipt has no single head".into())
            })?;
        Ok(Some(WriterTakeover {
            prior_editor_session_id: text("prior_editor_session_id")?,
            prior_writer_generation: number("prior_writer_generation")?,
            resulting_editor_session_id: text("resulting_editor_session_id")?,
            resulting_writer_generation: number("resulting_writer_generation")?,
            resulting_snapshot_id: text("resulting_snapshot_id")?,
            resulting_snapshot_activity_position: number("resulting_snapshot_activity_position")?,
            resulting_head,
        }))
    }
}
