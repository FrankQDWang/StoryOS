# StoryOS repository instructions

## Scope and authority

- This file applies to the whole repository. A nearer nested `AGENTS.md` may add or narrow instructions for its subtree; include only subtree-specific boundaries and commands, and do not duplicate this root file.
- Repository files and tracked design artifacts are the source of truth. Conversation history and Codex memory are supporting context only.
- Layering of authority: code and checked-in generated contracts state the current implementation reality; ADRs under `docs/adr/` and the current Wayfinder map record the product and architecture contract; this `AGENTS.md` states repository operating rules and coding style. When they diverge, code is the fact of what exists today, but a divergence from an ADR is either a defect to fix or an ADR-recorded exception — never a silent override.
- One task has one execution owner. Subagents may investigate or review, but they do not independently mutate the same deliverable or authoritative state.
- Preserve unrelated user changes. Do not rewrite, discard, or clean them up as part of another task.
- Write all repository artifacts in ASD-STE100 Simplified Technical English: code, comments, documentation, commit messages, GitHub Issues, and pull requests. Talk to the user in Simplified Chinese. Always read `CONTEXT.md` files, and use their ubiquitous language.

## GitHub Issue and stacked-PR execution

### Single-main development policy

- Develop only on the checked-out local `main` branch. Do not create Git worktrees or local feature branches.
- Commit on local `main`, but never push implementation commits directly to `origin/main`. Push the required commit range from local `main` to a temporary remote feature branch, open a pull request, merge only after its required checks and reviews pass, then delete that remote branch.

- StoryOS works directly from GitHub Issues. The locked current Issue body, its Claim lock, exact baseline, applicable `AGENTS.md` files, and the tracked contracts explicitly linked by the Issue form the execution contract.
- Keep one implementation Issue and one concentrated task context. When the implementation is larger than the review-size limits in Tests and change review, decompose it into one coherent GitHub-native stacked-PR chain within that same Issue rather than creating finer Issues solely to reduce PR size.
- A PR stack is an implementation and review decomposition only. It does not create new scope, a second specification, parallel execution ownership, or permission to claim later-layer behavior early.
- Git branches, commits, PR bases, reviews, checks, and merge state in GitHub are the authoritative representation of the stack.
- Create the first temporary remote stack branch from the Issue's locked `main` baseline. Create every later temporary remote branch from the immediately preceding stack branch, and target each PR at that predecessor until it merges.
- Every layer must name its stack position and predecessor, link the full Issue title, and use `Part of #<issue>` rather than an auto-closing keyword. Do not let an intermediate PR close the owning Issue.
- Apply the hand-written, non-mechanical line limits in Tests and change review to each PR layer. Classify deterministic generated artifacts separately, keep their editable source in the owning earlier layer, and review generated drift explicitly.
- Require independent read-only review and the layer-appropriate targeted checks for every PR. A lower layer must not claim upper-layer acceptance, and an upper layer must not hide a lower-layer contract, security, migration, or recovery defect.
- Merge the stack bottom-up with ordinary merge commits. Do not squash-merge or rebase-merge a stack layer unless the locked Issue explicitly overrides this rule. After each merge, refresh `main`, then recheck the next PR's base, exact remaining diff, commit ancestry, review validity, and fresh required checks; a pre-retarget green check is not fresh evidence.
- Close the Issue only after every stack layer is merged, the final composed state on current `main` passes the complete non-mutating verification, and the single tracker-compliant Resolution records all merged PRs, the final commit/tree, and retained evidence. Only then create or claim the next serial implementation Issue.

### Verification evidence

- The required GitHub `verify` check is a pull-request synthetic-merge sentinel. It does not replace complete local verification.
- Before merging each PR layer, run `make verify` on a tree that matches the required `verify` synthetic-merge tree. Record the head, tree, command, PASS result, and clean worktree in one PR comment.
- A new commit invalidates that evidence. A retarget always requires a fresh `verify`; rerun local verification when the candidate tree changes. After the stack merges, synchronize `main`, rerun `make verify`, and record the final commit, tree, and evidence in the Resolution.

## Reference source policy

- `.reference/` is local-only, Git-ignored reference material. Do not add any entry under it to Git, and do not edit an individual reference except when a task explicitly refreshes that reference.
- `.reference/**` must not enter the StoryOS Cargo workspace, dependency graph, build, test, package, release, or product runtime.
- Learn from upstream patterns, but independently design StoryOS around its domain. Do not fork, embed, or wrap the Codex runtime.
- Before copying upstream implementation code, verify architectural fit, isolate the copied unit, review its license obligations, and record provenance. Copying a design idea does not make upstream a production dependency.
- The Rust guidance below is self-contained in StoryOS. Agents must not rely on opening `.reference/` copies to discover these rules.

## Rust engineering rules

Coding style is part of architectural consistency: a uniform Rust style keeps boundaries, call sites, and reviews legible across the workspace.

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

### Tests and change review

- Add a test only for a realistic observable regression, a non-trivial invariant or boundary, or a concrete bug. A code change or a coverage increase is not enough reason to add a test.
- Prefer existing coverage at the public behavior boundary.
- Changes to Agent Loop behavior, tool execution, authorization, recovery, or other user-visible Agent semantics require integration tests at the public boundary.
- When writing tests, prefer comparing the equality of entire objects over fields one by one.
- Do not add tests that copy literals, mappings, obvious control flow, or implementation details.
- Do not add tests for values that are statically defined.
- Do not add tests for a removed feature unless the absence is itself a contract.
- For concurrent work, use deterministic coordination or controlled scheduling. Do not use sleeps when a deterministic wait is available.
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
- Write comments that explain non-obvious rationale, invariants, safety constraints, or external quirks. Do not restate the code.
- Document a public API by its observable contract. Do not document incidental implementation details.
- Treat changes to ToolSpec, MCP adapters, Skill manifests, Artifact and Run events, external APIs, configuration, persisted data, or recovery formats as contract changes and review their breaking and migration impact explicitly.
- Unless the change is mechanical the total number of changed lines should not exceed 800 lines.
- For complex logic changes the size should be under 500 lines.
- Base the staging suggestion on the actual diff, dependencies, and affected call sites.

### Verification

- Use StoryOS-owned repository commands for formatting, linting, tests, schema generation, and verification.
- Run the relevant targeted checks after changes and the final non-mutating verification command before declaring completion.
