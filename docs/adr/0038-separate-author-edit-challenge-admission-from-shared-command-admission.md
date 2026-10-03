---
status: accepted
---

# Separate Author Edit Challenge Admission From Shared Command Admission

The Server derives one Challenge Rate Class from the exact project command kind. The author cannot select it, and the Protected Web Client cannot select it. `applyAuthorEdit` and `undoLatestAuthorAction` are in the `author_edit` class. All other project commands are in the `shared` class. Each class has its own immutable rate policy and its own counter. Thus, sustained writing and frequent Author Undo cannot use the budget for Editor Session creation, Proposal actions, chapter and volume actions, AgentRun control, or export and import. Those commands also cannot stop normal saves or Undo.

| Class | Command kinds | Policy revision | Window | Capacity per window |
| --- | --- | --- | --- | --- |
| `author_edit` | `applyAuthorEdit`, `undoLatestAuthorAction` | `storyos.project-command-challenge-rate.author-edit.fixed-window.v1` | 60 seconds, UTC-aligned, database time | 120 |
| `shared` | All other project command kinds | `storyos.project-command-challenge-rate.fixed-window.v1` | 60 seconds, UTC-aligned, database time | 10 |

The key of each counter is the Server-derived User, Project, Client Session Binding generation, and policy revision. The rules for the `shared` class do not change. The following rules apply to both classes. An exact retry returns the original Command Challenge and uses no unit. A changed-input conflict uses no unit. A refusal before Challenge insertion uses no unit. Exhaustion returns `429 rate_limited` with a `Retry-After` value to the end of the window. The nonce, session, digest, idempotency, and Project Scope bindings do not change.

A policy revision fixes the window, capacity, counter key, and retry semantics. This ADR records the command kinds of each class. A change of class membership does not change a revision. Thus, the move of `undoLatestAuthorAction` (issue 888) added no revision and no migration.

## Capacity

The Web Client sends Author Edits one at a time. It closes a batch at each Chinese composition confirmation, paste, cut, drop, block split, block join, move, and retype. It also closes a batch after a 250 ms idle pause. Fast pinyin input is approximately 150 to 200 Chinese characters per minute. One confirmation usually commits approximately two characters. This gives approximately 75 to 100 confirmations per minute, plus some structural edits and idle boundaries. A capacity of 120 per minute (2 per second on average) covers this rate with margin. Ten per minute stopped normal writing after a few sentences.

Author Undo is part of the same writing flow. Ctrl/Cmd+Z sends one Undo for each press. Each Undo compensates one earlier Forward Author Action, and most of these actions are Author Edits. Thus, in one session, the Undo count cannot exceed the count of earlier Forward Author Actions. The two commands share one counter, and the margin above the writing estimate (20 to 45 per minute) is available for Undo. The writing estimate is not measured for writing mixed with Undo. If the sum exceeds the capacity, the client waits (see below) and the editor stays usable. In the shared class, the 11th Undo in one minute made the editor read-only.

The bound on resource use is 120 new Command Challenge and idempotency rows per minute for each User, Project, and session generation. Usually each of these rows also causes one authoritative write. A fixed window can admit up to two windows of capacity near a window boundary. This is the same property as the `shared` class.

## Visible behavior at the limit

When the Server refuses an Author Edit Challenge with `429`, the save state stays `saving`. The editor does not close submission and does not show `needs_attention`. The client waits for `Retry-After` and then requests a Challenge again for the same frozen submission group and idempotency key. The refused request inserted no idempotency record and used no unit. Thus, the new request is a first issuance and not a conflict. The client continues to retry while the page is open. Each retry occurs at least one second after the prior refusal.

The Local Edit Journal has at most one unsettled submission group. While the client waits, the editor shows new input, and the input waits in the bounded in-memory submission queue (240 operations). The input enters the Journal after the frozen group settles. A crash during the wait can lose the queued input, at most one `Retry-After` window. At the selected capacity, normal writing does not reach this path. If necessary, a later change can let the Journal keep several unsettled groups.

The editor has at most one Author Undo in progress, from the key press until the Undo settles. Another Ctrl/Cmd+Z during that period does nothing. New editor input during that period abandons the Undo, because the new edit changes the Author Undo Frontier and the Undo can then only conflict. The editor keeps the new input and does not apply the Undo result to the editor. If the Undo request was not sent, the abandonment loses nothing: a refused Challenge request inserted no idempotency record. If the Undo request was sent, the Server can still apply the Undo, and the Author Edit of the new input then follows the ordinary conflict path. When the editor closes, the Undo is abandoned in the same way.

