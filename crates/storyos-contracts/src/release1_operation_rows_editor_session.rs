//! The rows of the Editor Session and writer takeover operations in the operation table.

use ts_rs::TS;

use super::{
    Edit, Fixture, Headers, NO_PATCHES, OperationRow, Patch, SchemaSource, SchemaSources, U64,
    raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1::{
    CREATE_EDITOR_SESSION, CreateEditorSessionRequest, CreateEditorSessionResponse,
    EditorBaseSnapshot, EditorReadOnlyReason, EditorSessionBinding, EditorWriterProjection,
    GET_EDITOR_SESSION, GetEditorSessionResponse,
};
use crate::release1_operation_registry::{
    ClientOverride, ContractGraphEntry, OperationKind, RegisteredOperation,
};
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};
use crate::release1_takeover::{
    TAKE_OVER_PROJECT_WRITER, TakeOverProjectWriterRequest, TakeOverProjectWriterResponse,
    TakeOverProjectWriterResult, TakeoverCompareFailedReason,
};
use crate::release1_wire::U64_WIRE;

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
        EditorReadOnlyReason::decl,
        EditorWriterProjection::decl,
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
      "schema_id": "storyos.command.create-editor-session.response.v1",
      "writer": {"kind": "current_writer", "writer_generation": "1"}
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
                editor: EditorRule::ObservedWriter,
                idempotency: Idempotency::None,
                settlement: Settlement::Read,
                error_profile: ErrorProfile::QueryCanonical,
            },
        ),
    ),
    summary: "Read one exact Editor Session",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::path_parameters("editor-session-get-request"),
        SchemaSource::typed(
            "editor-session-get-response",
            raw_schema::<GetEditorSessionResponse>,
            "StoryOS Get Editor Session Response",
            GET_EDITOR_SESSION_RESPONSE,
        ),
    ),
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
      "schema_id": "storyos.query.editor-session.response.v1",
      "writer": {"kind": "current_writer", "writer_generation": "1"}
    }"#,
    ),
};

pub(super) const TAKE_OVER_PROJECT_WRITER_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation {
        client: ClientOverride::None,
        operation: &TAKE_OVER_PROJECT_WRITER,
        kind: OperationKind::Command,
        graph: ContractGraphEntry::Absent(&[
            "server_derived_project_scope",
            "session_scope_join",
            "observed_writer_generation_current_before_admission",
            "editor_contract_revision",
        ]),
        catalog: OperationCatalog::new(
            "writer:takeover",
            Owner::Editor,
            Binding {
                action_class: ActionClass::ExplicitEditorCommand,
                admission: Admission::AuthorCommand,
                editor: EditorRule::ExplicitTakeover,
                idempotency: Idempotency::HeaderAndAdmissionRecord,
                settlement: Settlement::CommittedOrReconfirmation,
                error_profile: ErrorProfile::EditorCommand,
            },
        )
        .activity(
            &[ProjectActivityKind::WriterTakeoverApplied],
            &[ProjectActivityKind::WriterTakeoverCompareFailed],
        )
        .notes(TAKE_OVER_PROJECT_WRITER_NOTES),
    },
    summary: "Take over the current Project writer generation",
    headers: Headers::NONE.origin().idempotency_key().anti_forgery(),
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "take-over-project-writer-request",
            raw_schema::<TakeOverProjectWriterRequest>,
            "StoryOS Take Over Project Writer Request",
            TAKE_OVER_PROJECT_WRITER_REQUEST,
        ),
        SchemaSource::typed(
            "take-over-project-writer-response",
            raw_schema::<TakeOverProjectWriterResponse>,
            "StoryOS Take Over Project Writer Response",
            TAKE_OVER_PROJECT_WRITER_RESPONSE,
        ),
    ),
    typescript: &[
        TakeOverProjectWriterRequest::decl,
        TakeoverCompareFailedReason::decl,
        TakeOverProjectWriterResult::decl,
        TakeOverProjectWriterResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000042",
      "command_id": "018f0000-0000-7001-8000-000000000041",
      "correlation_id": "018f0000-0000-7001-8000-000000000040",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": null,
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000042",
        "authoritative_commit_ids": [],
        "authoritative_revision_ids": [],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.takeOverProjectWriter.jcs.v1",
          "value_hex_lowercase": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
        },
        "command_kind": "takeOverProjectWriter",
        "condition_refs": [],
        "created_at": "2026-08-19T06:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": ["018f0000-0000-7001-8000-000000000004"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000044",
        "prior_heads": ["018f0000-0000-7001-8000-000000000004"],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000043",
        "result": "no_effect",
        "resulting_heads": ["018f0000-0000-7001-8000-000000000004"]
      },
      "result": {
        "kind": "takeover_applied",
        "prior_editor_session_id": "018f0000-0000-7001-8000-000000000021",
        "prior_writer_generation": "1",
        "resulting_editor_session_id": "018f0000-0000-7001-8000-000000000040",
        "resulting_heads": ["018f0000-0000-7001-8000-000000000004"],
        "resulting_snapshot_activity_position": "1",
        "resulting_snapshot_id": "018f0000-0000-7001-8000-000000000032",
        "resulting_writer_generation": "2"
      },
      "schema_id": "storyos.command.take-over-project-writer.response.v1"
    }"#,
    ),
};

