use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    CREATE_PROJECT_COMMAND_CHALLENGE, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, LIMIT_PROFILE_REVISION,
    PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID, PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
};
use crate::release1_operation_registry::{
    FixtureMembership, GeneratedSchema, OperationArtifacts, RegisteredOperation, method,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{json_bytes, schema_value};

pub(super) const CHALLENGE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-command-challenge-request.schema.json";

pub(super) const CHALLENGE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-command-challenge-response.schema.json";

const CHALLENGE_FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::challenge(
        &CREATE_PROJECT_COMMAND_CHALLENGE,
        &[
            "server_derived_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "route_method_schema_digest_match",
            "closed_command_schema",
        ],
        OperationCatalog::new(
            "challenge:project-command",
            Owner::Admission,
            Binding::CHALLENGE,
        )
        .notes("Creates no Admission, Receipt, domain Attempt, or authority."),
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
                path: CHALLENGE_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&schema_value::<CreateProjectCommandChallengeRequest>(
                    PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID,
                    "StoryOS Project Command Challenge Request",
                )),
            },
            GeneratedSchema {
                schema_id: PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
                path: CHALLENGE_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&challenge_response_schema()),
            },
        ]
    },
    openapi: || method(&CREATE_PROJECT_COMMAND_CHALLENGE, challenge_openapi()),
    typescript_types: || {
        let config = Config::default();
        format!(
            "export {}\n\nexport {}",
            CreateProjectCommandChallengeRequest::decl(&config),
            CreateProjectCommandChallengeResponse::decl(&config),
        )
    },
    fixtures: &[FixtureMembership {
        path: CHALLENGE_FIXTURE_PATH,
        bytes: |_| challenge_fixture_bytes(),
    }],
};

fn challenge_openapi() -> String {
    let operation = &CREATE_PROJECT_COMMAND_CHALLENGE;
    let request_schema = CHALLENGE_REQUEST_SCHEMA_PATH
        .strip_prefix("generated/")
        .unwrap();
    let response_schema = CHALLENGE_RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .unwrap();
    let responses = operation.responses.iter().map(|(status, description)| format!(
        "        '{status}':\n          description: {description}\n{}{}",
        if *status == 429 { "          headers:\n            Retry-After:\n              required: true\n              schema:\n                type: integer\n                minimum: 1\n                maximum: 60\n" } else { "" },
        if *status == 200 { format!("          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n") } else { String::new() }
    )).collect::<String>();
    format!(
        "    post:\n      operationId: {}\n      summary: Issue one exact Project command challenge\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n      requestBody:\n        required: true\n        content:\n          application/json:\n            schema:\n              $ref: '../{request_schema}'\n      responses:\n{responses}",
        operation.operation_id,
    )
}

fn challenge_response_schema() -> Value {
    let mut schema = schema_value::<CreateProjectCommandChallengeResponse>(
        PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
        "StoryOS Project Command Challenge Response",
    );
    schema["properties"]["expires_at"]["format"] = Value::String("date-time".to_owned());
    schema
}

fn challenge_fixture() -> Value {
    json!({
        "nonce": "4f9f5ad05c4d1294d4114fb15595d831b64e3f4312a17e639213ad36e941ca71",
        "expires_at": "2026-08-12T08:05:00.000Z",
        "limit_profile_revision": LIMIT_PROFILE_REVISION
    })
}

fn challenge_fixture_bytes() -> Vec<u8> {
    json_bytes(&challenge_fixture())
}
