---
status: accepted
---

# Separate Author Edit Challenge Admission From Shared Command Admission

The Server derives one Challenge Rate Class from the exact project command kind. The author cannot select it, and the Protected Web Client cannot select it. `applyAuthorEdit` is in the `author_edit` class. All other project commands are in the `shared` class. Each class has its own immutable rate policy and its own counter. Thus, sustained writing cannot use the budget for Editor Session creation, Proposal actions, chapter and volume actions, AgentRun control, or export and import. Those commands also cannot stop normal saves.

| Class | Policy revision | Window | Capacity per window |
| --- | --- | --- | --- |
| `author_edit` | `storyos.project-command-challenge-rate.author-edit.fixed-window.v1` | 60 seconds, UTC-aligned, database time | 120 |
| `shared` | `storyos.project-command-challenge-rate.fixed-window.v1` | 60 seconds, UTC-aligned, database time | 10 |

The key of each counter is the Server-derived User, Project, Client Session Binding generation, and policy revision. The rules for the `shared` class do not change. The following rules apply to both classes. An exact retry returns the original Command Challenge and uses no unit. A changed-input conflict uses no unit. A refusal before Challenge insertion uses no unit. Exhaustion returns `429 rate_limited` with a `Retry-After` value to the end of the window. The nonce, session, digest, idempotency, and Project Scope bindings do not change.

## Capacity

The Web Client sends Author Edits one at a time. It closes a batch at each Chinese composition confirmation, paste, cut, drop, block split, block join, move, and retype. It also closes a batch after a 250 ms idle pause. Fast pinyin input is approximately 150 to 200 Chinese characters per minute. One confirmation usually commits approximately two characters. This gives approximately 75 to 100 confirmations per minute, plus some structural edits and idle boundaries. A capacity of 120 per minute (2 per second on average) covers this rate with margin. Ten per minute stopped normal writing after a few sentences.

The bound on resource use is 120 new Command Challenge and idempotency rows per minute for each User, Project, and session generation. Usually each of these rows also causes one authoritative write. A fixed window can admit up to two windows of capacity near a window boundary. This is the same property as the `shared` class.

## Visible behavior at the limit

When the Server refuses an Author Edit Challenge with `429`, the save state stays `saving`. The editor does not close submission and does not show `needs_attention`. The client waits for `Retry-After` and then requests a Challenge again for the same frozen submission group and idempotency key. The refused request inserted no idempotency record and used no unit. Thus, the new request is a first issuance and not a conflict. The client continues to retry while the page is open. Each retry occurs at least one second after the prior refusal.

The Local Edit Journal has at most one unsettled submission group. While the client waits, the editor shows new input, and the input waits in the bounded in-memory submission queue (240 operations). The input enters the Journal after the frozen group settles. A crash during the wait can lose the queued input, at most one `Retry-After` window. At the selected capacity, normal writing does not reach this path. If necessary, a later change can let the Journal keep several unsettled groups.

## Compatibility and pending Challenges

The rate policy revision controls Challenge issuance only. When the Server consumes a Challenge, and when it resolves an Author Edit outcome, it accepts the revision stored on the Challenge row if that revision is accepted for the command kind. For `applyAuthorEdit`, the accepted revisions are the `author_edit` revision and the earlier `shared` revision. Thus, a Challenge issued before deployment stays usable until it expires (5 minutes). Historical Author Edit outcome queries also stay available. The accepted revision for all other commands stays the `shared` revision.

A migration changes the rate-window capacity check from one fixed value of 10 to a check for each policy revision. Existing rows stay valid. Acceptance refusal records continue to bind the `shared` revision, because `acceptProposal` stays in the `shared` class. Thus, the bound in [ADR 0013](0013-trust-the-storyos-web-client-for-author-command-admission.md) on new refusal records for each window does not change.

## Considered options

- A larger number for the one shared budget was rejected. Sustained writing would still use the budget of other author commands.
- Removing the limit for Author Edit was rejected because it removes the bound on resource use.
- A limit on outstanding Author Edit Challenges was rejected for this change. An abandoned Challenge would hold a slot until it expires, and the change has a larger contract surface.
- A separate class for costly operations was not added. The issue evidence shows that only Author Edit submissions occur at a high rate.
