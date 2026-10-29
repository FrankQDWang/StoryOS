//! The catalog-level sections of the route catalog.

use serde_json::{Value, json};

use super::{ErrorProfile, Owner};
use crate::release1::{
    COMPATIBILITY_PROFILE, CONTRACT_REVISION, PUBLIC_PROTOCOL_RELEASE, RELEASE_IDENTITY_SCHEMA_ID,
};

pub(super) const ERROR_SOURCE: &str = "docs/foundation/versioned-command-query-artifact-event-protocol.md#11-problem-details-and-stable-errors";

impl Owner {
    pub(super) const ALL: [Self; 13] = [
        Self::Protocol,
        Self::Core,
        Self::Admission,
        Self::Editor,
        Self::Assistance,
        Self::Artifact,
        Self::Replay,
        Self::Storage,
        Self::Retention,
        Self::Deletion,
        Self::Release,
        Self::Context,
        Self::Memory,
    ];

    /// The tracked document, and its anchor when it has one, that owns the meaning.
    pub(super) fn source(self) -> &'static str {
        match self {
            Self::Protocol => {
                "docs/foundation/versioned-command-query-artifact-event-protocol.md#16-rust-source-generation-and-verification-contract"
            }
            Self::Core => {
                "docs/foundation/manuscript-revision-proposal-state-machine.md#13-normative-commands-events-and-records"
            }
            Self::Admission => {
                "docs/foundation/author-command-admission.md#3-exact-admission-bindings"
            }
            Self::Editor => {
                "docs/foundation/web-editor-session-synchronization-and-recovery-semantics.md#22-one-current-writer-generation-per-project"
            }
            Self::Assistance => {
                "docs/foundation/plain-language-discovery-writing-assistance-semantics.md#4-prose-change-request-and-proposal-handoff"
            }
            Self::Artifact => "docs/foundation/artifact-domain-model.md#1-purpose",
            Self::Replay => {
                "docs/adr/0009-require-snapshot-resync-at-replay-generation-boundaries.md"
            }
            Self::Storage => {
                "docs/foundation/postgresql-project-storage-isolation-and-migration-contract.md#1-scope-and-authority"
            }
            Self::Retention => {
                "docs/foundation/run-event-mailbox-snapshot-retention-and-archival-semantics.md#10-explicit-project-deletion-lifecycle"
            }
            Self::Deletion => "docs/adr/0011-require-explicit-project-deletion-settlement.md",
            Self::Release => {
                "docs/foundation/ai-independent-editor-first-release-baseline-and-handoff-criteria.md#3-complete-acceptance-journey"
            }
            Self::Context => {
                "docs/foundation/context-assembly-retrieval-and-outbound-disclosure-semantics.md#15-required-verification-scenarios"
            }
            Self::Memory => {
                "docs/foundation/fiction-memory-and-research-provenance-semantics.md#11-author-interaction"
            }
        }
    }
}

impl ErrorProfile {
    const ALL: [Self; 11] = [
        Self::CommandSync,
        Self::EditorCommand,
        Self::AsyncCommand,
        Self::SessionControl,
        Self::QueryCanonical,
        Self::QueryCurrent,
        Self::QueryCurrentChapter,
        Self::QueryProjection,
        Self::QueryProtocol,
        Self::Challenge,
        Self::Stream,
    ];

