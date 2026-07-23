---
status: accepted
---

# Adopt Deterministic Contract Verification

StoryOS verifies implementation through an independently maintained contract
oracle, contract-faithful fake destinations, named semantic fault points, a
virtual deterministic scheduler, and non-secret replayable evidence bundles.
This deliberately proves StoryOS admission, authority, durability, recovery,
and disclosure evidence without claiming model quality, Provider-internal use,
or opaque destination behavior. The decision resolves [Define Deterministic
Verification and Failure-Recovery Gates](https://github.com/FrankQDWang/StoryOS/issues/60);
its normative gate catalogue is [Deterministic Verification and Failure-Recovery
Gates](../foundation/deterministic-verification-and-failure-recovery-gates.md).

## Considered options

- Real Provider calls as required CI gates were rejected because their
  availability and opaque behavior cannot deterministically prove StoryOS
  semantics or Provider-internal facts.
- Production-shaped mocks, wall-clock sleeps, and arbitrary code-line crash
  hooks were rejected because they couple verification to implementation and
  turn recovery coverage into flaky, unreproducible observations.

## Consequences

- Every new durable or externally irreversible semantic boundary requires a
  named fault point, oracle classification, and fail-closed evidence bundle.
- Contract walks remain synthetic conformance paths; they do not select the
  first production vertical slice or weaken the standalone author-facing Eval
  boundary.
