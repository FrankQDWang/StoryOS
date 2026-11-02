//! The rows of the manuscript queries in the operation table.

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, NO_PATCHES, OperationRow, Patch, QueryValue, SCHEMA_ID,
    SchemaSource, SchemaSources, U64, UUID, raw_schema,
};
use crate::release1::{GET_CHAPTER, GetChapterResponse};
use crate::release1_manuscript_search::{
    ManuscriptSearchCompleteness, ManuscriptSearchMatch, ManuscriptSearchSelection,
    SEARCH_MANUSCRIPT, SearchManuscriptRequest, SearchManuscriptResponse,
};
use crate::release1_manuscript_statistics::{
    ChapterStatistics, GET_STATISTICS, GetStatisticsResponse, ManuscriptStatisticsCompleteness,
    ManuscriptTotals,
};
use crate::release1_manuscript_tree::{
    GET_MANUSCRIPT_TREE, GetManuscriptTreeResponse, ManuscriptChapterNode, ManuscriptVolumeNode,
};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};
use crate::release1_wire::U64_WIRE;

pub(super) const GET_MANUSCRIPT_TREE_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_MANUSCRIPT_TREE,
        &[
            "server_derived_project_scope",
            "canonical_snapshot",
            "tree_scope_join",
        ],
        OperationCatalog::new(
            "manuscript-tree:read",
            Owner::Release,
            Binding::CANONICAL_QUERY,
        ),
    ),
    summary: "Read the ordered canonical manuscript tree",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::path_parameters("manuscript-tree-request"),
        SchemaSource::typed(
            "manuscript-tree-response",
            raw_schema::<GetManuscriptTreeResponse>,
            "StoryOS Manuscript Tree Response",
            GET_MANUSCRIPT_TREE_RESPONSE,
        ),
    ),
    typescript: &[
        ManuscriptChapterNode::decl,
        ManuscriptVolumeNode::decl,
        GetManuscriptTreeResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000014",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.query.manuscript-tree.response.v1",
      "snapshot": {
        "created_at": "2026-07-21T10:16:30.000Z",
        "expires_at": null,
        "project_activity_position": "1",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "projection_generations": {},
        "redaction_profile": "storyos.author.v1",
        "replay_generation": "1",
        "schema_profile": "storyos.public.release.1",
        "snapshot_id": "018f0000-0000-7001-8000-000000000032",
        "snapshot_kind": "canonical",
        "source_watermarks": {}
      },
      "tree_revision": "1",
      "volumes": []
    }"#,
    ),
};

pub(super) const GET_CHAPTER_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
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
    ),
    summary: "Read the controlled Project current Chapter",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::path_parameters("chapter-request"),
        SchemaSource::typed(
            "chapter-response",
            raw_schema::<GetChapterResponse>,
            "StoryOS Chapter Query Response",
            NO_PATCHES,
        ),
    ),
    typescript: &[GetChapterResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "chapter": {
        "chapter_id": "018f0000-0000-7001-8000-000000000003",
        "current_revision": {
          "blocks": [
            {
              "block_kind": "paragraph",
              "manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1",
              "text": "雨落在窗沿。"
            }
          ],
          "body": "雨落在窗沿。",
          "revision_id": "018f0000-0000-7001-8000-000000000004"
        },
        "title": "第一章"
      },
      "correlation_id": "018f0000-0000-7001-8000-000000000006",
      "project_activity_position": "0",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.query.chapter.response.v1"
    }"#,
    ),
};

