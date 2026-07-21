# Context Assembly, Retrieval, and Outbound Disclosure Source Audit

Status: research evidence only; future input to Wayfinder ticket #54, not an accepted StoryOS decision or implementation specification.

## A. Scope, method, and source versions

### A.1 Scope

This sidecar examines how three local reference implementations expose or imply semantics for mandatory context, dynamic retrieval, ranking and admission, model-visible replay, bounded context, compaction and caching, provider egress, Tool/MCP content, trust, and context controls. Its purpose is to sharpen the human decisions required by Wayfinder ticket #54. It does not alter StoryOS authority, define an implementation, or adopt any upstream behavior. StoryOS explicitly treats `.reference/**` as read-only learning material rather than a runtime or dependency. [S1]

The audit uses four evidence labels:

- **Observed** means the cited local source directly implements or declares the behavior.
- **Inference** means the conclusion follows from cited observed behavior but was not declared as a product contract.
- **Current official documentation** means a vendor's current documentation, distinct from a version-pinned local artifact.
- **Unknown** means the available or timeboxed evidence does not prove the behavior.

The three local references remain the primary implementation evidence. Current
official vendor and standards documentation is used only as an auxiliary
comparison and is never projected backward onto the Claude Code 2.1.88 package
or either inspected Git revision.

### A.2 Method and source state

- The repository root `AGENTS.md`, the accepted StoryOS context and authority artifacts, and `.reference/codex/AGENTS.md` were read before inspecting Codex source. The Research skill's background worker performed the local-source audit; the main thread independently checked StoryOS's accepted contracts, live ticket state, and current official web sources before reviewing this report.
- StoryOS was on `main...origin/main`. Before this report was created, the only visible worktree differences were the pre-existing reference state: modified `.reference/codex` plus untracked `.reference/claude-code/` and `.reference/grok-build/`. Those paths were not modified.
- **Codex:** the StoryOS superproject gitlink records `1f0566d3f59298d1bb88820a0d35294f1eeb07ea`, while the detached, clean local reference worktree inspected here was `c9ef7eff005c3299a5a5f0004c34c6a3eedf2564`. The latter is uncommitted local reference evidence, not an accepted StoryOS pin update. StoryOS's repository policy still identifies the tracked reference as read-only upstream material. [S1]
- **grok-build:** clean `main` checkout at `a881e6703f46b01d8c7d4a5437683546df30449d`, origin `https://github.com/xai-org/grok-build.git`. Its `SOURCE_REV` records `c5c4ce03436b4bb2cec43d3feaa27dee0109bf37`; this report records both values and does not assume they are interchangeable. [G0]
- **Claude Code:** unpacked, non-Git npm package `@anthropic-ai/claude-code` version `2.1.88`. The package declares `cli.js` as its executable. [A0] Snapshot SHA-256 values recorded during the audit were: `package.json` `e21f9e98fa4ea8b4d007063d92c631df1bbed6d11c9e79c5fcdeb9f4859dc8fa`; `cli.js` `75c9611929d9a770fe2e3a393219d8b98f5de17fde539b2a7355c6db3fd2795f`; `cli.js.map` `7965012b7a5fc9e09d8d747a04c5c32b94696924536e217f686bb1e7ee70a657`; and `sdk-tools.d.ts` `53d249727389f9c9af5192f99bf66c4513b89d710e4326082ce1d218c3e6c7fc`.
- **Ticket:** GitHub issue #54 was read-only inspected on 2026-07-21. Its open question asks for the exact pipeline governing mandatory context, dynamic retrieval, source ranking, `ContextManifest`, disclosure minimization, provider egress, and author inspection. No issue state was changed.

### A.3 Interpretation discipline

Upstream runtime mechanisms are evidence, not StoryOS product decisions. In particular, Codex plan mode, Codex or grok-build project-instruction files, and upstream agent prompt regimes are not a StoryOS Author Plan. StoryOS's accepted discovery-writing and authority boundaries control interpretation. [S1] [S2] [S3]

## B. Observed facts by implementation

### B.1 Codex local worktree at `c9ef7eff005c3299a5a5f0004c34c6a3eedf2564`

#### Mandatory context and request assembly

- **Observed:** the logical prompt object contains the conversation input, tool specifications including external MCP tools, parallel-tool policy, base instructions, and optional output schema. The provider request serializes the conversation and tools and sends base instructions either as a developer message or an `instructions` field, depending on model/provider behavior. [C1] [C2]
- **Observed:** initial model-visible context can include developer instructions, skill metadata under a budget, recommended plugins, extension context contributors, world state, context bundles, and the result of a dynamic MCP `notes.thread_hint` contribution. Once a baseline exists, later turns record context diffs rather than blindly reinjecting the full initial block. [C3]
- **Observed:** project instructions are discovered from the repository root toward the current working directory and concatenated under a byte limit; provenance is retained during discovery, and later files can be truncated once the budget is exhausted. [C4]
- **Inference:** Codex has a mandatory session/turn envelope assembled from heterogeneous sources, but those sources do not all share one admission rule or one trust class in the inspected code. [C1] [C3] [C4]

#### Dynamic retrieval, ranking, and trust

