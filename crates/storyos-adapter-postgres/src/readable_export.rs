use storyos_application::{
    CanonicalSnapshot, ExportHumanReadableManuscriptAdmission, ExportHumanReadableManuscriptError,
    ExportHumanReadableManuscriptInput, GetHumanReadableManuscriptExport,
    HumanReadableManuscriptExportPage, HumanReadableManuscriptExportProgress,
    HumanReadableManuscriptExportReader, ProjectCommandEnvelope, ProjectCommandError,
    ProjectReadError, ProjectScope, ReadableExportOperation, RefusableCommandError,
    readable_volumes_from_canonical_facts,
};
use storyos_core::{
    ExportHumanReadableManuscript as CoreExport, ExportHumanReadableManuscriptResult,
    ProjectPresence, READABLE_EXPORT_PROFILE, ReadableExportVolume,
    export_human_readable_manuscript as classify_export,
};
use tokio_postgres::Client;

use super::*;
use crate::command_replay::{CommandReplay, EffectRecord, ReplayFault};
use crate::command_sequence::{
    Admission, AdmitCommand, AdmitSpec, CommandIsolation, LockedProject, MissingAdmission,
    ProjectActionClass, ProjectResponse, RateLimitedChallenge, admit_project_command, unavailable,
};
use crate::pinned_export_source::check_export_settlement;

impl PostgresProjectReader {
    /// Admits one exportHumanReadableManuscript. An archived Project is a refusal before
    /// Admission. The Worker settles the admitted operation later.
    pub async fn export_human_readable_manuscript(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ExportHumanReadableManuscriptInput,
    ) -> Result<ExportHumanReadableManuscriptAdmission, ExportHumanReadableManuscriptError> {
        admit_project_command(self, envelope, input).await
    }
}

/// The pinned Snapshot and the live manuscript that the export pins before its Admission.
pub(crate) struct ReadableExportFacts {
    snapshot: CanonicalSnapshot,
    volumes: Vec<ReadableExportVolume>,
}

impl AdmitCommand for ExportHumanReadableManuscriptInput {
    const SPEC: AdmitSpec = AdmitSpec {
        kind: "exportHumanReadableManuscript",
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        work_query: READABLE_EXPORT_WORK,
    };
    type Error = ExportHumanReadableManuscriptError;
    type Response = ProjectResponse;
    type Facts = ReadableExportFacts;
    type Work = ReadableExportOperation;

    async fn load_facts(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<ReadableExportFacts, ExportHumanReadableManuscriptError> {
        match classify_export(&CoreExport {
            presence: ProjectPresence::Present,
            current_lifecycle: project.lifecycle,
        }) {
            ExportHumanReadableManuscriptResult::Admitted => {}
            ExportHumanReadableManuscriptResult::Refused { reason } => {
                return Err(RefusableCommandError::RefusedBeforeAdmission(reason));
            }
        }
        let scope = &envelope.project_scope;
        let snapshot = crate::snapshot::load_latest_canonical_snapshot(client, scope)
            .await
            .map_err(unavailable)?
            .ok_or_else(|| {
                unavailable("canonical snapshot is required for human-readable export")
            })?;
        let tree = crate::manuscript_tree::load_live_tree_facts(client, scope, snapshot.clone())
            .await
            .map_err(unavailable)?;
        let chapters = crate::manuscript_search::read_live_chapter_blocks(
            client,
            scope,
            crate::manuscript_search::LiveChapterReadExtent::AllChapters,
        )
        .await
        .map_err(unavailable)?;
        Ok(ReadableExportFacts {
            volumes: readable_volumes_from_canonical_facts(&tree, &chapters),
            snapshot,
        })
    }

    fn admission(&self, _facts: &ReadableExportFacts) -> Admission {
        Admission::Project(ProjectActionClass::ExplicitProjectCommand)
    }

    async fn write_work(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        facts: ReadableExportFacts,
    ) -> Result<ReadableExportOperation, ProjectCommandError> {
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.human_readable_manuscript_export_operations
                   (owner_user_id, project_id, export_id, author_command_admission_id, command_id,
                    command_kind, idempotency_key, source_snapshot_id, source_activity_position,
                    export_profile)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, 'exportHumanReadableManuscript', $6::text::uuid,
                         $7::text::uuid, $8::text::bigint, $9)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.export_id,
                    &envelope.ids.author_command_admission_id,
                    &envelope.ids.command_id,
                    &envelope.challenge_binding.idempotency_key,
                    &facts.snapshot.snapshot_id,
                    &facts.snapshot.project_activity_position.to_string(),
                    &READABLE_EXPORT_PROFILE,
                ],
            )
            .await
            .map_err(unavailable)?;
        crate::pinned_export_source::insert_human_readable_pinned_export_source(
            client,
            scope,
            &self.export_id,
            &facts.snapshot.snapshot_id,
            &facts.volumes,
        )
        .await?;
        Ok(ReadableExportOperation {
            export_id: self.export_id.clone(),
            source_snapshot: facts.snapshot,
        })
    }

    fn decode_work(&self, work: &EffectRecord) -> Result<ReadableExportOperation, ReplayFault> {
        Ok(ReadableExportOperation {
            export_id: work.required("export_id")?,
            source_snapshot: CanonicalSnapshot {
                snapshot_id: work.required("snapshot_id")?,
                project_activity_position: work.required_u64("project_activity_position")?,
                replay_generation: work.required_u64("replay_generation")?,
                floor_position: work.required_u64("floor_position")?,
                redaction_profile: work.required("redaction_profile")?,
                schema_profile: work.required("schema_profile")?,
                created_at: work.required("created_at")?,
                expires_at: work.text("expires_at")?,
            },
        })
    }

    fn check_settlement(&self, replay: &CommandReplay) -> Result<(), ReplayFault> {
        check_export_settlement(replay)
    }
}

