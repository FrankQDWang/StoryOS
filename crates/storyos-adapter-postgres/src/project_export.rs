use storyos_application::{
    ArchiveExportOperation, ArchiveExportRefusal, CanonicalSnapshot, ExportOperationPage,
    ExportOperationProgress, ExportOperationReader, ExportProjectArchiveAdmission,
    ExportProjectArchiveError, ExportProjectArchiveInput, GetExportOperation,
    PROJECT_EXPORT_ARCHIVE_PATH_PROFILE, PROJECT_EXPORT_ARCHIVE_PROFILE, PinnedArchiveFamily,
    ProjectCommandEnvelope, ProjectCommandError, ProjectReadError, ProjectScope,
    RefusableCommandError,
};
use storyos_core::{
    ExportProjectArchive as CoreExport, ExportProjectArchiveResult, ProjectPresence,
    export_project_archive as classify_export,
};
use tokio_postgres::Client;

use super::*;
use crate::command_replay::{CommandReplay, EffectRecord, ReplayFault};
use crate::command_sequence::{
    Admission, AdmitCommand, AdmitSpec, CommandIsolation, LockedProject, MissingAdmission,
    ProjectActionClass, ProjectResponse, RateLimitedChallenge, admit_project_command, unavailable,
};
use crate::pinned_export_source::check_export_settlement;
use crate::project_archive_build::{
    ArchiveBuildError, collect_exportable_families, reload_admission_families,
};

impl PostgresProjectReader {
    /// Admits one exportProjectArchive. An archived Project and each archive build refusal are
    /// refusals before Admission. The Worker settles the admitted operation later.
    pub async fn export_project_archive(
        &self,
        envelope: &ProjectCommandEnvelope,
        input: &ExportProjectArchiveInput,
    ) -> Result<ExportProjectArchiveAdmission, ExportProjectArchiveError> {
        admit_project_command(self, envelope, input).await
    }
}

/// The pinned Snapshot and the exportable families that the export collects before its
/// Admission.
pub(crate) struct ArchiveExportFacts {
    snapshot: CanonicalSnapshot,
    families: Vec<PinnedArchiveFamily>,
}

impl AdmitCommand for ExportProjectArchiveInput {
    const SPEC: AdmitSpec = AdmitSpec {
        kind: "exportProjectArchive",
        isolation: CommandIsolation::Serializable,
        missing_admission: MissingAdmission::InvalidChallenge,
        rate_limited: RateLimitedChallenge::Unavailable,
        work_query: ARCHIVE_EXPORT_WORK,
    };
    type Error = ExportProjectArchiveError;
    type Response = ProjectResponse;
    type Facts = ArchiveExportFacts;
    type Work = ArchiveExportOperation;

    async fn load_facts(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        project: &LockedProject,
    ) -> Result<ArchiveExportFacts, ExportProjectArchiveError> {
        match classify_export(&CoreExport {
            presence: ProjectPresence::Present,
            current_lifecycle: project.lifecycle,
        }) {
            ExportProjectArchiveResult::Admitted => {}
            ExportProjectArchiveResult::Refused { reason } => {
                return Err(RefusableCommandError::RefusedBeforeAdmission(
                    ArchiveExportRefusal::Lifecycle(reason),
                ));
            }
        }
        let scope = &envelope.project_scope;
        let snapshot = crate::snapshot::load_latest_canonical_snapshot(client, scope)
            .await
            .map_err(unavailable)?
            .ok_or_else(|| {
                unavailable("canonical snapshot is required for Project Export admission")
            })?;
        let families =
            collect_exportable_families(client, scope)
                .await
                .map_err(|error| match error {
                    ArchiveBuildError::Refused(reason) => {
                        RefusableCommandError::RefusedBeforeAdmission(
                            ArchiveExportRefusal::ArchiveBuild(reason),
                        )
                    }
                    ArchiveBuildError::Unavailable(source) => {
                        ProjectCommandError::Unavailable(source).into()
                    }
                })?;
        Ok(ArchiveExportFacts { snapshot, families })
    }

    fn admission(&self, _facts: &ArchiveExportFacts) -> Admission {
        Admission::Project(ProjectActionClass::ExplicitProjectCommand)
    }

    async fn write_work(
        &self,
        client: &Client,
        envelope: &ProjectCommandEnvelope,
        facts: ArchiveExportFacts,
    ) -> Result<ArchiveExportOperation, ProjectCommandError> {
        let scope = &envelope.project_scope;
        client
            .execute(
                "INSERT INTO storyos.project_export_operations
                   (owner_user_id, project_id, export_id, author_command_admission_id, command_id,
                    command_kind, idempotency_key, source_snapshot_id, source_activity_position,
                    archive_profile, archive_path_profile)
                 VALUES ($1::text::uuid, $2::text::uuid, $3::text::uuid, $4::text::uuid,
                         $5::text::uuid, 'exportProjectArchive', $6::text::uuid, $7::text::uuid,
                         $8::text::bigint, $9, $10)",
                &[
                    &scope.owner_user_id.as_ref(),
                    &scope.project_id.as_ref(),
                    &self.export_id,
                    &envelope.ids.author_command_admission_id,
                    &envelope.ids.command_id,
                    &envelope.challenge_binding.idempotency_key,
                    &facts.snapshot.snapshot_id,
                    &facts.snapshot.project_activity_position.to_string(),
                    &PROJECT_EXPORT_ARCHIVE_PROFILE,
                    &PROJECT_EXPORT_ARCHIVE_PATH_PROFILE,
                ],
            )
            .await
            .map_err(unavailable)?;
        let mut families = facts.families;
        reload_admission_families(client, scope, &mut families)
            .await
            .map_err(unavailable)?;
        crate::pinned_export_source::insert_archive_pinned_export_source(
            client,
            scope,
            &self.export_id,
            &facts.snapshot.snapshot_id,
            &families,
        )
        .await?;
        Ok(ArchiveExportOperation {
            export_id: self.export_id.clone(),
            archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
            archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
            source_snapshot: facts.snapshot,
        })
    }

