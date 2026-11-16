//! The rows of the Project Conversation queries in the operation table (ADR 0094 section 10).

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources,
    UUID, raw_schema,
};
use crate::release1_operation_registry::RegisteredOperation;
use crate::release1_project_conversation::{
    GET_PROJECT_CONVERSATION, GetProjectConversationResponse, LIST_PROJECT_CONVERSATIONS,
    ListProjectConversationsResponse, ProjectConversationCompleteness, ProjectConversationEntry,
    ProjectConversationRun,
};
use crate::release1_route_catalog::{Binding, OperationCatalog, Owner};

pub(super) const LIST_PROJECT_CONVERSATIONS_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &LIST_PROJECT_CONVERSATIONS,
        &[
            "server_derived_project_scope",
            "bounded_project_conversation_list",
        ],
        OperationCatalog::new("conversation:list", Owner::Core, Binding::CANONICAL_QUERY),
    ),
    summary: "List the Project Conversations of one Project",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "project-conversations-response",
        raw_schema::<ListProjectConversationsResponse>,
        "StoryOS Project Conversations Response",
        LIST_PROJECT_CONVERSATIONS_RESPONSE,
    )),
    typescript: &[
        ProjectConversationCompleteness::decl,
        ProjectConversationEntry::decl,
        ListProjectConversationsResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "completeness": "complete",
      "conversations": [
        {
          "conversation_id": "018f0000-0000-7001-8000-000000000d02",
          "first_run_created_at": "2026-11-16T09:30:00.000Z",
          "label": "Help with this passage.",
          "newest_run_created_at": "2026-11-16T10:15:00.000Z",
          "newest_run_status": "completed"
        }
      ],
      "correlation_id": "018f0000-0000-7001-8000-000000000d01",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "schema_id": "storyos.query.project-conversations.response.v1"
    }"#,
    ),
};

pub(super) const GET_PROJECT_CONVERSATION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_PROJECT_CONVERSATION,
        &[
            "server_derived_project_scope",
            "conversation_scope_join",
            "bounded_project_conversation_runs",
        ],
        OperationCatalog::new("conversation:read", Owner::Core, Binding::CANONICAL_QUERY),
    ),
    summary: "Read the Runs of one Project Conversation",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "project-conversation-response",
        raw_schema::<GetProjectConversationResponse>,
        "StoryOS Project Conversation Response",
        GET_PROJECT_CONVERSATION_RESPONSE,
    )),
    typescript: &[
        ProjectConversationRun::decl,
        GetProjectConversationResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "completeness": "complete",
      "conversation_id": "018f0000-0000-7001-8000-000000000d02",
      "correlation_id": "018f0000-0000-7001-8000-000000000d11",
      "earlier_turns_expired": false,
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000201"
      },
      "runs": [
        {
          "author_message": "Help with this passage.",
          "chapter_id": "018f0000-0000-7001-8000-000000000301",
          "created_at": "2026-11-16T09:30:00.000Z",
          "run_id": "018f0000-0000-7001-8000-000000000d12",
          "status": "completed"
        },
        {
          "author_message": "Keep the voice of the narrator.",
          "chapter_id": "018f0000-0000-7001-8000-000000000301",
          "created_at": "2026-11-16T10:15:00.000Z",
          "run_id": "018f0000-0000-7001-8000-000000000d13",
          "status": "completed"
        }
      ],
      "schema_id": "storyos.query.project-conversation.response.v1"
    }"#,
    ),
};

const LIST_PROJECT_CONVERSATIONS_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectConversationEntry/properties/conversation_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        DATE_TIME,
        &[
            "/$defs/ProjectConversationEntry/properties/first_run_created_at",
            "/$defs/ProjectConversationEntry/properties/newest_run_created_at",
        ],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/ProjectConversationEntry/properties/label"],
    ),
    Patch(
        Edit::MaxLength(80),
        &["/$defs/ProjectConversationEntry/properties/label"],
    ),
    Patch(Edit::MaxItems(64), &["/properties/conversations"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const GET_PROJECT_CONVERSATION_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/ProjectConversationRun/properties/chapter_id",
            "/$defs/ProjectConversationRun/properties/run_id",
            "/$defs/ProjectScope/properties/owner_user_id",
            "/$defs/ProjectScope/properties/project_id",
            "/properties/conversation_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(
        DATE_TIME,
        &["/$defs/ProjectConversationRun/properties/created_at"],
    ),
    Patch(
        Edit::MinLength(1),
        &["/$defs/ProjectConversationRun/properties/author_message"],
    ),
    Patch(
        Edit::MaxLength(8000),
        &["/$defs/ProjectConversationRun/properties/author_message"],
    ),
    Patch(Edit::MaxItems(64), &["/properties/runs"]),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];
