//! The rows of the Author Edit and Author Undo operations in the operation table.

use ts_rs::TS;

use super::{
    DATE_TIME, Edit, Fixture, Headers, OperationRow, Patch, SCHEMA_ID, SchemaSource, SchemaSources,
    U64, UUID, raw_schema,
};
use crate::ProjectActivityKind;
use crate::release1_author_edit::{
    APPLY_AUTHOR_EDIT, ApplyAuthorEditEffect, ApplyAuthorEditRequest, ApplyAuthorEditResponse,
    AuthorEditConflictReason, AuthorEditPrimitive, AuthorEditProposalTarget,
    AuthorEditRefusalReason, AuthorEditUnit, DomainReceipt, DomainReceiptCommandKind,
    DomainReceiptProducerCause, DomainReceiptResult, NoEffectReason, SelectionSnapshot,
};
use crate::release1_author_edit_outcome::{
    ApplyAuthorEditOutcome, ApplyAuthorEditReconfirmationReason, ApplyAuthorEditRejectionReason,
    ApplyAuthorEditUnknownObservation, GET_APPLY_AUTHOR_EDIT_OUTCOME,
    GetApplyAuthorEditOutcomeRequest, GetApplyAuthorEditOutcomeResponse,
};
use crate::release1_operation_registry::{ClientOverride, RegisteredOperation};
use crate::release1_route_catalog::{
    ActionClass, Admission, Binding, EditorRule, ErrorProfile, Idempotency, OperationCatalog,
    Owner, Settlement,
};
use crate::release1_undo_latest_author_action::{
    UNDO_LATEST_AUTHOR_ACTION, UndoLatestAuthorActionConflictReason, UndoLatestAuthorActionEffect,
    UndoLatestAuthorActionInput, UndoLatestAuthorActionRequest, UndoLatestAuthorActionResponse,
    UndoLatestAuthorActionUnavailableReason,
};
use crate::release1_wire::U64_WIRE;

pub(super) const APPLY_AUTHOR_EDIT_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &APPLY_AUTHOR_EDIT,
        &[
            "server_derived_project_scope",
            "strict_origin",
            "protected_client_session_binding",
            "project_command_challenge",
            "editor_session_writer_generation",
            "expected_authoritative_head",
        ],
        OperationCatalog::new(
            "manuscript:apply-author-edit",
            Owner::Core,
            Binding {
                action_class: ActionClass::DirectEditorAction,
                admission: Admission::AuthorCommand,
                editor: EditorRule::CurrentWriter,
                idempotency: Idempotency::HeaderAndAdmissionRecord,
                settlement: Settlement::CommittedOrReconfirmation,
                error_profile: ErrorProfile::EditorCommand,
            },
        )
        .activity(&[ProjectActivityKind::AuthoritativeAuthorEditApplied], &[])
        .notes(APPLY_AUTHOR_EDIT_NOTES),
    ),
    summary: "Settle one direct Author Edit",
    headers: Headers::NONE.origin().idempotency_key().anti_forgery(),
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "apply-author-edit-request",
            raw_schema::<ApplyAuthorEditRequest>,
            "StoryOS Apply Author Edit Request",
            APPLY_AUTHOR_EDIT_REQUEST,
        ),
        SchemaSource::typed(
            "apply-author-edit-response",
            raw_schema::<ApplyAuthorEditResponse>,
            "StoryOS Apply Author Edit Response",
            APPLY_AUTHOR_EDIT_RESPONSE,
        ),
    ),
    typescript: &[
        AuthorEditPrimitive::decl,
        SelectionSnapshot::decl,
        AuthorEditUnit::decl,
        AuthorEditProposalTarget::decl,
        ApplyAuthorEditRequest::decl,
        DomainReceiptCommandKind::decl,
        DomainReceiptProducerCause::decl,
        DomainReceiptResult::decl,
        DomainReceipt::decl,
        NoEffectReason::decl,
        AuthorEditConflictReason::decl,
        AuthorEditRefusalReason::decl,
        ApplyAuthorEditEffect::decl,
        ApplyAuthorEditResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
      "command_id": "018f0000-0000-7001-8000-000000000031",
      "completed_intent_record_id": "018f0000-0000-7001-8000-000000000036",
      "correlation_id": "018f0000-0000-7001-8000-000000000030",
      "effect": {
        "author_action_sequence": "1",
        "authoritative_commit_id": "018f0000-0000-7001-8000-000000000035",
        "authoritative_revision": {
          "blocks": [
            {
              "block_kind": "paragraph",
              "manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1",
              "text": "雨落在窗沿。"
            }
          ],
          "body": "雨落在窗沿。",
          "revision_id": "018f0000-0000-7001-8000-000000000034"
        },
        "kind": "authoritative_applied",
        "project_activity_position": "1"
      },
      "local_intent_sequence": "1",
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": "1",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
        "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000035"],
        "authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000034"],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.applyAuthorEdit.jcs.v1",
          "value_hex_lowercase": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        },
        "command_kind": "applyAuthorEdit",
        "condition_refs": [],
        "created_at": "2026-08-15T08:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": ["018f0000-0000-7001-8000-000000000004"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000037",
        "prior_heads": ["018f0000-0000-7001-8000-000000000004"],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000033",
        "result": "authoritative_applied",
        "resulting_heads": ["018f0000-0000-7001-8000-000000000034"]
      },
      "schema_id": "storyos.command.apply-author-edit.response.v2"
    }"#,
    ),
};

