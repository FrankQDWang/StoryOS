//! The Release 1 operation registry: one `OperationArtifacts` value for each artifacts module.
#![expect(
    dead_code,
    reason = "the generator reads the registry after the expand-contract switch"
)]

use crate::Release1ProtocolProfile;
use crate::release1::QueryOperation;

/// The generated wire surface of one artifacts module.
pub(super) struct OperationArtifacts {
    pub(super) operations: &'static [RegisteredOperation],
    pub(super) schemas: fn() -> Vec<GeneratedSchema>,
    pub(super) openapi: fn() -> Vec<OpenApiMethod>,
    pub(super) typescript_types: fn() -> String,
    pub(super) typescript_client: fn() -> String,
    pub(super) typescript_declarations: fn() -> &'static str,
    pub(super) fixtures: fn() -> Vec<FixtureMembership>,
}

pub(super) struct RegisteredOperation {
    pub(super) operation: &'static QueryOperation,
    pub(super) kind: OperationKind,
    pub(super) graph: ContractGraphEntry,
}

impl RegisteredOperation {
    pub(super) const fn query(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
    ) -> Self {
        Self::graphed(operation, OperationKind::Query, preconditions)
    }

    pub(super) const fn command(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
    ) -> Self {
        Self::graphed(operation, OperationKind::Command, preconditions)
    }

    pub(super) const fn challenge(
        operation: &'static QueryOperation,
        preconditions: &'static [&'static str],
    ) -> Self {
        Self::graphed(operation, OperationKind::Challenge, preconditions)
    }

    const fn graphed(
        operation: &'static QueryOperation,
        kind: OperationKind,
        preconditions: &'static [&'static str],
    ) -> Self {
        Self {
            operation,
            kind,
            graph: ContractGraphEntry::Preconditions(preconditions),
        }
    }
}

pub(super) enum ContractGraphEntry {
    Preconditions(&'static [&'static str]),
    /// The reviewed contract graph has no entry for this operation.
    Absent,
}

#[derive(Clone, Copy)]
pub(super) enum OperationKind {
    Query,
    Command,
    Challenge,
}

impl OperationKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Command => "command",
            Self::Challenge => "challenge",
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
    pub(super) path: String,
    pub(super) yaml: String,
}

/// One golden-wire fixture in the fixture corpus.
#[derive(Clone, Copy)]
pub(super) struct FixtureMembership {
    pub(super) path: &'static str,
    pub(super) fixture_id: &'static str,
    pub(super) classification: &'static str,
    pub(super) operation_id: &'static str,
    pub(super) bytes: fn(&Release1ProtocolProfile) -> Vec<u8>,
}

/// Modules sort by the reviewed route-catalog position of their first operation.
#[expect(
    dead_code,
    reason = "the generator reads the registry after the expand-contract switch"
)]
pub(super) const RELEASE1_OPERATIONS: &[&OperationArtifacts] = &[
    &crate::release1_create_project_artifacts::ARTIFACTS,
    &crate::release1_list_projects_artifacts::ARTIFACTS,
    &crate::release1_update_project_artifacts::ARTIFACTS,
    &crate::release1_project_assistance_artifacts::ARTIFACTS,
    &crate::release1_archive_project_artifacts::ARTIFACTS,
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

pub(super) fn fixture_triple(
    paths: [&'static str; 3],
    operation: &QueryOperation,
    producers: [fn(&Release1ProtocolProfile) -> Vec<u8>; 3],
) -> [FixtureMembership; 3] {
    let [positive, invalid, boundary] = producers;
    [
        ("positive", paths[0], operation.fixtures[0], positive),
        ("invalid", paths[1], operation.fixtures[1], invalid),
        ("boundary", paths[2], operation.fixtures[2], boundary),
    ]
    .map(
        |(classification, path, fixture_id, bytes)| FixtureMembership {
            path,
            fixture_id,
            classification,
            operation_id: operation.operation_id,
            bytes,
        },
    )
}

pub(super) fn method(operation: &QueryOperation, yaml: String) -> Vec<OpenApiMethod> {
    vec![OpenApiMethod {
        path: operation.path.to_owned(),
        yaml,
    }]
}

// Splits path-keyed OpenAPI YAML into method blocks until each module returns blocks itself.
pub(super) fn path_items(yaml: String) -> Vec<OpenApiMethod> {
    let mut methods: Vec<OpenApiMethod> = Vec::new();
    for line in yaml.split_inclusive('\n') {
        match line.strip_prefix("  ").filter(|rest| rest.starts_with('/')) {
            Some(path) => methods.push(OpenApiMethod {
                path: path.trim_end().trim_end_matches(':').to_owned(),
                yaml: String::new(),
            }),
            None => methods
                .last_mut()
                .expect("OpenAPI YAML starts with a path key")
                .yaml
                .push_str(line),
        }
    }
    methods
}
