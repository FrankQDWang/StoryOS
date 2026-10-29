//! The Release 1 operation registry: one `OperationArtifacts` value for each artifacts module.

use crate::Release1ProtocolProfile;
use crate::release1::QueryOperation;
use crate::release1_route_catalog::OperationCatalog;

/// The generated wire surface of one artifacts module.
pub(super) struct OperationArtifacts {
    pub(super) operations: &'static [RegisteredOperation],
    pub(super) schemas: fn() -> Vec<GeneratedSchema>,
    pub(super) openapi: fn() -> Vec<OpenApiMethod>,
    pub(super) typescript_types: fn() -> String,
    pub(super) fixtures: &'static [FixtureMembership],
}

pub(super) struct RegisteredOperation {
    pub(super) operation: &'static QueryOperation,
    pub(super) kind: OperationKind,
    pub(super) graph: ContractGraphEntry,
    pub(super) catalog: OperationCatalog,
    pub(super) client: ClientOverride,
}

impl RegisteredOperation {
    pub(super) const fn query(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
        catalog: OperationCatalog,
    ) -> Self {
        Self::graphed(operation, OperationKind::Query, preconditions, catalog)
    }

    pub(super) const fn command(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
        catalog: OperationCatalog,
    ) -> Self {
        Self::graphed(operation, OperationKind::Command, preconditions, catalog)
    }

    pub(super) const fn stream(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
        catalog: OperationCatalog,
    ) -> Self {
        Self::graphed(operation, OperationKind::Stream, preconditions, catalog)
    }

    pub(super) const fn challenge(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
        catalog: OperationCatalog,
    ) -> Self {
        Self::graphed(operation, OperationKind::Challenge, preconditions, catalog)
    }

    const fn graphed(
        operation: &'static QueryOperation,
        kind: OperationKind,
        preconditions: &'static [&'static str],
        catalog: OperationCatalog,
    ) -> Self {
        Self {
            operation,
            kind,
            graph: ContractGraphEntry::Preconditions(preconditions),
            catalog,
            client: ClientOverride::None,
        }
    }

    pub(super) const fn client(self, client: ClientOverride) -> Self {
        Self { client, ..self }
    }
}

/// The one TypeScript client behavior of an operation that its method, path, and kind do not give.
#[derive(Clone, Copy)]
pub(super) enum ClientOverride {
    None,
    /// `getProtocolProfile`: the response type is not `<Operation>Response`.
    ResponseType(&'static str),
    /// `createProject`: the client exports no request digest function.
    NoDigest,
    /// `createEditorSession`: the request digest covers only these request fields.
    DigestFields(&'static [&'static str]),
    /// `withdrawProposal`: the current-producer cause sends no anti-forgery header.
    OptionalAntiForgery,
    /// `getAgentRun`, `getStatistics`: one optional query parameter.
    OptionalQueryParameter(&'static str),
    /// `getApplyAuthorEditOutcome`, `activityStream`: header and query bindings that no path gives.
    Source {
        client: fn() -> String,
        declaration: &'static str,
    },
}

pub(super) enum ContractGraphEntry {
    Preconditions(&'static [&'static str]),
    /// The reviewed contract graph has no entry for this operation; the route catalog lists these preconditions.
    Absent(&'static [&'static str]),
}

impl ContractGraphEntry {
    pub(super) fn preconditions(&self) -> &'static [&'static str] {
        match self {
            Self::Preconditions(preconditions) | Self::Absent(preconditions) => preconditions,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum OperationKind {
    Query,
    Command,
    Challenge,
    Stream,
}

impl OperationKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Command => "command",
            Self::Challenge => "challenge",
            Self::Stream => "stream",
        }
    }
}

pub(super) struct GeneratedSchema {
    pub(super) schema_id: &'static str,
    pub(super) path: &'static str,
    pub(super) bytes: Vec<u8>,
}

/// One OpenAPI method block; the generator groups blocks that share a path under one path key.
pub(super) struct OpenApiMethod {
    pub(super) path: &'static str,
    pub(super) yaml: String,
}

