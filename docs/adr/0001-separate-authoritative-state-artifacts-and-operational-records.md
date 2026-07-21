---
status: accepted
---

# Separate Authoritative State, Artifacts, and Operational Records

StoryOS separates author-owned Authoritative State, inspectable Artifacts, and execution-oriented Operational Records instead of representing them as one universal object with mutable authority levels. Agent, Tool, MCP, and extension outputs remain non-authoritative; only a StoryOS Core Proposal can cross the authority boundary through explicit author Acceptance, while deterministic direct author manipulation uses narrow domain commands. This boundary preserves natural writing, inspectable assistance, durable provenance, and extension safety without allowing status changes or third-party handlers to become an alternate write path.

[ADR 0004](0004-adopt-postgresql-service-and-project-isolation-boundary.md) adds the deployment-independent ownership boundary: every project-scoped member of these three durable spaces binds one exact `ProjectScope { owner_user_id, project_id }`. This clarification changes none of their authority or lifecycle semantics.

## Considered options

- A universal Artifact whose `authority_level` changes from draft to canonical was rejected because it conflates workflow, confidence, approval, locking, and truth.
- Fully separate objects without shared revision and provenance concepts were rejected because they would duplicate identity, audit, and source-tracing machinery.
- Open extension-defined Acceptance handlers were rejected because they would let third-party schemas bypass StoryOS domain invariants.

## Consequences

- Acceptance creates new Authoritative Revisions and an Authoritative Commit; it never promotes an Artifact in place.
- Artifact and authoritative histories remain independently versioned and are joined by exact provenance and immutable Receipts.
- Extensions may add safely preservable views and data, but authoritative mutations always return to closed StoryOS Core Proposal kinds and validators.
- Authoritative State, Artifacts, Operational Records, their references, and their derived projections cannot cross a Project Scope.
