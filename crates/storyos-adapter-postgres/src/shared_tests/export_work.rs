//! The shared test support from `export_work_tests.rs`.

use crate::PostgresProjectReader;
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, ExportHumanReadableManuscriptInput,
    IssueProjectCommandChallenge, ProjectCommandChallengeBinding, ProjectCommandEnvelope,
    ProjectScope, issue_project_command_challenge,
};
use storyos_contracts::CommandKind;

pub(crate) const PROJECT_EXPORT_COMMAND_KIND: &str = CommandKind::ExportProjectArchive.as_str();

pub(crate) const PROJECT_EXPORT_ROUTE: &str = "/api/v1/projects/{project_id}/exports";

pub(crate) const PROJECT_EXPORT_REQUEST_SCHEMA: &str =
    "storyos.command.export-project-archive.request.v1";

pub(crate) const PROJECT_EXPORT_DIGEST_PROFILE: &str =
    "storyos.command.exportProjectArchive.jcs.v1";

pub(crate) const HUMAN_READABLE_EXPORT_COMMAND_KIND: &str =
    CommandKind::ExportHumanReadableManuscript.as_str();

pub(crate) const HUMAN_READABLE_EXPORT_ROUTE: &str =
    "/api/v1/projects/{project_id}/manuscript/exports";

pub(crate) const HUMAN_READABLE_EXPORT_REQUEST_SCHEMA: &str =
    "storyos.command.export-human-readable-manuscript.request.v1";

pub(crate) const HUMAN_READABLE_EXPORT_DIGEST_PROFILE: &str =
    "storyos.command.exportHumanReadableManuscript.jcs.v1";

pub(crate) async fn admit_readable_export(
    store: &PostgresProjectReader,
    scope: &ProjectScope,
) -> String {
    let (envelope, input) = readable_export_command(scope);
    issue_project_command_challenge(
        store,
        &IssueProjectCommandChallenge {
            binding: envelope.challenge_binding.clone(),
            nonce: envelope
                .nonce_digest
                .trim_start_matches("sha256:")
                .to_owned(),
            nonce_digest: envelope.nonce_digest.clone(),
            earlier_revision_nonces: Vec::new(),
        },
    )
    .await
    .unwrap();
    store
        .export_human_readable_manuscript(&envelope, &input)
        .await
        .unwrap();
    input.export_id
}

pub(crate) fn fresh_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

pub(crate) fn readable_export_command(
    scope: &ProjectScope,
) -> (ProjectCommandEnvelope, ExportHumanReadableManuscriptInput) {
    let client_binding = EditorClientBinding {
        binding_ref: "binding:author-edit".to_owned(),
        session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
    };
    let canonical_command_bytes =
        br#"{"command_schema":"storyos.command.export-human-readable-manuscript.request.v1"}"#
            .to_vec();
    let command_digest = command_digest(
        HUMAN_READABLE_EXPORT_DIGEST_PROFILE,
        &canonical_command_bytes,
    );
    let export_id = fresh_id();
    let envelope = ProjectCommandEnvelope {
        project_scope: scope.clone(),
        client_binding: client_binding.clone(),
        challenge_binding: ProjectCommandChallengeBinding {
            project_scope: scope.clone(),
            client_session_binding_digest: client_binding.binding_ref.clone(),
            client_session_generation: 1,
            client_contract_revision: client_binding.client_contract_revision.clone(),
            security_policy_revision: client_binding.security_policy_revision.clone(),
            limit_profile_revision: "storyos.foundation.absolute.v1".to_owned(),
            challenge_rate_policy_revision:
                "storyos.project-command-challenge-rate.fixed-window.v1".to_owned(),
            method: "POST".to_owned(),
            route_template: HUMAN_READABLE_EXPORT_ROUTE.to_owned(),
            command_schema: HUMAN_READABLE_EXPORT_REQUEST_SCHEMA.to_owned(),
            command_kind: HUMAN_READABLE_EXPORT_COMMAND_KIND.to_owned(),
            canonical_command_digest: command_digest,
            idempotency_key: fresh_id(),
        },
        nonce_digest: format!("sha256:nonce-export-work-readable-{export_id}"),
        canonical_command_bytes,
        correlation_id: fresh_id(),
        ids: AuthorCommandAdmissionIds {
            command_id: fresh_id(),
            author_command_admission_id: fresh_id(),
            receipt_id: fresh_id(),
        },
    };
    (envelope, ExportHumanReadableManuscriptInput { export_id })
}

pub(crate) fn command_digest(profile: &str, canonical_command_bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let value = Sha256::digest(canonical_command_bytes).iter().fold(
        String::with_capacity(64),
        |mut value, byte| {
            use std::fmt::Write as _;
            write!(value, "{byte:02x}").expect("writing to String cannot fail");
            value
        },
    );
    format!("sha256:{profile}:{value}")
}

pub(crate) async fn remove_export_work_rows(admin: &tokio_postgres::Client) {
    admin
        .batch_execute(
            "DELETE FROM storyos.human_readable_manuscript_exports;
             DELETE FROM storyos.pinned_export_sources;
             DELETE FROM storyos.human_readable_manuscript_export_operations;
             DELETE FROM storyos.project_export_entries;
             DELETE FROM storyos.project_export_manifests;
             DELETE FROM storyos.project_export_operations;",
        )
        .await
        .unwrap();
}
