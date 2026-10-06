---
status: accepted
---

# Hold Author Edit Command Requests in One Exact-Dist Browser Command

The author accepted this decision on 2026-10-06. It records one narrow exception to the Browser Command families of [ADR 0015](0015-adopt-typescript-and-vitest-browser-mode-for-the-protected-web-client.md). It is the result of [Wait for the Saved State Instead of the Transient Saving State in Three Exact-Dist Journeys](https://github.com/FrankQDWang/StoryOS/issues/985).

## Context

The S1-JRN-001 expected journey records five saving projections: after a typed input, after an IME input, after a paste, before the interrupt reload, and after the recovery. The journey polls for each projection after the input.

The saving state is transient:

- Paste and IME confirmation are hard boundaries. The Web Client submits them at once.
- A typed input submits after the Author Edit batch idle time of 250 ms.

Thus the save can complete before the poll reads the projection. Issue #985 found this race in three Stage 2 journeys and told the S1-JRN-001 review to change S1-JRN-001 if it has the same race. It has the same race.

The journey cannot hold the submission without a Browser Command:

- The Web Client binds `fetch` when it starts. The test cannot replace it in the child application frame before the bundle runs.
- The HTTP policy blocks unused Workers.
- The product has no test seam that holds a submission.

ADR 0015 allows four Browser Command families. A command must not accept a network destination, and it must not operate on the Vitest orchestrator page. A Playwright route is a page-level primitive.

## Decision

`browser-exact-dist` gets one more private Browser Command, `storyosAuthorEditSubmissionHold`.

- The request is closed: `{ "action": "hold" }` or `{ "action": "release" }`. The result is closed: `{ "kind": "author_edit_submission_hold_updated" }`.
- `hold` registers one route on the provider page for the fixed path `/api/v1/projects/{project_id}/manuscript/author-edits`. The route delays each `POST` from a child frame until `release`. Then it continues the request without a change.
- The route does not change, fulfill, or abort a request. It does not read a request or response body. It continues a request from the orchestrator frame and every other method at once.
- One hold can exist at a time. A second `hold` fails. `release` without a hold has no effect.
- The command accepts no URL, method, code, callback, SQL, file path, or network destination.
- A reload aborts a held request. The command then ignores the refusal of Playwright to continue it.
- Each journey that uses the command releases the hold in `afterEach`.

S1-JRN-001 holds the requests before each of its three inputs and releases them after it records the saving projection. It holds the requests before the fourth input and keeps the hold through the reload. It releases them after it records the recovered projection. The S1-JRN-001 expected journey and its Authority assertions do not change.

## Considered options

- **Keep S1-JRN-001 without a change.** Rejected: the journey keeps a race that issue #985 told this work to remove.
- **Remove the saving projections from the S1-JRN-001 expected journey.** Rejected: the projections are the Stage 1 save-truth evidence.
- **Add a product test seam that holds a submission.** Rejected: a test-only path in the production bundle is a larger change than one test command.

## Consequences

- The saving observations of S1-JRN-001 no longer depend on time.
- The interrupt step must still read the Local Edit Journal before the batch idle time collects the fourth group. The hold does not stop that collection, because the collection occurs before the request.
- The Browser Commands `storyosSettleWorkerOnce` and `storyosCommandChallengeRateWindows` also have no ADR record. This decision does not record them.