const CREATE_EDITOR_SESSION_NOTES: &str = "Opening a session is a session Operational Record, not an author-owned Core command and not an Author Command Admission.";

const TAKE_OVER_PROJECT_WRITER_NOTES: &str = "Both TakeoverApplied and TakeoverCompareFailed are Receipt-backed Project Activity outcomes; stale pre-admission observation has no Admission or Receipt.";

const CREATE_EDITOR_SESSION_RESPONSE: &[Patch] = &[Patch(
    U64,
    &[
        "/$defs/EditorBaseSnapshot/properties/project_activity_position",
        "/$defs/EditorSessionBinding/properties/client_session_generation",
        "/$defs/EditorWriterProjection/oneOf/0/properties/writer_generation",
        "/$defs/EditorWriterProjection/oneOf/1/properties/observed_writer_generation",
    ],
)];

const GET_EDITOR_SESSION_RESPONSE: &[Patch] = &[Patch(
    U64,
    &[
        "/$defs/EditorBaseSnapshot/properties/project_activity_position",
        "/$defs/EditorSessionBinding/properties/client_session_generation",
        "/$defs/EditorWriterProjection/oneOf/0/properties/writer_generation",
        "/$defs/EditorWriterProjection/oneOf/1/properties/observed_writer_generation",
    ],
)];

const TAKE_OVER_PROJECT_WRITER_REQUEST: &[Patch] =
    &[Patch(U64, &["/properties/observed_writer_generation"])];

const TAKE_OVER_PROJECT_WRITER_RESPONSE: &[Patch] = &[
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
    Patch(
        U64,
        &[
            "/$defs/EditorWriterProjection/oneOf/0/properties/writer_generation",
            "/$defs/EditorWriterProjection/oneOf/1/properties/observed_writer_generation",
            "/$defs/TakeOverProjectWriterResult/oneOf/0/properties/prior_writer_generation",
            "/$defs/TakeOverProjectWriterResult/oneOf/0/properties/resulting_snapshot_activity_position",
            "/$defs/TakeOverProjectWriterResult/oneOf/0/properties/resulting_writer_generation",
            "/$defs/TakeOverProjectWriterResult/oneOf/1/properties/current_snapshot_activity_position",
            "/$defs/TakeOverProjectWriterResult/oneOf/1/properties/current_writer_generation",
            "/$defs/TakeOverProjectWriterResult/oneOf/1/properties/observed_writer_generation",
        ],
    ),
];