- **Observed:** `tool_search` is deferred discovery of tool metadata. It uses BM25 over tool search text, validates a query and limit, and exposes matching tool definitions on a later model call. It is not evidence of project-knowledge retrieval or truth admission. [C5]
- **Observed:** `read_mcp_resource` takes an explicit server and resource URI, checks model access, reads the resource, serializes it, and truncates the returned output under a configured policy. The inspected handler does not attach an epistemic trust or prompt-injection classification to the returned content. [C6]
- **Inference:** in these inspected paths, relevance or explicit addressability controls whether content is returned, not whether its claims are authoritative. That distinction aligns with StoryOS's accepted rule that retrieval rank cannot confer truth, evidence, permission, or authority. [C5] [C6] [S6]

#### Durable inspection and replay

- **Observed:** the in-memory context manager stores model-visible history oldest first, normalizes it for a prompt, tracks a history version when history is rewritten, and enforces pairing/normalization rules for tool messages. It estimates tokens and can remove the oldest item. [C7]
- **Observed:** ordinary rollout items can include response items, a compaction replacement history, turn context, world state, and events. Rollouts are JSONL session records, and reconstruction finds the newest replacement checkpoint and rebuilds the active logical history from that checkpoint plus its suffix. [C8] [C9] [C10]
- **Observed:** inference-request tracing can record a model-visible request when enabled, but the trace mode can be disabled and recording is best-effort. Separately, websocket transport may send an incremental delta with a previous-response identifier when the new logical input is a strict extension and non-input request properties match. [C11] [C12]
- **Inference:** Codex's ordinary rollout is useful for replaying logical conversation state, but the inspected ordinary rollout surface is not equivalent to an always-present, attempt-scoped receipt of the complete provider projection. Optional best-effort inference tracing narrows that gap but does not prove a universal durable manifest. [C8] [C9] [C10] [C11] [C12]

#### Bounds, truncation, compaction, and cache effects

- **Observed:** model-visible function and custom-tool outputs are truncated before prompt construction, while messages and other items follow their own normalization path. Unsupported modalities can be stripped from prompt history. [C7]
- **Observed:** automatic compaction asks a model to summarize history. On a context-window error it can remove the oldest history item while retaining a prefix/recency strategy; after summarization it creates replacement history, reinjects defined initial context, persists a compaction item, and warns that long threads and repeated compaction reduce accuracy. Recent user messages are selected under a token budget and the boundary message can itself be truncated. [C13]
- **Observed:** the provider request carries a prompt-cache key that defaults to the session identifier. Incremental websocket reuse requires strict input extension and equality of the other request properties. [C2] [C12]
- **Inference:** cache stability is an optimization constraint that can influence when history is rewritten, but it does not establish what context is semantically mandatory or trustworthy. [C12] [C13]

#### Provider egress and controls

- **Observed:** the request builder sends the assembled input, serialized tool definitions, base instructions, model and reasoning controls, cache key, and metadata to the configured provider path. [C2]
- **Inference:** no minimum-necessary disclosure decision or item-level disclosure receipt is visible in that request-construction path; this is a scoped observation about the cited builder, not a claim that no control exists anywhere in Codex. [C2]
- **Observed:** the inspected controls include project-instruction byte limits, prompt normalization/truncation, deferred tool-definition exposure, explicit MCP resource addressing, and compaction. These are selection or size controls; none of the cited paths makes selected content authoritative. [C4] [C5] [C6] [C7] [C13]

### B.2 grok-build checkout at `a881e6703f46b01d8c7d4a5437683546df30449d`

#### Mandatory context and inspectable prompt inputs

- **Observed:** `PromptContext` is a serializable set of inputs to system-prompt rendering. It records prompt mode and audience, prompt body/template choice, discovered project-instruction files, memory enablement and paths, role/persona instructions, operating environment, working directory, date, and other render inputs. Primary and subagent audiences select different base templates; subagent normalization clears persona summaries but retains full project instructions. [G1]
- **Observed:** project-instruction discovery walks home/vendor locations and the repository root-to-CWD chain, reads named files and rule files, and stores each selected file's absolute path and content. Rendering preserves root-to-CWD order, states deeper precedence, and escapes reminder-like tags in untrusted file content so it cannot forge the harness delimiter. [G2]
- **Observed:** session startup persists the structured prompt context, installs the rendered system prompt in conversation history, inserts the project-instruction reminder when absent, and saves the exact rendered system prompt. [G3]
- **Inference:** grok-build makes the initial prompt inputs substantially more inspectable than a transcript alone, but `PromptContext` is session-level render provenance, not an attempt-level record of every later retrieved item, request-copy pruning decision, or tool definition. [G1] [G3] [G8]

#### Dynamic memory retrieval and admission