pub(super) const UNDO_LATEST_AUTHOR_ACTION_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::command(
        &UNDO_LATEST_AUTHOR_ACTION,
        &[
            "server_derived_project_scope",
            "project_active",
            "editor_session_writer_generation",
            "author_undo_frontier_exact",
            "expected_authoritative_head",
        ],
        OperationCatalog::new("author-action:undo", Owner::Core, Binding::EDITOR_COMMAND),
    ),
    summary: "Undo one exact admitted Author Action",
    headers: Headers::COMMAND,
    schemas: SchemaSources::new(
        SchemaSource::typed(
            "undo-latest-author-action-request",
            raw_schema::<UndoLatestAuthorActionRequest>,
            "StoryOS Undo Latest Author Action Request",
            UNDO_LATEST_AUTHOR_ACTION_REQUEST,
        ),
        SchemaSource::typed(
            "undo-latest-author-action-response",
            raw_schema::<UndoLatestAuthorActionResponse>,
            "StoryOS Undo Latest Author Action Response",
            UNDO_LATEST_AUTHOR_ACTION_RESPONSE,
        ),
    ),
    typescript: &[
        UndoLatestAuthorActionInput::decl,
        UndoLatestAuthorActionRequest::decl,
        UndoLatestAuthorActionConflictReason::decl,
        UndoLatestAuthorActionUnavailableReason::decl,
        UndoLatestAuthorActionEffect::decl,
        UndoLatestAuthorActionResponse::decl,
    ],
    fixture: Fixture::Json(
        r#"{
      "author_command_admission_id": "018f0000-0000-7001-8000-000000000932",
      "command_id": "018f0000-0000-7001-8000-000000000931",
      "correlation_id": "018f0000-0000-7001-8000-000000000930",
      "effect": {
        "author_action_sequence": "2",
        "authoritative_commit_id": "018f0000-0000-7001-8000-000000000807",
        "authoritative_revision": {
          "blocks": [
            {
              "block_kind": "paragraph",
              "manuscript_block_id": "018f0000-0000-7001-8000-000000000808",
              "text": "Hello"
            }
          ],
          "body": "Hello",
          "revision_id": "018f0000-0000-7001-8000-000000000806"
        },
        "kind": "compensated",
        "project_activity_position": "3",
        "source_sequence": "1"
      },
      "project": {
        "open": {
          "current_chapter_id": "018f0000-0000-7001-8000-000000000803",
          "kind": "current_chapter"
        },
        "project_id": "018f0000-0000-7001-8000-000000000002",
        "title": "Project A"
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "receipt": {
        "artifact_lifecycle_event_refs": [],
        "author_action_sequence": "2",
        "author_command_admission_id": "018f0000-0000-7001-8000-000000000932",
        "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000807"],
        "authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000806"],
        "command_digest": {
          "algorithm": "sha256",
          "profile": "storyos.command.undoLatestAuthorAction.jcs.v1",
          "value_hex_lowercase": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd"
        },
        "command_kind": "undoLatestAuthorAction",
        "condition_refs": [],
        "created_at": "2026-08-30T12:00:00.000Z",
        "draft_artifact_refs": [],
        "expected_heads": ["018f0000-0000-7001-8000-000000000805"],
        "idempotency_key": "018f0000-0000-7001-8000-000000000934",
        "prior_heads": ["018f0000-0000-7001-8000-000000000805"],
        "producer_cause": "author_command_admission",
        "project_scope": {
          "owner_user_id": "018f0000-0000-7001-8000-000000000001",
          "project_id": "018f0000-0000-7001-8000-000000000002"
        },
        "proposal_revision_ids": [],
        "receipt_id": "018f0000-0000-7001-8000-000000000933",
        "result": "authoritative_applied",
        "resulting_heads": ["018f0000-0000-7001-8000-000000000806"]
      },
      "schema_id": "storyos.command.undo-latest-author-action.response.v1"
    }"#,
    ),
};

