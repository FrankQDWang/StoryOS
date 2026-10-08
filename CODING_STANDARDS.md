# StoryOS coding standards

Read this file before you change Rust code, a test, a generated artifact, or a contract. The Standards reviewer compares a diff with `AGENTS.md`, this file, the glossary (`GLOSSARY.md` and its area files under `docs/glossary/`), and ASD-STE100.

Put a new coding rule in this file and a new operating rule in `AGENTS.md`. These rules are complete in StoryOS. Use this file as their source, not a `.reference/` copy.

A uniform Rust style keeps boundaries, call sites, and reviews legible across the workspace.

## Guard-owned rules

`rustfmt.toml` and the Clippy lints in `Cargo.toml` own format, inline `format!` arguments, collapsed `if` statements, and method references. The guards in the [Daily loop](docs/agents/verification.md#daily-loop) own positional-literal comments and the size advisory.

## Call sites

- Give each call site a self-documenting shape, not `foo(false)` or `bar(None)`. Prefer an enum, a newtype, a method with a clear name, or a similar idiomatic shape to a `bool` or an ambiguous `Option` parameter.
- When the API must keep a positional literal, put an exact `/*param_name*/` comment before an opaque `None`, boolean, or numeric argument. The name in the comment is the parameter name in the callee signature.
- A sole non-self argument needs no comment when the method and parameter names are the same, for example `.enabled(false)` for `fn enabled(&self, enabled: bool)`. The [exemption list](docs/agents/rust-literal-exemptions.json) holds these methods for the literal guard.
- Add a comment to a string or char literal only when it adds real clarity.

## API design

- Make `match` statements exhaustive when possible, with no wildcard arm.
- Give each new trait a doc comment that explains its role and how implementations use it.
- Spell the future contract of an async trait method explicitly: `fn foo(&self, ...) -> impl std::future::Future<Output = T> + Send;`.
- An implementation can use `async fn foo(&self, ...) -> T` when it satisfies that contract.
- Avoid `#[async_trait]` and `#[allow(async_fn_in_trait)]` in Rust traits.
- Prefer private modules and an explicitly exported public crate API.
- Keep crate API surfaces as small as possible. Keep test-only functions out of the crate API and the main implementation.

## Crates and modules

- Put a new concept in the crate that owns it. Introduce a focused crate when that gives a clearer dependency boundary. Do not grow a central Agent crate only because it is convenient.
- Inline a small helper method that only one call site uses.
- Keep a Rust module under 500 LoC, without its tests. Add a new module in preference to a larger existing module.
- When a file is larger than approximately 800 LoC, put new functionality in a new module. Extend the file only for a strong documented reason.
- When you extract code from a large module, move its tests and its module and type docs with it. Then the invariants stay near the code that owns them.

## Observability

- Instrument async work at the function or method definition with `#[tracing::instrument(...)]`. Attach no span to a future with `.instrument(...)` at a call site.
- Before you add instrumentation, find whether the callee, or the implementation method it directly delegates to, already has it.

## Tests

- Add a test only for a realistic observable regression, a non-trivial invariant or boundary, or a concrete bug. A code change or a coverage increase alone is not a reason for a test.
- Prefer existing coverage at the public behavior boundary.
- A change to Agent Loop behavior, tool execution, authorization, recovery, or other user-visible Agent semantics requires integration tests at the public boundary.
- Compare the equality of entire objects in preference to fields one by one.
- Test observable behavior. Do not add tests that copy literals, mappings, obvious control flow, implementation details, or statically defined values.
- Add a test for a removed feature only when the absence is itself a contract.
- Use deterministic coordination or controlled scheduling for concurrent work. Use a sleep only when no deterministic wait is available.
- Use existing test helpers when they make a test shorter and easier to read.
- Pass environment-derived flags or dependencies from above, in place of a change to the process environment in a test.
- Put a new test module in a separate sibling file. Use an explicit `#[path = "..._tests.rs"]` attribute that gives the file a descriptive name:

  ```rust
  #[cfg(test)]
  #[path = "parser_tests.rs"]
  mod tests;
  ```

- This applies only to a new test module. Keep an existing inline `#[cfg(test)] mod tests { ... }` module where it is when this convention is the only reason to move it.

## Project commands

- A project command consumes one Project Command Challenge and settles one Author Command Admission. It settles through `command_sequence` in `storyos-adapter-postgres`. Do not write its transaction, its replay query, an application Store trait, or a binding self-check. ADR 0043 lists the operations that stay outside the sequence.
- Implement `ProjectCommand` for a command that settles in one transaction. Implement `AdmittedCommand` for a command that has an admit step and a settle step. Declare its `SettlementProfile`, its applied variants, and its zero-authority outcomes.
- An applied variant that writes a Forward Author Action names its own `ForwardCommand` entry. Give that entry its Author Undo Disposition in `ForwardCommand::disposition`: a compensation in the module of the forward command, or a Barrier. Record the disposition in an ADR.
- Add rows to the table-driven contract suite (`command_sequence_tests/`) for each applied variant and zero-authority outcome. The rows prove replay equality, the declared records only, rollback, an in-progress exact retry, and pre-capture versus damaged evidence.

## Comments and documentation

- Prefer no comment. A comment that stays gives one non-obvious reason in one line. Code changes later, and comments do not.
- Document a public API by its observable contract, in one line where possible. Leave incidental implementation details out.

## Contracts and generated artifacts

- Treat a change to ToolSpec, MCP adapters, Skill manifests, Artifact and Run events, external APIs, configuration, persisted data, or recovery formats as a contract change. Review its breaking and migration impact explicitly.
- Editable sources own deterministic generated artifacts. Change and stage a generated artifact with the editable source that produces it.
- Regenerate it with the StoryOS-owned command, classify its diff separately from hand-written lines, and review the generated diff for drift.

## Change size

- Keep a change at or below 800 changed lines, unless the change is mechanical. Keep a complex logic change below 500 changed lines.
- `make verify-status` shows a change above these limits in its `changeSize` advisory.
- Base the staging suggestion on the actual diff, dependencies, and affected call sites.
