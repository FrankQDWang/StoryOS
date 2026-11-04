//! Global Tool Registrations, their lifecycle, and Tool Contract Drift (ADR 0034, ADR 0083).
//!
//! Each digest is `sha256:` and the hex SHA-256 of the `canonical_json` text of a value: the
//! canonicalization of the Wire Payload Projections. The input schema digest covers the complete
//! schema, as the Discovery Record gives it. The input contract digest covers the same schema
//! without its `description` keywords. The provenance digest covers the tool description and the
//! annotations. Descriptions and annotations are untrusted descriptive provenance, so a change
//! of only them is not drift.

use serde::Serialize;

use crate::{ServerArguments, ToolSpec};

/// The source kind of a Tool Registration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolRegistrationKind {
    /// A remote MCP server over the Streamable HTTP transport.
    McpRemote,
}

impl ToolRegistrationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::McpRemote => "mcp_remote",
        }
    }
}

/// The adapter revision of the MCP client crate.
pub const MCP_STREAMABLE_HTTP_ADAPTER_REVISION: &str = "storyos.mcp-streamable-http.v1";

/// One approved operation: the server tool name, its trusted ToolSpec, and the pinned contract.
#[derive(Debug, Eq, PartialEq)]
pub struct RegisteredTool {
    pub name: &'static str,
    pub spec: &'static ToolSpec,
    pub arguments: ServerArguments,
    pub input_schema_digest: &'static str,
    pub input_contract_digest: &'static str,
    /// `None` when the server declares no output schema.
    pub output_schema_digest: Option<&'static str>,
    pub provenance_digest: &'static str,
}

/// One host-owned, global, immutable Tool Registration revision. It holds no Project data,
/// Credential Reference, grant, or use state.
#[derive(Debug, Eq, PartialEq)]
pub struct ToolRegistration {
    pub revision: &'static str,
    pub kind: ToolRegistrationKind,
    pub server_url: &'static str,
    pub server_name: &'static str,
    pub protocol_revision: &'static str,
    /// Descriptive provenance only. A changed version alone is not drift.
    pub server_version: &'static str,
    pub adapter_revision: &'static str,
    /// Only the approved operations. Each other tool of the server has no ToolSpec.
    pub tools: &'static [RegisteredTool],
}

impl ToolRegistration {
    /// The approved operation of a server tool name. `None` is an uncallable tool.
    pub fn tool(&self, name: &str) -> Option<&RegisteredTool> {
        self.tools.iter().find(|tool| tool.name == name)
    }
}

/// The lifecycle of a Tool Registration revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolRegistrationLifecycle {
    Active,
    Quarantined,
    Retired,
}

/// The refusal of a new ToolCall before dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolCallRefusal {
    ToolRegistrationUnavailable,
}

impl ToolCallRefusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::ToolRegistrationUnavailable => "tool_registration_unavailable",
        }
    }
}

impl ToolRegistrationLifecycle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Quarantined => "quarantined",
            Self::Retired => "retired",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        [Self::Active, Self::Quarantined, Self::Retired]
            .into_iter()
            .find(|lifecycle| lifecycle.as_str() == value)
    }

    /// Only an active revision admits a new ToolCall and derives Exposure. A ToolCall that
    /// already pinned the revision keeps it, and a new revision gets no authority from this one.
    pub fn admit_new_tool_call(self) -> Result<(), ToolCallRefusal> {
        match self {
            Self::Active => Ok(()),
            Self::Quarantined | Self::Retired => Err(ToolCallRefusal::ToolRegistrationUnavailable),
        }
    }
}

/// The comparison of a fresh Tool Discovery Record with a pinned revision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractCheck {
    Unchanged,
    /// Only descriptive provenance changed: the server version, a tool description, or an
    /// annotation. The revision stays active.
    ProvenanceChange,
    /// Tool Contract Drift. The revision becomes quarantined.
    Drift(Vec<ContractDifference>),
}

/// One difference of the pinned contract. A tool that the Registration does not list is not a
/// difference: it stays uncallable.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContractDifference {
    ServerUrl {
        pinned: String,
        observed: String,
    },
    ServerName {
        pinned: String,
        observed: String,
    },
    ProtocolRevision {
        pinned: String,
        observed: String,
    },
    ToolRemoved {
        tool: String,
    },
    InputSchema {
        tool: String,
        pinned: String,
        observed: String,
    },
    OutputSchema {
        tool: String,
        pinned: Option<String>,
        observed: Option<String>,
    },
}
