use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::ProjectActivityKind;
use crate::release1_create_project::{
    CREATE_PROJECT, CREATE_PROJECT_CHALLENGE, CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
    CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID, CREATE_PROJECT_DIGEST_PROFILE,
    CREATE_PROJECT_REQUEST_SCHEMA_ID, CREATE_PROJECT_RESPONSE_SCHEMA_ID,
    CreateProjectChallengeRequest, CreateProjectChallengeResponse, CreateProjectInput,
    CreateProjectRequest, CreateProjectResponse,
};
use crate::release1_operation_registry::{
    ClientOverride, FixtureMembership, OpenApiMethod, OperationArtifacts, RegisteredOperation,
    operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{generated_ref, json_bytes, schema_value};

const CREATE_PROJECT_CHALLENGE_NOTES: &str = "Allocates only a prospective ProjectId and one challenge-bound idempotency record; it creates no Project, Admission, Receipt, or Core effect.";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[
        RegisteredOperation::challenge(
            &CREATE_PROJECT_CHALLENGE,
            &[
                "server_derived_user",
                "prospective_project_scope",
                "strict_origin",
                "protected_client_session_binding",
                "closed_create_project_schema",
                "body_idempotency_key",
            ],
            OperationCatalog::new(
                "challenge:prospective-project",
                Owner::Admission,
                Binding::CHALLENGE,
            )
            .notes(CREATE_PROJECT_CHALLENGE_NOTES),
        ),
        RegisteredOperation::command(
            &CREATE_PROJECT,
            &[
                "server_derived_user",
                "prospective_project_scope",
                "strict_origin",
                "protected_client_session_binding",
                "challenge_nonce_record",
                "expected_absent_project",
            ],
            OperationCatalog::new("project:create", Owner::Release, Binding::PROJECT_COMMAND)
                .activity(&[ProjectActivityKind::ProjectCreated], &[])
                .notes("The only command admitted against a prospective Project Scope."),
        )
        .client(ClientOverride::NoDigest),
    ],
    schemas: || {
        [
            operation_schemas(
                &CREATE_PROJECT_CHALLENGE,
                (
                    CHALLENGE_REQUEST_SCHEMA_PATH,
                    challenge_request_schema_bytes(),
                ),
                (
                    CHALLENGE_RESPONSE_SCHEMA_PATH,
                    challenge_response_schema_bytes(),
                ),
            ),
            operation_schemas(
                &CREATE_PROJECT,
                (REQUEST_SCHEMA_PATH, command_request_schema_bytes()),
                (RESPONSE_SCHEMA_PATH, command_response_schema_bytes()),
            ),
        ]
        .into_iter()
        .flatten()
        .collect()
    },
    openapi,
    typescript_types: typescript_type_declarations,
    fixtures: &[
        FixtureMembership {
            path: CHALLENGE_FIXTURE_PATH,
            bytes: |_| challenge_fixture_bytes(),
        },
        FixtureMembership {
            path: FIXTURE_PATH,
            bytes: |_| fixture_bytes(),
        },
    ],
};

pub(super) const CHALLENGE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-challenge-request.schema.json";
pub(super) const CHALLENGE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-challenge-response.schema.json";
pub(super) const CHALLENGE_FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/create-project-challenge.json";
pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/create-project-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/create-project.json";

pub(super) fn challenge_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectChallengeRequest>(
        CREATE_PROJECT_CHALLENGE_REQUEST_SCHEMA_ID,
        "StoryOS Create Project Challenge Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(CREATE_PROJECT_REQUEST_SCHEMA_ID);
    schema["properties"]["idempotency_key"]["format"] = json!("uuid");
    schema["$defs"]["CreateProjectInput"]["properties"]["correlation_id"]["format"] = json!("uuid");
    let title = &mut schema["$defs"]["CreateProjectInput"]["properties"]["title"];
    title["minLength"] = json!(1);
    title["x-storyos-max-utf8-bytes"] = json!(1024);
    json_bytes(&schema)
}

pub(super) fn challenge_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectChallengeResponse>(
        CREATE_PROJECT_CHALLENGE_RESPONSE_SCHEMA_ID,
        "StoryOS Create Project Challenge Response",
    );
    schema["properties"]["prospective_project_id"]["format"] = json!("uuid");
    schema["properties"]["expires_at"]["format"] = json!("date-time");
    schema["properties"]["nonce"]["pattern"] = json!("^[0-9a-f]{64}$");
    schema["properties"]["limit_profile_revision"]["const"] = json!(crate::LIMIT_PROFILE_REVISION);
    schema["$defs"]["DigestValue"]["properties"]["profile"]["const"] =
        json!(CREATE_PROJECT_DIGEST_PROFILE);
    schema["$defs"]["DigestValue"]["properties"]["value_hex_lowercase"]["pattern"] =
        json!("^[0-9a-f]{64}$");
    json_bytes(&schema)
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        CreateProjectInput::decl(&config),
        CreateProjectChallengeRequest::decl(&config),
        CreateProjectChallengeResponse::decl(&config),
        CreateProjectRequest::decl(&config),
        CreateProjectResponse::decl(&config),
    )
}