/// The admitted operation and its pinned Snapshot with the current replay floor.
const READABLE_EXPORT_WORK: &str = "SELECT json_build_object(
        'export_id', operation.export_id::text,
        'snapshot_id', snapshot.snapshot_id::text,
        'project_activity_position', snapshot.project_activity_position::text,
        'replay_generation', snapshot.replay_generation::text,
        'redaction_profile', snapshot.redaction_profile,
        'schema_profile', snapshot.schema_profile,
        'created_at', to_char(snapshot.created_at AT TIME ZONE 'UTC',
                              'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
        'expires_at', to_char(snapshot.expires_at AT TIME ZONE 'UTC',
                              'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
        'floor_position', current_floor.floor_position::text)::text
   FROM storyos.human_readable_manuscript_export_operations AS operation
LEFT JOIN storyos.project_snapshots AS snapshot
     ON (snapshot.owner_user_id, snapshot.project_id, snapshot.snapshot_id) =
        (operation.owner_user_id, operation.project_id, operation.source_snapshot_id)
LEFT JOIN LATERAL (
         SELECT replay_generation FROM storyos.replay_generations AS generation
          WHERE (generation.owner_user_id, generation.project_id) =
                (snapshot.owner_user_id, snapshot.project_id)
          ORDER BY generation.replay_generation DESC LIMIT 1
       ) AS current_generation ON true
LEFT JOIN storyos.replay_floors AS current_floor
     ON (current_floor.owner_user_id, current_floor.project_id,
         current_floor.replay_generation) =
        (snapshot.owner_user_id, snapshot.project_id, current_generation.replay_generation)
  WHERE operation.owner_user_id = $1::text::uuid
    AND operation.project_id = $2::text::uuid
    AND operation.author_command_admission_id = $3::text::uuid";

impl HumanReadableManuscriptExportReader for PostgresProjectReader {
    async fn read_human_readable_manuscript_export(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<GetHumanReadableManuscriptExport, ProjectReadError> {
        let mut client = self.connect().await?;
        // One snapshot: the read does not see a Worker settlement that commits between its reads.
        let transaction = client
            .build_transaction()
            .isolation_level(tokio_postgres::IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .await
            .map_err(read_error)?;
        set_scope(&transaction, scope).await?;
        let Some(project) = transaction
            .query_opt(
                "SELECT lifecycle_state FROM storyos.projects
                  WHERE owner_user_id = $1::text::uuid AND project_id = $2::text::uuid",
                &[&scope.owner_user_id.as_ref(), &scope.project_id.as_ref()],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Missing);
        };
        let archived = project.get::<_, String>(0) == "archived";
        let ready = transaction
            .query_opt(
                "SELECT export.export_id::text,
                        export.export_profile,
                        export.content_sha256,
                        export.manuscript_utf8,
                        snapshot.snapshot_id::text,
                        snapshot.project_activity_position::text,
                        snapshot.replay_generation::text,
                        snapshot.redaction_profile,
                        snapshot.schema_profile,
                        to_char(snapshot.created_at AT TIME ZONE 'UTC',
                                'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'),
                        CASE WHEN snapshot.expires_at IS NULL THEN NULL
                             ELSE to_char(snapshot.expires_at AT TIME ZONE 'UTC',
                                          'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')
                        END,
                        current_floor.floor_position::text,
                        snapshot.expires_at IS NOT NULL
                          AND snapshot.expires_at <= clock_timestamp()
                   FROM storyos.human_readable_manuscript_exports AS export
                   JOIN storyos.project_snapshots AS snapshot
                     ON snapshot.owner_user_id = export.owner_user_id
                    AND snapshot.project_id = export.project_id
                    AND snapshot.snapshot_id::text = export.source_snapshot_id
                   JOIN LATERAL (
                     SELECT replay_generation FROM storyos.replay_generations
                      WHERE owner_user_id = snapshot.owner_user_id
                        AND project_id = snapshot.project_id
                      ORDER BY replay_generation DESC LIMIT 1
                   ) AS current_generation ON true
                   JOIN storyos.replay_floors AS current_floor
                     ON (current_floor.owner_user_id, current_floor.project_id,
                         current_floor.replay_generation) =
                        (snapshot.owner_user_id, snapshot.project_id,
                         current_generation.replay_generation)
                  WHERE export.owner_user_id = $1::text::uuid
                    AND export.project_id = $2::text::uuid
                    AND export.export_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &export_id,
                ],
            )
            .await
            .map_err(read_error)?;
        if let Some(row) = ready {
            if row.get::<_, bool>(12) {
                transaction.commit().await.map_err(read_error)?;
                return Ok(GetHumanReadableManuscriptExport::Expired);
            }
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetHumanReadableManuscriptExport::Ready(Box::new(
                HumanReadableManuscriptExportPage {
                    project_scope: scope.clone(),
                    export_id: row.get(0),
                    export_profile: row.get(1),
                    content_sha256: row.get(2),
                    manuscript_utf8: row.get(3),
                    source_snapshot: snapshot_from_joined_row(&row, /*start*/ 4)?,
                },
            )));
        }
        let Some(operation) = transaction
            .query_opt(
                "SELECT operation.export_id::text,
                        operation.export_profile,
                        operation.source_snapshot_id::text,
                        operation.claim_generation,
                        operation.settled_result,
                        operation.lease_expires_at IS NOT NULL
                          AND operation.lease_expires_at <= clock_timestamp()
                   FROM storyos.human_readable_manuscript_export_operations AS operation
                  WHERE operation.owner_user_id = $1::text::uuid
                    AND operation.project_id = $2::text::uuid
                    AND operation.export_id = $3::text::uuid",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &export_id,
                ],
            )
            .await
            .map_err(read_error)?
        else {
            transaction.commit().await.map_err(read_error)?;
            return Ok(if archived {
                GetHumanReadableManuscriptExport::Archived
            } else {
                GetHumanReadableManuscriptExport::Missing
            });
        };
        let snapshot_id = operation.get::<_, String>(2);
        let source_snapshot =
            crate::snapshot::load_canonical_snapshot_by_id(&transaction, scope, &snapshot_id)
                .await?
                .ok_or_else(|| {
                    ProjectReadError::unavailable(std::io::Error::other(
                        "pinned Snapshot is required for an admitted human-readable export",
                    ))
                })?;
        let progress = HumanReadableManuscriptExportProgress {
            project_scope: scope.clone(),
            export_id: operation.get(0),
            export_profile: operation.get(1),
            source_snapshot,
        };
        let settled_result = operation.get::<_, Option<String>>(4);
        let claim_generation: i64 = operation.get(3);
        let lease_expired: bool = operation.get(5);
        let status = match (settled_result.as_deref(), claim_generation, lease_expired) {
            (Some("failed"), _, _) => GetHumanReadableManuscriptExport::Failed(Box::new(progress)),
            (None, generation, true) if generation > 0 => {
                GetHumanReadableManuscriptExport::OutcomeUnknown(Box::new(progress))
            }
            (None, _, _) => GetHumanReadableManuscriptExport::InProgress(Box::new(progress)),
            (Some(_), _, _) => {
                return Err(ProjectReadError::unavailable(std::io::Error::other(
                    "unsupported human-readable export settled_result",
                )));
            }
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(status)
    }
}

fn snapshot_from_joined_row(
    row: &tokio_postgres::Row,
    start: usize,
) -> Result<CanonicalSnapshot, ProjectReadError> {
    Ok(CanonicalSnapshot {
        snapshot_id: row.get(start),
        project_activity_position: row
            .get::<_, String>(start + 1)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        replay_generation: row
            .get::<_, String>(start + 2)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
        redaction_profile: row.get(start + 3),
        schema_profile: row.get(start + 4),
        created_at: row.get(start + 5),
        expires_at: row.get(start + 6),
        floor_position: row
            .get::<_, String>(start + 7)
            .parse()
            .map_err(ProjectReadError::unavailable)?,
    })
}