pub(super) const GET_STATISTICS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
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
    .client(ClientOverride::OptionalQueryParameter("required_watermark")),
    summary: "Rebuild Chapter and manuscript statistics",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::json(
            "statistics-request",
            "StoryOS Manuscript Statistics Request",
            r#"{
              "$schema": "https://json-schema.org/draft/2020-12/schema",
              "additionalProperties": false,
              "properties": {
                "project_id": {"format": "uuid", "type": "string"},
                "required_watermark": {
                  "anyOf": [
                    {
                      "pattern": "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$",
                      "type": "string"
                    },
                    {"type": "null"}
                  ]
                }
              },
              "required": ["project_id"],
              "type": "object"
            }"#,
        ),
        SchemaSource::typed("statistics-response", raw_schema::<GetStatisticsResponse>, "StoryOS Manuscript Statistics Response", GET_STATISTICS_RESPONSE),
    )
    .query_value(QueryValue::Decimal),
    typescript: &[ManuscriptStatisticsCompleteness::decl, ChapterStatistics::decl, ManuscriptTotals::decl, GetStatisticsResponse::decl],
    fixture: Fixture::Json(r#"{
      "completeness": "complete",
      "correlation_id": "018f0000-0000-7001-8000-000000000072",
      "counting_profile": "storyos.statistics.unicode-16.0.0.v1",
      "current_chapter": {
        "chapter_id": "018f0000-0000-7001-8000-000000000003",
        "character_count": "5",
        "word_count": "1"
      },
      "lag": "0",
      "limit_profile_revision": "storyos.foundation.absolute.v1",
      "manuscript": {"chapter_count": "1", "character_count": "5", "word_count": "1"},
      "next_cursor": null,
      "page_bytes": "2",
      "page_count": "1",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "projection_generation": "4",
      "projection_kind": "manuscript_statistics",
      "projection_watermark": "4",
      "query_id": "018f0000-0000-7001-8000-000000000071",
      "redaction_profile": "storyos.author.v1",
      "required_watermark": null,
      "schema_id": "storyos.query.statistics.response.v1",
      "source_snapshot": {
        "created_at": "2026-08-31T00:00:00.000Z",
        "expires_at": null,
        "project_activity_position": "4",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "projection_generations": {},
        "redaction_profile": "storyos.author.v1",
        "replay_generation": "1",
        "schema_profile": "storyos.public.release.1",
        "snapshot_id": "018f0000-0000-7001-8000-000000000032",
        "snapshot_kind": "canonical",
        "source_watermarks": {}
      }
    }"#),
};

pub(super) const SEARCH_MANUSCRIPT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &SEARCH_MANUSCRIPT,
        &[
            "server_derived_project_scope",
            "bounded_search_query",
            "projection_watermark_if_required",
            "redaction_profile",
        ],
        OperationCatalog::new(
            "manuscript:search",
            Owner::Release,
            Binding::PROJECTION_QUERY,
        )
        .notes(SEARCH_MANUSCRIPT_NOTES),
    ),
    summary: "Search the current Chapter or manuscript",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "manuscript-search-request",
            raw_schema::<SearchManuscriptRequest>,
            "StoryOS Manuscript Search Request",
            SEARCH_MANUSCRIPT_REQUEST,
        ),
        SchemaSource::typed(
            "manuscript-search-response",
            raw_schema::<SearchManuscriptResponse>,
            "StoryOS Manuscript Search Response",
            SEARCH_MANUSCRIPT_RESPONSE,
        ),
    ),
    typescript: &[
        ManuscriptSearchSelection::decl,
        SearchManuscriptRequest::decl,
        ManuscriptSearchMatch::decl,
        ManuscriptSearchCompleteness::decl,
        SearchManuscriptResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "completeness": "complete",
      "correlation_id": "018f0000-0000-7001-8000-000000000062",
      "items": [],
      "lag": "0",
      "limit_profile_revision": "storyos.foundation.absolute.v1",
      "next_cursor": null,
      "page_bytes": "2",
      "page_count": "0",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "projection_generation": "4",
      "projection_kind": "manuscript_search",
      "projection_watermark": "4",
      "query_id": "018f0000-0000-7001-8000-000000000061",
      "redaction_profile": "storyos.author.v1",
      "required_watermark": null,
      "schema_id": "storyos.query.manuscript-search.response.v1",
      "source_snapshot": {
        "created_at": "2026-08-31T00:00:00.000Z",
        "expires_at": null,
        "project_activity_position": "4",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "projection_generations": {},
        "redaction_profile": "storyos.author.v1",
        "replay_generation": "1",
        "schema_profile": "storyos.public.release.1",
        "snapshot_id": "018f0000-0000-7001-8000-000000000032",
        "snapshot_kind": "canonical",
        "source_watermarks": {}
      }
    }"#,
    ),
};