When the Server refuses an Author Undo Challenge with `429`, the editor stays editable and the save state shows `saving`. The client waits for `Retry-After` and then requests a Challenge again for the same Undo request and idempotency key. The client continues to retry until the Undo is admitted or abandoned. Each retry occurs at least one second after the prior refusal. Author Edit and Author Undo use the same Web Client unit for this wait.

## Compatibility and pending Challenges

The rate policy revision controls Challenge issuance only. When the Server consumes a Challenge, and when it resolves an Author Edit outcome, it accepts the revision stored on the Challenge row if that revision is accepted for the command kind. For each command kind in the `author_edit` class, the accepted revisions are the `author_edit` revision and the earlier `shared` revision. Thus, a Challenge issued before deployment stays usable until it expires (5 minutes). Historical Author Edit outcome queries also stay available. The accepted revision for all other commands stays the `shared` revision.

One case is not covered. A Challenge request for the same idempotency key, repeated after deployment, returns `409 idempotency_binding_conflict` if the earlier Challenge was issued under the `shared` revision. The cause is that the nonce derivation binds the revision. The Web Client repeats an Author Edit Challenge request only if the page stopped between Challenge issuance and the durable send record in the Local Edit Journal. After the send record exists, the client uses the outcome query, and the outcome query accepts the earlier revision. The Web Client repeats an Author Undo Challenge request only for a durable Refused Edit Draft Undo record after a reload, because the nonce is not durable. In that case, the client reconciles the Draft and otherwise shows `needs_attention`.

A migration changes the rate-window capacity check from one fixed value of 10 to a check for each policy revision. Existing rows stay valid. Acceptance refusal records continue to bind the `shared` revision, because `acceptProposal` stays in the `shared` class. Thus, the bound in [ADR 0013](0013-trust-the-storyos-web-client-for-author-command-admission.md) on new refusal records for each window does not change.

## Other author commands

Issue 888 examined every other project command kind. Each one stays in the `shared` class:

- Navigation and writer control: `setCurrentChapter`, `createEditorSession`, and `takeOverProjectWriter`. One Chapter switch uses at most two units. A switch or a writer takeover is a deliberate action and does not repeat for each key press.
- Proposal creation and review: `createReplacementProposal`, `acceptProposal`, `rejectProposalOperations`, `withdrawProposal`, `replanProposal`, `reopenWithdrawnProposal`, `reopenRejectedOperations`, `supersedeProposal`, `completeReadyPartialProposal`, and `continueProposalGeneration`. Each one is one decision for each click. Acceptance already records a `429` problem with its `Retry-After` value.
- Refused Edit Draft handling: `closeEditorFlowDraft` and `expandRefusedEditDraftToProposal`. These occur only after a refused Author Edit.
- Manuscript structure: `createVolume`, `updateVolume`, `deleteVolume`, `createChapter`, `updateChapter`, and `deleteChapter`. These are infrequent. `deleteChapter` and `deleteVolume` already wait for `Retry-After`.
- Project settings and lifecycle: `updateProject`, `updateProjectAssistance`, `archiveProject`, and `deleteProject`. These are infrequent.
- AgentRun control: `createAgentRun`, `steerAgentRun`, `pauseAgentRun`, `resumeAgentRun`, `cancelAgentRun`, `resolveWait`, `decideApproval`, and `updateConversationMemorySettings`. These are infrequent, and some of them start costly work.
- Export and import: `exportHumanReadableManuscript`, `exportProjectArchive`, and `importProjectArchive`. These are infrequent and costly.

`createProject` is not a project command. It uses the User-level Create Project Challenge before the Project exists, so no Challenge Rate Class applies to it.

## Considered options

- A larger number for the one shared budget was rejected. Sustained writing would still use the budget of other author commands.
- Removing the limit for Author Edit was rejected because it removes the bound on resource use.
- A limit on outstanding Author Edit Challenges was rejected for this change. An abandoned Challenge would hold a slot until it expires, and the change has a larger contract surface.
- A separate class for costly operations was not added. The issue evidence shows that only Author Edit and Author Undo submissions occur at a high rate.
- A separate `author_undo` class with its own revision was rejected. It needs a migration and a capacity with no measured Undo rate, and Undo is bounded by the earlier Author Edits.
- A new combined revision for Author Edit and Author Undo was rejected. Class membership is not part of a revision, so the new revision would add a migration and a third accepted revision with no change of window, capacity, key, or retry semantics.
- Holding new input until a waiting Undo settles was rejected. The Undo result replaces the editor content, and the held input would wait outside the Local Edit Journal.