pub(super) const GET_APPLY_AUTHOR_EDIT_OUTCOME_ROW: OperationRow = OperationRow {
    operation: RegisteredOperation::query(
        &GET_APPLY_AUTHOR_EDIT_OUTCOME,
        &[
            "server_derived_project_scope",
            "sensitive_safe_read_origin",
            "protected_client_session_binding",
            "idempotency_key",
            "project_command_challenge_proof",
            "receipt_first_settlement_validation",
        ],
        OperationCatalog::new(
            "apply-author-edit-outcome:read-by-idempotency-proof",
            Owner::Admission,
            Binding {
                action_class: ActionClass::None,
                admission: Admission::Query,
                editor: EditorRule::BoundToOriginalAdmission,
                idempotency: Idempotency::None,
                settlement: Settlement::Read,
                error_profile: ErrorProfile::QueryCanonical,
            },
        )
        .visibility(GET_APPLY_AUTHOR_EDIT_OUTCOME_VISIBILITY)
        .notes(GET_APPLY_AUTHOR_EDIT_OUTCOME_NOTES)
        .query_proof(&[
            ("identity", "apply_author_edit_idempotency_key"),
            ("header", "X-StoryOS-Anti-Forgery"),
            ("source", "original_project_command_challenge_nonce"),
            ("url", "forbidden"),
            ("response", "forbidden"),
            ("consumption", "none"),
            ("cache_control", "no-store"),
        ]),
    )
    .client(ClientOverride::Source {
        client: get_apply_author_edit_outcome_client_source,
        declaration: "export declare function getApplyAuthorEditOutcome(options: StoryOSQueryOptions & { projectId: string; idempotencyKey: string; antiForgery: string }): Promise<GetApplyAuthorEditOutcomeResponse>;\n",
    }),
    summary: "Read one exact Apply Author Edit outcome",
    headers: Headers::NONE.anti_forgery().no_store(),
    schemas: SchemaSources::new(
        SchemaSource::typed("apply-author-edit-outcome-request", raw_schema::<GetApplyAuthorEditOutcomeRequest>, "StoryOS Apply Author Edit Outcome Request", GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST),
        SchemaSource::typed("apply-author-edit-outcome-response", raw_schema::<GetApplyAuthorEditOutcomeResponse>, "StoryOS Apply Author Edit Outcome Response", GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE),
    ),
    typescript: &[GetApplyAuthorEditOutcomeRequest::decl, ApplyAuthorEditRejectionReason::decl, ApplyAuthorEditReconfirmationReason::decl, ApplyAuthorEditUnknownObservation::decl, ApplyAuthorEditOutcome::decl, GetApplyAuthorEditOutcomeResponse::decl],
    fixture: Fixture::Json(r#"{
      "correlation_id": "018f0000-0000-7001-8000-000000000081",
      "outcome": {
        "outcome_kind": "committed",
        "response": {
          "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
          "command_id": "018f0000-0000-7001-8000-000000000031",
          "completed_intent_record_id": "018f0000-0000-7001-8000-000000000036",
          "correlation_id": "018f0000-0000-7001-8000-000000000030",
          "effect": {
            "author_action_sequence": "1",
            "authoritative_commit_id": "018f0000-0000-7001-8000-000000000035",
            "authoritative_revision": {
              "blocks": [
                {
                  "block_kind": "paragraph",
                  "manuscript_block_id": "018f0000-0000-7001-8000-0000000000b1",
                  "text": "雨落在窗沿。"
                }
              ],
              "body": "雨落在窗沿。",
              "revision_id": "018f0000-0000-7001-8000-000000000034"
            },
            "kind": "authoritative_applied",
            "project_activity_position": "1"
          },
          "local_intent_sequence": "1",
          "project_scope": {
            "owner_user_id": "018f0000-0000-7001-8000-000000000001",
            "project_id": "018f0000-0000-7001-8000-000000000002"
          },
          "receipt": {
            "artifact_lifecycle_event_refs": [],
            "author_action_sequence": "1",
            "author_command_admission_id": "018f0000-0000-7001-8000-000000000032",
            "authoritative_commit_ids": ["018f0000-0000-7001-8000-000000000035"],
            "authoritative_revision_ids": ["018f0000-0000-7001-8000-000000000034"],
            "command_digest": {
              "algorithm": "sha256",
              "profile": "storyos.command.applyAuthorEdit.jcs.v1",
              "value_hex_lowercase": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "command_kind": "applyAuthorEdit",
            "condition_refs": [],
            "created_at": "2026-08-15T08:00:00.000Z",
            "draft_artifact_refs": [],
            "expected_heads": ["018f0000-0000-7001-8000-000000000004"],
            "idempotency_key": "018f0000-0000-7001-8000-000000000037",
            "prior_heads": ["018f0000-0000-7001-8000-000000000004"],
            "producer_cause": "author_command_admission",
            "project_scope": {
              "owner_user_id": "018f0000-0000-7001-8000-000000000001",
              "project_id": "018f0000-0000-7001-8000-000000000002"
            },
            "proposal_revision_ids": [],
            "receipt_id": "018f0000-0000-7001-8000-000000000033",
            "result": "authoritative_applied",
            "resulting_heads": ["018f0000-0000-7001-8000-000000000034"]
          },
          "schema_id": "storyos.command.apply-author-edit.response.v2"
        }
      },
      "project_scope": {
        "owner_user_id": "018f0000-0000-7001-8000-000000000001",
        "project_id": "018f0000-0000-7001-8000-000000000002"
      },
      "schema_id": "storyos.query.apply-author-edit-outcome.response.v1"
    }"#),
};

const APPLY_AUTHOR_EDIT_NOTES: &str = "This is the only author-edit ingress. Core classifies the whole command as AuthoritativeApplied, ProposalRevised, Refused, Conflicted, or NoEffect. The Web Client never chooses an authoritative-vs-Proposal route. A current single match may settle directly; selected multi-match or cross-location replacement remains on this route and is classified as a Core Proposal.";

const GET_APPLY_AUTHOR_EDIT_OUTCOME_NOTES: &str = "The protected Web Client may call this exact named GET after the first applyAuthorEdit acknowledgement is absent. It uses the client-first idempotency key plus the original challenge nonce only as a protected header, never as a URL or response value. Committed reuses the exact Receipt-backed response-v2 settlement; Rejected requires an expired unconsumed challenge under the arbiter locks; RequiresReconfirmation is the terminal no-Receipt settlement for an expired, binding-changed, or unrecoverable open Admission; StillUnknown grants no retry or success and is not the durable Admission outcome_unknown condition. The route is not applyAuthorEdit.settlement_query and creates no Admission, Command, idempotency record, or nonce. When the Query observes a clean open Admission, the Server may complete the already-admitted direct_editor_action or append RequiresReconfirmation.";

const GET_APPLY_AUTHOR_EDIT_OUTCOME_VISIBILITY: &str = "returns the current durable outcome; completing an already-admitted command may append that command's Receipt and Activity; this Query allocates no new command identity";

fn get_apply_author_edit_outcome_client_source() -> String {
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

const APPLY_AUTHOR_EDIT_REQUEST: &[Patch] = &[Patch(
    U64,
    &[
        "/properties/local_intent_sequence",
        "/properties/writer_generation",
    ],
)];

const APPLY_AUTHOR_EDIT_RESPONSE: &[Patch] = &[
    Patch(
        U64,
        &[
            "/$defs/ApplyAuthorEditEffect/oneOf/0/properties/author_action_sequence",
            "/$defs/ApplyAuthorEditEffect/oneOf/0/properties/project_activity_position",
            "/$defs/ApplyAuthorEditEffect/oneOf/1/properties/author_action_sequence",
            "/properties/local_intent_sequence",
        ],
    ),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
];

const UNDO_LATEST_AUTHOR_ACTION_REQUEST: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/$defs/UndoLatestAuthorActionInput/properties/correlation_id",
            "/$defs/UndoLatestAuthorActionInput/properties/editor_session_id",
            "/$defs/UndoLatestAuthorActionInput/properties/expected_authoritative_revision_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/command_schema"]),
];

