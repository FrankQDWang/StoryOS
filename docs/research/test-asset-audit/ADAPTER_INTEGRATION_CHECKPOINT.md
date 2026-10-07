# Adapter integration directory checkpoint

Both files under crates/storyos-adapter-postgres/tests are reviewed: 12 tests, 10 KEEP and 2 DELETE. The DELETE spans total 116 lines. See adapter.md for per-test evidence. Product and test files remain unchanged.

Retained differences are raw database RLS reads, composite foreign keys, expired settled challenge replay, incompatible stored binding fields, and legacy rate-policy compatibility. Public HTTP refusals that run before those database operations are not substitutes.

The full-u64 client generation test uses a value that the current Server main never emits (it sets generation 1). It is D5, with no claim that a normal HTTP generation test detects a high-half-only storage cast error. The other deletion is consumption rollback, owned by the public Reopen Rejected trigger-failure/retry scenario.

All verdicts await cross-directory reconciliation and the final random mutation self-check. This checkpoint is source review only.
