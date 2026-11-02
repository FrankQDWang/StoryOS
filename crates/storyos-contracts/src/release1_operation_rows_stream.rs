//! The rows of the Snapshot query and Project Activity stream in the operation table.

use ts_rs::TS;

use super::{
    DATE_TIME, Fixture, Headers, OperationRow, Patch, SchemaSource, SchemaSources, U64, UUID,
    raw_schema,
};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};
use crate::release1_snapshot::{
    ACTIVITY_STREAM, ActivityStreamRequest, CanonicalSnapshotMaps, GET_SNAPSHOT,
    GetSnapshotRequest, GetSnapshotResponse, SnapshotDescriptor, SnapshotKind,
};

pub(super) const GET_SNAPSHOT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_SNAPSHOT,
        &[
            "snapshot_scope_join",
            "snapshot_signature",
            "snapshot_lifecycle_available",
        ],
        OperationCatalog::new("snapshot:read", Owner::Replay, Binding::CANONICAL_QUERY)
            .notes(GET_SNAPSHOT_NOTES),
    ),
    summary: "Read one authorized canonical Snapshot",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "snapshot-request",
            raw_schema::<GetSnapshotRequest>,
            "StoryOS Snapshot Request",
            GET_SNAPSHOT_REQUEST,
        ),
        SchemaSource::typed(
            "snapshot-response",
            raw_schema::<GetSnapshotResponse>,
            "StoryOS Snapshot Response",
            GET_SNAPSHOT_RESPONSE,
        ),
    ),
    typescript: &[
        SnapshotKind::decl,
        CanonicalSnapshotMaps::decl,
        SnapshotDescriptor::decl,
        GetSnapshotRequest::decl,
        GetSnapshotResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000091",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.query.snapshot.response.v1",
      "snapshot": {
        "created_at": "2026-07-21T10:16:30.000Z",
        "expires_at": null,
        "project_activity_position": "0",
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

pub(super) const ACTIVITY_STREAM_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::stream(
            &ACTIVITY_STREAM,
            &[
                "server_derived_project_scope",
                "snapshot_binding_or_last_event_id",
                "activity_profile",
                "replay_generation",
                "filter_digest",
                "reauthorize_on_connect",
            ],
            OperationCatalog::new(
                "project-activity:sse",
                Owner::Replay,
                Binding {
                    action_class: ActionClass::None,
                    admission: Admission::Sse,
                    editor: EditorRule::Optional,
                    idempotency: Idempotency::StreamCursor,
                    settlement: Settlement::Stream,
                    error_profile: ErrorProfile::Stream,
                },
            )
            .notes(ACTIVITY_STREAM_NOTES),
        )
        .client(ClientOverride::Source {
            client: activity_stream_client_source,
            declaration: "export declare function activityStream(options: StoryOSQueryOptions & { projectId: string; snapshotId: string; protocolRelease: string; lastEventId?: string }): Promise<string>;\n",
        }),
    summary: "Stream Project Activity after an authorized Snapshot",
    headers: Headers::NONE,
    schemas: SchemaSources::new(
        SchemaSource::typed("activity-stream-request", raw_schema::<ActivityStreamRequest>, "StoryOS Activity Stream Request", ACTIVITY_STREAM_REQUEST),
        SchemaSource::json(
            "activity-stream-response",
            "StoryOS Activity Stream SSE Frame",
            r##"{
              "$defs": {
                "DigestValue": {
                  "additionalProperties": false,
                  "properties": {
                    "algorithm": {"const": "sha256", "type": "string"},
                    "profile": {"minLength": 1, "type": "string"},
                    "value_hex_lowercase": {"maxLength": 64, "minLength": 64, "type": "string"}
                  },
                  "required": ["algorithm", "profile", "value_hex_lowercase"],
                  "type": "object"
                },
                "IdentityRef": {
                  "additionalProperties": false,
                  "properties": {
                    "id": {"minLength": 1, "type": "string"},
                    "kind": {"minLength": 1, "type": "string"}
                  },
                  "required": ["kind", "id"],
                  "type": "object"
                },
                "ProjectActivityEvent": {
                  "additionalProperties": false,
                  "properties": {
                    "activity_profile": {"const": "storyos.project-activity.v1", "type": "string"},
                    "actor": {"$ref": "#/$defs/IdentityRef"},
                    "agent_run_id": {"format": "uuid", "type": ["string", "null"]},
                    "aggregate_ref": {"anyOf": [{"$ref": "#/$defs/IdentityRef"}, {"type": "null"}]},
                    "application_wire_record_ref": {"minLength": 1, "type": "string"},
                    "causation": {"$ref": "#/$defs/IdentityRef"},
                    "command_id": {"format": "uuid", "type": ["string", "null"]},
                    "correlation_id": {"format": "uuid", "type": "string"},
                    "envelope_version": {"const": 1, "type": "integer"},
                    "event_id": {"format": "uuid", "type": "string"},
                    "event_kind": {"minLength": 1, "type": "string"},
                    "event_schema": {"minLength": 1, "type": "string"},
                    "limit_profile_revision": {"minLength": 1, "type": "string"},
                    "occurred_at": {"format": "date-time", "type": "string"},
                    "payload": {"type": "object"},
                    "payload_digest": {"$ref": "#/$defs/DigestValue"},
                    "project_scope": {"$ref": "#/$defs/ProjectScope"},
                    "project_sequence": {
                      "pattern": "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$",
                      "type": "string"
                    },
                    "receipt_ref": {"anyOf": [{"$ref": "#/$defs/ReceiptRef"}, {"type": "null"}]},
                    "recorded_at": {"format": "date-time", "type": "string"},
                    "requester_user_id": {"format": "uuid", "type": "string"},
                    "run_sequence": {
                      "anyOf": [
                        {
                          "pattern": "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$",
                          "type": "string"
                        },
                        {"type": "null"}
                      ]
                    },
                    "run_step_id": {"format": "uuid", "type": ["string", "null"]},
                    "stream_sequence": {
                      "pattern": "^(?:0|[1-9][0-9]{0,18}|1[0-7][0-9]{18}|18[0-3][0-9]{17}|184[0-3][0-9]{16}|1844[0-5][0-9]{15}|18446[0-6][0-9]{14}|184467[0-3][0-9]{13}|1844674[0-3][0-9]{12}|184467440[0-6][0-9]{10}|1844674407[0-2][0-9]{9}|18446744073[0-6][0-9]{8}|1844674407370[0-8][0-9]{6}|18446744073709[0-4][0-9]{5}|184467440737095[0-4][0-9]{3}|1844674407370955[0-9]{2}|18446744073709551[0-5]|1844674407370955160|1844674407370955161[0-5])$",
                      "type": "string"
                    }
                  },
                  "required": [
                    "envelope_version",
                    "activity_profile",
                    "event_id",
                    "event_schema",
                    "event_kind",
                    "project_scope",
                    "requester_user_id",
                    "actor",
                    "project_sequence",
                    "stream_sequence",
                    "agent_run_id",
                    "run_step_id",
                    "run_sequence",
                    "aggregate_ref",
                    "correlation_id",
                    "causation",
                    "command_id",
                    "receipt_ref",
                    "occurred_at",
                    "recorded_at",
                    "payload",
                    "payload_digest",
                    "application_wire_record_ref",
                    "limit_profile_revision"
                  ],
                  "type": "object"
                },
                "ProjectScope": {
                  "additionalProperties": false,
                  "properties": {
                    "owner_user_id": {"format": "uuid", "type": "string"},
                    "project_id": {"format": "uuid", "type": "string"}
                  },
                  "required": ["owner_user_id", "project_id"],
                  "type": "object"
                },
                "ReceiptRef": {
                  "oneOf": [
                    {
                      "additionalProperties": false,
                      "properties": {
                        "id": {"minLength": 1, "type": "string"},
                        "kind": {"const": "domain_receipt", "type": "string"}
                      },
                      "required": ["kind", "id"],
                      "type": "object"
                    },
                    {
                      "additionalProperties": false,
                      "properties": {
                        "id": {"minLength": 1, "type": "string"},
                        "kind": {"const": "validation_receipt", "type": "string"}
                      },
                      "required": ["kind", "id"],
                      "type": "object"
                    },
                    {
                      "additionalProperties": false,
                      "properties": {
                        "id": {"minLength": 1, "type": "string"},
                        "kind": {"const": "acceptance_receipt", "type": "string"}
                      },
                      "required": ["kind", "id"],
                      "type": "object"
                    },
                    {
                      "additionalProperties": false,
                      "properties": {
                        "id": {"minLength": 1, "type": "string"},
                        "kind": {"const": "undo_acceptance_receipt", "type": "string"}
                      },
                      "required": ["kind", "id"],
                      "type": "object"
                    },
                    {
                      "additionalProperties": false,
                      "properties": {
                        "id": {"minLength": 1, "type": "string"},
                        "kind": {"const": "author_undo_receipt", "type": "string"}
                      },
                      "required": ["kind", "id"],
                      "type": "object"
                    }
                  ]
                }
              },
              "$schema": "https://json-schema.org/draft/2020-12/schema",
              "additionalProperties": false,
              "properties": {
                "data": {"$ref": "#/$defs/ProjectActivityEvent"},
                "event": {"const": "storyos.project-activity", "type": "string"},
                "id": {"minLength": 1, "type": "string"}
              },
              "required": ["id", "event", "data"],
              "type": "object"
            }"##,
        ),
    ),
    typescript: &[ActivityStreamRequest::decl],
    fixture: Fixture::Json(r#"{
      "data": {
        "activity_profile": "storyos.project-activity.v1",
        "actor": {"id": "018f0000-0000-7001-8000-000000000001", "kind": "author"},
        "agent_run_id": null,
        "aggregate_ref": {"id": "018f0000-0000-7001-8000-000000000042", "kind": "proposal"},
        "application_wire_record_ref": "018f0000-0000-7001-8000-000000000046",
        "causation": {"id": "018f0000-0000-7001-8000-000000000011", "kind": "command"},
        "command_id": "018f0000-0000-7001-8000-000000000011",
        "correlation_id": "018f0000-0000-7001-8000-000000000043",
        "envelope_version": 1,
        "event_id": "018f0000-0000-7001-8000-000000000041",
        "event_kind": "proposal_conflict_detected",
        "event_schema": "storyos.event.proposal-conflict-detected.v1",
        "limit_profile_revision": "storyos.foundation.absolute.v1",
        "occurred_at": "2026-07-21T10:17:00.000Z",
        "payload": {
          "observed_head_revision_id": "018f0000-0000-7001-8000-000000000045",
          "proposal_id": "018f0000-0000-7001-8000-000000000042"
        },
        "payload_digest": {
          "algorithm": "sha256",
          "profile": "storyos.event-payload.jcs.v1",
          "value_hex_lowercase": "073c643d461f8ce5d6a865fded918411ba1888f20cfd9a6a6418b1480cd9c62b"
        },
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "project_sequence": "44",
        "receipt_ref": {"id": "018f0000-0000-7001-8000-000000000044", "kind": "domain_receipt"},
        "recorded_at": "2026-07-21T10:17:00.005Z",
        "requester_user_id": "018f0000-0000-7001-8000-000000000001",
        "run_sequence": null,
        "run_step_id": null,
        "stream_sequence": "44"
      },
      "event": "storyos.project-activity",
      "id": "eyJwcm9maWxlIjoic3Rvcnlvcy1zY29wZWQtY3Vyc29yIn0"
    }"#),
};

