//! The rows of the Editor Session operations in the operation table.

use ts_rs::TS;

use super::{
    Fixture, Headers, NO_PATCHES, OperationRow, Patch, SchemaSource, SchemaSources, U64, raw_schema,
};
use crate::release1::{
    CREATE_EDITOR_SESSION, CreateEditorSessionRequest, CreateEditorSessionResponse,
    EditorBaseSnapshot, EditorSessionBinding, GET_EDITOR_SESSION, GetEditorSessionResponse,
};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};

pub(super) const CREATE_EDITOR_SESSION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &CREATE_EDITOR_SESSION,
        &[
            "server_derived_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "project_command_challenge",
        ],
        OperationCatalog::new(
            "editor-session:open",
            Owner::Editor,
            Binding {
                action_class: ActionClass::SessionControl,
                admission: Admission::SessionControl,
                editor: EditorRule::NewSession,
                idempotency: Idempotency::HeaderAndOperationRecord,
                settlement: Settlement::Committed,
                error_profile: ErrorProfile::SessionControl,
            },
        )
        .notes(CREATE_EDITOR_SESSION_NOTES),
    )
    .client(ClientOverride::DigestFields(&[
        "client_contract_revision",
        "command_schema",
        "correlation_id",
        "security_policy_revision",
    ])),
    summary: "Create one Editor Session",
    headers: Headers::NONE.origin().idempotency_key().anti_forgery(),
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "editor-session-create-request",
            raw_schema::<CreateEditorSessionRequest>,
            "StoryOS Create Editor Session Request",
            NO_PATCHES,
        ),
        SchemaSource::typed(
            "editor-session-create-response",
            raw_schema::<CreateEditorSessionResponse>,
            "StoryOS Create Editor Session Response",
            CREATE_EDITOR_SESSION_RESPONSE,
        ),
    ),
    typescript: &[
        CreateEditorSessionRequest::decl,
        EditorSessionBinding::decl,
        EditorBaseSnapshot::decl,
        CreateEditorSessionResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "base_snapshot": {
        "authoritative_head_revision_id": "018f0000-0000-7001-8000-000000000004",
        "chapter_id": "018f0000-0000-7001-8000-000000000003",
        "created_at": "2026-08-13T08:00:00.000Z",
        "materialized_payload_digest": {
          "algorithm": "sha256",
          "profile": "storyos.canonical-payload.sha256.v1",
          "value_hex_lowercase": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        },
        "materialized_revision": {
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
        "observed_ownership_partition": "authoritative",
        "project_activity_position": "0",
        "proposal_head_revision_ids": [],
        "snapshot_id": "018f0000-0000-7001-8000-000000000022",
        "target_refs": ["manuscript:018f0000-0000-7001-8000-000000000003"]
      },
      "correlation_id": "018f0000-0000-7001-8000-000000000020",
      "editor_session": {
        "client_contract_revision": "storyos.web-client.release-1.v3",
        "client_session_binding_ref": "binding:7af2",
        "client_session_generation": "1",
        "disposition": "open",
        "editor_session_id": "018f0000-0000-7001-8000-000000000021",
        "opened_at": "2026-08-13T08:00:00.000Z",
        "security_policy_revision": "storyos.web-security-policy.release-1.v1"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.command.create-editor-session.response.v2"
    }"#,
    ),
};

pub(super) const GET_EDITOR_SESSION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_EDITOR_SESSION,
        &[
            "server_derived_project_scope",
            "session_scope_join",
            "protected_client_session_binding",
        ],
        OperationCatalog::new(
            "editor-session:read",
            Owner::Editor,
            Binding {
                action_class: ActionClass::None,
                admission: Admission::Query,
                editor: EditorRule::Required,
                idempotency: Idempotency::None,
                settlement: Settlement::Read,
                error_profile: ErrorProfile::QueryCanonical,
            },
        ),
    ),
    summary: "Read one exact Editor Session",
    headers: Headers::NONE,
    schemas: SchemaSources::get(SchemaSource::typed(
        "editor-session-get-response",
        raw_schema::<GetEditorSessionResponse>,
        "StoryOS Get Editor Session Response",
        GET_EDITOR_SESSION_RESPONSE,
    )),
    typescript: &[GetEditorSessionResponse::decl],
    fixture: Fixture::Json(
        r#"{
      "base_snapshot": {
        "authoritative_head_revision_id": "018f0000-0000-7001-8000-000000000004",
        "chapter_id": "018f0000-0000-7001-8000-000000000003",
        "created_at": "2026-08-13T08:00:00.000Z",
        "materialized_payload_digest": {
          "algorithm": "sha256",
          "profile": "storyos.canonical-payload.sha256.v1",
          "value_hex_lowercase": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        },
        "materialized_revision": {
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
        "observed_ownership_partition": "authoritative",
        "project_activity_position": "0",
        "proposal_head_revision_ids": [],
        "snapshot_id": "018f0000-0000-7001-8000-000000000022",
        "target_refs": ["manuscript:018f0000-0000-7001-8000-000000000003"]
      },
      "correlation_id": "018f0000-0000-7001-8000-000000000020",
      "editor_session": {
        "client_contract_revision": "storyos.web-client.release-1.v3",
        "client_session_binding_ref": "binding:7af2",
        "client_session_generation": "1",
        "disposition": "open",
        "editor_session_id": "018f0000-0000-7001-8000-000000000021",
        "opened_at": "2026-08-13T08:00:00.000Z",
        "security_policy_revision": "storyos.web-security-policy.release-1.v1"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.query.editor-session.response.v2"
    }"#,
    ),
};

const CREATE_EDITOR_SESSION_NOTES: &str = "Opening a session is a session Operational Record, not an author-owned Core command and not an Author Command Admission.";

const CREATE_EDITOR_SESSION_RESPONSE: &[Patch] = &[Patch(
    U64,
    &[
        "/$defs/EditorBaseSnapshot/properties/project_activity_position",
        "/$defs/EditorSessionBinding/properties/client_session_generation",
    ],
)];

const GET_EDITOR_SESSION_RESPONSE: &[Patch] = &[Patch(
    U64,
    &[
        "/$defs/EditorBaseSnapshot/properties/project_activity_position",
        "/$defs/EditorSessionBinding/properties/client_session_generation",
    ],
)];