    /// The stable error codes of protocol section 11 that the profile can return.
    fn codes(self) -> &'static [&'static str] {
        match self {
            Self::CommandSync => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "domain_conflict",
                "idempotency_conflict",
                "command_in_progress",
                "precondition_failed",
                "payload_too_large",
                "unsupported_media_type",
                "unprocessable_content",
                "precondition_required",
                "rate_limited",
                "outcome_unknown",
                "upgrade_required",
                "historical_acknowledgement_unavailable",
                "limit_exceeded",
            ],
            Self::EditorCommand => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "domain_conflict",
                "idempotency_conflict",
                "command_in_progress",
                "precondition_failed",
                "payload_too_large",
                "unsupported_media_type",
                "unprocessable_content",
                "precondition_required",
                "rate_limited",
                "outcome_unknown",
                "snapshot_not_ready",
                "projection_not_ready",
                "upgrade_required",
                "limit_exceeded",
            ],
            Self::AsyncCommand => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "idempotency_conflict",
                "command_in_progress",
                "payload_too_large",
                "unsupported_media_type",
                "unprocessable_content",
                "precondition_required",
                "rate_limited",
                "outcome_unknown",
                "upgrade_required",
                "limit_exceeded",
            ],
            Self::SessionControl => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "idempotency_conflict",
                "command_in_progress",
                "precondition_failed",
                "unprocessable_content",
                "precondition_required",
                "rate_limited",
                "outcome_unknown",
                "upgrade_required",
                "historical_acknowledgement_unavailable",
            ],
            Self::QueryCanonical => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "activity_cursor_too_old",
                "cursor_scope_mismatch",
                "snapshot_expired",
                "snapshot_not_ready",
                "projection_not_ready",
                "payload_too_large",
                "unsupported_media_type",
                "unprocessable_content",
                "rate_limited",
                "upgrade_required",
                "limit_exceeded",
            ],
            Self::QueryCurrent => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "snapshot_not_ready",
                "projection_not_ready",
                "rate_limited",
            ],
            Self::QueryCurrentChapter => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "snapshot_expired",
                "snapshot_not_ready",
                "projection_not_ready",
                "rate_limited",
            ],
            Self::QueryProjection => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "cursor_scope_mismatch",
                "snapshot_expired",
                "snapshot_not_ready",
                "projection_not_ready",
                "payload_too_large",
                "unsupported_media_type",
                "unprocessable_content",
                "rate_limited",
                "upgrade_required",
                "limit_exceeded",
            ],
            Self::QueryProtocol => &[
                "invalid_request",
                "method_not_allowed",
                "rate_limited",
                "upgrade_required",
            ],
            Self::Challenge => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "idempotency_conflict",
                "payload_too_large",
                "unsupported_media_type",
                "unprocessable_content",
                "precondition_required",
                "rate_limited",
                "upgrade_required",
            ],
            Self::Stream => &[
                "invalid_request",
                "invalid_session",
                "operation_denied",
                "resource_not_found",
                "method_not_allowed",
                "activity_cursor_too_old",
                "cursor_scope_mismatch",
                "snapshot_expired",
                "snapshot_not_ready",
                "rate_limited",
                "upgrade_required",
            ],
        }
    }
}

/// An internal operation or Event that has no public route or public schema.
struct NonPublic {
    name: &'static str,
    source: Owner,
    reason: &'static str,
}

const NON_PUBLIC_OPERATIONS: &[NonPublic] = &[
    NonPublic {
        name: "AppendProposalGenerationBatch",
        source: Owner::Core,
        reason: "AgentRunDecision producer; internal Proposal generation contract, not a browser-selected public route.",
    },
    NonPublic {
        name: "PauseProposalGeneration",
        source: Owner::Core,
        reason: "EditorInputFence safety transition; internal coordination, not the explicit ContinueProposalGeneration route.",
    },
    NonPublic {
        name: "ValidateProposal",
        source: Owner::Core,
        reason: "Validation attempt is an internal Core/producer operation; its ValidationReceipt is exposed through Proposal/Receipt queries and Project Activity.",
    },
    NonPublic {
        name: "ToolMCPSkillAndAppExecution",
        source: Owner::Context,
        reason: "Host/Worker/Adapter/bridge surfaces are not Release 1 public /api/v1 mutations.",
    },
    NonPublic {
        name: "ProviderEmbeddingResearchDispatch",
        source: Owner::Context,
        reason: "External attempts use internal manifest-before-egress boundaries and are not author-selectable public routes.",
    },
    NonPublic {
        name: "RecordMemoryNote",
        source: Owner::Memory,
        reason: "Author-requested ToolCall records a Note Artifact, not an immediate Memory publication.",
    },
    NonPublic {
        name: "MaintainProjectMemory",
        source: Owner::Memory,
        reason: "Bounded extraction/consolidation uses existing jobs and publishes through the storage-owned atomic gate.",
    },
];

