# StoryOS repository instructions

## Scope and authority

- This file applies to the whole repository. A nearer nested `AGENTS.md` may add or narrow instructions for its subtree.
- Repository files and tracked design artifacts are the source of truth. Conversation history and Codex memory are supporting context only.
- One task has one execution owner. Subagents may investigate or review, but they do not independently mutate the same deliverable or authoritative state.
- Preserve unrelated user changes. Do not rewrite, discard, or clean them up as part of another task.

## Product invariants

- StoryOS is designed for discovery writing, drawing on Dean Koontz's page-by-page method: the author discovers and advances the novel by refining the current passage and making present creative choices, and the Agent assists the work currently placed before it.
- StoryOS has one general, novel-project-scoped Agent Loop. Task-specific behavior comes from Tools, MCP servers, Services, Skills, and policy, not separate fixed workflow runtimes.
- The author owns every authoritative creative state. Narrow deterministic direct author manipulation changes it through explicit domain commands; Agent-, Tool-, MCP-, extension-produced, bulk, cross-location, or not-fully-previsible changes require an inspectable StoryOS Core Proposal accepted by the author.
- Discovery is not authorization. Tools and extensions receive only the capabilities, context, budget, and outbound-data access explicitly granted to the current run.
- StoryOS-controlled project data is authoritative regardless of whether the StoryOS Server and PostgreSQL run locally or in a later controlled cloud deployment. Every project-scoped record and operation binds one exact owning User and Project; external model, embedding, service, Tool, or MCP destinations receive only the minimum context required for the approved step, with provenance and disclosure recorded.
- Agent runs, plans, tool calls, approvals, artifacts, and state transitions are durable and inspectable. A network connection or model process is never the source of truth.
- Transcript-native MCP Apps are sandboxed views/controllers over StoryOS-owned typed artifacts. They never become the authoritative data store.
- Prose proposals, editable diffs, accept/reject operations, and proposal conflict handling belong in the editor, not in an MCP App.

## Reference source policy

- `.reference/` is local-only, Git-ignored reference material. Do not add any entry under it to Git, and do not edit an individual reference except when a task explicitly refreshes that reference.
- `.reference/**` must not enter the StoryOS Cargo workspace, dependency graph, build, test, package, release, or product runtime.
- Learn from upstream patterns, but independently design StoryOS around its domain. Do not fork, embed, or wrap the Codex runtime.
- Before copying upstream implementation code, verify architectural fit, isolate the copied unit, review its license obligations, and record provenance. Copying a design idea does not make upstream a production dependency.
- The Rust guidance below is self-contained in StoryOS. Verbatim selections come from `.reference/codex/AGENTS.md` at commit `c9ef7eff005c3299a5a5f0004c34c6a3eedf2564` and `.reference/grok-build/rustfmt.toml` at commit `a881e6703f46b01d8c7d4a5437683546df30449d`; both references are Apache-2.0. Agents must not rely on opening the reference copies to discover these rules.

## Rust engineering rules

### Formatting and call sites

```toml
use_field_init_shorthand = true
```

- When using format! and you can inline variables into {}, always do that.
- Always collapse if statements per https://rust-lang.github.io/rust-clippy/master/index.html#collapsible_if
- Always inline format! args when possible per https://rust-lang.github.io/rust-clippy/master/index.html#uninlined_format_args
- Use method references over closures when possible per https://rust-lang.github.io/rust-clippy/master/index.html#redundant_closure_for_method_calls
- Avoid bool or ambiguous `Option` parameters that force callers to write hard-to-read code such as `foo(false)` or `bar(None)`. Prefer enums, named methods, newtypes, or other idiomatic Rust API shapes when they keep the callsite self-documenting.
- When you cannot make that API change and still need a small positional-literal callsite in Rust, follow the `argument_comment_lint` convention:
  - Use an exact `/*param_name*/` comment before opaque literal arguments such as `None`, booleans, and numeric literals when passing them by position.
  - A method's sole non-self argument is exempt when the method and parameter names match, such as `.enabled(false)` for `fn enabled(&self, enabled: bool)`.
  - Do not add these comments for string or char literals unless the comment adds real clarity; those literals are intentionally exempt from the lint.
  - The parameter name in the comment must exactly match the callee signature.

### API design

