# Codex skill management at the pinned StoryOS reference

- Status: research complete; decision input, not a StoryOS design decision
- Reference baseline: `openai/codex@1f0566d3f59298d1bb88820a0d35294f1eeb07ea`
- Scope: the read-only `.reference/codex` source tree only; no web or secondary sources

## Executive answer

The user's understanding is correct, with one important qualification:

1. **A user can explicitly invoke multiple Skills in one turn.** Codex collects a vector of structured Skill selections and `$skill-name`/linked mentions, deduplicates them, reads every selected main `SKILL.md`, and injects every successfully read body as a separate `<skill>...</skill>` user-context fragment. The model-visible usage policy says, verbatim in the source, that multiple mentions mean all mentioned Skills must be used.
2. **The agent can decide to load another Skill while executing a turn.** Codex initially shows the model a bounded catalog containing Skill names, descriptions, and locators. The prompt tells the model to choose a Skill when the task matches its description and then read its full instructions. Filesystem-backed Skills are read through ordinary filesystem/shell tools. Orchestrator-owned Skills have model-callable `skills.list` and `skills.read` tools. This is progressive disclosure driven by the model inside the general tool loop; it is not a host-created “active Skill binding.”
3. **Codex has no Primary/Supporting Skill distinction and no one-Skill exclusivity.** It stores selected Skills in an ordinary vector. There is no rank, role, binding, or active-Skill field in the Skill metadata, catalog, turn state, or injection records.
4. **Codex does not resolve semantic conflicts between Skill bodies.** It resolves identity and routing ambiguity—paths, namespaces, enabled state, provider authority, and some duplicate-name cases—but it does not compare instructions, select a winner, merge them, or enforce a Primary Skill. Multiple injected Skill bodies have the same `user` role. Normal system/developer/user message priority still applies outside the Skill subsystem.
5. **Skill availability is snapshotted per turn; explicit use is also turn-scoped.** A new `TurnContext` receives a `HostSkillsSnapshot`, while the usage prompt says not to carry Skills across turns unless re-mentioned. There is no per-run binding and no per-model-sampling-step Skill selection record. Executor Skill availability can be projected per step, and orchestrator catalogs/resources are cached at thread scope, but neither mechanism creates an active binding.

The architectural lesson is therefore not “one Primary Skill plus helpers.” Codex's proven model is **a set of independently attributable instruction packages, progressively disclosed as needed, with host-owned discovery and routing but model-owned semantic composition**.

## Evidence labels

This note separates three different kinds of behavior:

- **Host-enforced** — Rust code performs or rejects the behavior regardless of model cooperation.
- **Prompt policy** — Codex tells the model how to behave; the host does not independently prove that the model followed the instruction.
- **Inference** — a conclusion from the absence or composition of the inspected types and paths, explicitly identified as such.

## 1. Discovery and representation

### 1.1 Host filesystem Skills

