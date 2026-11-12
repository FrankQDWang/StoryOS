//! The one MCP research destination of Stage 5 (ADR 0083) and its Tool Registration.

use crate::{
    MCP_STREAMABLE_HTTP_ADAPTER_REVISION, RegisteredTool, ResultFormat, ServerArguments,
    ToolRegistration, ToolRegistrationKind, WEB_FETCH_TOOL_SPEC, WEB_SEARCH_TOOL_SPEC,
};

/// The remote Exa MCP server. It takes the MCP Streamable HTTP transport.
pub const EXA_MCP_SERVER_URL: &str = "https://mcp.exa.ai/mcp";
/// The Credential Reference of the operator key: the Worker setting that holds it (ADR 0050).
pub const EXA_CREDENTIAL_REFERENCE: &str = "settings:STORYOS_EXA_API_KEY";
/// The account boundary that the Processing Destination Identity of the server records.
pub const EXA_ACCOUNT_BOUNDARY: &str =
    "The Exa account of the deployment operator that holds the Exa API key";

/// The research destination that a deployment offers with its model destination: the
/// Registration and the Project-scoped facts that `createAgentRun` records (ADR 0090).
#[derive(Debug, Eq, PartialEq)]
pub struct ResearchToolOffer {
    pub registration: &'static ToolRegistration,
    /// The source of this Credential Reference is the operator quota.
    pub credential_reference: &'static str,
    pub account_boundary: &'static str,
}

/// The research destination of Stage 5.
pub const RESEARCH_TOOL_OFFER: ResearchToolOffer = ResearchToolOffer {
    registration: &EXA_TOOL_REGISTRATION,
    credential_reference: EXA_CREDENTIAL_REFERENCE,
    account_boundary: EXA_ACCOUNT_BOUNDARY,
};

/// The Exa Registration of the Discovery Record `docs/research/exa-mcp-discovery-2026-11-03.md`.
/// It approves only `web_search_exa` and `web_fetch_exa`; `agent_run`,
/// `web_search_advanced_exa`, and each later tool stay uncallable. The server requires
/// `objective` on a search, and the Registration derives it from the query.
pub const EXA_TOOL_REGISTRATION: ToolRegistration = ToolRegistration {
    id: "web_research",
    revision: "018f0000-0000-7001-8000-0000000e0001",
    kind: ToolRegistrationKind::McpRemote,
    server_url: EXA_MCP_SERVER_URL,
    server_name: "exa-search-server",
    protocol_revision: "2025-11-25",
    server_version: "3.5.0",
    adapter_revision: MCP_STREAMABLE_HTTP_ADAPTER_REVISION,
    tools: &[
        RegisteredTool {
            name: "web_search_exa",
            spec: &WEB_SEARCH_TOOL_SPEC,
            arguments: ServerArguments::Search {
                query: "query",
                goal: Some("objective"),
                result_count: "numResults",
            },
            result_format: ResultFormat::TitledEntries,
            input_schema_digest: "sha256:ea70b6dbd4ca5363f0ac82944520016e45d810ecfa332c39aa1c6ece65f2e4df",
            input_contract_digest: "sha256:0653bc2961e1ce90bd0ca07a029a919f31dab615ec2a7ac3850879f3cc4ea87f",
            output_schema_digest: None,
            provenance_digest: "sha256:bf787e0038f10ddaccd1dbd6b85db005c27a48cf5f1983ba7cea76f53e800ef7",
        },
        RegisteredTool {
            name: "web_fetch_exa",
            spec: &WEB_FETCH_TOOL_SPEC,
            arguments: ServerArguments::Fetch { urls: "urls" },
            result_format: ResultFormat::PageBlocks,
            input_schema_digest: "sha256:d72ee7687517df49ee4374713985811b9a87c252590b2ef26c9218786b0f805b",
            input_contract_digest: "sha256:1e44be9a7159e1a2e95b5214dc238706a3bf834b72290530d889c4d53513aaf0",
            output_schema_digest: None,
            provenance_digest: "sha256:da47e485bad2c59a80504436f5c831c1c6b8cca7ab511c75e0ac33f8af027595",
        },
    ],
};

/// The known Tool Registration of a stored revision. `None` is an unknown or damaged revision.
pub fn tool_registration(revision: &str) -> Option<&'static ToolRegistration> {
    [&EXA_TOOL_REGISTRATION]
        .into_iter()
        .find(|registration| registration.revision == revision)
}
