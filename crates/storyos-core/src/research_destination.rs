//! The one MCP research destination of Stage 5 (ADR 0083).

/// The hosted Exa MCP server. It takes the MCP Streamable HTTP transport.
pub const EXA_MCP_SERVER_URL: &str = "https://mcp.exa.ai/mcp";
/// The Credential Reference of the operator key: the Worker setting that holds it (ADR 0050).
pub const EXA_CREDENTIAL_REFERENCE: &str = "settings:STORYOS_EXA_API_KEY";