pub(super) fn openapi() -> Vec<OpenApiMethod> {
    let request_schema = generated_ref(CHALLENGE_REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(CHALLENGE_RESPONSE_SCHEMA_PATH);
    let responses = response_block(CREATE_PROJECT_CHALLENGE.responses, response_schema, &[200]);
    let challenge = format!(
        concat!(
            "    post:\n      operationId: {}\n      summary: Issue one prospective Project command challenge\n",
            "      parameters:\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        CREATE_PROJECT_CHALLENGE.operation_id, request_schema, responses,
    );
    vec![
        OpenApiMethod {
            path: CREATE_PROJECT_CHALLENGE.path,
            yaml: challenge,
        },
        OpenApiMethod {
            path: CREATE_PROJECT.path,
            yaml: command_openapi(),
        },
    ]
}

pub(super) fn command_request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectRequest>(
        CREATE_PROJECT_REQUEST_SCHEMA_ID,
        "StoryOS Create Project Request",
    );
    schema["properties"]["command_schema"]["const"] = json!(CREATE_PROJECT_REQUEST_SCHEMA_ID);
    schema["properties"]["prospective_project_id"]["format"] = json!("uuid");
    schema["$defs"]["CreateProjectInput"]["properties"]["correlation_id"]["format"] = json!("uuid");
    let title = &mut schema["$defs"]["CreateProjectInput"]["properties"]["title"];
    title["minLength"] = json!(1);
    title["x-storyos-max-utf8-bytes"] = json!(1024);
    json_bytes(&schema)
}

pub(super) fn command_response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<CreateProjectResponse>(
        CREATE_PROJECT_RESPONSE_SCHEMA_ID,
        "StoryOS Create Project Response",
    );
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["command_id"]["format"] = json!("uuid");
    schema["properties"]["author_command_admission_id"]["format"] = json!("uuid");
    schema["properties"]["schema_id"]["const"] = json!(CREATE_PROJECT_RESPONSE_SCHEMA_ID);
    if schema["$defs"].get("DomainReceipt").is_some() {
        schema["$defs"]["DomainReceipt"]["properties"]["author_action_sequence"] = json!({
            "anyOf": [
                {
                    "type": "string",
                    "pattern": "^(0|[1-9][0-9]{0,19})$"
                },
                {"type": "null"}
            ]
        });
        schema["$defs"]["DomainReceipt"]["properties"]["idempotency_key"]["format"] = json!("uuid");
        schema["$defs"]["DomainReceipt"]["properties"]["receipt_id"]["format"] = json!("uuid");
        schema["$defs"]["DomainReceipt"]["properties"]["author_command_admission_id"]["format"] =
            json!("uuid");
        schema["$defs"]["DomainReceipt"]["properties"]["created_at"]["format"] = json!("date-time");
    }
    json_bytes(&schema)
}

fn command_openapi() -> String {
    let request_schema = generated_ref(REQUEST_SCHEMA_PATH);
    let response_schema = generated_ref(RESPONSE_SCHEMA_PATH);
    let responses = response_block(CREATE_PROJECT.responses, response_schema, &[200]);
    format!(
        concat!(
            "    post:\n      operationId: {}\n      summary: Create one empty Project\n",
            "      parameters:\n        - name: Origin\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uri\n",
            "        - name: Idempotency-Key\n          in: header\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n",
            "      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{}'\n",
            "      responses:\n{}",
        ),
        CREATE_PROJECT.operation_id, request_schema, responses,
    )
}

pub(super) fn challenge_fixture_bytes() -> Vec<u8> {
    json_bytes(&challenge_fixture())
}

fn challenge_fixture() -> Value {
    json!({
        "prospective_project_id": "018f0000-0000-7001-8000-000000000201",
        "canonical_command_digest": {
            "algorithm": "sha256",
            "profile": CREATE_PROJECT_DIGEST_PROFILE,
            "value_hex_lowercase": "d".repeat(64)
        },
        "nonce": "e".repeat(64),
        "expires_at": "2026-08-23T08:05:00.000Z",
        "limit_profile_revision": crate::LIMIT_PROFILE_REVISION
    })
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&command_fixture())
}

fn command_fixture() -> Value {
    let project_scope = json!({
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
    });
    json!({
        "schema_id": CREATE_PROJECT_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000210",
        "project_scope": project_scope,
        "command_id": "018f0000-0000-7001-8000-000000000211",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000212",
        "receipt": {
            "receipt_id": "018f0000-0000-7001-8000-000000000213",
            "project_scope": project_scope,
            "command_kind": "createProject",
            "command_digest": {
                "algorithm": "sha256",
                "profile": CREATE_PROJECT_DIGEST_PROFILE,
                "value_hex_lowercase": "c".repeat(64)
            },
            "idempotency_key": "018f0000-0000-7001-8000-000000000214",
            "producer_cause": "author_command_admission",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000212",
            "expected_heads": [],
            "prior_heads": [],
            "resulting_heads": [],
            "authoritative_revision_ids": [],
            "proposal_revision_ids": [],
            "authoritative_commit_ids": [],
            "author_action_sequence": null,
            "draft_artifact_refs": [],
            "artifact_lifecycle_event_refs": [],
            "condition_refs": [],
            "result": "authoritative_applied",
            "created_at": "2026-08-25T08:05:00.000Z"
        },
        "project": {
            "project_id": "018f0000-0000-7001-8000-000000000201",
            "title": "Empty Novel",
            "open": {"kind": "empty"}
        }
    })
}

fn response_block(
    responses: &[(u16, &str)],
    response_schema: &str,
    success_statuses: &[u16],
) -> String {
    responses
        .iter()
        .map(|(status, description)| {
            let retry_after = if *status == 429 {
                "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n"
            } else {
                ""
            };
            let content = if success_statuses.contains(status) {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!(
                "        '{status}':\n          description: {description}\n{retry_after}{content}"
            )
        })
        .collect()
}
