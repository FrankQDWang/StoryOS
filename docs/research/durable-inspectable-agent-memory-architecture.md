# Durable, inspectable Agent memory architecture: source research and StoryOS implications

- Status: research complete; decision input for [Research Durable, Inspectable Agent Memory Architecture](https://github.com/FrankQDWang/StoryOS/issues/65), not a domain decision or implementation authorization
- Accepted downstream contract: [Fiction Memory and Research Provenance Semantics](../foundation/fiction-memory-and-research-provenance-semantics.md)
- Repository baseline: `main` as inspected on 2026-07-20
- External-source check date: 2026-07-20
- Reference baseline: `.reference/codex` at pinned commit `1f0566d3f59298d1bb88820a0d35294f1eeb07ea`; read-only implementation evidence, never a StoryOS dependency or contract
- Evidence policy: original papers, standards, official framework documentation, pinned first-party source, and current StoryOS repository contracts

## Research question and scope

What durable lifecycle can give one project-scoped StoryOS Agent useful continuity across threads and Runs while keeping every stored inference, retrieval, correction, deletion, and model-context use attributable and author-controllable?

This report covers:

- working versus cross-Run memory;
- episodic or experience records, semantic project knowledge, and procedural preferences or Skills;
- source-of-record evidence versus extracted and consolidated memory;
- write and admission gates, consolidation, correction, contradiction, supersession, confidence, and temporal scope;
- retention, retrieval decay, archival, tombstoning, projection rebuilding, and recovery;
- project, thread, and Run scope;
- author inspection, editing, exclusion, and deletion;
- provenance, privacy, outbound disclosure, and the main long-lived-memory failure modes.

This report does **not**:

- accept a final memory taxonomy or introduce final Core types;
- define Rust structs, wire schemas, tables, indexes, ranking weights, or UI layouts;
- make generated memory authoritative;
- let an Agent silently rewrite its policy, Skills, Authoritative State, or durable identity;
- replace the fiction-memory, context, storage, protocol, or Run-retention tickets;
- treat an experimental Agent result as a production safety guarantee.

## Evidence labels and limits

- **External source fact**: directly stated or demonstrated by an original paper, standard, official framework document, or pinned first-party implementation.
- **StoryOS-local fact**: already accepted in the repository or a closed decision ticket.
- **Evidence-backed inference**: a reasoned consequence of the two preceding classes; it is not an accepted StoryOS contract.
- **Recommendation**: a proposed answer for later HITL/domain-modeling work; it remains explicitly non-final.

The evidence has important limits. CoALA is a conceptual organizing framework. Generative Agents, Reflexion, MemGPT, and Voyager were evaluated on bounded research tasks, not author-owned creative projects. Letta and LangGraph document implementation choices, not universal semantics. The pinned Codex implementation is useful operational evidence but is neither a StoryOS dependency nor permission to copy its user-level memory policy.

## HOW coverage and decision boundary

The report answers the ticket's lifecycle question at the semantic level: working and cross-Run continuity are separated from episodic evidence, semantic claims, and procedural candidates; writes pass through settled-input, source-class, scope, privacy, and minimum-signal gates; extraction and consolidation preserve exact lineage; correction, contradiction, and supersession append inspectable history; retention distinguishes decay, archive, and tombstone; retrieval is separated from model-context admission; and every item is bounded to an explicit project/thread/Run/subject scope. The failure-mode table then covers poisoning, feedback loops, stale or recursively reinforced summaries, hidden self-modification, unbounded growth, scope and privacy leaks, lost corrections, ghost deletion, duplicate recovery, and false success lessons.

Those constraints remain research evidence rather than the accepted contract. The fiction-memory and research-provenance decisions were subsequently resolved in [Fiction Memory and Research Provenance Semantics](../foundation/fiction-memory-and-research-provenance-semantics.md). Context assembly, physical storage, protocol shape, and Run retention remain with [Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54), [Specify the Self-Contained Project Storage and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56), [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58), and [Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64).

## Primary-source facts

### 1. “Memory” spans distinct information and control roles

CoALA separates short-term working memory from long-term episodic, semantic, and procedural memory. Its working memory holds active percepts, goals, retrieved knowledge, and intermediate state across decision cycles; episodic memory holds prior experiences; semantic memory holds knowledge; procedural memory includes the code and procedures that determine behavior. CoALA also distinguishes retrieval, which reads long-term memory into working memory, from learning, which writes long-term memory. See [CoALA sections 4.1 and 4.3–4.5](https://ar5iv.labs.arxiv.org/html/2309.02427#S4.SS1).

CoALA explicitly warns that allowing an Agent to write procedural memory is riskier than writing episodic or semantic memory because it can introduce bugs or subvert designer intent. It also identifies modification and deletion as understudied compared with accumulation. See [CoALA, procedural memory and learning](https://ar5iv.labs.arxiv.org/html/2309.02427#S4.SS1) and [section 4.5](https://ar5iv.labs.arxiv.org/html/2309.02427#S4.SS5).

**Evidence limit:** these cognitive labels describe functional roles. They do not decide StoryOS authority, Artifact kinds, persistence boundaries, or author controls.

### 2. Append-only experience plus derived reflection can improve behavior, but derivation can recursively amplify error

Generative Agents records perceived experiences in a natural-language memory stream. Retrieval ranks entries using recency, importance, and relevance, then includes the highest-ranked entries that fit the context window. Reflections are higher-level LLM-generated abstractions stored back into the same stream; reflections may be derived from observations or earlier reflections, producing reflection trees. See [Generative Agents sections 4.1–4.2](https://ar5iv.labs.arxiv.org/html/2304.03442#S4.SS1) and [retrieval scoring](https://ar5iv.labs.arxiv.org/html/2304.03442#S4.SS1.SSS2).

The study's ablations found that observation, planning, and reflection all contributed to the evaluated believability measure, but the paper also reports retrieval failures, incomplete fragments, and hallucinated relationship knowledge. See [Generative Agents section 6](https://ar5iv.labs.arxiv.org/html/2304.03442#S6).

**Implication:** synthesis can be valuable, but a reflection cannot become stronger evidence merely because another reflection later cites it. Recursive derivation needs visible lineage back to source observations and must preserve uncertainty and contradiction.

### 3. Verbal lessons are useful only when their feedback is credible and their scope stays bounded

Reflexion separates a recent trajectory from a persistent buffer of verbal self-reflections. An Evaluator supplies a reward or feedback signal, an LLM derives a reflection, and the Actor conditions later trials on it. The experiments normally retained only one to three experiences because of context limits. The authors state that the method depends on LLM self-evaluation or heuristics and has no formal success guarantee; incorrect test feedback can produce harmful reflection. See [Reflexion sections 3 and 5](https://ar5iv.labs.arxiv.org/html/2303.11366#S3).

**Implication:** “the Agent learned this” must bind the claimed lesson to the task, exact observed outcome, evaluator, and applicable scope. Self-assessment alone is candidate evidence, not validation.

### 4. Tiered context management separates durable history from what is currently visible

MemGPT divides main context into read-only system instructions, writable working context, and a FIFO message queue. Incoming and generated messages are also written to recall storage. Under memory pressure, messages are evicted from the visible queue and recursively summarized, while the originals remain retrievable; archival storage is a separate searchable store. See [MemGPT sections 2.1–2.4](https://ar5iv.labs.arxiv.org/html/2310.08560#S2).

Current Letta documentation exposes persistent, always-visible memory blocks that can be Agent-writable or read-only, plus out-of-context archival memory searched by Tools. It warns that concurrent block updates are last-write-wins full replacements. See [Letta memory blocks](https://docs.letta.com/guides/core-concepts/memory/memory-blocks) and [Letta context hierarchy](https://docs.letta.com/guides/core-concepts/memory/context-hierarchy).

**Implication:** persistence, current visibility, write authority, and retrieval are independent axes. StoryOS should not equate “durable” with “always injected,” or accept last-write-wins Agent edits for shared project memory.

### 5. Procedural reuse becomes executable authority unless verification and capability remain separate

Voyager stores successful executable programs in an embedding-indexed skill library. It commits a program only after iterative execution feedback, error feedback, and an LLM self-verification step, then retrieves Skills by task-plan and environment-feedback similarity. See [Voyager sections 2.2–2.3](https://ar5iv.labs.arxiv.org/html/2305.16291#S2.SS2).

Voyager demonstrates that verified examples can support reuse in its Minecraft environment. It does not establish that model self-verification safely authorizes arbitrary production code or privileges.

**Implication:** a StoryOS procedural lesson may suggest a Skill or policy change, but it must not silently become executable code, capability, installation, or Agent instruction. Existing Skill snapshot, Tool Gateway, grant, and Approval boundaries still apply.

### 6. Official framework APIs expose useful scope/write trade-offs, not a finished semantics

LangGraph distinguishes thread-scoped short-term state persisted by a checkpointer from cross-thread long-term documents stored in custom namespaces. Its guide presents semantic, episodic, and procedural categories, and distinguishes writes performed in the interaction's hot path from asynchronous background extraction. It also notes update errors and over-insertion/over-updating risks. See [LangGraph memory overview](https://docs.langchain.com/oss/python/concepts/memory) and [memory guide](https://docs.langchain.com/oss/python/langgraph/add-memory).

**Implication:** StoryOS can record source evidence synchronously while extracting reusable candidates asynchronously. The background path still needs the same provenance, scope, policy, and author-control gates as foreground work.

### 7. Retrieval stores are a security boundary

AgentPoison demonstrates a backdoor attack that poisons long-term memory or a retrieval knowledge base so malicious demonstrations are preferentially retrieved and steer later Agent actions. In the reported experiments, very few poisoned records produced high retrieval and end-to-end attack success while leaving benign performance mostly unchanged. See [AgentPoison, NeurIPS 2024](https://papers.nips.cc/paper_files/paper/2024/file/eb113910e9c3f6242541c1652e30dfd6-Paper-Conference.pdf).

**Implication:** valid storage provenance does not make stored content trusted instructions. Write admission and retrieval-time context admission are separate controls; retrieved memory must remain inert data unless another established boundary authorizes an effect.

### 8. Provenance records production and influence, not truth or authority

W3C PROV models entities, activities, agents, generation, usage, derivation, and responsibility. It says provenance can support assessments of quality or trustworthiness; it does not make those assessments or certify truth. Its model is domain-agnostic. See [PROV-DM, W3C Recommendation](https://www.w3.org/TR/2013/REC-prov-dm-20130430/).

**Implication:** exact lineage is necessary for inspectable memory, but StoryOS still needs domain-specific validation, contradiction, authority, temporal-scope, and retention semantics.

### 9. The pinned Codex implementation shows a practical bounded extraction/consolidation pipeline

At the pinned reference commit, Codex runs memory only for eligible non-ephemeral root sessions with a state database. Phase 1 claims a bounded set of sufficiently idle rollouts, filters response items, extracts structured raw memory and a rollout summary in parallel, redacts secrets, persists outcomes, and permits no output. Phase 2 takes a global lease, selects a bounded input set using last use or generation time and usage count, synchronizes filesystem evidence, computes a workspace diff, serializes one consolidation Agent, and prunes stale selected summaries. See the pinned [memory pipeline README](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/README.md), [Phase 1 source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/write/src/phase1.rs), and [Phase 2 source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/write/src/phase2.rs).

The extraction instructions treat raw rollouts as immutable evidence, third-party text as data rather than instructions, user statements and tool validation as stronger evidence than assistant summaries, secrets as excluded, and no-op as preferable to low-signal storage. The read crate parses citations back to files, line spans, and rollout IDs. See the pinned [extraction prompt](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/write/templates/memories/stage_one_system.md) and [citation parser](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/read/src/citations.rs).

**What transfers as evidence:** bounded claims and leases, immutable inputs, two-stage extraction, secret filtering, no-op admission, one consolidating writer, explicit diffs, source citations, usage telemetry, and rebuildable derived files.

**What does not transfer as a contract:** free-form global memory files, usage-count-first selection, background Agent edits without StoryOS Artifact lifecycle, filesystem Git as product authority, implicit global user memory, or injection without a per-call ContextManifest.

## StoryOS-local facts

1. Closed decision [Give Each Project One Persistent Main Agent](https://github.com/FrankQDWang/StoryOS/issues/5#issuecomment-4933815720) chose one long-lived main Agent experience per project across multiple threads. Continuity must come from explicit persisted project state, threads, and memory, never hidden process state.
2. Closed decision [Combine Mandatory Context with Dynamic Retrieval](https://github.com/FrankQDWang/StoryOS/issues/15#issuecomment-4933828408) requires mandatory project/thread/selection/permission/pinned state plus on-demand chapter, memory, and research retrieval. Every call saves an inspectable `ContextManifest`; the author can pin, exclude, and inspect sources.
3. Closed decision [Forbid Hidden Persistent Agent Memory](https://github.com/FrankQDWang/StoryOS/issues/16#issuecomment-4933828965) requires typed long-term memory rather than one hidden bucket; vector indexes are retrieval projections; temporary reasoning drafts exist only in the current Run.
4. Current [CONTEXT.md](../../CONTEXT.md) supersedes older shorthand where terms differ. `Authoritative State` is binary author-approved current project truth. An `Operational Record` explains execution. An `Artifact` is durable typed content or evidence but is never authoritative in place.
5. `AgentRun` is a durable bounded execution aggregate that survives restart. Terminal Runs are immutable; retry or continuation creates a causally linked new Run. `Run Event` is an immutable sequenced committed transition. `Run Checkpoint` is replaceable and rebuildable.
6. A `Step Snapshot` records exact context sources, revisions, contracts, Skills, capabilities, budgets, and guardrails used for one decision. A `Subrun Context Bundle` is immutable, attributable, and hard-bounded; parent context never flows implicitly. Mailboxes and progress summaries are durable communication, not shared memory or execution truth.
7. The accepted [Artifact domain model](../foundation/artifact-domain-model.md) already defines immutable Artifact Revisions, Provenance, Derivation, Supersession, Research Source Snapshots, Research Syntheses and Claims, Candidates, Proposals, and retention states.
8. `available_as_context` differs from `supported_by`: availability proves exposure, not that the item supported a result. Derivation creates a new Artifact from exact source revisions and never rewrites a source kind in place.
9. Archived Artifacts remain inspectable but are excluded from normal retrieval and model context. Tombstoning removes owned payload, indexes, and derived caches while preserving the minimum non-content audit tombstone and explicit provenance gap.
10. Only a StoryOS Core Proposal plus Acceptance can apply Agent-, Tool-, MCP-, extension-, bulk-, cross-location-, or not-fully-previsible changes to Authoritative State. A memory write cannot bypass that boundary.
11. Local project data is authoritative. Outbound Disclosure requires an explicit grant and evidence of destination, purpose, data categories, and project sources; transforming content before transfer does not stop it being disclosure.
12. The adjacent [fiction-memory research](fiction-memory-and-research-provenance-semantics.md) already concludes that cognitive categories are not product types, fiction time and epistemic scope are separate from audit time, research evidence does not become creative authority, and semantic indexes are rebuildable projections.

## Architecture option comparison

| Option | Durable source | Write model | Strength | Critical StoryOS risk | Verdict |
|---|---|---|---|---|---|
| A. Transcript/Run replay only | Messages, Run Events, ToolCalls, results | Source records only | Maximum fidelity; no derived hallucination | Unbounded retrieval, weak reuse, repeated synthesis cost | Necessary source layer, insufficient memory architecture |
| B. Mutable always-visible memory blocks | Current block value | Agent or application overwrites | Simple continuity; predictable context presence | Opaque self-modification, lost history, concurrency loss, authority confusion | Reject as the general durable model; allow only explicitly owned revisioned projections |
| C. Append-only experience stream | Observations/episodes | Append observations; rank at read time | Inspectable history; simple recovery | Growth, poisoning, irrelevant recall, retrieval feedback loops | Useful episodic evidence pattern with strict admission and retention |
| D. Recursive summaries/reflections | Derived summaries plus sources | Background or threshold consolidation | Compression and reusable lessons | Hallucination amplification, stale summaries, circular support | Use only as revisioned derived Artifacts with exact lineage and conflict handling |
| E. Retrieved procedural examples/Skills | Verified trajectories or executable Skills | Commit after evaluation | Reuse of successful methods | Model validation is not authorization; executable privilege escalation | Keep procedural suggestion separate from Skill/package/capability admission |
| F. Typed source-first hybrid | Operational evidence + authoritative objects + derived Artifacts + rebuildable indexes | Synchronous evidence, gated candidate extraction, serialized consolidation | Fits existing authority/provenance/context contracts; supports correction and recovery | More explicit lifecycle and UI work | **Recommended for HITL discussion, non-final** |

The recommended direction is F: do not build a fourth durable truth space called “Agent memory.” Reuse the three accepted spaces and add only the minimum memory-specific semantics required to connect them.

## Recommended non-final semantic lifecycle

### Architectural position

Treat “Agent memory” as a **use-case view over typed durable records and Artifacts**, not a single store or authority class:

- working state belongs to the current Run/Step and its bounded context evidence;
- exact experience remains in Messages, Run Events, ToolCalls, results, receipts, and source Artifacts;
- extracted episodic lessons, project-knowledge claims, and procedural-preference candidates are derived Artifacts, never rewrites of their sources;
- author-approved project truth remains Authoritative State or explicit project configuration under its owning domain;
- search, embeddings, rank features, cached summaries, and “current memory” views are rebuildable projections;
- model exposure is a separate, recorded ContextManifest decision.

### Concrete pipeline

```text
Run evidence
  -> bounded candidate extraction
  -> write/admission gate
  -> validation and optional consolidation
  -> durable typed storage with exact provenance
  -> rebuildable retrieval projections
  -> retrieval candidates for one RunStep
  -> policy/context admission
  -> ContextManifest + Step Snapshot
  -> model use and resulting evidence
  -> correction / contradiction / supersession
  -> retain / archive / tombstone
  -> projection rebuild and recovery
```

1. **Record source evidence.** Persist the exact Message, Run Event range, ToolCall result, Domain Receipt, author feedback, Artifact Revision, and relevant Step Snapshot through their existing owners. Do not wait for memory extraction to make execution recoverable.
2. **Detect bounded candidate material.** A policy or explicit author action selects eligible, settled evidence. Active Runs, unconfirmed effects, hidden reasoning, secrets, raw untrusted instructions, and unsupported assistant claims are ineligible by default.
3. **Extract candidates without rewriting sources.** A model or deterministic extractor may propose one independently reviewable item at a time: an episode/lesson, a project-knowledge assertion, or a procedural preference/Skill suggestion. It records exact source revisions or Run event spans, extraction prompt/model/Skill snapshot, scope, applicable time, claimed confidence, and whether content came from author, tool/environment, model inference, or external source.
4. **Apply the admission gate.** Validate schema and bounds; reject or quarantine secrets, scope leaks, untrusted instructions, circular-only support, absent sources, unresolved outcomes, and duplicates. No-op is a successful result. Admission means only “durably available for review/retrieval,” never “true,” “author-approved,” or “authoritative.”
5. **Validate or consolidate.** Deterministic outcomes and explicit author statements may support a candidate directly. Model-derived lessons remain inferences. Consolidation creates a new derived revision or Artifact over exact inputs; it never overwrites raw evidence. One logical subject has one serialized writer or expected-revision conflict. Contradictions remain visible rather than being silently merged.
6. **Store typed durable content.** The conceptual memory item is an Artifact unless its content already belongs to Authoritative State, project policy/configuration, a SkillPackage, or an Operational Record. Do not duplicate the authoritative object into a second mutable memory truth; store a reference or derived view.
7. **Build projections.** Text, graph, embedding, recency, usage, contradiction, and scope indexes declare the exact source revision set and build generation they cover. They can lag, fail, be deleted, and be rebuilt without changing durable content or authority.
8. **Retrieve for a specific need.** Query within the project and permitted thread/Run scope. Filter by retention, author exclusions, capability, data sensitivity, temporal validity, contradiction state, and freshness before ranking. Usage can inform ranking but never truth, confidence, or promotion.
9. **Admit into model context.** The context assembler chooses exact revisions within hard per-item and aggregate budgets, with reason, rank inputs, source class, trust label, truncation or transformation, and disclosure classification. It records the decision in `ContextManifest`, then the `Step Snapshot` binds what the decision actually used.
10. **Record use separately from exposure.** `available_as_context` records that an item was supplied. A result may add `supported_by` or `derived_from` only when the producer explicitly identifies that role. Retrieval count is not evidence that the memory was correct or causally useful.
11. **Correct without erasing history.** An author edit appends a new Artifact Revision or uses the owning domain command. A contradiction may create a competing candidate or relation. Supersession is explicit, purpose-qualified, and preserves both histories. Confidence changes never mutate authority.
12. **Retain, archive, or tombstone.** Retrieval decay lowers discoverability only; it does not delete content or mean “less true.” Archive is reversible and excludes normal retrieval/context. Tombstone is terminal for owned payload, removes all projections/caches, retains the minimum audit record, and renders provenance gaps explicitly.
13. **Recover from durable truth.** Rebuild read models and indexes from durable records and Artifact revisions. Resume unfinished extraction/consolidation from claimed inputs and idempotency evidence. Never recover a memory write from model process state or an index alone.

## Conceptual type, owner, authority, source, and lifecycle matrix

These rows are semantic roles, not final type names.

| Role | Scope | Owning durable space | Source of record | Authority | Proposed lifecycle |
|---|---|---|---|---|---|
| Run working state | Run/Step | Operational Records | exact Run records and Step Snapshot | none beyond frozen grants | bounded active state; terminal evidence immutable |
| Thread conversation history | project + thread | Message Artifacts / transcript records | exact Message revisions and attachments | non-authoritative | revise only where Message contract permits; retain/archive/tombstone |
| Experience episode | project, sourced from one or more Runs | likely derived Artifact | exact Run/Message/Tool/Receipt spans | non-authoritative | candidate/open; revise, close, supersede, archive, tombstone |
| Procedural lesson | project, optionally task/domain-scoped | likely derived Artifact or Candidate | verified outcomes plus author feedback | non-authoritative | validate applicability; may source Skill/policy Proposal; never executes itself |
| Author procedural preference | project by default; wider scope requires a separate decision | unresolved: author-owned project setting or Artifact | explicit author action, not assistant inference | only its owning project-setting command may make it binding | inspect/edit/revoke/version; inferred candidates remain separate |
| Semantic project-knowledge claim | project + subject + valid-time/epistemic scope where relevant | Research Synthesis/Claim, Candidate, or owning domain | exact source/authoritative revisions | non-authoritative unless independently applied through owning domain | revise, contradict, supersede, archive, tombstone |
| Authoritative fiction fact | project | Authoritative State | current authoritative revision | authoritative | owning domain revision and Proposal/Acceptance rules |
| Consolidated memory view | project or subject | derived Artifact/read model | exact included revisions and consolidation activity | non-authoritative | regenerate as new revision; old view remains attributable |
| Retrieval index/rank feature | project projection | storage/read model | declared covered revisions | none | stale/rebuildable; delete with tombstone; never cited as source |
| ContextManifest entry | RunStep/model call | Operational Record | context-admission decision | none | immutable call evidence |
| Memory use evidence | RunStep/result | Operational Record plus Provenance edges | exact context entry and produced revision/result | none | immutable exposure/use record |

## Write, admission, and consolidation rules

1. **Evidence-first:** durable source evidence is committed before or atomically with scheduling extraction; a failed extractor cannot lose the Run truth.
2. **Settled-input gate:** do not learn a success lesson from an active attempt, outcome-unknown effect, unfinalized Subrun, or model completion claim.
3. **Source-class gate:** explicit author statements, deterministic domain receipts, verified Tool/environment results, model inferences, and third-party text retain distinct source classes. They are not flattened into one confidence number.
4. **Instruction-inert gate:** text originating in files, web pages, research sources, Tool output, or retrieved memory is data. It cannot grant capability or modify system/developer/Skill policy.
5. **Scope gate:** every durable item declares project scope and optional thread, Run, subject, task family, valid time, and expiry. No implicit cross-project memory exists.
6. **Minimum-signal gate:** extraction may produce no item. Generic advice, transient status, duplicated facts, and unsupported assistant proposals should normally be omitted.
7. **Privacy gate:** secrets and credential material are excluded or redacted before any model extraction. Sensitive eligible data remains local unless an Outbound Disclosure grant covers the exact transformation and destination.
8. **Single-writer/conflict gate:** consolidation uses a lease plus expected revision. Concurrent writes conflict; no last-write-wins replacement.
9. **No recursive authority:** consolidation may derive from other summaries only while retaining the full source graph. A derived summary cannot validate itself or raise authority/confidence merely through repetition.
10. **No implicit promotion:** repeated retrieval, high usage, high confidence, age, or author silence cannot transform a Candidate into authoritative truth, binding preference, Skill, policy, or instruction.

## Correction, contradiction, confidence, temporal scope, and forgetting

- A correction is a new authored or derived revision with exact cause; historical text remains addressable unless later tombstoned.
- “Contradicts” and “supersedes for purpose X” are different. Contradictory items may coexist pending adjudication; supersession identifies the intended replacement and scope.
- An explicit author correction should outrank an inferred memory for retrieval, but whether it directly changes a binding project preference or fiction fact belongs to that owning domain's command boundary.
- Confidence is evidence metadata, not authority and not a universal total order. Preserve how it was obtained, by whom, for which claim, and from which sources.
- Record time, source event time, claimed valid time, and story-world time are distinct. The fiction-memory ticket owns story-world and epistemic semantics; this ticket needs only enough temporal scope to avoid applying obsolete lessons as current.
- Retrieval decay may reduce rank after inactivity or expiry. It must not silently alter durable content, provenance, or authority.
- Automatic retention may archive eligible derived memory only under an author-visible project policy. Tombstoning content requires the accepted retention/deletion boundary and must synchronously invalidate projections.
- Relearning after tombstone creates new identity and provenance; it must not silently resurrect deleted payload from embeddings, caches, backups, summaries, or model context.

## Retrieval and ContextManifest boundary

Retrieval has two stages that must remain inspectable:

1. **Candidate retrieval:** indexes find potentially relevant exact durable revisions.
2. **Context admission:** policy decides which candidates may enter one model call.

The second stage should record at least, conceptually:

- request purpose, project/thread/Run/Step, and query or trigger;
- exact selected and excluded revision references;
- source class, scope, retention state, temporal applicability, contradiction/supersession state, and sensitivity;
- rank inputs and deterministic filters, including any author pin or exclude;
- projection generation and covered-source watermark;
- transformation, excerpt, summary, truncation, and byte/token counts;
- why the item was mandatory, pinned, dynamically retrieved, or rejected;
- model/provider destination and Outbound Disclosure evidence when external;
- the final ordered context contribution bound by `ContextManifest` and `Step Snapshot`.

A stale index must never return a tombstoned payload or silently omit an author-pinned mandatory item. If freshness cannot be proven, the system should rebuild, fall back to durable exact lookup, or expose a typed degraded/blocked result according to the later context decision. It must not pretend the projection is current.

## Failure modes and required controls

| Failure mode | Causal path | Required control direction |
|---|---|---|
| Memory poisoning | untrusted content is stored, preferentially retrieved, then treated as instruction | source/trust labels; instruction-inert content; write gate; retrieval screen; no capability inheritance; author quarantine/delete |
| Self-reinforcing hallucination | model inference becomes summary, reflection cites summary, repetition raises rank/confidence | exact source graph; prohibit circular-only support; separate exposure/use/validation; contradiction UI; source-class-aware ranking |
| Stale summary | source changes but consolidated view remains current-looking | exact revision dependencies; freshness projection; conflict marker; regenerate as new revision; inspect source |
| Retrieval feedback loop | frequently retrieved item gains usage rank and crowds out alternatives | usage never changes truth/confidence; cap usage contribution; diversity/subject coverage; log exclusions and outcomes |
| Opaque self-modification | Agent edits always-visible memory, policy, or procedure and thereby changes later behavior | revisioned writes; author-visible diffs; read-only mandatory policy; procedural candidate separate from Skill/policy admission |
| Unbounded growth | every turn produces memory and indexes retain everything | settled/minimum-signal gates; per-Run and per-project quotas; bounded extraction/consolidation; archive policy; no-op |
| Scope leak | thread/user/project facts cross into another project or Subrun | explicit namespaces; attenuated Subrun context; deny implicit inheritance; ContextManifest evidence |
| Privacy leak | sensitive local memory is summarized or sent to an external model/service | source classification; secret filter; minimum disclosure; explicit grant; recorded transformation and destination |
| Lost correction | overwrite or consolidation removes contrary history | immutable revisions; expected heads; explicit contradiction/supersession; no last-write-wins |
| Ghost deletion | payload is deleted but survives in embeddings, cache, summary, or prompt | tombstone fan-out to projections/caches; rebuild; deleted placeholders; backup/retention contract |
| Recovery duplication | crash repeats extraction, consolidation, or deletion | idempotency key; claimed input set; leases/heartbeats; atomic state plus event/outbox; deterministic rebuild |
| False success lesson | model completion or flaky evaluator is stored as verified procedure | finalization gate; exact Tool/domain evidence; evaluator identity; outcome class; candidate-only fallback |

## Questions prepared for downstream HITL

These questions were prepared for one-at-a-time resolution. Their fiction-memory and research-provenance answers now live in the accepted [Fiction Memory and Research Provenance Semantics](../foundation/fiction-memory-and-research-provenance-semantics.md); context, storage, protocol, and Run-retention questions remain with their owning tickets.

1. Is `MemoryCandidate` the single Core entry point for all extracted cross-Run lessons, or should episodic lesson, semantic claim, and procedural-preference candidate be separate Artifact kinds?
2. Is an explicit author procedural preference a versioned authoritative project setting, an author-owned Artifact, or a non-authoritative memory item that is merely mandatory context?
3. Which source events may trigger automatic extraction: every terminal Run, only successful/partial Runs, explicit feedback, accepted/rejected Proposals, Tool failures, or a configurable subset?
4. May automatic extraction call an external model by default, or must it be local/opt-in with a distinct Outbound Disclosure grant and budget?
5. Which candidate classes require author confirmation before ordinary retrieval, and which may be retrievable immediately while clearly labeled as inferred?
6. What is the exact status vocabulary for candidate validation: source-complete, corroborated, contradicted, author-confirmed, rejected, or something smaller?
7. How should multiple contradictory memories be presented and ranked before author adjudication? Is surfacing both mandatory for high-impact contexts?
8. What does supersession mean for a procedural lesson: replacement for one task family, time interval, Tool/Skill version, model/provider, or all future use?
9. Can confidence be stored at all before each class has a defined evidence method, or should early versions expose only source class and validation state?
10. Which time axes are required on cross-Run memory, and which belong exclusively to fiction-domain claims in ticket #51?
11. What are the default project quotas and per-item/context caps for source episodes, derived summaries, and retrieved contributions?
12. Can background consolidation archive low-use derived memory automatically, or may it only recommend an author-visible archival batch?
13. Which deletion actions are reversible archive versus terminal tombstone, and what minimum tombstone/provenance structure must remain for each memory class?
14. Must explicit author deletion prevent future re-extraction from still-retained Run evidence, and if so, what durable suppression record is required without retaining deleted content?
15. How is a stale or unavailable retrieval projection represented at query time: rebuild, exact fallback, degraded result, or hard block for pinned/mandatory context?
16. Which retrieval reasons, scores, exclusions, and transformation details must be visible in the normal author UI versus an advanced audit view?
17. Does retrieval usage affect rank at all? If yes, what prevents a self-reinforcing loop and how is the contribution inspectable?
18. When a memory influences a Proposal or Tool choice, is explicit `supported_by`/`derived_from` attribution required, or is Step Snapshot plus Agent Decision evidence sufficient?
19. May a procedural lesson automatically draft a Skill or project-policy Proposal, and what verification evidence must accompany it?
20. Is any memory allowed above project scope? If personal/global preferences are later desired, which separate authority, privacy, export, and deletion contract owns them?

## Ownership split identified by the research

| Ticket | Owns | Must not decide here |
|---|---|---|
| [#51 Specify Fiction Memory and Research Provenance Semantics](https://github.com/FrankQDWang/StoryOS/issues/51) | fiction-domain meanings of canon and Candidates, story-world time, epistemic scope, research Claims/source evidence, conflict meaning | Agent extraction scheduling, generic ContextManifest ranking, physical storage |
| [#54 Specify Context Assembly, Retrieval, and Outbound Disclosure Semantics](https://github.com/FrankQDWang/StoryOS/issues/54) | mandatory versus dynamic context, query and admission, pin/exclude/inspect, caps, ranking evidence, stale projection behavior, provider disclosure | fiction truth semantics, tables, wire encoding |
| [#56 Specify the Self-Contained Project Storage and Migration Contract](https://github.com/FrankQDWang/StoryOS/issues/56) | physical records/blobs, transactions, indexes, encryption, backup/export, migrations, projection rebuild, deletion execution | semantic authority and retrieval policy |
| [#58 Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58) | versioned DTOs, commands, queries, events, compatibility and protocol errors for accepted semantics | inventing the semantic lifecycle |
| [#64 Specify Run Event, Mailbox, Snapshot, Retention, and Archival Semantics](https://github.com/FrankQDWang/StoryOS/issues/64) | exact Run/Message/snapshot retention, compaction, archival, recovery evidence, mailbox payload lifecycle | fiction-domain memory meaning and general retrieval ranking |
| [#65 this research ticket](https://github.com/FrankQDWang/StoryOS/issues/65) | primary evidence, option comparison, cross-ticket lifecycle risks, and HITL questions | final domain decisions or implementation |

The accepted downstream specification now fixes the #51 semantics and hands the remaining mechanics to #54, #56, #58, and #64. Those tickets must not collapse all five concerns into one “memory service” mega-contract.

## Recommended decision sequence

1. Decide the minimum semantic roles and whether they reuse existing Artifact families.
2. Decide explicit-author preference authority separately from inferred procedural candidates.
3. Decide write eligibility and validation/admission states.
4. Decide correction, contradiction, supersession, and deletion/re-extraction behavior.
5. Hand context selection and ContextManifest details to #54.
6. Hand physical durability, indexes, migration, backup, and deletion fan-out to #56.
7. Hand accepted commands/events/queries to #58 and Run-source retention to #64.
8. Only then design one thin vertical slice and public-boundary integration tests.

## Source ledger

### External and implementation sources

- Sumers et al., [Cognitive Architectures for Language Agents (CoALA)](https://arxiv.org/abs/2309.02427), arXiv:2309.02427; memory roles, read/write actions, procedural-write risk, and open deletion questions.
- Park et al., [Generative Agents: Interactive Simulacra of Human Behavior](https://arxiv.org/abs/2304.03442), arXiv:2304.03442 / UIST 2023; experience stream, retrieval ranking, reflection, ablation, and observed failures.
- Packer et al., [MemGPT: Towards LLMs as Operating Systems](https://arxiv.org/abs/2310.08560), arXiv:2310.08560; tiered context, durable recall, recursive summary, and archival retrieval.
- Letta, [Memory blocks](https://docs.letta.com/guides/core-concepts/memory/memory-blocks) and [Context hierarchy](https://docs.letta.com/guides/core-concepts/memory/context-hierarchy); current official persisted-block and archival-memory behavior, including read-only and concurrency limitations.
- Shinn et al., [Reflexion: Language Agents with Verbal Reinforcement Learning](https://arxiv.org/abs/2303.11366), arXiv:2303.11366; evaluator-conditioned verbal lessons, bounded memory, and limitations.
- Wang et al., [Voyager: An Open-Ended Embodied Agent with Large Language Models](https://arxiv.org/abs/2305.16291), arXiv:2305.16291; executable Skill library, retrieval, execution feedback, and self-verification.
- Chen et al., [AgentPoison: Red-teaming LLM Agents via Poisoning Memory or Knowledge Bases](https://papers.nips.cc/paper_files/paper/2024/file/eb113910e9c3f6242541c1652e30dfd6-Paper-Conference.pdf), NeurIPS 2024; retrieval-memory poisoning attack evidence.
- LangChain, [LangGraph memory overview](https://docs.langchain.com/oss/python/concepts/memory) and [memory guide](https://docs.langchain.com/oss/python/langgraph/add-memory); official thread/cross-thread scope and hot-path/background-write implementation choices.
- W3C, [PROV-DM](https://www.w3.org/TR/2013/REC-prov-dm-20130430/), Recommendation 30 April 2013; provenance entities, activities, agents, derivation, and the boundary between lineage and trust judgment.
- OpenAI Codex pinned reference: [memory README](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/README.md), [Phase 1](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/write/src/phase1.rs), [Phase 2](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/write/src/phase2.rs), [extraction prompt](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/write/templates/memories/stage_one_system.md), and [citation parser](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/memories/read/src/citations.rs); implementation evidence only.

### StoryOS-local sources

- [CONTEXT.md](../../CONTEXT.md): current ubiquitous language and accepted Run, context, Skill, Artifact, provenance, retention, disclosure, Proposal, and authority boundaries.
- [ADR 0001](../adr/0001-separate-authoritative-state-artifacts-and-operational-records.md): separation of Authoritative State, Artifacts, and Operational Records.
- [Artifact and Authoritative-State Domain Model](../foundation/artifact-domain-model.md): exact revisions, provenance, Research Artifacts, Candidates, Derivation, Supersession, retention, Proposal, validation, and Acceptance.
- [Manuscript Revision and Proposal State Machine](../foundation/manuscript-revision-proposal-state-machine.md): linear authoritative revisions, conflicts, Core transitions, and recovery-safe application.
- Closed issues [#5](https://github.com/FrankQDWang/StoryOS/issues/5), [#15](https://github.com/FrankQDWang/StoryOS/issues/15), and [#16](https://github.com/FrankQDWang/StoryOS/issues/16): persistent project Agent, hybrid context, and no hidden durable memory.
- Open issues [#51](https://github.com/FrankQDWang/StoryOS/issues/51), [#54](https://github.com/FrankQDWang/StoryOS/issues/54), [#56](https://github.com/FrankQDWang/StoryOS/issues/56), [#58](https://github.com/FrankQDWang/StoryOS/issues/58), [#64](https://github.com/FrankQDWang/StoryOS/issues/64), and [#65](https://github.com/FrankQDWang/StoryOS/issues/65): current decision frontier and ownership boundaries.

## Final research conclusion

The evidence supports durable Agent continuity, but not a hidden or self-authorizing memory store. The safest StoryOS direction is a typed, source-first lifecycle in which exact Run and project evidence remains the source of record; extracted and consolidated memory remains revisioned, provenance-bearing, non-authoritative content; retrieval indexes remain disposable projections; every model-context use is separately admitted and recorded; and authors can inspect, correct, exclude, archive, and delete memory without losing the audit boundary.

The unresolved design work is not whether StoryOS needs memory. Closed decisions already require it. The remaining question is which minimum semantic roles become Core concepts, which existing owners they reuse, and which author decisions are required before an inferred lesson may shape future work. Those answers belong in the subsequent HITL specification, not in this research report.
