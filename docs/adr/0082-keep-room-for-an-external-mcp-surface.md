---
status: accepted
---

# Keep Room for an External MCP Surface

The author decided this on 2026-10-31 in conversation and asked on 2026-11-03 to record it. This ADR records an intent and the constraints that keep the intent possible. It does not start implementation, open a Stage, or change a contract. A later ADR owns the design.

## Context

StoryOS has one trusted client for author commands: the exact StoryOS Web Client build ([ADR 0013](0013-trust-the-storyos-web-client-for-author-command-admission.md)). In the repository, "MCP" has two meanings. Stage 5 ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)) makes StoryOS an MCP client of research tools and provider-hosted operations ([ADR 0034](0034-bound-provider-hosted-tool-operations.md)). Stage 8 ([#366](https://github.com/FrankQDWang/StoryOS/issues/366)) makes StoryOS the host of third-party MCP Apps in its own Agent transcript. No ADR, issue, glossary term, or research report describes a third meaning: StoryOS as an MCP server that an external host calls.

The author wants that third meaning later. After the Web Client, StoryOS is to run as a ChatGPT plugin, and later as a connector of other MCP hosts. The author read the public ChatGPT documentation on 2026-10-31. The facts below come from that reading. The design ADR must verify them again, because the vendor changes them.

- A ChatGPT plugin is an MCP server. Each operation that the model can call is one MCP tool with a name, a description, an input schema and an output schema in JSON Schema, and the read-only, destructive, and open-world annotations. The vendor review refuses a generic executor tool that discovers operations at run time.
- The plugin UI is the open MCP Apps standard: a tool names a UI resource, the host renders it in an iframe, and the two sides talk over JSON-RPC. The same UI runs on other MCP Apps hosts.
- The host authenticates each user to the server through per-user OAuth.
- The vendor review requires response minimization: a tool result carries no session identifier, trace identifier, request identifier, or timestamp unless the operation needs it.

The contracts generator ([ADR 0081](0081-generate-the-contract-artifacts-from-one-operation-table.md)) emits no MCP artifact. Cleanup phase 8 decided that on 2026-11-02, because no ADR recorded the surface. The foundation boundary document names "future authorized external consumers" of the public contract and nothing more.

## Decision

### 1. The intent

StoryOS will later expose a bounded set of its public operations to external MCP hosts as an External MCP Surface. ChatGPT is the first host. The surface is a StoryOS Server surface. It is not a fork of the Web Client, not a Tool Registration, and not an MCP App.

### 2. The constraints that stay in force until the design ADR

Each design decision of Stage 5 and later keeps these four constraints. A design that breaks one of them is a defect, or it records the exception in its ADR.

- **One public operation is one unit of exposure.** The Operation Registry row stays the source of the request schema, the response schema, and the route of an operation. The External MCP Surface will map one public operation to one MCP tool and will add one artifact kind to the one writer of ADR 0081. No design adds a generic operation that executes other operations by name.
- **The Core sees a User, not a connector.** An external host authenticates a User to the StoryOS Server. The Server maps that identity to the existing User and Project Scope at its boundary. No design puts a host identity, a connector identity, or a plugin installation into Core, into Authoritative State, or into a Receipt.
- **A public response stays projectable.** Each public response type stays one closed Rust type in the contracts crate. The External MCP Surface will read a projection of it that drops the Receipt internals, the command identifiers, and the timestamps that the operation does not need. No design moves a public response field out of the contracts crate or makes a response depend on the Web Client session.
- **An external caller changes Authoritative State only through the existing admission and Proposal paths.** The author command admission of ADR 0013 does not accept an external host today. The design ADR decides which commands the surface exposes, and under which admission profile. Until then, no design binds a public command to the Web Client more than ADR 0013 already binds it.

### 3. What waits for the design ADR

The design ADR, and the Stage that it opens, decide the trust boundary of an external caller, the OAuth binding to a User, the exposed operation set, the projection rules, the MCP artifact of the generator, the UI resources, and the vendor review obligations. The author decides when that design starts. It does not start before Stage 5 is released.

## Considered options

- **Design the surface now.** Refused. The design needs a trust decision that changes ADR 0013, and it competes with the eleven Stage 5 tickets for the same contracts and admission code.
- **Record nothing until the design starts.** Refused. A premise that no ADR records is not a contract. Stage 5 could then close the surface by accident, for example with a response shape that only the Web Client can read.
- **Record the intent and the constraints.** Chosen. The record is short, changes no code, and gives Stage 5 one ADR to cite.

## Consequences

- A Stage 5 ADR or specification that touches identity, a public response shape, the Operation Registry, or the generator cites this ADR and states which constraint it keeps.
- The contracts generator stays as ADR 0081 leaves it. The MCP artifact kind waits for the design ADR.
- The glossary gets one term, External MCP Surface, in the ToolSpec, Tool Gateway, and MCP area.
- The design ADR supersedes section 3 of this ADR when it lands. Sections 1 and 2 stay in force until that ADR says otherwise.
