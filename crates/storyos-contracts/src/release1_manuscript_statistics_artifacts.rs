use schemars::schema_for;
use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{LIMIT_PROFILE_REVISION, PUBLIC_PROTOCOL_RELEASE};
use crate::release1_manuscript_statistics::{
    ChapterStatistics, GET_STATISTICS, GET_STATISTICS_REQUEST_SCHEMA_ID,
    GET_STATISTICS_RESPONSE_SCHEMA_ID, GetStatisticsResponse, ManuscriptStatisticsCompleteness,
    ManuscriptTotals,
};
use crate::release1_operation_registry::{
    ClientOverride, FixtureMembership, OperationArtifacts, RegisteredOperation, method,
    operation_schemas,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{canonical_u64_wire_schema, json_bytes};

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_STATISTICS,
        &[
            "server_derived_project_scope",
            "canonical_snapshot_or_projection_watermark",
        ],
        OperationCatalog::new(
            "manuscript:statistics",
            Owner::Release,
            Binding::PROJECTION_QUERY,
        ),
    )
    .client(ClientOverride::OptionalQueryParameter("required_watermark"))],
    schemas: || {
        operation_schemas(
            &GET_STATISTICS,
            (REQUEST_SCHEMA_PATH, request_schema_bytes()),
            (RESPONSE_SCHEMA_PATH, response_schema_bytes()),
        )
        .into()
    },
    openapi: || method(&GET_STATISTICS, openapi()),
    typescript_types: typescript_type_declarations,
    fixtures: &[FixtureMembership {
        path: FIXTURE_PATH,
        bytes: |_| fixture_bytes(),
    }],
};

pub(super) const REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/statistics-request.schema.json";
pub(super) const RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/statistics-response.schema.json";
pub(super) const FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-statistics.json";

pub(super) fn request_schema_bytes() -> Vec<u8> {
    json_bytes(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": GET_STATISTICS_REQUEST_SCHEMA_ID,
        "title": "StoryOS Manuscript Statistics Request",
        "type": "object",
        "additionalProperties": false,
        "required": ["project_id"],
        "properties": {
            "project_id": {"type": "string", "format": "uuid"},
            "required_watermark": {
                "anyOf": [
                    canonical_u64_wire_schema(),
                    { "type": "null" }
                ]
            }
        }
    }))
}

pub(super) fn response_schema_bytes() -> Vec<u8> {
    let mut schema = serde_json::to_value(schema_for!(GetStatisticsResponse))
        .expect("statistics response schema serializes");
    schema["$id"] = json!(GET_STATISTICS_RESPONSE_SCHEMA_ID);
    schema["title"] = json!("StoryOS Manuscript Statistics Response");
    schema["additionalProperties"] = json!(false);
    schema["properties"]["schema_id"]["const"] = json!(GET_STATISTICS_RESPONSE_SCHEMA_ID);
    schema["properties"]["query_id"]["format"] = json!("uuid");
    schema["properties"]["correlation_id"]["format"] = json!("uuid");
    schema["properties"]["projection_kind"]["const"] = json!("manuscript_statistics");
    schema["properties"]["counting_profile"]["const"] =
        json!("storyos.statistics.unicode-16.0.0.v1");
    let canonical_u64 = canonical_u64_wire_schema();
    schema["properties"]["projection_generation"] = canonical_u64.clone();
    schema["properties"]["projection_watermark"] = canonical_u64.clone();
    schema["properties"]["required_watermark"] = json!({
        "anyOf": [canonical_u64.clone(), { "type": "null" }]
    });
    schema["properties"]["lag"] = canonical_u64.clone();
    schema["properties"]["page_count"] = canonical_u64.clone();
    schema["properties"]["page_bytes"] = canonical_u64.clone();
    if let Some(scope) = schema["$defs"].get_mut("ProjectScope") {
        scope["properties"]["owner_user_id"]["format"] = json!("uuid");
        scope["properties"]["project_id"]["format"] = json!("uuid");
    }
    if let Some(snapshot) = schema["$defs"].get_mut("SnapshotDescriptor") {
        snapshot["properties"]["snapshot_id"]["format"] = json!("uuid");
        snapshot["properties"]["project_activity_position"] = canonical_u64.clone();
        snapshot["properties"]["replay_generation"] = canonical_u64.clone();
        snapshot["properties"]["created_at"]["format"] = json!("date-time");
        snapshot["properties"]["expires_at"]["format"] = json!("date-time");
    }
    if let Some(chapter) = schema["$defs"].get_mut("ChapterStatistics") {
        chapter["properties"]["chapter_id"]["format"] = json!("uuid");
        chapter["properties"]["word_count"] = canonical_u64.clone();
        chapter["properties"]["character_count"] = canonical_u64.clone();
    }
    if let Some(totals) = schema["$defs"].get_mut("ManuscriptTotals") {
        totals["properties"]["chapter_count"] = canonical_u64.clone();
        totals["properties"]["word_count"] = canonical_u64.clone();
        totals["properties"]["character_count"] = canonical_u64;
    }
    json_bytes(&schema)
}

