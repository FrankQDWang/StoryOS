# StoryOS repository instructions

## Scope and authority

- This file applies to the whole repository. A nearer nested `AGENTS.md` may add or narrow instructions for its subtree.
- Repository files and tracked design artifacts are the source of truth. Conversation history and Codex memory are supporting context only.
- One task has one execution owner. Subagents may investigate or review, but they do not independently mutate the same deliverable or authoritative state.
- Preserve unrelated user changes. Do not rewrite, discard, or clean them up as part of another task.

## Product invariants

- StoryOS has one general, novel-project-scoped Agent Loop. Task-specific behavior comes from Tools, MCP servers, Services, Skills, and policy, not separate fixed workflow runtimes.
- The author owns every authoritative creative state. Prose, canon, characters, timeline, outline, structure, and author plans change only through inspectable proposals and explicit domain commands.
- Discovery is not authorization. Tools and extensions receive only the capabilities, context, budget, and outbound-data access explicitly granted to the current run.
- Local project data is authoritative. Any external model, service, or MCP server receives only the minimum context required for the approved step, with provenance and disclosure recorded.
- Agent runs, plans, tool calls, approvals, artifacts, and state transitions are durable and inspectable. A network connection or model process is never the source of truth.
- Transcript-native MCP Apps are sandboxed views/controllers over StoryOS-owned typed artifacts. They never become the authoritative data store.
- Prose proposals, editable diffs, accept/reject operations, and proposal conflict handling belong in the editor, not in an MCP App.

## Reference source policy

- `.reference/codex` is a read-only, commit-pinned upstream reference. Do not edit it except when a task explicitly updates the submodule pin.
- `.reference/**` must not enter the StoryOS Cargo workspace, dependency graph, build, test, package, release, or product runtime.
- Learn from upstream patterns, but independently design StoryOS around its domain. Do not fork, embed, or wrap the Codex runtime.
- Before copying upstream implementation code, verify architectural fit, isolate the copied unit, review its license obligations, and record provenance. Copying a design idea does not make upstream a production dependency.
- The Rust guidance below is self-contained in StoryOS. It was selected or adapted from `.reference/codex/AGENTS.md` at commit `1f0566d3f59298d1bb88820a0d35294f1eeb07ea` (Apache-2.0); agents must not rely on opening the reference copy to discover these rules.

## Rust engineering rules

### API design

- When using `format!` and variables can be inlined into `{}`, inline them.
- Collapse collapsible `if` statements and prefer method references over redundant closures.
- When possible, make `match` statements exhaustive and avoid wildcard arms.
- Avoid `bool` or ambiguous `Option` parameters that produce opaque call sites such as `foo(false)` or `bar(None)`. Prefer enums, newtypes, named methods, or other self-documenting API shapes.
- Newly added traits must have doc comments explaining their role and the contract expected of implementations.
- Discourage both `#[async_trait]` and `#[allow(async_fn_in_trait)]` in traits. Prefer native RPITIT methods with an explicit `Send` bound, for example `fn run(&self, ...) -> impl Future<Output = T> + Send`; implementations may use `async fn` when they satisfy that contract.
- Keep crate API surfaces as small as possible. Prefer private modules and explicitly export only the intended public API.

### Modules and observability

- Put a new concept in the crate that owns it. Do not grow a central Agent crate merely because it is convenient; introduce a focused crate when that creates a clearer dependency boundary.
- Avoid large modules. Target production Rust modules below roughly 500 lines, excluding tests. When a file approaches 800 lines, add functionality in a new module unless a documented reason makes that worse.
- When extracting a module, move the related tests, type documentation, and invariants with the implementation.
- Do not create a small helper referenced only once unless it names a meaningful domain boundary or materially improves clarity.
- Instrument async work at the function or method definition with `#[tracing::instrument(...)]`; first check that the callee or immediate delegate is not already instrumented.

### Model-visible context

- Build model-visible history incrementally; do not silently rewrite prior history.
- Every injected context fragment must be structured, attributable, inspectable, and subject to a hard size cap.
- No single injected item may exceed 10K tokens. A new item that can exceed 1K tokens requires explicit design review.
- Context assembly must remain bounded even when the project, transcript, or artifact store grows without bound.

### Tests and change review

- Changes to Agent Loop behavior, tool execution, authorization, recovery, or other user-visible Agent semantics require integration tests at the public boundary.
- Prefer equality comparisons of entire values over field-by-field assertions.
- Do not add tests for statically defined values, or negative tests for behavior that has been removed.
- Put a newly introduced test module in a descriptive sibling `*_tests.rs` file instead of embedding a large inline test module.
- Avoid mutating process environment in tests; pass environment-derived settings or dependencies explicitly.
- Treat changes to ToolSpec, MCP adapters, Skill manifests, Artifact and Run events, external APIs, configuration, persisted data, or recovery formats as contract changes and review their breaking and migration impact explicitly.
- Keep non-mechanical changes below roughly 800 changed lines and complex logic changes below roughly 500 when practical. Split larger work into the smallest coherent, reviewable stages.

### Verification

- Use StoryOS-owned repository commands for formatting, linting, tests, schema generation, and verification. Do not copy Codex-specific Bazel or `just` commands unless StoryOS actually adopts them.
- Once the Rust workspace is scaffolded, its checked-in task runner must provide targeted checks and one final non-mutating verification command. Run the relevant targeted checks after changes and the final verification command before declaring completion.
- When adding a nested `AGENTS.md`, include only subtree-specific boundaries and commands; do not duplicate this root file.

## Agent skills

### Issue tracker

StoryOS issues and Wayfinder maps live in GitHub Issues for `FrankQDWang/StoryOS`. See `docs/agents/issue-tracker.md`.

### Domain docs

StoryOS uses a single-context domain glossary at `CONTEXT.md` and architecture decisions under `docs/adr/`. See `docs/agents/domain.md`.
