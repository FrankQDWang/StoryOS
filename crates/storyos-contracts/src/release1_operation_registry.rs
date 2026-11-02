//! The registered operation of each row of the operation table, and the artifact parts that it gives.

use crate::release1::QueryOperation;
use crate::release1_route_catalog::OperationCatalog;

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