- **Observed:** memory search combines FTS5 BM25 and optional vector KNN, merges and normalizes scores, filters content-free chunks, applies temporal decay and source weights, adds an access-frequency boost, filters by minimum score, optionally applies MMR diversity reranking, and truncates to a maximum result count. Global and workspace memory are treated as evergreen; session memory decays. [G4] [G5]
- **Observed:** default search configuration returns at most six results, uses a minimum score of 0.35, weights vector/text signals 0.7/0.3, enables a seven-day session-memory half-life, and leaves MMR disabled by default. These are runtime defaults at the inspected revision, not StoryOS recommendations. [G6]
- **Observed:** the model can invoke `memory_search` with a query and optional result/score bounds, then use `memory_get` for a path and optional line range. Search results include score, source, file path, line range, staleness note, and snippet. Both tools declare read-only capability. [G7]
- **Observed:** separate first-turn injection is enabled by default. Unless configured otherwise, it uses a minimum score of 0.0, retrieves up to six results from the last user query or a broad greeting fallback, formats snippets with a 500-character per-result cap, and may persist the block into the leading system message. A previously persisted block is reused rather than reranked to preserve the prompt-cache prefix. [G8] [G9]
- **Inference:** grok-build's retrieval pipeline performs relevance qualification but not StoryOS-style durable truth admission. Its score combines similarity, source category, age, prior retrieval frequency, and optional diversity; those signals do not establish that a retrieved proposition is true or permitted for a particular outbound disclosure. [G4] [G5] [G6] [S6]

#### Request projection, bounds, and cache behavior

- **Observed:** request assembly starts from actor conversation state; near a 50 MB request-body ceiling it can replace older inline images, above 50% context utilization it can prune older tool results, and it can inject a memory reminder. It then sends the resulting items and tool definitions with model and sampling controls. Some operations mutate only the request clone; a memory reminder is persisted only when the caller requests persistence. [G10]
- **Observed:** old tool results can be replaced entirely or head/tail trimmed while recent turns are kept. Image eviction is deliberately delayed because rewriting earlier turns breaks the KV-cache prefix, and a lower reclaim target is used to avoid repeated cache busting. [G10]
- **Observed:** the default session compaction threshold is 85% of the model context window. Compaction is model-generated and bounded by a wall-clock budget; an optional two-pass mode can summarize a prefix before final compaction. [G11]
- **Observed:** the shared inter-compaction path drops system and tool items, strips tool-request content from assistant items, preserves user-visible material, chunks by a token budget, and creates summary blocks. Compaction output is validated before persistence, and checkpoint files plus update-stream markers support replay across compaction. [G12] [G13]
- **Inference:** what the model receives on one grok-build attempt may differ from durable full conversation state because request-copy pruning and image eviction are projection-time transformations. Prompt-context and chat-history artifacts therefore do not by themselves prove the exact attempt payload. [G3] [G10]

#### Durable records and runtime authority

- **Observed:** grok-build persists `prompt_context.json` best-effort for rerendering/inspection, saves `system_prompt.txt` as a convenience mirror, and keeps model-call conversation items in `chat_history.jsonl`. The exact rendered system prompt in the conversation head is treated as canonical over the mirror. [G3]
- **Observed:** for session export and reconstruction, `updates.jsonl` is described as the durable source of truth, while `chat_history.jsonl` is a derived cache used for LLM API calls and can be rebuilt from the update stream. [G14]
- **Observed:** the proxy metadata schema removed `prompt`, `full_prompt`, and a truncated-prompt local path at schema version 1.23, saying prompt content is no longer uploaded in metadata. That narrows debug-metadata disclosure; it does not mean the inference provider receives no prompt, because request assembly still sends conversation items and tools. [G15] [G10]
- **Inference:** these artifacts improve inspection and recovery but still do not form one complete, mandatory attempt manifest joining selected source revisions, ranking/admission reasons, projection-time omissions, tool exposure, provider route, and outbound disclosure. That conclusion is limited to the inspected persistence and request paths. [G3] [G10] [G14] [G15]

#### MCP and trust controls

- **Observed:** untrusted project plugins may be listed metadata-only, while hooks, MCP servers, and scripts are blocked. Independently, an untrusted workspace causes project-scoped MCP servers to be removed before spawn; managed allowlist/denylist policy can also disable servers with a recorded reason. [G16] [G17]
- **Observed:** project-instruction framing is escaped, but the retrieved memory and MCP/tool-result paths inspected here do not establish an item-level prompt-injection verdict that travels with content into the model request. The first half is directly observed; the second is a deliberately scoped absence in the cited paths. [G2] [G7] [G10] [G16] [G17]

### B.3 Claude Code npm snapshot `2.1.88`

- **Observed provenance only:** the unpacked snapshot identifies itself as `@anthropic-ai/claude-code` version `2.1.88`, with `cli.js` as the executable and no Git commit identity. [A0]
- **Unknown:** this audit does not prove the snapshot's exact context assembly, compaction, cache, MCP trust, provider-egress, or inspection behavior. The minified `cli.js` and generated source map were intentionally not exhaustively mined, and no behavior is inferred merely from the presence of `sdk-tools.d.ts`.
- **Unknown/current-doc boundary:** current Claude Code documentation is cited below only as a current-product comparison. Features described there are neither attributed to nor denied for this snapshot. Only local package evidence could elevate a behavior to “proven in 2.1.88.”

### B.4 Auxiliary current official and standards evidence

- **Current official documentation:** Claude Code currently documents `CLAUDE.md`
  and auto memory as cross-session mechanisms loaded as model context rather
  than enforced configuration. It also documents a bounded auto-memory index,
  `/context` inspection, deferred MCP tool definitions, and automatic context
  management that clears older Tool output and then summarizes conversation
  history, with possible loss of earlier detail. These facts are useful product
  comparisons, not proof of the local 2.1.88 bundle. [W1] [W2]
