//! The shared test support from `project_archive_build_tests.rs`.

use crate::shared_tests::export_work::{
    PROJECT_EXPORT_COMMAND_KIND, PROJECT_EXPORT_DIGEST_PROFILE, PROJECT_EXPORT_REQUEST_SCHEMA,
    PROJECT_EXPORT_ROUTE,
};
use storyos_application::{
    AuthorCommandAdmissionIds, EditorClientBinding, ExportProjectArchiveInput,
    ProjectCommandChallengeBinding, ProjectCommandEnvelope, ProjectScope,
};

/// StoryOS Project Export ZIP files use STORE only, so local headers carry the
/// uncompressed payload immediately after the name and extra fields.
pub(crate) fn zip_store_files(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    let mut offset = 0;
    while offset + 30 <= bytes.len() {
        if &bytes[offset..offset + 4] != b"PK\x03\x04" {
            break;
        }
        let name_len =
            u16::from_le_bytes(bytes[offset + 26..offset + 28].try_into().unwrap()) as usize;
        let extra_len =
            u16::from_le_bytes(bytes[offset + 28..offset + 30].try_into().unwrap()) as usize;
        let size = u32::from_le_bytes(bytes[offset + 18..offset + 22].try_into().unwrap()) as usize;
        let name_start = offset + 30;
        let name = String::from_utf8(bytes[name_start..name_start + name_len].to_vec()).unwrap();
        let data_start = name_start + name_len + extra_len;
        files.push((name, bytes[data_start..data_start + size].to_vec()));
        offset = data_start + size;
    }
    files
}

/// The export command of `scope`. Its identities end with `lead` and one more digit.
pub(crate) fn export_command(
    scope: &ProjectScope,
    lead: &str,
) -> (ProjectCommandEnvelope, ExportProjectArchiveInput) {
    let id = |last: u8| format!("018f0000-0000-7001-8000-000000000{lead}{last}");
    let client_binding = EditorClientBinding {
        binding_ref: "binding:author-edit".to_owned(),
        session_generation: 1,
        client_contract_revision: "storyos.web-client.release-1.v1".to_owned(),
        security_policy_revision: "storyos.web-security-policy.release-1.v1".to_owned(),
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
            route_template: PROJECT_EXPORT_ROUTE.to_owned(),
            command_schema: PROJECT_EXPORT_REQUEST_SCHEMA.to_owned(),
            command_kind: PROJECT_EXPORT_COMMAND_KIND.to_owned(),
            canonical_command_digest: command_digest,
            idempotency_key: id(4),
        },
        nonce_digest: format!("sha256:nonce-export-reconfirm-{lead}"),
        canonical_command_bytes,
        correlation_id: id(0),
        ids: AuthorCommandAdmissionIds {
            command_id: id(1),
            author_command_admission_id: id(2),
            receipt_id: id(3),
        },
    };
    let input = ExportProjectArchiveInput { export_id: id(6) };
    (envelope, input)
}