/// The one positive golden-wire fixture of an operation in the fixture corpus.
#[derive(Clone, Copy)]
pub(super) struct FixtureMembership {
    pub(super) path: &'static str,
    pub(super) bytes: fn(&Release1ProtocolProfile) -> Vec<u8>,
}

/// The declared order of the registry; every generated artifact, the route catalog too, follows it.
pub(super) const RELEASE1_OPERATIONS: &[&OperationArtifacts] = &[
    &crate::release1_protocol_profile_artifacts::ARTIFACTS,
    &crate::release1_create_project_artifacts::ARTIFACTS,
    &crate::release1_command_challenge_artifacts::ARTIFACTS,
    &crate::release1_list_projects_artifacts::ARTIFACTS,
    &crate::release1_project_query_artifacts::ARTIFACTS,
    &crate::release1_update_project_artifacts::ARTIFACTS,
    &crate::release1_project_assistance_artifacts::ARTIFACTS,
    &crate::release1_archive_project_artifacts::ARTIFACTS,
    &crate::release1_editor_session_artifacts::ARTIFACTS,
    &crate::release1_takeover_artifacts::ARTIFACTS,
    &crate::release1_create_volume_artifacts::ARTIFACTS,
    &crate::release1_update_volume_artifacts::ARTIFACTS,
    &crate::release1_delete_volume_artifacts::ARTIFACTS,
    &crate::release1_create_chapter_artifacts::ARTIFACTS,
    &crate::release1_update_chapter_artifacts::ARTIFACTS,
    &crate::release1_delete_chapter_artifacts::ARTIFACTS,
    &crate::release1_set_current_chapter_artifacts::ARTIFACTS,
    &crate::release1_author_edit_artifacts::ARTIFACTS,
    &crate::release1_accept_proposal_artifacts::ARTIFACTS,
    &crate::release1_reject_proposal_operations_artifacts::ARTIFACTS,
    &crate::release1_withdraw_proposal_artifacts::ARTIFACTS,
    &crate::release1_replan_proposal_artifacts::ARTIFACTS,
    &crate::release1_reopen_withdrawn_proposal_artifacts::ARTIFACTS,
    &crate::release1_reopen_rejected_operations_artifacts::ARTIFACTS,
    &crate::release1_proposal_generation_decision_artifacts::ARTIFACTS,
    &crate::release1_undo_latest_author_action_artifacts::ARTIFACTS,
    &crate::release1_agent_run_artifacts::ARTIFACTS,
    &crate::release1_agent_run_control_artifacts::ARTIFACTS,
    &crate::release1_project_export_artifacts::ARTIFACTS,
    &crate::release1_readable_export_artifacts::ARTIFACTS,
    &crate::release1_manuscript_tree_artifacts::ARTIFACTS,
    &crate::release1_chapter_query_artifacts::ARTIFACTS,
    &crate::release1_manuscript_statistics_artifacts::ARTIFACTS,
    &crate::release1_proposal_artifacts::ARTIFACTS,
    &crate::release1_open_proposal_list_artifacts::ARTIFACTS,
    &crate::release1_author_edit_outcome_artifacts::ARTIFACTS,
    &crate::release1_snapshot_artifacts::ARTIFACTS,
    &crate::release1_manuscript_search_artifacts::ARTIFACTS,
    &crate::release1_project_export_query_artifacts::ARTIFACTS,
    &crate::release1_readable_export_query_artifacts::ARTIFACTS,
];

/// The request and response schemas of one operation.
pub(super) fn operation_schemas(
    operation: &QueryOperation,
    request: (&'static str, Vec<u8>),
    response: (&'static str, Vec<u8>),
) -> [GeneratedSchema; 2] {
    [
        GeneratedSchema {
            schema_id: operation.request_schema,
            path: request.0,
            bytes: request.1,
        },
        GeneratedSchema {
            schema_id: operation.response_schema,
            path: response.0,
            bytes: response.1,
        },
    ]
}

pub(super) fn method(operation: &QueryOperation, yaml: String) -> Vec<OpenApiMethod> {
    vec![OpenApiMethod {
        path: operation.path,
        yaml,
    }]
}