    fn decode_work(&self, work: &EffectRecord) -> Result<ArchiveExportOperation, ReplayFault> {
        Ok(ArchiveExportOperation {
            export_id: work.required("export_id")?,
            archive_profile: work.required("archive_profile")?,
            archive_path_profile: work.required("archive_path_profile")?,
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

/// The admitted operation, its two profiles, and its pinned Snapshot with the current replay
/// floor.
const ARCHIVE_EXPORT_WORK: &str = "SELECT json_build_object(
        'export_id', operation.export_id::text,
        'archive_profile', operation.archive_profile,
        'archive_path_profile', operation.archive_path_profile,
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
   FROM storyos.project_export_operations AS operation
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

impl ExportOperationReader for PostgresProjectReader {
    async fn read_export_operation(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<GetExportOperation, ProjectReadError> {
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
            return Ok(GetExportOperation::Missing);
        };
        let archived = project.get::<_, String>(0) == "archived";
        let ready = transaction
            .query_opt(
                "SELECT export.export_id::text,
                        export.archive_profile,
                        export.archive_path_profile,
                        export.immutable_root,
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
                   FROM storyos.project_export_manifests AS export
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
                    AND export.export_id = $3::text::uuid
                    AND export.immutable_root IS NOT NULL",
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
                return Ok(GetExportOperation::Expired);
            }
            transaction.commit().await.map_err(read_error)?;
            return Ok(GetExportOperation::Ready(Box::new(ExportOperationPage {
                project_scope: scope.clone(),
                export_id: row.get(0),
                archive_profile: row.get(1),
                archive_path_profile: row.get(2),
                immutable_root: row.get(3),
                source_snapshot: snapshot_from_joined_row(&row, /*start*/ 4)?,
            })));
        }
        let Some(operation) = transaction
            .query_opt(
                "SELECT operation.export_id::text,
                        operation.archive_profile,
                        operation.archive_path_profile,
                        operation.source_snapshot_id::text,
                        operation.claim_generation,
                        operation.settled_result,
                        operation.lease_expires_at IS NOT NULL
                          AND operation.lease_expires_at <= clock_timestamp()
                   FROM storyos.project_export_operations AS operation
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
                GetExportOperation::Archived
            } else {
                GetExportOperation::Missing
            });
        };
        let snapshot_id = operation.get::<_, String>(3);
        let source_snapshot =
            crate::snapshot::load_canonical_snapshot_by_id(&transaction, scope, &snapshot_id)
                .await?
                .ok_or_else(|| {
                    ProjectReadError::unavailable(std::io::Error::other(
                        "pinned Snapshot is required for an admitted Project Export",
                    ))
                })?;
        let progress = ExportOperationProgress {
            project_scope: scope.clone(),
            export_id: operation.get(0),
            archive_profile: operation.get(1),
            archive_path_profile: operation.get(2),
            source_snapshot,
        };
        let settled_result = operation.get::<_, Option<String>>(5);
        let claim_generation: i64 = operation.get(4);
        let lease_expired: bool = operation.get(6);
        let status = match (settled_result.as_deref(), claim_generation, lease_expired) {
            (Some("failed"), _, _) => GetExportOperation::Failed(Box::new(progress)),
            (None, generation, true) if generation > 0 => {
                GetExportOperation::OutcomeUnknown(Box::new(progress))
            }
            (None, _, _) => GetExportOperation::InProgress(Box::new(progress)),
            (Some(_), _, _) => {
                return Err(ProjectReadError::unavailable(std::io::Error::other(
                    "unsupported Project Export settled_result",
                )));
            }
        };
        transaction.commit().await.map_err(read_error)?;
        Ok(status)
    }

    async fn read_verified_export_archive(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<storyos_application::VerifiedExportArchive, ProjectReadError> {
        match self.read_export_operation(scope, export_id).await? {
            GetExportOperation::Missing => Ok(storyos_application::VerifiedExportArchive::Missing),
            GetExportOperation::Archived => {
                Ok(storyos_application::VerifiedExportArchive::Archived)
            }
            GetExportOperation::Expired => Ok(storyos_application::VerifiedExportArchive::Expired),
            GetExportOperation::InProgress(_)
            | GetExportOperation::Failed(_)
            | GetExportOperation::OutcomeUnknown(_) => {
                Ok(storyos_application::VerifiedExportArchive::Unsettled)
            }
            GetExportOperation::Ready(page) => {
                let mut client = self.connect().await?;
                let transaction = client.transaction().await.map_err(read_error)?;
                set_scope(&transaction, scope).await?;
                let archive =
                    crate::project_archive_build::package_stored_export(&transaction, scope, &page)
                        .await;
                match &archive {
                    Ok(_) => transaction.commit().await.map_err(read_error)?,
                    Err(_) => {
                        let _rollback = transaction.rollback().await;
                    }
                }
                archive
            }
        }
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
