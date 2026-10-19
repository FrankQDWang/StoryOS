---
status: accepted
---

# Keep Challenge Wait Input in the Local Edit Journal as Records That Follow an Unsettled Group

The specification [Keep Author Edit Input Durable During a Challenge Wait and Across a Rate Policy Revision](https://github.com/FrankQDWang/StoryOS/issues/1222) owns this ADR. The ticket [Save the Input of a Challenge Wait in the Local Edit Journal at Once](https://github.com/FrankQDWang/StoryOS/issues/1231) delivers it.

## Context

Before this change, the Local Edit Journal had at most one unsettled submission group (ADR 0038). While a frozen group waited for Challenge admission after `429`, input stayed in the in-memory submission queue. A reload or a crash during the wait lost that input. When the queue reached its bound, the editor stopped input, and the input of the wait was lost (ADR 0036).

## Decision

- **Records that follow a group.** Authoritative input enters the Journal at once while a frozen group is unsettled. It also does so while uncovered records that follow a group exist. Its record gets `follows_submission_group` with the identity of the latest group. Its base fields keep the projection base at the time of the append. They are not its submission binding.
- **Later groups.** A later group does not freeze during the wait. Its request needs the Authoritative Revision that the earlier group installs. Thus the Journal can keep one frozen unsettled group and several later runs of follower records. They stay in strict intent order. A run breaks where a submission group breaks. These are the idle time, a hard input origin, a new undo group, and 240 units.
- **One chain for each run.** Each run of follower records starts its own payload chain. Its checkpoint is the full projection text before the first record of the run. Thus each later group collects alone, and collection and prefix retirement do not change.
- **Binding.** Take the group that covers the record just before the run.
  - The run binds the installed base of that group when three conditions are true. The group is applied. Its installed base is the session base. The checkpoint of the run is equal to the installed text. Its request uses the installed Authoritative Revision, Proposal Heads, target references, and ownership partition.
  - While that group is unsettled, or while the record before the run is not covered, the run waits.
  - When that group ends in another way, the run has no binding.
- **Recovery.** A run with no binding never freezes, and no request sends it. The projection is `needs_attention`, the same zero-authority state that the earlier group shows. The editor becomes read-only. The records and their chains stay in the Journal and survive a reload in the same state.
- **Controller.** The Challenge wait does not hold the submission queue. On `429`, the controller records the wait, starts the `Retry-After` timer, and returns. The timer queues the retry of the same frozen group and idempotency key. A flush resolves only after the wait and its retry settle. A new undo group starts when the wait starts and at each run boundary during the wait.
- **Bound.** This rule applies while a wait is active or while follower records exist. Then input stops when the unsettled Journal records and the input that is not in the Journal yet are more than 240. In other states, the queue bound of ADR 0038 stays. At the stop, the controller cancels the wait. Queued appends still enter the Journal, but the controller submits nothing. The alert tells the author that the input in the Journal continues to save after a reload and that the last inputs can be lost.
- **Candidate input.** The in-memory hold of ADR 0046 stays for candidate input during a wait. It also stays for each input while the latest group is a candidate group. This is the current rule, not a permanent rule. The ticket [Save Candidate Input of a Challenge Wait in the Local Edit Journal at Once](https://github.com/FrankQDWang/StoryOS/issues/1232) owns its change. The binding is an object, so that ticket can add a Proposal Revision binding to it without a new Journal version.
- **Journal version.** IndexedDB version 5 carries the binding (ADR 0036). A reader fails closed on a binding that names a missing group when no working boundary exists, a later group, or a candidate group.
- **Settlement.** When follower records exist, the applied settlement of a group compares two texts. These are the applied text and the local text at the last record of that group.

## Replaced rules

- ADR 0038 said that the Journal has at most one unsettled submission group and that input waits in the in-memory queue during a Challenge wait. Authoritative input now enters the Journal at once.
- ADR 0036 said that the input bound stops input during a wait and that only the frozen group recovers after a reload. The input of the wait now also recovers.

## Considered options

- **Continue the chain of the earlier group.** Rejected: collection needs a chain with records of only one group. Also, prefix retirement stops at the first group that is not collected. A shared chain needs fences for more than one group and a new retirement proof.
- **Freeze later groups during the wait.** Rejected: their request needs the Authoritative Revision that the earlier group installs.
