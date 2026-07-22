---
status: accepted
---

# Adopt Foundation Monorepo Governance

StoryOS keeps its Rust workspace, production Web Client, external contracts crate, and checked-in generated contract artifacts in one repository. The contracts crate remains the sole editable source for external contract shapes; generated artifacts are reviewed and reproducibly checked, never independently maintained. This prevents a Server, Worker, Adapter, and Web Client from silently advancing on incompatible contract releases while preserving a zero-configuration author experience.

## Considered options

- Splitting the runtime and Web Client into separate repositories was rejected because contract generation, compatibility review, and drift verification would require a separate cross-repository release protocol before there is an independently consumed product SDK.

## Consequences

- This decision does not select a directory tree, generator implementation, process topology, deployment layout, or first production slice.
- Disposable prototypes and `.reference/**` remain outside production workspace, dependency, build, test, package, release, and runtime boundaries.
