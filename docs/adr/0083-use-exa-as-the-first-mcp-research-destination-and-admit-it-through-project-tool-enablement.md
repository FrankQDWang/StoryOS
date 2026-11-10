---
status: accepted
---

# Use Exa as the First MCP Research Destination and Admit It Through Project Tool Enablement

The author decided this on 2026-11-03 in conversation, at the start of Stage 5 ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)). This ADR records the vendor decision and the admission shape that follows from it. It does not change the Tool contract of [ADR 0034](0034-bound-provider-hosted-tool-operations.md); it binds that contract to one service and one grant.

## Context

Stage 5 needs one approved MCP research tool before the first Tool Registration, the first ToolCall, and the first captured Source Snapshot can exist. The planning of Stage 5 named no service. Since then the repository changed in ways that a vendor decision must respect:

- A third-party MCP server is an External Processing Destination. The standing grant of [ADR 0051](0051-record-the-standing-grant-scope-and-refuse-a-disclosure-outside-it.md) covers only the model destination. A submission to any other destination needs a Project Destination Grant or a Destination Disclosure Approval, or it refuses.
- Credentials live in typed settings of the process that uses them, and never in Authoritative State ([ADR 0050](0050-load-server-and-worker-settings-through-one-typed-settings-layer.md)).
- Adapter crates hold the HTTP and TLS dependency; the Server never depends on a real adapter ([ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md)).
- Issue [#1102](https://github.com/FrankQDWang/StoryOS/issues/1102) planned a one-shot Destination Disclosure Approval outside the standing grant as the Stage 5 path for a new destination.
- The author read the public Exa documentation and the Volcengine MCP documentation on 2026-11-03. The facts below come from that reading; S5-01a verifies them against the live server and records the Discovery Record under `docs/research/`.

Exa offers a remote MCP server at `https://mcp.exa.ai/mcp` over the MCP Streamable HTTP transport. It authenticates with an `x-api-key` header. On 2026-11-03, its default tool set had three tools: `web_search_exa`, `web_fetch_exa`, and `agent_run`. Only `web_search_exa` and `web_fetch_exa` are approved. `web_search_advanced_exa` is optional and needs a query parameter at the URL. The free tier gives a monthly credit; search and page reads are metered per thousand.

Volcengine offers a Doubao web search MCP server as a local `stdio` Python process started with `uvx`, with its own key in a process environment variable. StoryOS has no `stdio` transport, no Python runtime in the Worker, and no sidecar process model.

## Decision

### 1. The service

The remote Exa MCP server is the first and, for Stage 5, the only MCP research destination of StoryOS. The Tool Registration binds the server URL, the MCP protocol revision, and the SHA-256 digests of the input schemas of exactly two operations: `web_search_exa` and `web_fetch_exa`. Each other tool of the server has no StoryOS ToolSpec and stays uncallable. A later service, a second server, or a widened operation set needs a new ADR.

### 2. The transport and the key

A new adapter crate, `storyos-adapter-mcp-client`, holds the MCP Streamable HTTP transport and the HTTP and TLS dependency. The Worker binary composes it; the Server does not depend on it. The Exa key is one typed Worker setting, `STORYOS_EXA_API_KEY`, read through the settings path of ADR 0050. Records hold the Credential Reference `settings:STORYOS_EXA_API_KEY`, in the form of the Agent Plan reference, and never a value. The key belongs to the operator of the deployment; Stage 5 adds no per-author key.

### 3. Enablement records the grant

Project Tool Enablement of the Exa Registration is the admission boundary of the destination. In one settlement it records the Processing Destination Identity of the server, the Project External Use Binding with the Credential Reference, the External Contract Compatibility Decision against the pinned digests, and one Project Destination Grant with the scope `research_tool_v1`: Purpose `research`, source classes `agent_query` and `agent_url_selection`, no Ambient Context, no author-private content. Disablement writes a new enablement revision and keeps the grant as history.

Thus each Stage 5 submission is inside a grant: the model destination under the standing grant, which gains the source class `tool_result` as `project_writing_assistance_v3`, and the Exa destination under `research_tool_v1`. No reachable path needs a one-shot Destination Disclosure Approval. Issue #1102 closes as not planned. The Run Wait, the `decideApproval` command, and the Worker resume move to S5-04 under the Tool Approval target, with a declared but unreachable disclosure target in the union.

### 4. No approval prompt

The model decides itself when it searches the web and when it reads a page. Both operations are inside `research_tool_v1` and dispatch with no prompt; the author gave the consent one time, at enablement, where the panel names the service. The panel shows each search and each page read after the fact. A Tool Approval ([ADR 0034](0034-bound-provider-hosted-tool-operations.md)) stays the contract for an effect outside the grant, such as an external write; Stage 5 has no such operation, so the Run Wait and the `decideApproval` command of S5-04 are built only when a Stage 5 operation needs them.

### 5. No hosted track

Superseded on 2026-11-03 by [ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md): StoryOS performs no Provider-hosted operation. The one qualification of the model route that Stage 5 needs is function calling (S5-10).

## Considered options

- **Volcengine Doubao search MCP.** Rejected. It is a local `stdio` Python sidecar with its own runtime and process model. StoryOS would need a second transport, a Python runtime in the Worker image, and a process supervisor before the first call.
- **Exa without a key.** Rejected. The keyless mode gives no account boundary, no spend boundary, and no stable identity for the Processing Destination Identity.
- **A one-shot Destination Disclosure Approval for each search (#1102 as planned).** Rejected. The author would approve each query of each Run; the grant that the enablement records expresses the same consent one time, at the boundary where the author reads the service name.
- **A Tool Approval for each page read, one per AgentRun.** Rejected. The author reads the service name once at enablement. A prompt for each page interrupts the conversation, and the model, not the author, knows which page its answer needs. The after-the-fact line in the panel keeps the read visible.
- **Exa through the Agent Plan route as a remote MCP tool.** Rejected; ADR 0084 withdraws the hosted track. The route takes the key and the server URL in the request body, which moves the Credential Reference to the model Provider and leaves the Host without the per-call disclosure evidence that ADR 0034 requires.

## Consequences

- Stage 5 has one live external dependency beyond the model Provider. Its availability, its schema drift, and its cost are the operator's. The Registration quarantine of S5-01b makes drift visible, not silent.
- 2026-11-10 (S5-03): the quarantine event is a Run Event (`tool_registration_quarantined` in `agent_run_events`) of the Run whose drift check found the drift. The quarantine itself stays on the global Registration revision row.
- The operator spends from the Exa account for each search and each page. Each live request of an executor needs the author's planned count on the ticket, as ADR 0049 requires for the model route.
- The source classes of the Context contract grow by `tool_result`, `agent_query`, and `agent_url_selection`. The Context contract document records them when S5-03 and S5-05 land.
- [GLOSSARY.md](../../GLOSSARY.md) and the tool glossary get the terms Project Destination Grant scope `research_tool_v1` and Research Artifact when their tickets land.
- The External MCP Surface of [ADR 0082](0082-keep-room-for-an-external-mcp-surface.md) is not affected: the Exa Registration is a Tool Registration of a client, and the four constraints of ADR 0082 stay in force.
