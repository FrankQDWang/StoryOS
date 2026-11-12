---
status: accepted
---

# Make Web Research Available in Each Project Without an Enablement Switch

The author decided this on 2026-11-12 in conversation ([#1449](https://github.com/FrankQDWang/StoryOS/issues/1449)): web research is available in each Project by default. The writing assistant panel has no switch for it, and the author does no action to turn it on. The switch of Stage 5 was not a requirement of the author.

## Context

[ADR 0083](0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md) made Project Tool Enablement the admission boundary of the Exa destination. The `setProjectToolEnablement` command wrote an enablement revision and, in the same settlement, the research records: the Processing Destination Identity with its evidence revision, the Project Destination Grant with the scope `research_tool_v1`, the Project External Use Binding with the Credential Reference, and the External Contract Compatibility Decision. The panel showed one switch that sent the command, and ADR 0083 section 4 called that switch the consent of the author.

The Host already writes the records of the model destination with no action of the author ([ADR 0048](0048-use-the-deployment-model-destination-by-default.md)): `createAgentRun` binds the deployment destination in its own transaction when the Project has no current binding.

## Decision

### 1. The configuration of the operator is the consent

The operator of the deployment configures the research destination. That configuration is the consent. Each deployment that offers a model destination offers the research destination of ADR 0083. The grant scope `research_tool_v1` does not change: Purpose `research`, source classes `agent_query` and `agent_url_selection`, no Ambient Context, no author-private content. The panel still shows each search and each page read after the fact. No author action enables or disables research, and no Project setting holds a research state.

### 2. `createAgentRun` makes the research records current

Before it admits a Run, `createAgentRun` makes sure that the Project has the current research records. It writes the five records of ADR 0083 section 3 in its own transaction, under its own Receipt, when they are absent or not current. The records are current when the latest research Decision of the Project (decision kind `tool_registration_compatible`) joins a binding with the offered Registration revision and Credential Reference, identity evidence with the offered endpoint and account boundary, and a grant with the scope `research_tool_v1`. It writes nothing when the records are current, when the offered Registration revision is quarantined or retired, or when the deployment offers no research destination. Earlier records stay unchanged, and earlier Runs keep the binding that they pinned.

### 3. The readers use the latest research Decision

Tool Exposure, the validation of a ToolCall batch, and the admission of a ToolCall dispatch read the latest research Decision of the Project and its binding. No enablement record exists, so no "not enabled" state and no `tool_not_enabled` refusal stay.

### 4. The "not available" states stay

- A deployment with no research destination exposes no research Tool.
- A quarantined Registration revision exposes no research Tool and gets no new research records.
- A batch that asks for a Tool whose Registration was quarantined after the Run pinned its Exposure refuses with `research_tool_unavailable`.
- A batch that asks for a Tool that the Run never exposed refuses with `tool_request_invalid`.

### 5. What this supersedes

This ADR supersedes the sentences of ADR 0083 section 3 that make Project Tool Enablement the admission boundary and that describe disablement, and the sentence of ADR 0083 section 4 that puts the consent at enablement. The rest of ADR 0083 stays in force: the service, the two operations, the transport, the key, the grant scope, and no approval prompt.

## Considered options

- **Keep the switch, on by default.** Rejected. The author asked for no switch. A switch that is on by default still needs a command, a table, and a state that no author uses.
- **A separate Host command before the first Run.** Rejected. It adds a second command and a second Receipt for each new Project. The model binding already settles inside `createAgentRun` (ADR 0048), and research follows the same path.
- **A per-author setting that turns research off.** Rejected. The author did not ask for it. A later need for it needs a new ADR.

## Consequences

- The operation `setProjectToolEnablement`, its command kind, Receipt kind, Activity kind `project_tool_enablement_set`, route, and the table `project_tool_enablements` are removed. The Release 1 baseline changes directly ([ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md)). The Project Export Archive has one table less.
- The assistance query response has no `research_tool` state.
- Each Run of a Project in a deployment with a model destination declares the two research Tools, unless the offered Registration revision is quarantined.
- A change of the offered Credential Reference, endpoint, or account boundary writes a new set of research records at the next `createAgentRun`.
- [ADR 0076](0076-settle-every-project-command-through-one-sequence-skeleton.md) loses `setProjectToolEnablement` from the `ActivityOnly` row and from the commands outside Author Undo Order.