const UNDO_LATEST_AUTHOR_ACTION_RESPONSE: &[Patch] = &[
    Patch(
        UUID,
        &[
            "/properties/author_command_admission_id",
            "/properties/command_id",
            "/properties/correlation_id",
        ],
    ),
    Patch(SCHEMA_ID, &["/properties/schema_id"]),
];

const GET_APPLY_AUTHOR_EDIT_OUTCOME_REQUEST: &[Patch] = &[Patch(
    UUID,
    &["/properties/idempotency_key", "/properties/project_id"],
)];

const GET_APPLY_AUTHOR_EDIT_OUTCOME_RESPONSE: &[Patch] = &[
    Patch(
        U64,
        &[
            "/$defs/ApplyAuthorEditEffect/oneOf/0/properties/author_action_sequence",
            "/$defs/ApplyAuthorEditEffect/oneOf/0/properties/project_activity_position",
            "/$defs/ApplyAuthorEditEffect/oneOf/1/properties/author_action_sequence",
            "/$defs/ApplyAuthorEditResponse/properties/local_intent_sequence",
        ],
    ),
    Patch(
        DATE_TIME,
        &["/$defs/ApplyAuthorEditUnknownObservation/oneOf/0/properties/expires_at"],
    ),
    Patch(
        Edit::NullablePattern(U64_WIRE),
        &["/$defs/DomainReceipt/properties/author_action_sequence"],
    ),
];