const GET_SNAPSHOT_NOTES: &str = "Expired or unavailable Snapshot returns snapshot_expired and a safe resync query; the Server never mixes page boundaries.";

const ACTIVITY_STREAM_NOTES: &str = "SSE id is a scoped cursor, not Event identity or a bearer capability; replay-floor misses return activity_cursor_too_old before stream start.";

fn activity_stream_client_source() -> String {
    let path = ACTIVITY_STREAM
        .path
        .replace("{project_id}", "${encodeURIComponent(projectId)}");
    format!(
        concat!(
            "\nexport async function activityStream({{ projectId, snapshotId, protocolRelease, lastEventId, ...options }} = {{}}) {{\n",
            "  if (typeof projectId !== \"string\" || projectId.length === 0) throw new TypeError(\"activityStream requires projectId\");\n",
            "  if (typeof snapshotId !== \"string\" || typeof protocolRelease !== \"string\") throw new TypeError(\"activityStream requires snapshot bindings\");\n",
            "  const headers = {{ accept: \"text/event-stream\" }};\n",
            "  if (typeof lastEventId === \"string\") headers[\"last-event-id\"] = lastEventId;\n",
            "  return queryText({{ ...options, path: `{}?snapshot_id=${{encodeURIComponent(snapshotId)}}&protocol_release=${{encodeURIComponent(protocolRelease)}}`, queryHeaders: headers }});\n",
            "}}\n",
        ),
        path,
    )
}

const GET_SNAPSHOT_REQUEST: &[Patch] = &[Patch(
    UUID,
    &["/properties/project_id", "/properties/snapshot_id"],
)];

const GET_SNAPSHOT_RESPONSE: &[Patch] = &[
    Patch(
        DATE_TIME,
        &[
            "/$defs/SnapshotDescriptor/properties/created_at",
            "/$defs/SnapshotDescriptor/properties/expires_at",
        ],
    ),
    Patch(
        U64,
        &[
            "/$defs/SnapshotDescriptor/properties/project_activity_position",
            "/$defs/SnapshotDescriptor/properties/replay_generation",
        ],
    ),
];

const ACTIVITY_STREAM_REQUEST: &[Patch] = &[Patch(
    UUID,
    &["/properties/project_id", "/properties/snapshot_id"],
)];