- **Current official documentation:** xAI's compaction API returns an opaque
  `encrypted_content` item that is meaningful only when sent back to xAI. The
  compaction call itself receives the conversation being compacted, and xAI
  documents that a client may replace its local message list with the opaque
  result. This is evidence that provider compaction can reduce runtime context
  while weakening independent inspection; it is not a suitable replacement for
  StoryOS-owned exact sources or historical context evidence. [W3]
- **Current official documentation:** xAI documents Grok Build as assembling
  prompt and file content locally, sending it through its inference proxy to the
  model, executing Tools locally, and changing provider retention under its ZDR
  mode. OpenAI separately documents endpoint-specific application-state and
  prompt-cache retention, third-party retention for remote MCP servers, and
  server-compaction behavior. Together these sources show that transfer,
  provider processing, caching, and retention are distinct facts. [W4] [W5]
- **Current specification:** MCP leaves resource inclusion to the host
  application and permits explicit selection, search/filtering, or heuristic
  inclusion. Its Tool specification requires clients to treat annotations as
  untrusted unless the server is trusted and recommends showing Tool inputs
  before sensitive calls to reduce accidental or malicious exfiltration. MCP
  discovery therefore supplies neither StoryOS admission nor disclosure
  authority by itself. [W6]
- **Current standards guidance:** W3C Privacy Principles recommends restricting
  transferred data to what is necessary for the user's goal or aligned with the
  user's wishes, with granular controls over communicated personal data. This
  supports destination- and purpose-specific minimization, but does not decide
  StoryOS's exact grants or UI. [W7]

## C. Cross-system comparison

| Dimension | Codex local `c9ef7e…` | grok-build `a881e6…` | Claude Code `2.1.88` | StoryOS-relevant reading |
|---|---|---|---|---|
| Mandatory envelope | Base instructions, history, tools, initial/turn context, project instructions. [C1] [C3] [C4] | Rendered system prompt, persisted prompt inputs, project-instruction reminder. [G1] [G2] [G3] | Local behavior unknown; current docs describe `CLAUDE.md` and auto memory as context, not enforcement. [A0] [W1] | Define mandatory categories independently of any vendor prompt template. [S1] [S6] |
| Dynamic retrieval | Deferred BM25 tool discovery and explicit MCP resource reads in inspected paths. [C5] [C6] | Hybrid cross-session memory search, explicit memory read, plus default first-turn injection. [G4] [G7] [G8] | Unknown. [A0] | Retrieval eligibility/admission must precede ranking; rank must not create authority. [S6] |
| Ranking signals | BM25 for tool metadata; no generic knowledge-ranking pipeline established here. [C5] | BM25, vectors, source weight, temporal decay, access count, threshold, optional MMR. [G4] [G5] [G6] | Unknown. [A0] | Signals are relevance evidence only, never truth or permission. [S6] |
| Durable inspection | Logical rollout history, world/turn state, compaction replacement; optional best-effort inference trace. [C8] [C9] [C10] [C11] | Structured prompt context, exact system-prompt mirror, model-call history, update-stream authority, compaction checkpoints. [G3] [G13] [G14] | Unknown. [A0] | A session transcript or prompt mirror is not automatically an attempt-level disclosure receipt. [S1] [S3] |
| Projection-time mutation | Tool-output truncation, modality normalization, compaction, incremental transport. [C7] [C12] [C13] | Request-copy tool pruning, image eviction, memory injection, compaction. [G10] [G11] [G12] | Local behavior unknown; current docs describe Tool-output clearing followed by summary compaction. [A0] [W2] | Persist both durable source history and what each attempt actually received; do not conflate them. [S1] [S6] |
| Cache influence | Session prompt-cache key and strict-extension reuse. [C2] [C12] | Persist/reuse memory block and delay history/image rewrites to preserve KV prefix. [G8] [G9] [G10] | Unknown. [A0] | Cache efficiency must remain subordinate to semantic correctness and disclosure policy. [S1] |
| Provider egress | Assembled input, tools, instructions, and request controls are sent to provider. [C2] | Projected conversation items, tools, and sampling controls are sent; debug metadata separately omits prompt text. [G10] [G15] | Unknown. [A0] | “Not in telemetry” is not “not disclosed to the model provider.” [G10] [G15] [S1] |
| Trust boundary | Explicit MCP addressing/access check; no item trust label established in inspected handler. [C6] | Workspace/plugin trust gates server execution; project-instruction delimiters are escaped. [G2] [G16] [G17] | Unknown. [A0] | Execution trust, content trust, authority, and disclosure permission are separate questions. [S1] [S2] |
| Include/exclude controls | Instruction budget, truncation, deferred tool exposure, explicit resource selection. [C4] [C5] [C6] | Memory enable/threshold/result controls, first-injection toggle, MCP disable/allow/deny gates. [G6] [G8] [G17] | Unknown. [A0] | Ticket #54 must decide author-visible controls without importing an upstream workflow regime. [S6] |

The strongest shared lesson is negative: none of the inspected evidence justifies equating “present in durable history,” “eligible for retrieval,” “selected for one request,” “sent to a provider,” “used by the model,” and “authoritative in StoryOS.” Each is a different state or claim. [C2] [C8] [C11] [G3] [G10] [G14] [S1] [S6]

## D. Accepted StoryOS constraints bounding interpretation

