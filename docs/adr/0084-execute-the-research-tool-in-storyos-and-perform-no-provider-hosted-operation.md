---
status: accepted
---

# Execute the Research Tool in StoryOS and Perform No Provider-hosted Operation

The author decided this on 2026-11-03 in conversation, on the first day of Stage 5 execution ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)). It withdraws the Provider-hosted Operation from the product and narrows [ADR 0034](0034-bound-provider-hosted-tool-operations.md) and section 5 of [ADR 0083](0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md).

## Context

The Stage 5 plan of 2026-09-15 had two ways to give the model a web search tool:

- A StoryOS ToolCall: the model asks for a tool in its Agent Decision, StoryOS validates the request, executes it through the Tool Gateway against a registered service, records the call and its result, and feeds the result back through Context Assembly.
- A Provider-hosted Operation: StoryOS asks the model Provider to run a search or a remote MCP server inside the Provider, and the Host sees only the result and its citations. ADR 0034 bounded that operation; stories 16 through 21 of #363, ticket S5-11, and three of the four probes of S5-10 belonged to it.

On 2026-11-03 the author stated that web search is one tool, that StoryOS executes it, and that a Provider-side search was never requested. The author also stated that the search service may change later: Exa today, another service such as Tavily later. The code of 2026-11-03 holds no Tool implementation and one trace of the hosted plan: a `Hosted` stream item role with a `hosted_report` field in Core, contracts, application, server, the fake destination script, and the Postgres observation record.

## Decision

### 1. One tool path

A research tool is a StoryOS ToolCall. The model requests it as a function call; the Host validates the complete request against the pinned Exposure and the ToolSpec intake; the Tool Gateway dispatches it to the registered service through that service's adapter crate; the result enters the next model step through Context Assembly. No other path exists.

### 2. No Provider-side tool

A StoryOS request to a model Provider never carries a Provider-side tool type: no `web_search`, no `mcp`, no `max_tool_calls`, and no other field that lets the Provider run a tool. A native response item of a type that the request did not ask for makes the Agent Decision invalid through the ordinary validation path, with the item type kept as evidence. The Model Capability Profile has no hosted entries. The Provider-hosted Operation and the Provider-hosted Tool Destination are withdrawn from the product contract; the glossary marks the terms as withdrawn.

### 3. The service is replaceable

The service behind a research tool is a deployment choice, not a product contract. Each service has one Tool Registration and one adapter crate; the ToolSpec, the Tool Gateway, the ToolCall record, the Source Snapshot, and the Context re-entry do not name the service. A later service is a new Registration and a new adapter crate under a new ADR, with no change to the path of section 1.

### 4. Remove the hosted trace

The `Hosted` stream item role, the `hosted_report` field, the `"hosted"` arm of the successor effect check, and the fake `SCRIPT:hosted` script are dead code and are removed in S5-10 ([#696](https://github.com/FrankQDWang/StoryOS/issues/696)), with the contract artifacts regenerated.

### 5. What changes in Stage 5

S5-10 qualifies function calling only. S5-11 ([#697](https://github.com/FrankQDWang/StoryOS/issues/697)) and S5-04 ([#400](https://github.com/FrankQDWang/StoryOS/issues/400)) are closed as not planned. Stories 16 through 21 and the Approval stories 4 and 18 of #363 are waived. The Tool Approval contract of ADR 0034 stays as a contract for a later Stage that adds an effect outside the Project grant.

## Considered options

- **Keep the hosted track as an optional second path.** Rejected. The author never asked for it; it doubled the design (two admission shapes, two Approval targets, two recovery rules) for a path whose calls the Host cannot see or record.
- **Keep the `Hosted` item role as a defensive catch-all.** Rejected. A field named for a withdrawn concept misleads a reader; the invalid-Decision rule of section 2 keeps the defense without the concept.

## Consequences

- ADR 0034 stays accepted for the Tool Gateway, the Tool Approval targets, and pre-submission configuration; its Provider-hosted Operation sections are withdrawn by this ADR, and its head says so.
- Section 5 of ADR 0083 is superseded by this ADR.
- The Stage 5 ticket set loses two tickets and one probe; no ticket gains scope except the dead-code removal in S5-10.
- The glossary entries Provider-hosted Tool Destination and Provider-hosted Operation are marked withdrawn and point here.