`SkillsService` owns host discovery, immutable snapshots, caching, invalidation, and extra roots ([`SkillsService`](../../.reference/codex/codex-rs/core-skills/src/service.rs#L65-L74)). For each turn, session setup resolves plugins, constructs `SkillsLoadInput`, asks `SkillsService` for a snapshot, and installs that snapshot into `TurnContext` ([turn construction](../../.reference/codex/codex-rs/core/src/session/turn_context.rs#L730-L778)).

The loader builds roots from:

- config-layer `skills` directories;
- the deprecated `$CODEX_HOME/skills` location;
- `$HOME/.agents/skills`;
- embedded system Skills cached under `$CODEX_HOME/skills/.system`;
- admin/system configuration roots;
- plugin-provided and explicitly added roots; and
- `.agents/skills` directories from the project root down to the current working directory.

These are host-enforced discovery rules ([root assembly](../../.reference/codex/codex-rs/core-skills/src/loader.rs#L242-L289), [config-layer roots](../../.reference/codex/codex-rs/core-skills/src/loader.rs#L292-L375), [project-to-cwd roots](../../.reference/codex/codex-rs/core-skills/src/loader.rs#L378-L412)). Discovery is bounded by scan depth, directory count, entry count, and concurrent load count ([discovery limits](../../.reference/codex/codex-rs/core-skills/src/loader/discovery.rs#L16-L17), [`discover_skills`](../../.reference/codex/codex-rs/core-skills/src/loader/discovery.rs#L52-L80)).

Each discovered `SKILL.md` supplies YAML-frontmatter name and description. Optional `agents/openai.yaml` metadata adds interface fields, tool dependencies, and policy such as `allow_implicit_invocation` ([frontmatter and metadata shapes](../../.reference/codex/codex-rs/core-skills/src/loader.rs#L55-L121), [`parse_skill_file`](../../.reference/codex/codex-rs/core-skills/src/loader.rs#L628-L674), [`load_skill_metadata`](../../.reference/codex/codex-rs/core-skills/src/loader.rs#L738-L807)). The resulting `SkillMetadata` contains name, descriptions, interface, dependencies, policy, `SKILL.md` path, scope, and optional plugin ID ([`SkillMetadata`](../../.reference/codex/codex-rs/core-skills/src/model.rs#L14-L25)). It contains no version, digest, rank, Primary/Supporting role, or active-binding identifier.

Plugin namespaces qualify names as `namespace:skill` ([`ResolvedSkillNamespace::qualify`](../../.reference/codex/codex-rs/core-skills/src/loader/namespace.rs#L166-L180)). Root merging deduplicates identical canonical `SKILL.md` paths and sorts Skills by scope, name, and path, but deliberately does not collapse all equal names into one Skill ([`merge_skill_root_snapshots`](../../.reference/codex/codex-rs/core-skills/src/root_loader.rs#L93-L157)). Configuration rules can enable or disable Skills by path or name ([`resolve_disabled_skill_paths`](../../.reference/codex/codex-rs/core-skills/src/config_rules.rs#L71-L103)). These mechanisms decide availability and display order, not semantic precedence between instructions.

### 1.2 Executor and orchestrator Skills

The newer Skills extension generalizes representation to a `SkillCatalogEntry`: opaque package ID, source authority, name, description, main resource, optional display path/dependencies, and enabled/prompt-visible flags ([catalog types](../../.reference/codex/codex-rs/ext/skills/src/catalog.rs#L40-L120)). Its source kinds are Host, Executor, Orchestrator, or Custom ([`SkillSourceKind`](../../.reference/codex/codex-rs/ext/skills/src/catalog.rs#L4-L17)). Provider dispatch is based on source authority; duplicate catalog entries are suppressed only when both authority and package ID match ([provider routing](../../.reference/codex/codex-rs/ext/skills/src/sources.rs#L143-L183), [`SkillCatalog::push_entry`](../../.reference/codex/codex-rs/ext/skills/src/catalog.rs#L176-L201)).

Orchestrator Skills are discovered as bounded MCP resources with MIME type `mcp/skill`; their package and resource IDs are opaque `skill://` URIs ([orchestrator provider limits and discovery](../../.reference/codex/codex-rs/ext/skills/src/provider/orchestrator.rs#L24-L35), [resource-to-catalog mapping](../../.reference/codex/codex-rs/ext/skills/src/provider/orchestrator.rs#L222-L247)). Executor catalogs are projected from selected capability roots into step world state. The extension caches the first executor catalog for a selected root and caches orchestrator catalog/resources at thread scope ([`SkillsThreadState`](../../.reference/codex/codex-rs/ext/skills/src/state.rs#L28-L68), [executor/orchestrator snapshots](../../.reference/codex/codex-rs/ext/skills/src/state.rs#L75-L139)). Again, these are availability and read-routing snapshots, not active Skill assignments.

## 2. Catalog injection versus full Skill loading

Codex uses progressive disclosure.

At context construction, it renders enabled, implicitly routable Skill metadata into a bounded **developer** section. The default budget is 2% of the model context window, falling back to an 8,000-character budget, and descriptions may be truncated or omitted while retaining as many entries as possible ([render budgets](../../.reference/codex/codex-rs/core-skills/src/render.rs#L15-L24), [`default_skill_metadata_budget`](../../.reference/codex/codex-rs/core-skills/src/render.rs#L144-L160), [`build_available_skills`](../../.reference/codex/codex-rs/core-skills/src/render.rs#L162-L198)). Core adds this catalog to the initial developer bundle ([initial context assembly](../../.reference/codex/codex-rs/core/src/session/mod.rs#L3301-L3325)). The extension likewise renders its available-Skills fragment as developer context ([fragment roles](../../.reference/codex/codex-rs/ext/skills/src/fragments.rs#L7-L40)).

The full `SKILL.md` is not included for every catalog entry. Full bodies enter model context in two ways:

1. **Host pre-injection for explicit user mentions.** Before the first model sample for the turn, Codex resolves every explicit selection, reads each selected main prompt, and converts each one to a `<skill>` fragment with role `user` ([turn preparation](../../.reference/codex/codex-rs/core/src/session/turn.rs#L579-L624), [`SkillInstructions`](../../.reference/codex/codex-rs/core-skills/src/skill_instructions.rs#L6-L35)). The extension performs the equivalent operation across provider-owned catalogs ([extension selection and reads](../../.reference/codex/codex-rs/ext/skills/src/extension.rs#L243-L299)).
2. **Agent-directed reads during execution.** Once the model sees the catalog, it can use an ordinary filesystem/tool call to read a filesystem-backed `SKILL.md`, or call the namespaced `skills.list`/`skills.read` tools for orchestrator-owned resources. The latter tools are registered by the extension ([tool registration](../../.reference/codex/codex-rs/ext/skills/src/tools/mod.rs#L33-L52)); `skills.read` validates that the requested package is currently enabled and available under the requested authority before routing the read ([`ReadTool::handle`](../../.reference/codex/codex-rs/ext/skills/src/tools/read.rs#L46-L110)). At this pin, the namespaced tools support only orchestrator authority ([`SkillToolAuthority`](../../.reference/codex/codex-rs/ext/skills/src/tools/mod.rs#L84-L107)).

The second path proves that additional Skill resources can be loaded after the turn has started. It does **not** prove that Codex creates a new host-side active-Skill set after each read. The content simply returns through the normal tool-result/context path, and the Skill subsystem has no activation operation.

## 3. Explicit invocation: multiple Skills are supported

This is host-enforced, not merely suggested by the prompt.

`collect_explicit_skill_mentions` returns `Vec<SkillMetadata>`. It first resolves all structured `UserInput::Skill` selections by exact enabled path, then scans all text inputs for `$name` and linked mentions, deduplicating selected names/paths ([collector contract and implementation](../../.reference/codex/codex-rs/core-skills/src/injection.rs#L138-L205)). `build_skill_injections` loops over the complete vector and emits one `SkillInjection` per successfully read Skill ([`build_skill_injections`](../../.reference/codex/codex-rs/core-skills/src/injection.rs#L58-L116)). The unit test `collect_explicit_skill_mentions_prioritizes_structured_inputs` passes one structured Skill and one text-mentioned Skill and expects both in the result ([multiple-selection test](../../.reference/codex/codex-rs/core-skills/src/injection_tests.rs#L150-L170)).

Selection order is mechanical, not a priority model:

- structured inputs are collected before text mentions;
- legacy text resolution preserves the loaded catalog's order rather than mention order;
- repeated path/name selections are deduplicated.

The extension also stores `selected_entries` as a vector and reads each entry in a loop ([`SkillsTurnState`](../../.reference/codex/codex-rs/ext/skills/src/state.rs#L259-L265), [extension injection loop](../../.reference/codex/codex-rs/ext/skills/src/extension.rs#L259-L299)). No selected entry is tagged Primary or Supporting.

The model-visible policy reinforces the same behavior: if the user names a Skill or the task matches its description, the model must use it for that turn; **multiple mentions mean use them all** ([absolute-path usage policy](../../.reference/codex/codex-rs/core-skills/src/render.rs#L25-L42), [alias-path usage policy](../../.reference/codex/codex-rs/core-skills/src/render.rs#L44-L61)). That sentence is prompt policy, while the vector collection and multi-body injection above are host behavior.

## 4. Agent-decided invocation during execution

Codex's model-visible policy explicitly authorizes and requires model-side selection: a Skill must be used not only when the user names it, but also when “the task clearly matches” its catalog description. It then instructs the agent to read the selected `SKILL.md` completely before taking task actions and to open only relevant referenced resources ([usage policy](../../.reference/codex/codex-rs/core-skills/src/render.rs#L25-L42)). This is **prompt policy**. The Rust host exposes the catalog and tools that make it possible, but it does not independently run a semantic task-to-Skill classifier.

For orchestrator-owned Skills, model-directed loading is a first-class tool path:

- `skills.list` returns enabled Skills plus the opaque package and main-resource handles needed by `skills.read` ([`ListTool`](../../.reference/codex/codex-rs/ext/skills/src/tools/list.rs#L51-L101));
- `skills.read` reads one complete resource and verifies authority/package availability first ([`ReadTool`](../../.reference/codex/codex-rs/ext/skills/src/tools/read.rs#L46-L110)).

For filesystem-backed Skills, the model follows the locator and reads via the owning filesystem/tool. Codex's code sometimes calls this an **implicit Skill invocation**, but that term must not be confused with automatic loading. `detect_implicit_skill_invocation_for_command` only recognizes shell commands that read a known `SKILL.md` or execute a script under a known Skill's `scripts` directory ([detector](../../.reference/codex/codex-rs/core-skills/src/invocation_utils.rs#L31-L44), [script/read matching](../../.reference/codex/codex-rs/core-skills/src/invocation_utils.rs#L83-L115)). `maybe_emit_implicit_skill_invocation` then deduplicates a telemetry event and reports analytics ([telemetry function](../../.reference/codex/codex-rs/core/src/skills.rs#L43-L101)). It does **not** read the Skill body, inject context, activate a Skill, change the available set, or rebind the turn.

Therefore:

- “Can the agent decide to load another Skill later in the same turn?” — **Yes.** The catalog plus filesystem or `skills.read` enables it.
- “Does the host automatically re-run Skill routing on every model/tool step?” — **No evidence of that.** The semantic match is model behavior, while the host records a turn snapshot and dispatches reads.
- “Does reading another Skill create a durable active-Skill binding?” — **No.** No such state transition or type exists in the inspected subsystem.

## 5. No Primary/Supporting hierarchy, exclusivity, or semantic conflict resolver

### Proven absence from the operative types and paths

The operative representations are:

- `SkillMetadata` for host Skills;
- `SkillCatalogEntry` for authority-aware provider Skills;
- `SkillInjection`/`SkillInstructions` for full prompt content; and
- `SkillsTurnState { catalog, selected_entries, warnings, main_prompts_injected }`.

None contains a Primary/Supporting role, weight, precedence, exclusivity group, compatibility declaration, selected-by reason, activation interval, or conflict outcome ([host model](../../.reference/codex/codex-rs/core-skills/src/model.rs#L14-L25), [catalog entry](../../.reference/codex/codex-rs/ext/skills/src/catalog.rs#L107-L141), [injection record](../../.reference/codex/codex-rs/core-skills/src/injection.rs#L19-L30), [turn state](../../.reference/codex/codex-rs/ext/skills/src/state.rs#L259-L265)). The injection loops process every selection rather than selecting one winner.

This is strong evidence that the pinned implementation has no Primary/Supporting or exclusive binding model. It remains an **inference from the complete operative Skill paths inspected**, not a claim that no unrelated string anywhere in the repository uses those English words.

### What Codex does resolve

Codex resolves operational ambiguity, not instruction semantics:

- plugin namespaces qualify names;
- exact linked paths identify a Skill even when names collide;
- disabled Skills are not selected;
- legacy plain-name selection requires one enabled Skill of that name and no connector slug conflict, while an explicit path still works ([plain-name checks](../../.reference/codex/codex-rs/core-skills/src/injection.rs#L355-L430));
- provider reads are routed through exact authority/package/resource identities; and
- identical authority/package entries and repeated selections are deduplicated.

There is a transitional difference worth noting: the extension collector's plain-name path takes the first enabled equal-name catalog entry, whereas the legacy host collector rejects an ambiguous plain name ([extension collector](../../.reference/codex/codex-rs/ext/skills/src/selection.rs#L45-L75)). Neither behavior is a semantic precedence rule.

### What Codex does not resolve

Codex does not inspect two selected bodies for contradictory instructions, rank them, ask the user to choose, or merge them into a normalized plan. Each full Skill body is rendered as a `user` contextual fragment ([core fragment role](../../.reference/codex/codex-rs/core-skills/src/skill_instructions.rs#L22-L35), [extension fragment role](../../.reference/codex/codex-rs/ext/skills/src/fragments.rs#L43-L68)). Thus:

- system/developer instructions retain their normal higher message-role authority;
- two selected Skill bodies are peers at the same role;
- any reconciliation between their semantic instructions is left to the model and the general instruction hierarchy, not a Skill-specific host algorithm.

## 6. Turn, step, thread, and “run” scope

`TurnSkillsContext` contains one immutable `HostSkillsSnapshot` plus a per-turn set used only to suppress duplicate implicit-invocation telemetry ([`TurnSkillsContext`](../../.reference/codex/codex-rs/core/src/session/turn_context.rs#L26-L39)). A new turn constructs a fresh context from the effective per-turn configuration ([turn snapshot construction](../../.reference/codex/codex-rs/core/src/session/turn_context.rs#L730-L778)). Explicit selections are derived from that turn's `UserInput` and stored in extension `SkillsTurnState`.

The prompt-level rule is explicit: “Do not carry skills across turns unless re-mentioned” ([usage policy](../../.reference/codex/codex-rs/core-skills/src/render.rs#L25-L42)). That is a model instruction, not a host deletion of historical context. The source here proves that there is no active selection carried forward by the Skill subsystem; it does not prove that prior `<skill>` messages are physically removed from every possible conversation-history representation.

Executor Skill **availability** is placed in `ExecutorSkillsStepState`, which is a step-scoped catalog snapshot ([world-state contribution](../../.reference/codex/codex-rs/ext/skills/src/extension.rs#L143-L179), [step state type](../../.reference/codex/codex-rs/ext/skills/src/state.rs#L267-L268)). That is not a per-step choice of active Skills: explicit selection remains in turn state, and later agent reads are ordinary tool calls/context additions.

The inspected subsystem has no StoryOS-style `AgentRun` entity and no per-run Skill binding. Its actual scopes are:

| Scope | Codex Skill state |
| --- | --- |
| Thread | Extension configuration and orchestrator/executor caches |
| Turn | Host Skill snapshot, explicit selected-entry vector, injected main prompts, telemetry dedupe set |
| Step | Executor Skill availability catalog projected from ready capability roots |
| Tool call | A filesystem read/script execution or `skills.list`/`skills.read` call; no activation record |

## 7. Host behavior versus instruction convention

| Behavior | Classification | Evidence |
| --- | --- | --- |
| Discover and parse Skill packages from bounded roots | Host-enforced | `SkillsService`, loader, provider catalogs |
| Expose a bounded metadata catalog | Host-enforced | context rendering and extension contributions |
| Resolve structured/path mentions and inject every selected main prompt | Host-enforced | collectors and injection loops |
| Use every explicitly mentioned Skill | Both | host injects all; prompt also tells model to use all |
| Select a Skill because the task matches its description | Prompt policy | `SKILLS_HOW_TO_USE_*`; no semantic classifier found |
| Read another Skill during execution | Host-enabled, model-decided | ordinary filesystem tools or `skills.read` |
| Treat an agent file/script access as “implicit invocation” | Telemetry only | detector plus analytics emitter |
| Keep Skills turn-scoped and do not carry them automatically | Prompt policy plus turn-scoped selection state | usage prompt and `TurnSkillsContext` |
| Assign Primary/Supporting roles or enforce one active Skill | Not implemented | no field or selection transition in operative types/paths |
| Resolve contradictory Skill instructions | Not implemented | peer user fragments; no conflict algorithm |

## 8. Direct answer for the StoryOS decision

The pinned Codex source does **not** support using “Codex does it” as evidence for a mandatory `Primary Skill + Supporting Skills` contract. Codex instead supports:

- zero, one, or many Skills in the same turn;
- explicit user-selected Skills injected together;
- additional model-selected Skills loaded progressively during execution;
- per-turn availability/selection snapshots rather than per-run binding; and
- no host-level semantic precedence among selected Skills.

If StoryOS introduces Primary/Supporting roles, that would be an independent StoryOS design for explainability or conflict handling, not an adaptation of Codex's current Skill model. The closest faithful adaptation is a **set of Skill package references actually consulted by each RunStep**, with source/version/digest and selection provenance recorded, while allowing the set to grow when the agent encounters a newly relevant Skill. Whether StoryOS additionally needs an explicit lead Skill remains a product/domain decision that this source does not settle.