1. **Discovery writing is the product invariant.** The author discovers and advances the novel through present creative choices; the Agent assists the work currently before it. Context semantics cannot introduce an Agent-authored outline or planning regime that displaces this method. [S1] [S4]
2. **Authoritative state remains author-owned.** Agent, Tool, MCP, extension, bulk, cross-location, or not-fully-previsible changes require an inspectable StoryOS Core Proposal accepted by the author. Retrieval or context inclusion cannot itself authorize a state change. [S1] [S2]
3. **Authority, artifacts, and operational records are distinct.** Durable runtime evidence may explain what happened without becoming authoritative creative state. Network connections, providers, model processes, and MCP Apps are never the source of truth. [S1] [S2] [S3]
4. **Local project data is authoritative; outbound disclosure is bounded.** An external provider or MCP server receives only the minimum context required for the approved step, with provenance and disclosure recorded. Discovery never grants capabilities, context, budget, or outbound-data access beyond the current run's grant. [S1] [S5]
5. **Context remains bounded and inspectable.** Model-visible history is built incrementally rather than silently rewritten; each injected fragment is structured, attributable, inspectable, and hard-capped, and context assembly stays bounded as project and transcript stores grow. [S1]
6. **Accepted memory semantics qualify before ranking.** Retrieval indices are disposable projections; eligibility, suppression, source revision, retention, scope, permission, and evidence must be revalidated before context use. Ranking only orders currently eligible candidates and cannot establish truth, evidence, permission, or authority. [S6]
7. **Historical model-visible evidence is not rewritten.** A run records the exact admitted memory/source revisions and admission decisions through its context evidence; later correction or suppression affects future eligibility without falsifying what an earlier run received. “Available as context” does not prove model use or support for a conclusion. [S6]
8. **Ticket #54 owns a bounded frontier.** The accepted preceding specification assigns #54 mandatory/dynamic selection, ranking, budgets, explanation, author controls, and outbound disclosure, while preserving qualification-before-ranking, source/admission evidence, and non-interruptive discovery writing. [S6]

## E. Implications and options for Wayfinder #54

These are evidence-backed options for human decision, not accepted StoryOS decisions.

1. **Separate the pipeline into distinct semantic gates.** One option is to specify mandatory eligibility, dynamic candidate discovery, admission/qualification, ranking, budgeted projection, outbound disclosure authorization, provider attempt, and durable receipt as separate stages. The evidence shows that collapsing these stages hides projection-time pruning and encourages rank or cache behavior to masquerade as policy. [C2] [C7] [C11] [G4] [G10] [S6]
2. **Define a closed mandatory-context policy.** Rather than importing vendor prompt templates, #54 could decide which StoryOS-owned context categories are mandatory for a step, why each is mandatory, its source revision, hard cap, and failure behavior. Codex and grok-build both assemble heterogeneous mandatory envelopes, but neither upstream envelope is a StoryOS authority model. [C1] [C3] [C4] [G1] [G2] [S1]
3. **Require qualification before relevance ranking.** Candidate eligibility can account for project/run scope, suppression, retention, permission, source revision, trust class, and outbound destination before any BM25/vector/recency/MMR scoring. grok-build is useful evidence for ranking mechanics, while the accepted StoryOS memory spec supplies the authority boundary those mechanics lack. [G4] [G5] [G6] [S6]
4. **Distinguish durable history from an attempt receipt.** A robust option is to retain source-bearing history and compaction/replay records while also recording the actual logical projection for each provider attempt, including omissions/truncations and exposed tools. Codex rollout reconstruction and grok-build prompt/history artifacts are valuable patterns, but both leave scoped gaps at the exact attempt projection. [C8] [C10] [C11] [G3] [G10] [G14]
5. **Treat compaction as a sourced transformation, not silent continuity.** A compaction summary can be recorded as a new attributed item with input boundary, output, reason, and loss/limit indicators while preserving historical evidence. Both references demonstrate that compaction filters or replaces material and can lose fidelity. [C13] [G11] [G12] [G13] [S1]
6. **Make outbound disclosure a destination-specific gate.** The minimum necessary projection for a model provider, MCP server, embedding provider, or other service may differ even during one run. Provider request payload, telemetry/debug metadata, and durable local records should not be treated as the same disclosure surface. [C2] [G10] [G15] [S1]
7. **Carry trust and provenance with content.** Execution trust for a server/plugin, content trust for returned text, epistemic authority, and permission to disclose are separate dimensions. grok-build's server gates and delimiter escaping are useful controls, but neither proves that retrieved content is safe instruction. [C6] [G2] [G16] [G17] [S1]
8. **Expose author controls without interrupting discovery.** Candidate controls for decision include inspect, explain inclusion, pin for a bounded scope, explicit include, exclude for a run/attempt, and suppress from future retrieval. The accepted memory specification requires suppression and non-interruptive behavior; the precise control semantics remain #54 work. [S6]
9. **Do not let cache identity define semantic identity.** Reusing an old memory block or avoiding a context rewrite may improve cache efficiency but can preserve stale selection. #54 can explicitly state when policy/revision changes invalidate a cached projection and require a new disclosure decision. [C12] [G8] [G9] [G10] [S6]
10. **Keep runtime evidence non-authoritative.** A manifest, trace, summary, or replay artifact may be durable and inspectable while remaining an Operational Record rather than creative authority. [S2] [S3] [S4]
11. **Treat opaque provider continuity as a provider projection.** An encrypted
    compaction item or prior-response identifier may support continuation, but
    it cannot be the only StoryOS evidence of what sources were selected or what
    the model effectively received. [C12] [W3]
