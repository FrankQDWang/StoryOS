---
status: accepted
---

# Record a Research Synthesis as One Immutable Research Artifact

S5-07 ([#403](https://github.com/FrankQDWang/StoryOS/issues/403)) of Stage 5 ([#363](https://github.com/FrankQDWang/StoryOS/issues/363)) makes this decision. It uses the typed-output mapping of [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md), the visible research state of [ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md), the continuation of [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md), and the baseline of [ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md).

## Context

At `main` `7b3d5eea`, a research Run captures Source Snapshots and Source Gaps (S5-06), and the continuation RunStep reads each ToolResult. The model can reply only with an Advisory, a prose change, or a clarification. Nothing records what the research found, which source supports which statement, or which requested page was not available.

## Decision

### 1. The variant

The Agent Decision has the variant `research_synthesis`. It has a summary and an ordered list of claims. Each claim has a text and a list of citations. A citation is the identity of one Source Snapshot revision or of one Source Gap of this AgentRun. The model replies with one JSON object in the message text:

```json
{"kind":"research_synthesis","summary":"…","claims":[{"text":"…","citations":["<id>"]}]}
```

In the tool loop profile of [ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md), the model gives the same summary and claims as the arguments of the Host Tool `submit_research`, and the description of that Tool holds the cite rules of section 2. The reply form goes with the reply form profile.

### 2. How the model learns the citable identities

The function output of each resolved ToolCall has one data-only `tool_sources` tag after its `tool_result` boundary. The tag lists each Snapshot of the result in result order, then each Gap in request order. Each entry has its identity and locator, and a Gap entry has its reason. The Host escapes each value, so no locator closes the tag. The token bound of a ToolResult ([ADR 0052](0052-set-the-context-item-limit-from-the-measured-model-input.md)) cuts only the result text, never the tag, so each identity stays whole. When the complete tag and the minimum boundary are above the item limit, the Context Assembly rejects the item as `over_item_token_limit` and blocks the continuation with `exact_required_over_limit`, because a cut identity cannot be cited. The Host instructions of a continuation request add the reply form above and the cite rules. A first RunStep has no ToolResult. A synthesis there cites nothing that the Run captured, so the validation refuses it.

### 3. Core validation

`storyos_core::validate_research_synthesis` is the one validation. It reads the citable sources that the store loaded for the claimed Run. It refuses the whole Decision with one reason:

| Reason | Rule |
|---|---|
| `no_claim` | The synthesis has no claim. |
| `empty_text` | The summary or a claim text is blank. |
| `unstorable_text` | The summary or a claim text has U+0000, which the Artifact store cannot hold. |
| `uncited_claim` | A claim has no citation. |
| `uncaptured_citation` | A citation names no Snapshot and no Gap that this AgentRun captured. |
| `other_scope_citation` | A citation names a source of another Project Scope or another AgentRun. |

An invalid synthesis gets the existing repair rule of the typed-output mapping: the RunStep has no Decision, and the Run completes with no result. The Model Attempt that observed the response keeps the reason as `synthesis_refusal`: the decision attempt of a create or a successor, or the retrieval attempt of a Retrieve. A synthesis that reaches a successor or a Retrieve is on the first RunStep, which has no captured source, so its reason is `uncaptured_citation`. A reply that is not valid JSON, or that has a citation that is not a string, stays an Advisory, and the panel shows the malformed reply sentence (#1145).

In the tool loop profile, the Tool Gateway runs this validation when it executes a `submit_research` call, against the sources that the Run captured before the response of the call. A refusal makes the call a refused ToolCall, and its result gives the reason, so the model can correct the call (ADR 0091 section 3). A summary, a claim text, or a citation with U+0000 does not fit the typed intake of the Tool, so the Host refuses that call before this validation, with the reason `unstorable_text`.

### 4. The Research Artifact

A valid and selected synthesis writes one row of `storyos.research_artifacts` in the transaction of its Decision. The row has the content (the summary and the claims with typed citations), its `sha256` digest, and the lineage: the Run, the RunStep position, the Operation Requirement, the Model Attempt, the Decision, and the ToolCall identities of the RunStep. Foreign keys bind the Run, the Model Attempt, and the Operation Requirement. One Decision has at most one Artifact.

In the tool loop profile, the transaction that settles a valid `submit_research` call writes the row. The Decision of the lineage is the Agent Decision that holds the call, and the Model Attempt is the Attempt that observed its response. One Run records at most one Artifact (ADR 0091 section 2). The public `decision` of the completed Run is `research_synthesis` with that Artifact, also when the closing response has no text, and its continuation binding is the one of the closing response.

- The row is immutable. The store has no update path and no delete path. The runtime role has only `SELECT` and `INSERT`. Row Level Security keeps each row in its exact Project Scope.
- The table is in `PROJECT_EXPORT_TABLES` and in the archive builder (`canonical/research_artifacts.json`).
- A Research Artifact in Release 1 has one revision. The term "Research Artifact Revision" names that one row.

### 5. Partial deliverables

A synthesis whose claims cite only Gaps is valid. A summary that states that the research did not meet its objective is valid. Both are a partial deliverable, and they write an Artifact. An incomplete, failed, or cancelled response supplies no candidate, so it writes no Artifact.

### 6. The query

`getResearchArtifact` (`GET /api/v1/projects/{project_id}/research-artifacts/{research_artifact_id}`) is one row of the operation table. Its closed response has the lineage, the summary, the claims with citation numbers from 1, and the cited sources in the order of their first citation. Each cited source is the full Snapshot or Gap that `listResearchSources` shows. A request in another Project Scope finds no Artifact and gets `resource_unavailable`. `getAgentRun` shows the Decision with its `research_artifact_id`.

### 7. The panel

The panel shows the summary, each claim with its citation numbers, and the numbered sources. A Snapshot shows its title, or its URL when it has no title, with the capture time. A Gap shows its URL, "无法获取", and its reason. The panel shows no identifier and no support, conflict, or quality classification, because the Host computes none.

### 8. "Use this in the story"

The button sends the ordinary `createAgentRun` request of S5-05 in the same Project Conversation, with a fixed author message and no selection reference. The model reads the synthesis through the prior response of the continuation binding. When the next Decision opens a prose Proposal, `proposals.source_lineage` has the kind `research_artifact`. It names the RunStep of that Proposal, the Model Attempt that produced it (the successor of an unknown create, or else the decision attempt), and the Artifact whose response the request continued. A request with a full input reads no earlier response, so its Proposal names no Artifact. A rejection of the Proposal changes no Artifact and no source.

## Considered options

- **Give the model the identities in the Host instructions.** Rejected. The instructions are not data of one result, and a long list would cross the instruction bound.
- **Cite by locator.** Rejected. Two revisions of one URL have one locator, so a locator does not name one revision.
- **Keep a valid claim and drop an invalid claim.** Rejected. The ticket makes the whole Decision invalid, so that no partial edit of the model output looks like its reply.
- **Classify the support of each citation in the Host.** Rejected. It is out of scope, and the Host has no reliable method for it.

## Consequences

- S5-09 (#405) can read a synthesis through `getAgentRun` and `getResearchArtifact`, and it can count the Artifacts of a Run.
- A later ticket can add more revisions to one Research Artifact. It then needs a new table or a revision column.
