---
status: accepted
---

# Require Explicit Project Deletion Settlement

StoryOS deletes an entire Project only through an explicit author-owned Project
Deletion Request followed by a durable Project Deletion Settlement. The request
immediately blocks new work and disclosure; settlement records known in-flight
outcomes or OutcomeUnknown, fences future work, makes the Scope unreadable and
unrestorable, then begins bounded physical cleanup. Retention never implicitly
deletes an entire Project.

## Considered options

- Dropping Project rows immediately was rejected because durable workers or
  outbox entries could still settle or dispatch after the deletion appears to
  have completed, and past external-effect uncertainty would be erased.
- Allowing a capacity or retention threshold to delete whole Projects was
  rejected because a novel Project is author-owned creative state, not a cache.

## Consequences

- A deletion request stops future Context Assembly and outbound disclosure
  before physical cleanup completes.
- Project Export and Project Restore do not recreate a deleted Project Scope;
  disaster recovery remains subject to Recovery Visibility Proof and the
  deletion lifecycle.
