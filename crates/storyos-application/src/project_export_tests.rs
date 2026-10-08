use super::*;
use crate::{ProjectId, UserId};

struct Reader {
    page: ExportOperationPage,
}

impl ExportOperationReader for Reader {
    async fn read_export_operation(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<GetExportOperation, ProjectReadError> {
        if scope != &self.page.project_scope || export_id != self.page.export_id {
            return Ok(GetExportOperation::Missing);
        }
        Ok(GetExportOperation::Ready(Box::new(self.page.clone())))
    }

    async fn read_verified_export_archive(
        &self,
        scope: &ProjectScope,
        export_id: &str,
    ) -> Result<VerifiedExportArchive, ProjectReadError> {
        match self.read_export_operation(scope, export_id).await? {
            GetExportOperation::Missing => Ok(VerifiedExportArchive::Missing),
            GetExportOperation::Archived => Ok(VerifiedExportArchive::Archived),
            GetExportOperation::Expired => Ok(VerifiedExportArchive::Expired),
            GetExportOperation::InProgress(_)
            | GetExportOperation::Failed(_)
            | GetExportOperation::OutcomeUnknown(_) => Ok(VerifiedExportArchive::Unsettled),
            GetExportOperation::Ready(_) => {
                Ok(VerifiedExportArchive::Ready(b"PK\x03\x04".to_vec()))
            }
        }
    }
}

fn snapshot() -> CanonicalSnapshot {
    CanonicalSnapshot {
        snapshot_id: "snapshot".to_owned(),
        project_activity_position: 2,
        replay_generation: 1,
        floor_position: 0,
        redaction_profile: "storyos.author.v1".to_owned(),
        schema_profile: "storyos.public.release.1".to_owned(),
        created_at: "2026-09-01T00:00:00.000Z".to_owned(),
        expires_at: None,
    }
}

fn owned_scope() -> ProjectScope {
    ProjectScope::new(UserId::new("user"), ProjectId::new("project"))
}

#[tokio::test]
async fn get_export_operation_reports_ready_only_with_an_immutable_root() {
    let page = ExportOperationPage {
        project_scope: owned_scope(),
        export_id: "export".to_owned(),
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
        source_snapshot: snapshot(),
        immutable_root: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .to_owned(),
    };
    let got = get_export_operation(&Reader { page: page.clone() }, &owned_scope(), "export")
        .await
        .unwrap();
    assert_eq!(got, GetExportOperation::Ready(Box::new(page)));
}

#[tokio::test]
async fn verified_archive_bytes_are_refused_while_in_progress() {
    struct ProgressReader {
        progress: ExportOperationProgress,
    }
    impl ExportOperationReader for ProgressReader {
        async fn read_export_operation(
            &self,
            scope: &ProjectScope,
            export_id: &str,
        ) -> Result<GetExportOperation, ProjectReadError> {
            if scope != &self.progress.project_scope || export_id != self.progress.export_id {
                return Ok(GetExportOperation::Missing);
            }
            Ok(GetExportOperation::InProgress(Box::new(
                self.progress.clone(),
            )))
        }

        async fn read_verified_export_archive(
            &self,
            scope: &ProjectScope,
            export_id: &str,
        ) -> Result<VerifiedExportArchive, ProjectReadError> {
            match self.read_export_operation(scope, export_id).await? {
                GetExportOperation::InProgress(_)
                | GetExportOperation::Failed(_)
                | GetExportOperation::OutcomeUnknown(_) => Ok(VerifiedExportArchive::Unsettled),
                GetExportOperation::Missing => Ok(VerifiedExportArchive::Missing),
                GetExportOperation::Archived => Ok(VerifiedExportArchive::Archived),
                GetExportOperation::Expired => Ok(VerifiedExportArchive::Expired),
                GetExportOperation::Ready(_) => {
                    Ok(VerifiedExportArchive::Ready(b"PK\x03\x04".to_vec()))
                }
            }
        }
    }
    let progress = ExportOperationProgress {
        project_scope: owned_scope(),
        export_id: "export".to_owned(),
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
        source_snapshot: snapshot(),
    };
    assert_eq!(
        get_verified_export_archive(&ProgressReader { progress }, &owned_scope(), "export")
            .await
            .unwrap(),
        VerifiedExportArchive::Unsettled
    );
}