12. **Separate disclosure from retention claims.** ZDR, endpoint retention,
    prompt-cache lifetime, and omitted telemetry fields change what persists at
    a destination; they do not undo the transfer that already occurred. [G15]
    [W4] [W5]

### Anti-patterns to reject during specification

- Treating retrieval score, frequency of prior retrieval, or source category as truth or author consent. [G4] [G5] [S6]
- Treating a transcript, session prompt mirror, or reconstructed logical history as proof of the exact payload sent on every provider attempt. [C8] [C11] [G3] [G10]
- Treating prompt removal from telemetry metadata as proof that no prompt left the device. [G10] [G15]
- Silently rewriting earlier context during truncation or compaction without durable transformation evidence. [C13] [G12] [G13] [S1]
- Treating MCP/server execution trust as proof that returned content is safe instruction or authoritative project fact. [C6] [G16] [G17]
- Treating provider cache stability as a reason to retain context that has become ineligible, suppressed, stale, or overbroad for disclosure. [C12] [G8] [G9] [S6]
- Treating an opaque provider compaction object as independently inspectable
  project history or as a substitute for exact local source references. [W3]
- Treating ZDR or an omitted telemetry field as proof that project data was not
  disclosed to the inference provider. [G15] [W4] [W5]
- Importing Codex plan mode, `AGENTS.md`, grok-build project instructions, or any upstream prompt regime as a StoryOS Author Plan. [S1]

## F. Unresolved HITL decision questions

The following are questions for human decision, not answers:

1. Which StoryOS-owned context categories are mandatory for every model step, which are conditional, and what exact source revision must represent each category? [C1] [C3] [G1] [S6]
2. Which context may be discovered automatically, which requires an explicit author/run grant before search, and may any dynamic result be injected before the author or Agent asks for it? [G7] [G8] [S1]
3. What eligibility and admission checks must complete before ranking, and which failures exclude a candidate rather than merely lower its score? [G4] [G5] [S6]
4. Which ranking signals are acceptable for each source class, and may prior access frequency, temporal decay, or source weights influence StoryOS retrieval? [G4] [G5] [G6]
5. Does “pin” freeze content bytes, a source revision, an eligibility decision, a rank position, or only an inclusion preference, and for what scope and duration? [S6]
6. What must a `ContextManifest` prove: the selected logical items, rendered bytes, provider-specific projection, tool definitions, route/adapter, retry attempt, cache linkage, or all applicable layers? [C2] [C11] [G3] [G10]
7. When a provider protocol uses incremental input, prior-response identifiers, or cache reuse, how will inspection show the complete effective context rather than only the transmitted delta? [C12]
8. Which projection-time truncations or omissions are permitted, which require warning or approval, and which must fail the attempt rather than silently degrade it? [C7] [C13] [G10] [S1]
9. What compaction boundary, model, prompt, retained source evidence, and loss indicator are required, and can an author exclude compaction-generated summaries from future creative context? [C13] [G11] [G12] [G13]
10. How is minimum-necessary disclosure decided separately for the model provider, an embedding provider, each MCP server, hosted tools, and telemetry/debug systems? [C2] [G10] [G15] [S1]
11. Which Tool/MCP outputs are always marked untrusted model input, and can any Tool or server ever produce instructions rather than data without a separate explicit grant? [C6] [G16] [G17]
12. Which author controls exist at project, run, step, attempt, source, and individual-fragment scope for inspect, include, exclude, pin, and suppress? [S6]
13. How do suppression, correction, source revision, permission revocation, and retention expiry invalidate future retrieval and provider-cache reuse without rewriting historical receipts? [C12] [G9] [S6]
14. If durable manifest persistence fails but a provider attempt could still proceed, must the attempt fail closed, and which local recovery evidence is sufficient to retry safely? [C11] [G3] [G13]
15. What may the author inspect before disclosure, after disclosure, and during replay, and which sensitive values must be represented by provenance or hashes rather than repeated plaintext? [S1] [S6]

## G. Explicit non-decisions and limits

- This report does not select a context pipeline, algorithm, default, ranking formula, threshold, budget, provider, cache strategy, or disclosure policy.
- It does not define Rust structs, database tables, implementation schemas, persistence formats, migrations, APIs, or UI.
- It does not create an Agent-authored outline, planning regime, or StoryOS Author Plan.
- It does not make any upstream reference a StoryOS dependency or authority. The inspected Codex worktree revision is uncommitted local evidence and is not an accepted gitlink update. [S1]
- It does not claim exhaustive Codex or grok-build coverage. Absence claims are limited to the cited request, persistence, retrieval, and trust paths.
- It does not claim any Claude Code runtime behavior beyond package identity/version. The package has no local Git commit identity, and the minified/generated bundle was timeboxed rather than exhaustively read. [A0]
- It does not use current Claude Code documentation as evidence for 2.1.88, and it does not deny newer documented features.
- Current vendor policy and product documentation is a dated auxiliary snapshot,
  not a contractual guarantee for StoryOS or evidence about the local package's
  exact implementation.
