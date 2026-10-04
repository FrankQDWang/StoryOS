use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    CREATE_PROJECT_COMMAND_CHALLENGE, CreateProjectCommandChallengeRequest,
    CreateProjectCommandChallengeResponse, LIMIT_PROFILE_REVISION,
    PROJECT_COMMAND_CHALLENGE_REQUEST_SCHEMA_ID, PROJECT_COMMAND_CHALLENGE_RESPONSE_SCHEMA_ID,
};
use crate::release1_operation_registry::{
    GeneratedSchema, OperationArtifacts, RegisteredOperation, fixture_triple, method,
};
use crate::release1_wire::{json_bytes, schema_value};

pub(super) const CHALLENGE_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-command-challenge-request.schema.json";

pub(super) const CHALLENGE_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/project-command-challenge-response.schema.json";

const CHALLENGE_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.json",
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.invalid.json",
    "generated/golden-wire/storyos-public-release-1/create-project-command-challenge.boundary.json",
];

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
    typescript_client: || {
        format!(
            concat!(
                "\nexport async function createProjectCommandChallenge({{ projectId, request, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"createProjectCommandChallenge requires projectId\");\n",
                "  if (!request || typeof request !== \"object\") throw new TypeError(\"createProjectCommandChallenge requires request\");\n",
                "  return commandJson({{ ...options, path: `{}`, body: request }});\n}}\n",
            ),
            CREATE_PROJECT_COMMAND_CHALLENGE
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}"),
        )
    },
    typescript_declarations: || "export declare function createProjectCommandChallenge(options: StoryOSQueryOptions & { projectId: string; request: CreateProjectCommandChallengeRequest }): Promise<CreateProjectCommandChallengeResponse>;\n",
    fixtures: || {
        fixture_triple(
            CHALLENGE_FIXTURE_PATHS,
            &CREATE_PROJECT_COMMAND_CHALLENGE,
            [
                |_| challenge_fixture_bytes(),
                |_| invalid_challenge_fixture_bytes(),
                |_| boundary_challenge_fixture_bytes(),
            ],
        )
        .into()
    },
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

fn invalid_challenge_fixture_bytes() -> Vec<u8> {
    let mut invalid = challenge_fixture();
    invalid.as_object_mut().unwrap().remove("nonce");
    json_bytes(&invalid)
}

fn boundary_challenge_fixture_bytes() -> Vec<u8> {
    let mut boundary = challenge_fixture();
    boundary["expires_at"] = json!("1970-01-01T00:00:00.000Z");
    json_bytes(&boundary)
}