const SEARCH_MANUSCRIPT_NOTES: &str = "Search selection is a read projection. A direct current visible replacement uses ApplyAuthorEdit; selected multi-match or cross-location replacement remains an author-edit classification input and cannot choose a proposal route.";

const GET_MANUSCRIPT_TREE_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ManuscriptChapterNode/properties/chapter_id",
            "/$defs/ManuscriptVolumeNode/properties/volume_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SnapshotDescriptor/properties/snapshot_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/ManuscriptChapterNode/properties/order",
            "/$defs/ManuscriptVolumeNode/properties/order",
            "/$defs/SnapshotDescriptor/properties/project_activity_position",
            "/$defs/SnapshotDescriptor/properties/replay_generation",
            "/properties/tree_revision",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &[
            "/$defs/ManuscriptChapterNode/properties/title",
            "/$defs/ManuscriptVolumeNode/properties/title",
        ],
    ),
    Patch(
        Edit::MaxUtf8Bytes(1024),
        &[
            "/$defs/ManuscriptChapterNode/properties/title",
            "/$defs/ManuscriptVolumeNode/properties/title",
        ],
    ),
    Patch(
        DATE_TIME,
        &[
            "/$defs/SnapshotDescriptor/properties/created_at",
            "/$defs/SnapshotDescriptor/properties/expires_at",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const GET_STATISTICS_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ChapterStatistics/properties/chapter_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SnapshotDescriptor/properties/snapshot_id",
            "/properties/correlation_id",
            "/properties/query_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/ChapterStatistics/properties/character_count",
            "/$defs/ChapterStatistics/properties/word_count",
            "/$defs/ManuscriptTotals/properties/chapter_count",
            "/$defs/ManuscriptTotals/properties/character_count",
            "/$defs/ManuscriptTotals/properties/word_count",
            "/$defs/SnapshotDescriptor/properties/project_activity_position",
            "/$defs/SnapshotDescriptor/properties/replay_generation",
            "/properties/lag",
            "/properties/page_bytes",
            "/properties/page_count",
            "/properties/projection_generation",
            "/properties/projection_watermark",
        ],
    ),
    Patch(
        DATE_TIME,
        &[
            "/$defs/SnapshotDescriptor/properties/created_at",
            "/$defs/SnapshotDescriptor/properties/expires_at",
        ],
    ),
    Patch(
        Edit::Const("storyos.statistics.unicode-16.0.0.v1"),
        &["/properties/counting_profile"],
    ),
    Patch(
        Edit::Const("manuscript_statistics"),
        &["/properties/projection_kind"],
    ),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/properties/required_watermark"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const SEARCH_MANUSCRIPT_REQUEST: &[Patch] = &[
    Patch(Edit::MinLength(1), &["/properties/query_text"]),
    Patch(Edit::MaxUtf8Bytes(1048576), &["/properties/query_text"]),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/properties/required_watermark"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const SEARCH_MANUSCRIPT_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ManuscriptSearchMatch/properties/chapter_id",
            "/$defs/ManuscriptSearchMatch/properties/manuscript_block_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/$defs/SnapshotDescriptor/properties/snapshot_id",
            "/properties/correlation_id",
            "/properties/query_id",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/ManuscriptSearchMatch/properties/end",
            "/$defs/ManuscriptSearchMatch/properties/start",
            "/$defs/SnapshotDescriptor/properties/project_activity_position",
            "/$defs/SnapshotDescriptor/properties/replay_generation",
            "/properties/lag",
            "/properties/page_bytes",
            "/properties/page_count",
            "/properties/projection_generation",
            "/properties/projection_watermark",
        ],
    ),
    Patch(
        DATE_TIME,
        &[
            "/$defs/SnapshotDescriptor/properties/created_at",
            "/$defs/SnapshotDescriptor/properties/expires_at",
        ],
    ),
    Patch(
        Edit::Const("manuscript_search"),
        &["/properties/projection_kind"],
    ),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/properties/required_watermark"],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
