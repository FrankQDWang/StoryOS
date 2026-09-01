use std::sync::Mutex;

use super::*;
use crate::{ProjectId, UserId};

struct Store(Mutex<usize>);

impl ExportProjectArchiveStore for Store {
    async fn export_project_archive(
        &self,
        command: &ExportProjectArchiveCommand,
    ) -> Result<ExportProjectArchiveSettlement, ExportProjectArchiveError> {
        *self.0.lock().unwrap() += 1;
        Ok(ExportProjectArchiveSettlement {
            ids: command.ids.clone(),
            export_id: command.export_id.clone(),
            effect: ExportProjectArchiveSettlementEffect::Admitted {
                archive_profile: command.archive_profile.clone(),
                archive_path_profile: command.archive_path_profile.clone(),
                source_snapshot: command.source_snapshot.clone(),
            },
            receipt_created_at: "2026-09-01T00:00:00.000Z".to_owned(),
            project_activity_position: 3,
            project_activity_event_id: "event".to_owned(),
        })
    }
}

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
        Ok(GetExportOperation::InProgress(Box::new(self.page.clone())))
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

fn command() -> ExportProjectArchiveCommand {
    let project_scope = owned_scope();
    let client_binding = EditorClientBinding {
        binding_ref: "binding".to_owned(),
        session_generation: 1,
        client_contract_revision: "client".to_owned(),
        security_policy_revision: "security".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.export-project-archive.request.v1"}"#.to_vec();
    let command_digest = {
        use sha2::{Digest as _, Sha256};
        let value = Sha256::digest(&canonical_command_bytes).iter().fold(
            String::with_capacity(64),
            |mut value, byte| {
                use std::fmt::Write as _;
                write!(value, "{byte:02x}").expect("writing to String cannot fail");
                value
            },
        );
        format!("sha256:{PROJECT_EXPORT_DIGEST_PROFILE}:{value}")
    };
    ExportProjectArchiveCommand {
        project_scope: project_scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope,
            client_session_binding_digest: client_binding.binding_ref,
            client_session_generation: 1,
            client_contract_revision: "client".to_owned(),
            security_policy_revision: "security".to_owned(),
            limit_profile_revision: "limit".to_owned(),
            challenge_rate_policy_revision: "rate".to_owned(),
            method: "POST".to_owned(),
            route_template: PROJECT_EXPORT_ROUTE.to_owned(),
            command_schema: PROJECT_EXPORT_REQUEST_SCHEMA.to_owned(),
            command_kind: PROJECT_EXPORT_COMMAND_KIND.to_owned(),
            canonical_command_digest: command_digest,
            idempotency_key: "key".to_owned(),
        },
        nonce_digest: "nonce".to_owned(),
        canonical_command_bytes,
        correlation_id: "correlation".to_owned(),
        ids: AuthorCommandAdmissionIds {
            command_id: "command".to_owned(),
            author_command_admission_id: "admission".to_owned(),
            receipt_id: "receipt".to_owned(),
        },
        export_id: "export".to_owned(),
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
        source_snapshot: snapshot(),
    }
}

#[tokio::test]
async fn an_exact_export_binding_reaches_the_store() {
    let store = Store(Mutex::new(0));
    let settlement = request_export_project_archive(&store, &command())
        .await
        .unwrap();
    assert_eq!(*store.0.lock().unwrap(), 1);
    assert_eq!(settlement.export_id, "export");
    assert_eq!(
        settlement.effect,
        ExportProjectArchiveSettlementEffect::Admitted {
            archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
            archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
            source_snapshot: snapshot(),
        }
    );
}

#[tokio::test]
async fn a_changed_export_binding_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.challenge_binding.command_kind = "createVolume".to_owned();
    assert!(matches!(
        request_export_project_archive(&store, &changed).await,
        Err(ExportProjectArchiveError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn a_changed_format_profile_is_refused_before_the_store() {
    let store = Store(Mutex::new(0));
    let mut changed = command();
    changed.archive_profile = "storyos.project-export.other.v1".to_owned();
    assert!(matches!(
        request_export_project_archive(&store, &changed).await,
        Err(ExportProjectArchiveError::BindingConflict)
    ));
    assert_eq!(*store.0.lock().unwrap(), 0);
}

#[tokio::test]
async fn get_export_operation_does_not_report_success_without_an_immutable_root() {
    let page = ExportOperationPage {
        project_scope: owned_scope(),
        export_id: "export".to_owned(),
        archive_profile: PROJECT_EXPORT_ARCHIVE_PROFILE.to_owned(),
        archive_path_profile: PROJECT_EXPORT_ARCHIVE_PATH_PROFILE.to_owned(),
        source_snapshot: snapshot(),
        immutable_root: None,
    };
    let got = get_export_operation(&Reader { page: page.clone() }, &owned_scope(), "export")
        .await
        .unwrap();
    assert_eq!(got, GetExportOperation::InProgress(Box::new(page.clone())));
    match got {
        GetExportOperation::InProgress(page) => assert_eq!(page.immutable_root, None),
        GetExportOperation::Missing | GetExportOperation::Archived => {
            panic!("an admitted export must stay inspectable")
        }
    }
}