pub(super) fn openapi() -> String {
    let response_schema = RESPONSE_SCHEMA_PATH
        .strip_prefix("generated/")
        .expect("schema is a generated artifact");
    let responses = GET_STATISTICS
        .responses
        .iter()
        .map(|(status, description)| {
            let content = if *status == 200 {
                format!(
                    "          content:\n            application/json:\n              schema:\n                $ref: '../{response_schema}'\n"
                )
            } else {
                String::new()
            };
            format!("        '{status}':\n          description: {description}\n{content}")
        })
        .collect::<String>();
    format!(
        concat!(
            "    get:\n      operationId: {}\n      summary: Rebuild Chapter and manuscript statistics\n",
            "      parameters:\n        - name: project_id\n          in: path\n          required: true\n          schema:\n            type: string\n            format: uuid\n",
            "        - name: required_watermark\n          in: query\n          required: false\n          schema:\n            type: string\n            pattern: '^(0|[1-9][0-9]*)$'\n",
            "      responses:\n{}",
        ),
        GET_STATISTICS.operation_id, responses,
    )
}

pub(super) fn typescript_type_declarations() -> String {
    let config = Config::default();
    format!(
        "export {}\n\nexport {}\n\nexport {}\n\nexport {}",
        ManuscriptStatisticsCompleteness::decl(&config),
        ChapterStatistics::decl(&config),
        ManuscriptTotals::decl(&config),
        GetStatisticsResponse::decl(&config),
    )
}

pub(super) fn fixture_bytes() -> Vec<u8> {
    json_bytes(&statistics_fixture())
}

fn statistics_fixture() -> Value {
    json!({
        "schema_id": GET_STATISTICS_RESPONSE_SCHEMA_ID,
        "query_id": "018f0000-0000-7001-8000-000000000071",
        "correlation_id": "018f0000-0000-7001-8000-000000000072",
        "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "source_snapshot": {
            "snapshot_id": "018f0000-0000-7001-8000-000000000032",
            "project_scope": {
                "owner_user_id": "018f0000-0000-7001-8000-000000000001",
                "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "snapshot_kind": "canonical",
            "project_activity_position": "4",
            "source_watermarks": {},
            "projection_generations": {},
            "redaction_profile": "storyos.author.v1",
            "schema_profile": PUBLIC_PROTOCOL_RELEASE,
            "replay_generation": "1",
            "created_at": "2026-08-31T00:00:00.000Z",
            "expires_at": null
        },
        "projection_kind": "manuscript_statistics",
        "projection_generation": "4",
        "projection_watermark": "4",
        "required_watermark": null,
        "completeness": "complete",
        "lag": "0",
        "counting_profile": "storyos.statistics.unicode-16.0.0.v1",
        "current_chapter": {
            "chapter_id": "018f0000-0000-7001-8000-000000000003",
            "word_count": "1",
            "character_count": "5"
        },
        "manuscript": {
            "chapter_count": "1",
            "word_count": "1",
            "character_count": "5"
        },
        "next_cursor": null,
        "page_count": "1",
        "page_bytes": "2",
        "redaction_profile": "storyos.author.v1",
        "limit_profile_revision": LIMIT_PROFILE_REVISION
    })
}
