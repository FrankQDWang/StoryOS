# Agent Plan recovery qualification, 2026-10-13

Evidence record of [S4-04a: Read the Stored Response of an Unknown Real Create](https://github.com/FrankQDWang/StoryOS/issues/1129) and [S4-04b: Qualify the Abort Behavior of the Real Route](https://github.com/FrankQDWang/StoryOS/issues/1130). It records the live requests that qualified the retrieval of a stored response on the Volcengine Agent Plan Responses route, and the probe of a cancel operation (ADR 0057). The profile revision `storyos.volcengine-agent-plan.doubao-seed-2.1-pro.capability.v12` cites it. It is not a claim about model quality, Provider retention, training, or billing of an interrupted request.

## Authorization and plan

- Author authorization of live requests: the [standing authorization on #362](https://github.com/FrankQDWang/StoryOS/issues/362#issuecomment-6110067072). On 2026-10-13 the author asked, through the coordinator, for a probe of an undocumented cancel operation with at most 3 requests.
- The plan was written before the first request: about 8 requests, a cap of 12 requests, 30,000 input tokens, and 40,000 output tokens. After R2, the plan was corrected in the same order of magnitude and with the same caps.
- Sent: 11 requests (5 Create, 4 Retrieve, 2 cancel probes). No retry, no repeat, and no anomaly.
- Each step ran in one `storyos-worker --once` process with the debug log. No Worker log has an authorization header or a key.

## Setup

| Fact | Value |
| --- | --- |
| Endpoint | `https://ark.cn-beijing.volces.com/api/plan/v3` |
| Model | `doubao-seed-2.1-pro` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh` with the controlled fixture (Project A, Chapter A), Server with `STORYOS_WORKER=0` |
| Credential | Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the value. |
| Request bounds | `agent_plan_request_v2`: 32,768 output tokens, 600 s |
| Build | Measurement commit `077f11c5`, tree `1fe2bbd1`, tag `evidence/s4-04-recovery-measurement`. It is not for merge. It maps Abort to `POST /responses/{reference}/cancel` for the probe, and it prints the native result of each Retrieve and probe. |
| Registration | `018f0000-0000-7001-8000-00000000fa12`, profile v11 (retrieval pending), mapping `responses.create.v4`, decision `qualification_authorized` |

## Fault injection on the disposable database

- A crash: the executor killed the Worker with `SIGKILL` after the stream committed the response reference, before the Worker recorded the observation. A new `storyos-worker --once` claimed the Run after the lease expired.
- A replaced reference (R4, R10): after the crash, an admin statement replaced the committed reference with the reference of a stored complete response of the same Project. Thus the Retrieve read a response that the destination had stored.
- A repeated Retrieve (R6): an admin statement removed the retrieval subject and the retrieval rows of the R4 Run, and set the committed reference to the R1 reference. This removed the retrieval Attempt row of R5; this record keeps its result.
- A cancellation (R7, R8): an admin statement made the effect of the cancel command on a Run with an uncertain Attempt: status `cancelled`, a new fence, and the wakeup.

No other record changed. The database was removed after the run.

## Requests

| # | Time (UTC) | Purpose | Model Attempt | Wire digest | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | 05:45:14 | Create, Worker killed 1.6 s after the start | `01a133cb-28c5-7a03-8f69-c19af20e83a3` | `sha256:3ef373f3…` | Reference `resp_0217918703148544aa8fafbe9055982037cfb2f5ec29c226f4ffd` committed at its first report | unknown |
| R2 | 05:47:18 | Retrieve of the R1 reference after a new claim | `01a133cd-0d0c-71b3-964e-ac9f87459dde` | `sha256:df358339…` | `http_404:ResourceNotFound`; subject kept `unknown_result`, Run waiting | unknown |
| R3 | 05:48:40 | Create in a new Conversation | `01a133ce-4e29-7683-8a56-2377d80d26c7` | `sha256:bda87fe6…` | Terminal Clarification in 3.5 min, reference X `resp_0217918705210427dd244b6caaa2409295dae72fd71f679d2dda0` | 306 / 6,772 |
| R4 | 05:52:37 | Create that continues R3, Worker killed after 2 s | `01a133d1-ea22-7e40-b9f9-dc11aa0676c4` | `sha256:baacc393…` | Reference `resp_0217918707575629147904061828da2e34644a32f13ef27a15be1`, replaced with X | unknown |
| R5 | 05:53:20 | Retrieve of X, Run `01a133d1-c850-7f32-9286-8301ade7aa8b` | Not retained: the reset before R6 removed the row, and the Worker log has no Attempt ID | not retained | 200, terminal, each item complete, usage reported (Worker log) | 306 / 6,772 (of X) |
| R6 | 05:53:40 | Retrieve of the R1 reference, 8.5 min after R1 | `01a133d2-e165-7f61-a0cf-cec5add7664b` | `sha256:df358339…` | `http_404:ResourceNotFound` | unknown |
| R7 | 05:54:03 | Cancel probe `POST /responses/X/cancel` | `01a133d3-391f-7e20-a3ea-1550f8f1c0e3` | `sha256:9b403861…` | `http_400:InvalidParameter` | unknown |
| R8 | 05:54:19 | Cancel probe `POST /responses/{R1 reference}/cancel` | `01a133d3-7878-7530-9266-d006101ecacc` | `sha256:a2046d7d…` | `http_400:InvalidParameter` | unknown |
| R9 | 05:55:21 | Create in a new Conversation | `01a133d4-6d51-7d30-983f-c0ca64d408b5` | `sha256:b219e385…` | Terminal Advisory in 4 s, reference Z `resp_021791870922197cf23db4d38572816d86b7975cf989d612f8e69`. The kill came too late, and the Run completed. | 311 / 100 |
| R10 | 05:56:06 | Create that continues R9, Worker killed after 1.6 s | `01a133d5-1962-7253-aab8-ff1b6109276c` | `sha256:7e096871…` | Reference `resp_021791870966248b3148bc0f12eba71a521be8450222824152905`, replaced with Z | unknown |
| R11 | 05:56:40 | Retrieve of Z | `01a133d5-a15d-7313-8bee-b9196ea09bdf` | `sha256:f3bf9328…` | 200, terminal, complete. The Host settled the selected Advisory on the R10 Attempt, released the reservation, and made the continuation binding. The Run completed, and the panel showed the answer. | 311 / 100 (of Z) |

Each Attempt has its own Outbound Disclosure Event. Each decision Attempt records `runtime_qualification = qualification_authorized`.

Known usage of the Create requests: 617 input and 6,872 output tokens. The usage of R1, R4, and R10 is unknown.

## Results

- The `/api/plan/v3` route serves `GET /responses/{reference}` for a stored complete response. The body is the complete response object with its usage, and it maps like a Create response (R5, R11).
- The route returns `http_404:ResourceNotFound` for the reference of a streamed response whose client connection closed before the terminal event. This was true after 2 minutes (R2) and after 8.5 minutes (R6). R3 took 3.5 minutes, so a longer generation does not explain R6. StoryOS cannot tell from this result whether the destination stopped, discarded, or did not store that response. The Attempt keeps its unknown outcome and its reservation.
- Thus a Retrieve can recover a response that completed before the Host lost it, for example a Worker crash after the stream ended and before the record. It does not recover a stream that closed while the response was in progress.
- The route returns `http_400:InvalidParameter` for `POST /responses/{reference}/cancel`, for a stored complete response (R7) and for a reference that a GET does not find (R8). The same native code for both references shows that the route does not examine the reference. The route has no usable cancel operation.
- A committed reference survives a Worker crash: the re-observation after the restart used it (R2, R5, R11).

## What this evidence qualifies

The qualification of retrieval rests on R11, which has an attributable Model Attempt and Outbound Disclosure Event. R5 is a supporting observation only, because its Attempt row was not retained.

- Retrieval of a stored response on the exact route, its mapping, and its settlement through the Model Gateway. Retrieval is a separate request with its own Attempt and Outbound Disclosure Event, and it repeats no create.
- The rejection of the cancel probe. S4-04b records the abort decision.

## What it does not qualify

- Recovery of a stream that closed while its response was in progress.
- The billing of an interrupted request. Its usage stays unknown, never zero.
- Expiry of a stored response. The default expiry is 72 hours after creation; no reference in this run was that old.
- Any guarantee that a stored response stays available.

## Addendum: probe of a cancel operation on an in-progress response, 2026-10-13

The Spec review of [S4-04b](https://github.com/FrankQDWang/StoryOS/issues/1130) asked for a cancel probe on a response that is still in progress. R7 and R8 did not satisfy this. The coordinator approved more requests: a cap of 16, then a cap of 21, with the same token caps. The same measurement build and a new disposable database with the controlled fixture ran these requests.

### Approved exception to the limit of 3 cancel probes

The author limited the cancel probes to 3. This run sent 4: R7, R8, R16, and R21. R16 was the third probe, and it did not reach an in-progress response, because the stream of A2 closed first. On 2026-10-13 the executor stopped and reported this. The coordinator then approved option (a) of that report: 4 more requests, which include one more cancel probe that reaches an in-progress response (R21). The author delegated this approval to the coordinator for the S4-04 live work. Thus R21 is an approved exception to the limit, and no other cancel probe was sent.

### Method

- A long Create (A) streams through its own Worker, which stays connected to the end.
- A second Create (B) goes out, and its Worker is killed after the reference commit. An admin statement on the disposable database moves the lease of B one hour later, so that the Worker of A claims A first.
- After A commits stream events, an admin statement replaces the committed reference of B with the reference of A. Another admin statement makes the effect of the cancel command on B: status `cancelled`, a new fence, and the wakeup.
- A third Worker then does the cancellation duties of B through the Model Gateway: one fenced Retrieve and one Abort of the reference of A, while A still streams.

### First attempt (R12 to R16 and one recovery Retrieve), B in the same Project

| # | Time (UTC) | Purpose | Model Attempt | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- |
| R12 | 08:29:47 | Create A1 | `01a13461-d0c3-7ab1-8e39-bcca5fa9c03f` | Terminal in 88 s, before B existed. Not in progress for the probe. | 329 / 1,976 |
| R13 | 08:32:14 | Create B, Worker killed after 1.5 s | `01a13464-0bc6-7223-9041-c78894bc2497` | Reference `resp_021791880334523cd31eac8229c74daa7d4d74052dc3c32f25b49` | unknown |
| R14 | 08:33:05 | Create A2 | `01a13464-d2ed-7dd0-ad69-2b01bbe1e73c` | At 08:33:06.883 a stream commit failed with SQLSTATE `40001`, a serialization conflict with the Worker of B in the same Project. The exchange stopped and the stream closed. | unknown |
| (extra) | 08:33:07.073 | Recovery Retrieve of A2 by its own Worker | `01a13464-d951-73e1-a4c0-518d204fa0d1` | `http_404:ResourceNotFound`. This request was not in the plan, and it made the total 17, one over the cap. The executor stopped. | unknown |
| R15 | 08:33:07.073 | Fenced Retrieve of A2 by B | `01a13464-d93c-7d30-9067-b8f0d1635bc9` | `http_404:ResourceNotFound` | unknown |
| R16 | 08:33:07.348 | Cancel probe of A2 by B | `01a13464-da1f-75d2-a7be-0504d469ce9b` | `http_400:InvalidParameter`. The stream of A2 had closed already, so A2 was not in progress. | unknown |

The stream stop on `40001` is a separate defect: [Retry a stream commit after a serialization failure instead of stopping the stream](https://github.com/FrankQDWang/StoryOS/issues/1137).

### Second attempt (R18 to R21), B in a different Project

| # | Time (UTC) | Purpose | Model Attempt | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- |
| R18 | 08:45:12 | Create B2 in the Project "Probe B", Worker killed after 7 s | `01a1346f-eb6a-7302-8a59-f2a951a7748c` | Reference `resp_0217918811188613906a72ab000b5ec26f7911656631d35a002df` | unknown |
| R19 | 08:46:01 | Create A3 in Project A, Worker connected to the end | `01a13470-ace2-7820-99d9-76ede82a4f14` | Reference `resp_02179188116218941b543c0bdbbf2d644f33f27e04d485e5b1c20`. Stream commits from 08:46:03.07 to 08:51:05.59, with no commit failure. Terminal at 08:51:05. | 321 / 6,780 |
| R20 | 08:46:04 | Fenced Retrieve of A3 by B2, while A3 streamed | `01a13470-b420-74b1-addc-d88ebcc15438` | `http_404:ResourceNotFound` | unknown |
| R21 | 08:46:04 | Cancel probe of A3 by B2, while A3 streamed | `01a13470-b5af-7da1-b01e-666d468f60c8` | `http_400:InvalidParameter`. A3 continued and completed 5 minutes later. | unknown |

Each Attempt has its own Outbound Disclosure Event.

### Results of the addendum

- The cancel operation returned `http_400:InvalidParameter` for an in-progress response (R21), and the response continued to its terminal event. With R7, R8, and R16, the four probes returned the same code for a stored, a discarded, a just closed, and an in-progress response. The route has no usable cancel operation.
- A GET of an in-progress response returned `http_404:ResourceNotFound` (R20). Thus a response is retrievable only after it finished and the destination stored it.
- Totals of the whole run: 21 requests plus the noted recovery Retrieve in the first attempt, 22 in all. Known usage of all Create requests: 1,267 input and 15,628 output tokens.

## Journey: a recovered request on the qualified route, 2026-10-13

Evidence of [S4-04d: Show the Author a Stopped Request Without a Result](https://github.com/FrankQDWang/StoryOS/issues/1132). The coordinator approved 2 requests with a cap of 4.

| Fact | Value |
| --- | --- |
| Build | S4-04d branch commit `1d9833e2`, tree `29e69c82`, with the head Registration `fa13` (profile v12, qualified). No qualification authorization. |
| Deployment | A new disposable database with the controlled fixture, Server with `STORYOS_WORKER=0` |

| # | Time (UTC) | Purpose | Model Attempt | Outcome | Usage (input / output) |
| --- | --- | --- | --- | --- | --- |
| R22 | 10:34:26 | Create, a short Advisory. After the reference commit, the Worker was frozen with `SIGSTOP` at 10:34:29.0, with no stream item committed, and killed with `SIGKILL` at 10:35:29.1. | `01a134d3-ee16-74d2-b202-a3b6d53c3633` | Reference `resp_021791887666996e28a43f868cac88f2cf2873497029e3a9787c9` | 311 / 121 (from R23) |
| R23 | 10:35:37 | Retrieve by a new Worker after the lease expired | `01a134d5-0445-7e12-b0b6-280e2b525282` | Terminal and complete. The Host settled the Advisory on the R22 Attempt, made the continuation binding, and completed the Run. The panel showed the answer. | (of R22) |

Each Attempt has its own Outbound Disclosure Event. No database statement changed a record in this journey.

The frozen Worker kept its connection open, so the destination finished and stored the short response. Then the kill closed the connection, and the stored response was retrievable. This is the path that a crash after the end of a stream takes.

Totals of all S4-04 live work: 23 requests plus the noted recovery Retrieve, 24 in all.
