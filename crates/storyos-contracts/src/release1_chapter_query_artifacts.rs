use serde_json::{Value, json};
use ts_rs::{Config, TS};

use crate::release1::{
    CHAPTER_REQUEST_SCHEMA_ID, CHAPTER_RESPONSE_SCHEMA_ID, GET_CHAPTER, GetChapterResponse,
};
use crate::release1_operation_registry::{
    GeneratedSchema, OperationArtifacts, RegisteredOperation, fixture_triple, method,
};
use crate::release1_wire::{
    json_bytes, path_request_schema, query_openapi, schema_value, with_boundary_project_scope,
    without_project_scope,
};

const CHAPTER_REQUEST_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-request.schema.json";

pub(super) const CHAPTER_RESPONSE_SCHEMA_PATH: &str =
    "generated/json-schema/storyos-public-release-1/chapter-response.schema.json";

pub(super) const CHAPTER_FIXTURE_PATHS: [&str; 3] = [
    "generated/golden-wire/storyos-public-release-1/get-chapter.json",
    "generated/golden-wire/storyos-public-release-1/get-chapter.invalid.json",
    "generated/golden-wire/storyos-public-release-1/get-chapter.boundary.json",
];

pub(super) const ARTIFACTS: OperationArtifacts = OperationArtifacts {
    operations: &[RegisteredOperation::query(
        &GET_CHAPTER,
        &[
            "server_derived_project_scope",
            "chapter_scope_join",
            "canonical_snapshot",
        ],
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
    typescript_client: || {
        format!(
            concat!(
                "\nexport async function getChapter({{ projectId, chapterId, ...options }} = {{}}) {{\n",
                "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"getChapter requires projectId\");\n",
                "  if (typeof chapterId !== \"string\" || chapterId.length === 0) throw new TypeError(\"getChapter requires chapterId\");\n",
                "  return queryJson({{ ...options, path: `{}` }});\n}}\n",
            ),
            GET_CHAPTER
                .path
                .replace("{project_id}", "${encodeURIComponent(projectId)}")
                .replace("{chapter_id}", "${encodeURIComponent(chapterId)}"),
        )
    },
    typescript_declarations: || "export declare function getChapter(options: StoryOSQueryOptions & { projectId: string; chapterId: string }): Promise<GetChapterResponse>;\n",
    fixtures: || {
        fixture_triple(
            CHAPTER_FIXTURE_PATHS,
            &GET_CHAPTER,
            [
                |_| chapter_fixture_bytes(),
                |_| invalid_chapter_fixture_bytes(),
                |_| boundary_chapter_fixture_bytes(),
            ],
        )
        .into()
    },
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

fn invalid_chapter_fixture_bytes() -> Vec<u8> {
    without_project_scope(chapter_fixture())
}

fn boundary_chapter_fixture_bytes() -> Vec<u8> {
    with_boundary_project_scope(chapter_fixture())
}