- It is a static source audit, not a network capture or runtime experiment. It does not prove provider-side transformations, hidden transport behavior, actual cache hits, model attention/use, or the bytes observed beyond the local client boundary.
- It does not change StoryOS issues, accepted context vocabulary, ADRs, reference sources, repository state, or any file other than this report.

## H. Source index

All local line citations below refer to the source state recorded in section A. Access date for repository and GitHub issue evidence: 2026-07-21.

### StoryOS accepted sources

- **[S1]** StoryOS root `AGENTS.md:3-27, 49-54, 72-80` — repository authority, discovery-writing/product invariants, reference-source policy, model-visible context, and agent-skill routing.
- **[S2]** `docs/adr/0001-separate-authoritative-state-artifacts-and-operational-records.md:1-19` — accepted separation of authority, artifacts, operational records, Proposal, and Acceptance.
- **[S3]** `docs/adr/0002-specify-transcript-and-mcp-app-lifecycle-semantics.md:1-50, 155-175, 198-216` — MCP App runtime versus StoryOS durable authority, context-policy boundary, and non-inherited authority.
- **[S4]** `CONTEXT.md:7-32, 90-112, 406-475` — Discovery Writing, Authoritative State, Operational Record, Agent Memory, Working Context, Retrieval Index, AgentRun, Agent Decision, RunPlan, Run Budget, Outbound Disclosure, Capability Grant, and Model Gateway vocabulary.
- **[S5]** `CONTEXT.md:474-535, 562-607` — provider routing/attempt vocabulary and Tool discovery/exposure/gateway/effect vocabulary.
- **[S6]** `docs/foundation/fiction-memory-and-research-provenance-semantics.md:1-38, 50-93, 222-286, 300-373, 384-424` — accepted memory scope, source-bearing semantics, qualification-before-ranking, model-visible evidence, suppression/revision rules, #54 ownership, and invariants.

### Codex local worktree sources (`c9ef7eff005c3299a5a5f0004c34c6a3eedf2564`)

- **[C1]** `.reference/codex/codex-rs/core/src/client_common.rs:16-36, 51-61` — logical prompt fields and input formatting.
- **[C2]** `.reference/codex/codex-rs/core/src/client.rs:470-474, 824-908` — prompt-cache key and provider request construction.
- **[C3]** `.reference/codex/codex-rs/core/src/session/mod.rs:3197-3464, 3541-3638` — initial model-visible context, extension/MCP contribution, baseline versus context diffs, and persistence order.
- **[C4]** `.reference/codex/codex-rs/core/src/agents_md.rs:1-16, 83-151, 310-417` — repository instruction discovery, byte budget, provenance, and rendering.
- **[C5]** `.reference/codex/codex-rs/core/src/tools/handlers/tool_search_spec.rs:7-61`; `.reference/codex/codex-rs/core/src/tools/handlers/tool_search.rs:68-94, 116-180` — deferred BM25 tool metadata discovery.
- **[C6]** `.reference/codex/codex-rs/core/src/tools/handlers/mcp_resource/read_mcp_resource.rs:48-120` — explicit MCP resource access and output truncation.
- **[C7]** `.reference/codex/codex-rs/core/src/context_manager/history.rs:38-60, 123-207, 324-386, 437-479` — prompt history, normalization, token estimation, removal, pairing, and tool-output truncation.
- **[C8]** `.reference/codex/codex-rs/protocol/src/protocol.rs:3209-3261` — rollout item and compaction replacement-history variants.
- **[C9]** `.reference/codex/codex-rs/rollout/src/recorder.rs:75-88, 939-1040` — JSONL rollout persistence and resume/load behavior.
- **[C10]** `.reference/codex/codex-rs/core/src/session/rollout_reconstruction.rs:139-185, 317-385` — replacement-checkpoint discovery and active-history reconstruction.
- **[C11]** `.reference/codex/codex-rs/rollout-trace/src/inference.rs:30-78, 169-197`; `.reference/codex/codex-rs/rollout-trace/src/thread.rs:352-398` — disabled/enabled best-effort model-visible request tracing.
- **[C12]** `.reference/codex/codex-rs/core/src/client.rs:304-360, 1165-1210, 1603-1652` — request-property matching, strict-extension incremental websocket input, and logical-request trace behavior.
- **[C13]** `.reference/codex/codex-rs/core/src/compact.rs:92-120, 221-377, 528-662`; `.reference/codex/codex-rs/core/src/session/mod.rs:3042-3081` — automatic compaction, context-window recovery, replacement history, initial-context reinjection, recent-user selection, and persistence.

### grok-build local sources (`a881e6703f46b01d8c7d4a5437683546df30449d`)

