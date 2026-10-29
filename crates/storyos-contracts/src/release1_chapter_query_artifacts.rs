use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    CHAPTER_REQUEST_SCHEMA_ID, CHAPTER_RESPONSE_SCHEMA_ID, GET_CHAPTER, GetChapterResponse,
};
use crate::release1_operation_registry::{
    FixtureMembership, GeneratedSchema, OperationArtifacts, RegisteredOperation, method,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::{json_bytes, path_request_schema, query_openapi, schema_value};

const CHAPTER_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-request.schema.json";

pub(super) const CHAPTER_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-response.schema.json";

pub(super) const CHAPTER_FIXTURE_PATH: &str =
    "generated/golden-wire/storyos-public-release-1/get-chapter.json";

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "canonical_snapshot",
        ],
        OperationCatalog::new(
            "chapter:read",
            Owner::Release,
            Binding::CURRENT_CHAPTER_QUERY,
        ),
    )],
    schemas: || {
        vec![
            GeneratedSchema {
                schema_id: CHAPTER_REQUEST_SCHEMA_ID,
                path: CHAPTER_REQUEST_SCHEMA_PATH,
                bytes: json_bytes(&path_request_schema(
                    CHAPTER_REQUEST_SCHEMA_ID,
                    &["project_id", "chapter_id"],
                )),
            },
            GeneratedSchema {
                schema_id: CHAPTER_RESPONSE_SCHEMA_ID,
                path: CHAPTER_RESPONSE_SCHEMA_PATH,
                bytes: json_bytes(&schema_value::<GetChapterResponse>(
                    CHAPTER_RESPONSE_SCHEMA_ID,
                    "StoryOS Chapter Query Response",
                )),
            },
        ]
    },
    openapi: || {
        method(
            &GET_CHAPTER,
            query_openapi(
                &GET_CHAPTER,
                "Read the controlled Project current Chapter",
                CHAPTER_RESPONSE_SCHEMA_PATH,
                &["project_id", "chapter_id"],
            ),
        )
    },
    typescript_types: || format!("export {}", GetChapterResponse::decl(&Config::default())),
    fixtures: &[FixtureMembership {
        path: CHAPTER_FIXTURE_PATH,
        bytes: |_| chapter_fixture_bytes(),
    }],
};

fn chapter_fixture() -> Value {
    json!({
        "schema_id": CHAPTER_RESPONSE_SCHEMA_ID,
        "correlation_id": "018f0000-0000-7001-8000-000000000006",
        "project_scope": {"owner_user_id": "018f0000-0000-7001-8000-000000000001", "project_id": "018f0000-0000-7001-8000-000000000002"},
        "project_activity_position": "0",
        "chapter": {"chapter_id": "018f0000-0000-7001-8000-000000000003", "title": "第一章", "current_revision": {"revision_id": "018f0000-0000-7001-8000-000000000004", "body": "雨落在窗沿。", "blocks": [{"manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1", "block_kind": "paragraph", "text": "雨落在窗沿。"}]}}
    })
}

fn chapter_fixture_bytes() -> Vec<u8> {
    json_bytes(&chapter_fixture())
}