const NON_PUBLIC_EVENTS: &[NonPublic] = &[NonPublic {
    name: "AuthorActionRecorded",
    source: Owner::Core,
    reason: "Core-only Author Action ordering evidence; public Receipt and Activity projections expose only the permitted resulting action identity and sequence.",
}];

/// The catalog without its operations and events.
pub(super) fn catalog_sections() -> Value {
    json!({
        "catalog_id": "storyos.public.route-catalog.release-1.v1",
        "contract_revision": CONTRACT_REVISION,
        "api_root": "/api/v1",
        "public_release": PUBLIC_PROTOCOL_RELEASE,
        "compatibility_profile": COMPATIBILITY_PROFILE,
        "source_of_truth": "StoryOS-owned Rust operation registry; semantic meaning stays with each named owner",
        "owners": Owner::ALL.map(|owner| (owner, owner.source())).into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "release_identity": {
            "schema_id": RELEASE_IDENTITY_SCHEMA_ID,
            "required_fields": [
                "api_major", "public_protocol_release", "envelope_profile",
                "contract_graph_revision", "contract_graph_digest", "web_client_contract_revision",
                "server_contract_revision", "worker_contract_revision", "generated_client_revision",
                "openapi_digest", "json_schema_catalog_digest", "typescript_artifact_digest",
                "fixture_corpus_digest", "activity_profile", "limit_profile_revision",
            ],
            "components": [
                "Protected Web Client", "StoryOS Server", "StoryOS Worker", "Rust contract graph",
                "generated TypeScript client/types", "OpenAPI", "JSON Schema catalog",
                "fixture/golden wire corpus",
            ],
            "mismatch_code": "upgrade_required",
            "mismatch_boundary": "before domain attempt, cursor advancement, or external effect",
        },
        "release1_capability_coverage": super::capabilities::capability_coverage(),
        "error_source": ERROR_SOURCE,
        "error_profiles": ErrorProfile::ALL.map(|profile| (profile, profile.codes())).into_iter().collect::<std::collections::BTreeMap<_, _>>(),
        "non_public_operations": NON_PUBLIC_OPERATIONS.iter().map(|operation| json!({
            "name": operation.name, "source": operation.source, "public_route": null, "reason": operation.reason,
        })).collect::<Vec<_>>(),
        "non_public_events": NON_PUBLIC_EVENTS.iter().map(|event| json!({
            "name": event.name, "source": event.source, "public_schema": null, "reason": event.reason,
        })).collect::<Vec<_>>(),
        "coverage_contract": {
            "every_operation_has": [
                "one operation_id", "one method/path", "one request schema ID",
                "one response schema ID", "one semantic owner", "one wire owner",
                "one resource/action shape", "one admission/idempotency/precondition profile",
                "one settlement/status profile", "one error profile", "one Project Activity mapping",
                "one positive golden fixture", "one OpenAPI operation",
                "one TypeScript generated operation", "request/response JSON Schema IDs",
                "conditional cause profiles when the operation has multiple legal causes",
            ],
            "every_public_event_has": [
                "one Event schema ID", "one stable event_kind", "one semantic owner",
                "one Project Activity wire profile", "one positive and one negative golden fixture",
                "one operation mapping",
            ],
            "non_public_events_are_explicit": true,
            "uniqueness": [
                "operation_id", "method+path", "OpenAPI operation ID", "TypeScript operation name",
                "all request/response/cause JSON Schema IDs", "all operation and Event fixture IDs",
                "Event schema ID", "event_kind", "positive fixture ID",
                "accepted settlement-query operation ID",
            ],
            "generated_outputs": {
                "openapi_3_1": "generated/openapi/storyos-public-release-1.yaml",
                "json_schema_2020_12": "generated/json-schema/storyos-public-release-1/",
                "typescript_client_and_types": "generated/typescript/storyos-public-release-1/",
                "golden_wire_corpus": "generated/golden-wire/storyos-public-release-1/",
            },
            "no_fixed_operation_count": true,
        },
    })
}