- When possible, make `match` statements exhaustive and avoid wildcard arms.
- Newly added traits should include doc comments that explain their role and how implementations are expected to use them.
- Discourage both `#[async_trait]` and `#[allow(async_fn_in_trait)]` in Rust traits.
  - Preferred trait shape:
    `fn foo(&self, ...) -> impl std::future::Future<Output = T> + Send;`
  - Implementations may still use `async fn foo(&self, ...) -> T` when they satisfy that contract.
  - Do not use `#[allow(async_fn_in_trait)]` as a shortcut around spelling the future contract explicitly.
- Prefer private modules and explicitly exported public crate API.
- Keep crate API surfaces as small as possible. Avoid proliferating test-only helpers.

### StoryOS architecture

- Put a new concept in the crate that owns it. Do not grow a central Agent crate merely because it is convenient; introduce a focused crate when that creates a clearer dependency boundary.

### Modules and observability

- Do not create small helper methods that are referenced only once.
- For tracing async work, instrument the function or method definition with
  `#[tracing::instrument(...)]` instead of attaching spans to futures with
  `.instrument(...)` at call sites. Before adding instrumentation, check whether the callee—or
  the implementation method it immediately delegates to—is already instrumented.
- Avoid large modules:
  - Prefer adding new modules instead of growing existing ones.
  - Target Rust modules under 500 LoC, excluding tests.
  - If a file exceeds roughly 800 LoC, add new functionality in a new module instead of extending
    the existing file unless there is a strong documented reason not to.
  - When extracting code from a large module, move the related tests and module/type docs toward
    the new implementation so the invariants stay close to the code that owns them.

### Model-visible context

- No history rewrite - the context must be built up incrementally.
- Avoid frequent changes to context that cause cache misses.
- No unbounded items - everything injected in the model context must have a bounded size and a hard cap.
- No items larger than 10K tokens.
- Highlight new individual items that can cross >1k tokens as P0. These need an additional manual review.
- Every injected context fragment must be structured, attributable, and inspectable.
- Context assembly must remain bounded even when the project, transcript, or artifact store grows without bound.

### Tests and change review

- Changes to Agent Loop behavior, tool execution, authorization, recovery, or other user-visible Agent semantics require integration tests at the public boundary.
- When writing tests, prefer comparing the equality of entire objects over fields one by one.
- Do not add tests for values that are statically defined.
- Do not add negative tests for logic that was removed.
- When adding a new test module, define its contents in a separate sibling file rather than inline in the implementation file.
- Use an explicit `#[path = "..._tests.rs"]` attribute so the test filename is descriptive and easy to locate:

  ```rust
  #[cfg(test)]
  #[path = "parser_tests.rs"]
  mod tests;
  ```

- This applies only when introducing a new test module. Do not move or rewrite existing inline `#[cfg(test)] mod tests { ... }` modules solely to follow this convention.
- Avoid test-only functions in the main implementation.
- Check whether there are existing helpers to make tests more streamlined and readable.
- Avoid mutating process environment in tests; prefer passing environment-derived flags or dependencies from above.
- Treat changes to ToolSpec, MCP adapters, Skill manifests, Artifact and Run events, external APIs, configuration, persisted data, or recovery formats as contract changes and review their breaking and migration impact explicitly.
- Unless the change is mechanical the total number of changed lines should not exceed 800 lines.
- For complex logic changes the size should be under 500 lines.
- If the change is larger, explore whether it can be split into reviewable stages and identify the smallest coherent stage to land first.
- Base the staging suggestion on the actual diff, dependencies, and affected call sites.

### Verification

- Use StoryOS-owned repository commands for formatting, linting, tests, schema generation, and verification. Do not copy Codex-specific Bazel or `just` commands unless StoryOS actually adopts them.
- Once the Rust workspace is scaffolded, its checked-in task runner must provide targeted checks and one final non-mutating verification command. Run the relevant targeted checks after changes and the final verification command before declaring completion.
- When adding a nested `AGENTS.md`, include only subtree-specific boundaries and commands; do not duplicate this root file.

## Agent skills

### Issue tracker

StoryOS issues and Wayfinder maps live in GitHub Issues for `FrankQDWang/StoryOS`. See `docs/agents/issue-tracker.md`.

### Domain docs

StoryOS uses a single-context domain glossary at `CONTEXT.md` and architecture decisions under `docs/adr/`. See `docs/agents/domain.md`.
