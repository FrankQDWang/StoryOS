use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1_author_edit_artifacts as author_edit_artifacts;
use crate::release1_author_edit_outcome::{
    ApplyAuthorEditOutcome, ApplyAuthorEditReconfirmationReason, ApplyAuthorEditRejectionReason,
    ApplyAuthorEditUnknownObservation, GET_APPLY_AUTHOR_EDIT_OUTCOME,
    GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
    GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID, GetApplyAuthorEditOutcomeRequest,
    GetApplyAuthorEditOutcomeResponse,
};
use crate::release1_operation_registry::{
    OperationArtifacts, RegisteredOperation, fixture_triple, method, operation_schemas,
};
use crate::release1_wire::{canonical_u64_wire_schema, json_bytes, schema_value};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_APPLY_AUTHOR_EDIT_OUTCOME,
        &[
            "server_derived_project_scope",
            "sensitive_safe_read_origin",
            "protected_client_session_binding",
            "idempotency_key",
            "project_command_challenge_proof",
            "receipt_first_settlement_validation",
        ],
    )],
    schemas: || {
        operation_schemas(
            &GET_APPLY_AUTHOR_EDIT_OUTCOME,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&GET_APPLY_AUTHOR_EDIT_OUTCOME, openapi()),
    typescript_types: typescript_type_declarations,
    typescript_client: typescript_client_source,
    typescript_declarations,
    fixtures: || {
        fixture_triple(
            FIXTURE_PATHS,
            &GET_APPLY_AUTHOR_EDIT_OUTCOME,
            [
                |_| fixture_bytes(),
                |_| invalid_fixture_bytes(),
                |_| boundary_fixture_bytes(),
            ],
        )
        .into()
    },
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/apply-author-edit-outcome-response.schema.json";
pub(super) const FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-apply-author-edit-outcome.json",
    "generated/golden-wire/storyos-public-release-1/get-apply-author-edit-outcome.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-apply-author-edit-outcome.boundary.json",
];

pub(super) fn request_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetApplyAuthorEditOutcomeRequest>(
        GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST_SCHEMA_ID,
        "StoryOS Apply Author Edit Outcome Request",
    );
    schema["properties"]["project_id"]["format"] = json!("uuid");
    schema["properties"]["idempotency_key"]["format"] = json!("uuid");
    json_bytes(&schema)
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = schema_value::<GetApplyAuthorEditOutcomeResponse>(
        GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
        "StoryOS Apply Author Edit Outcome Response",
    );
    let canonical_u64 = canonical_u64_wire_schema();
    author_edit_artifacts::apply_u64_wire_constraints(&mut schema, &canonical_u64);
    schema["$defs"]["ApplyAuthorEditResponse"]["properties"]["local_intent_sequence"] =
        canonical_u64;
    schema["$defs"]["ApplyAuthorEditUnknownObservation"]["oneOf"][0]["properties"]["expires_at"]
        ["format"] = json!("date-time");
    for definition in [
        "ApplyAuthorEditOutcome",
        "ApplyAuthorEditUnknownObservation",
    ] {
        for variant in schema["$defs"][definition]["oneOf"]
            .as_array_mut()
            .expect("tagged outcome variants must exist")
        {
            variant["additionalProperties"] = Value::Bool(false);
        }
    }
    json_bytes(&schema)
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}\n\nexport {}",
        GetApplyAuthorEditOutcomeRequest::decl(&config),
        ApplyAuthorEditRejectionReason::decl(&config),
        ApplyAuthorEditReconfirmationReason::decl(&config),
        ApplyAuthorEditUnknownObservation::decl(&config),
        ApplyAuthorEditOutcome::decl(&config),
        GetApplyAuthorEditOutcomeResponse::decl(&config),
    )
}

pub(super) fn typescript_client_source() -> String {
    let path = GET_APPLY_AUTHOR_EDIT_OUTCOME
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}")
        .replace("{idempotency_key}", "${encodeURIComponent(idempotencyKey)}");
    format!(
        concat!(
            "\nexport async function getApplyAuthorEditOutcome({{ projectId, idempotencyKey, antiForgery, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getApplyAuthorEditOutcome requires projectId\");\n",
            "  if (typeof idempotencyKey !== \"string\" || typeof antiForgery !== \"string\") throw new TypeError(\"getApplyAuthorEditOutcome requires security bindings\");\n",
            "  return queryJson({{ ...options, path: `{}`, queryHeaders: {{ \"x-storyos-anti-forgery\": antiForgery }} }});\n",
            "}}\n",
        ),
        path,
    )
}

pub(super) fn typescript_declarations() -> &'static str {
    "export declare function getApplyAuthorEditOutcome(options: StoryOSQueryOptions & { projectId: string; idempotencyKey: string; antiForgery: string }): Promise<GetApplyAuthorEditOutcomeResponse>;\n"
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("response schema is generated");
    let responses = GET_APPLY_AUTHOR_EDIT_OUTCOME
        .responses
        .iter()
        .map(|(status, description)| {
            format!(
                "        '{status}':\n          description: {description}\n          headers:\n            Cache-Control:\n              required: true\n              schema:\n                type: string\n                const: no-store\n{}",
                if *status == 200 {
                    format!(
                        "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                    )
                } else {
                    String::new()
                }
            )
        })
        .collect::<String>();
    format!(
        "    get:\n      operationId: {}\n      summary: Read one exact Apply Author Edit outcome\n      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: idempotency_key\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n        - name: X-StoryOS-Anti-Forgery\n          in: header\n          required: true\n          schema:\n            type: string\n            pattern: '^[0-9a-f]{{64}}$'\n      responses:\n{responses}",
        GET_APPLY_AUTHOR_EDIT_OUTCOME.operation_id,
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&fixture())
}

pub(super) fn invalid_fixture_bytes() -> Vec<u8> {
    let mut invalid = fixture();
    invalid
        .as_object_mut()
        .expect("fixture is an object")
        .remove("project_scope");
    json_bytes(&invalid)
}

pub(super) fn boundary_fixture_bytes() -> Vec<u8> {
    let mut boundary = fixture();
    boundary["outcome"] = json!({
        "outcome_kind": "still_unknown",
        "observation": {
            "observation_kind": "admission_committed",
            "command_id": "018f0000-0000-7001-8000-000000000031",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
            "reconciliation_required": true
        }
    });
    json_bytes(&boundary)
}

fn fixture() -> Value {
    json!({
        "schema_id": GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000081",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "outcome": {
            "outcome_kind": "committed",
            "response": author_edit_artifacts::fixture()
        }
    })
}
