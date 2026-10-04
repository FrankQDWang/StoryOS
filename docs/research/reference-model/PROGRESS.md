# Reference Model Progress

## Goal and fixed source

Compare an independent contract model with Release 1 Server, Core, and PostgreSQL through public HTTP. Complete four stages: structure and Current Chapter; Author Edit and Undo; Proposal lifecycle from a fake-destination AgentRun; idempotency, Command Challenge, and writer takeover. Each contract outcome needs 20 observed hits or a documented reachability limit. Classify each difference and retain minimal replay evidence.

- Base: `479224809cdaae997cda51cb8853e3fafa242b65` (`origin/main`, fetched 2026-10-05).
- Branch: `codex/reference-model`.
- Worktree: `/Users/frankqdwang/.codex/worktrees/reference-model/StoryOS`.
- Only `prototypes/reference-model/` and `docs/research/reference-model/` may change.
- No product changes, existing test changes, generated changes, PRs, Issues, main changes, or complete verification runs.
- Database lifecycle: `scripts/dev-postgres.sh run`; dynamic service ports.
- Model expectations come only from repository contracts. Product source may supply launch and transport mechanics and defect locations, never expected state transitions.

## Step 1: Isolated baseline

Done: read repository rules and domain entry; fetched origin/main; created the isolated worktree and required branch. The primary checkout was clean and remained on main. Initial `make verify-status BASE=origin/main` reported stale historical evidence. No complete run was started.

Decision: use a small Python standard-library harness with independent state, seeded commands, HTTP transport, and retained JSON traces. Keep it outside default build and verification discovery. Do not add harness self-tests. Contract gaps remain explicit instead of inferred from implementation.

Next: inventory the public command contracts, prepare the existing release package, and execute one Empty Project to Volume to Chapter chain.

## Stage status

| Stage | Status | Evidence |
| --- | --- | --- |
| 1. Structure and Current Chapter | Started | Baseline only |
| 2. Author Edit and Undo | Pending | None |
| 3. Proposal lifecycle | Pending | None |
| 4. Idempotency and writer | Pending | None |