- **[G0]** `.reference/grok-build/SOURCE_REV:1` — embedded source revision `c5c4ce03436b4bb2cec43d3feaa27dee0109bf37`.
- **[G1]** `.reference/grok-build/crates/codegen/xai-grok-agent/src/prompt/context.rs:1-9, 68-171, 198-296` — serializable prompt inputs, audience normalization, placeholders, and rendering.
- **[G2]** `.reference/grok-build/crates/codegen/xai-grok-agent/src/prompt/agents_md.rs:144-308, 310-360` — project-instruction discovery, precedence, provenance, and delimiter neutralization.
- **[G3]** `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session.rs:1241-1264, 1310-1355`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/spawn.rs:942-980` — prompt-context/system-prompt persistence and startup injection.
- **[G4]** `.reference/grok-build/crates/codegen/xai-grok-memory/src/search.rs:1-18, 26-58, 108-190, 193-388` — hybrid retrieval, filtering, decay, scoring, threshold, MMR call, and result cap.
- **[G5]** `.reference/grok-build/crates/codegen/xai-grok-memory/src/mmr.rs:1-13, 49-117` — optional diversity reranking algorithm.
- **[G6]** `.reference/grok-build/crates/codegen/xai-grok-config-types/src/memory.rs:11-28, 52-158, 184-220, 226-258` — chunk/search defaults, temporal decay, MMR, first-turn injection, and session-save defaults.
- **[G7]** `.reference/grok-build/crates/codegen/xai-grok-tools/src/implementations/memory/search_tool.rs:13-108`; `.reference/grok-build/crates/codegen/xai-grok-tools/src/implementations/memory/get_tool.rs:33-120`; `.reference/grok-build/crates/codegen/xai-grok-tools/src/implementations/memory/types.rs:6-52` — model-initiated memory search/read controls and output provenance.
- **[G8]** `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/turn.rs:1512-1595`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/acp_session_impl/memory_dream.rs:12-34` — first-turn retrieval query, result cap, threshold, cache-preserving reuse, and injection.
- **[G9]** `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/helpers/memory_context.rs:1-64` — persisted memory-block detection and 500-character snippet formatting.
- **[G10]** `.reference/grok-build/crates/codegen/xai-chat-state/src/actor/request_builder.rs:20-148, 155-265` — request-copy memory injection, tool-result pruning, image eviction, cache considerations, and request assembly.
- **[G11]** `.reference/grok-build/crates/codegen/xai-grok-agent/src/compaction.rs:1-45`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/compaction.rs:1711-1820` — default compaction policy and auto-compaction trigger.
- **[G12]** `.reference/grok-build/crates/common/xai-grok-compaction/src/inter_compaction/compact.rs:40-225`; `.reference/grok-build/crates/common/xai-grok-compaction/src/history/filter.rs:10-70` — filtering, chunking, summary assembly, and re-compaction behavior.
- **[G13]** `.reference/grok-build/crates/common/xai-grok-compaction/src/history/validate.rs:1-70`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/compaction.rs:2073-2119`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/helpers/replay.rs:33-88, 250-431` — validation, durable checkpoints, and replay.
- **[G14]** `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/storage/mod.rs:24-34, 94-150`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/export.rs:1-8` — update-stream authority and derived chat-history cache.
- **[G15]** `.reference/grok-build/prod/mc/cli-chat-proxy-types/src/metadata_types.rs:1-7, 10-39, 49-110` — prompt metadata version history and removal of prompt content from metadata.
- **[G16]** `.reference/grok-build/crates/codegen/xai-grok-agent/src/plugins/trust.rs:1-18, 35-74` — plugin execution trust and blocked untrusted capabilities.
- **[G17]** `.reference/grok-build/crates/codegen/xai-grok-shell/src/agent/folder_trust.rs:440-474`; `.reference/grok-build/crates/codegen/xai-grok-shell/src/session/managed_mcp.rs:110-145, 147-225` — project MCP folder-trust gate and managed allow/deny policy.

### Claude Code local package source

- **[A0]** `.reference/claude-code/package.json:1-16` — npm identity, version `2.1.88`, executable, author, and project homepage. No commit is asserted.

### Current official web and standards sources

Access date: 2026-07-21. These sources describe current public contracts or
guidance and are not evidence of the local Claude Code 2.1.88 implementation.

- **[W1]** Anthropic, [How Claude remembers your project](https://code.claude.com/docs/en/memory) — current `CLAUDE.md`, auto-memory, scope, load, inspection, and context-versus-enforcement behavior.
- **[W2]** Anthropic, [How Claude Code works](https://code.claude.com/docs/en/how-claude-code-works) — current context-window contents, deferred Tool definitions, inspection, and compaction behavior.
- **[W3]** xAI, [Context Compaction](https://docs.x.ai/developers/advanced-api-usage/context-compaction) — provider-side compaction input, opaque `encrypted_content`, reuse, and documented limits.
- **[W4]** xAI, [Grok Build Enterprise Deployments: privacy and data lifecycle](https://docs.x.ai/build/enterprise#privacy--data-lifecycle) — prompt/file transfer, local Tool execution, and ZDR retention behavior.
- **[W5]** OpenAI, [Data controls in the OpenAI platform](https://developers.openai.com/api/docs/guides/your-data#default-usage-policies-by-endpoint) — endpoint application-state, cache, compaction, and third-party MCP retention boundaries.
- **[W6]** Model Context Protocol 2025-11-25, [Resources](https://modelcontextprotocol.io/specification/2025-11-25/server/resources) and [Tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools) — host-controlled resource inclusion, untrusted Tool annotations, disclosure-oriented confirmation, and result validation.
- **[W7]** W3C, [Privacy Principles: Data Minimization](https://www.w3.org/TR/privacy-principles/#data-minimization) — purpose-bound transfer minimization and granular user controls.
