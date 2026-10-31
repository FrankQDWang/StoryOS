use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::ProjectActivityKind;
use crate::release1::PUBLIC_PROTOCOL_RELEASE;
use crate::release1_operation_registry::{
    FixtureMembership, OperationArtifacts, RegisteredOperation, method, operation_schemas,
};
use crate::release1_project_export::{
    EXPORT_PROJECT_ARCHIVE, EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID,
    EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID, ExportProjectArchiveEffect,
    ExportProjectArchiveInput, ExportProjectArchiveRequest, ExportProjectArchiveResponse,
    ProjectExportRef,
};
use crate::release1_project_export_query::GET_EXPORT_OPERATION;
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::command(
        &EXPORT_PROJECT_ARCHIVE,
        &[
            "server_derived_project_scope",
            "consistent_source_snapshot",
            "archive_path_profile",
            "integrity_protection_before_completion",
        ],
        OperationCatalog::new(
            "project-archive:export",
            Owner::Storage,
            Binding {
                action_class: ActionClass::ExplicitProjectCommand,
                admission: Admission::AuthorCommandOrAsyncOperation,
                editor: EditorRule::Optional,
                idempotency: Idempotency::HeaderAndAdmissionRecord,
                settlement: Settlement::Accepted {
                    query: &GET_EXPORT_OPERATION,
                    operation_ref: "project_export",
                },
                error_profile: ErrorProfile::AsyncCommand,
            },
        )
        .activity(&[ProjectActivityKind::ProjectExportSettled], &[]),
    )],
    schemas: || {
        operation_schemas(
            &EXPORT_PROJECT_ARCHIVE,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&EXPORT_PROJECT_ARCHIVE, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};
pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/export-project-archive-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/export-project-archive-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/export-project-archive.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExportProjectArchiveRequest>(
        EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID,
        "StoryOS Export Project Archive Request",
    );
    schema["properties"]["command_schema"]["const"] =
        json!(EXPORT_PROJECT_ARCHIVE_REQUEST_SCHEMA_ID);
    let input = &mut schema["$defs"]["ExportProjectArchiveInput"]["properties"];
    input["correlation_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<ExportProjectArchiveResponse>(
        EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID,
        "StoryOS Export Project Archive Response",
    );
    schema["properties"]["schema_id"]["const"] = json!(EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID);
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = EXPORT_PROJECT_ARCHIVE
        .responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if *status == 202 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{retry_after}{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "    post:\n      operationId: {}\n      summary: Admit one durable Project Export Archive operation\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        EXPORT_PROJECT_ARCHIVE.operation_id, request_schema, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ExportProjectArchiveInput::decl(&config),
        ExportProjectArchiveRequest::decl(&config),
        ProjectExportRef::decl(&config),
        ExportProjectArchiveEffect::decl(&config),
        ExportProjectArchiveResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    json!({
        "schema_id": EXPORT_PROJECT_ARCHIVE_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000920",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000201"
        },
        "command_id": "018f0000-0000-7001-8000-000000000921",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000922",
        "acknowledgement": "accepted",
        "operation_ref": {
            "kind": "project_export",
            "export_id": "018f0000-0000-7001-8000-000000000926"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {"kind": "empty"}
        },
        "effect": {
            "kind": "admitted",
            "export_id": "018f0000-0000-7001-8000-000000000926",
            "archive_profile": "storyos.project-export.v1",
            "archive_path_profile": "storyos.archive-path.utf8-nfc-unicode-16.0.0.v1",
            "source_snapshot": {
                "snapshot_id": "018f0000-0000-7001-8000-000000000040",
                "project_scope": {
                    "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                    "project_id": "018f0000-0000-7001-8000-000000000201"
                },
                "snapshot_kind": "canonical",
                "project_activity_position": "2",
                "source_watermarks": {},
                "projection_generations": {},
                "redaction_profile": "storyos.author.v1",
                "schema_profile": PUBLIC_PROTOCOL_RELEASE,
                "replay_generation": "1",
                "created_at": "2026-09-01T12:00:00.000Z",
                "expires_at": null
            }
        }
    })
}
