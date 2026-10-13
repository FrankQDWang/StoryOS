---
status: accepted
---

# Retrieve the Stored Response of an Unknown Agent Plan Create

On 2026-10-13, [S4-04a: Read the Stored Response of an Unknown Real Create](https://github.com/FrankQDWang/StoryOS/issues/1129) qualified the retrieval of a stored response on the Volcengine Agent Plan route. This ADR records the Retrieve mapping, the Host admission of retrieval, and the evidence. It applies the Stage 3 recovery decisions of S3-23 to the real route and keeps their state machine.

## Context

Before this decision, the Agent Plan adapter refused Retrieve and Abort before the dispatch claim. A streamed Create learned its response reference at `response.created`, but kept it only in memory. A Worker crash lost it, and a re-observation after the restart reported no reference. The retrieval subject also compared the reported mapping with the Host-fake mapping, so a real reference did not pass the mapping gate.

The general Ark reference documents `GET /responses/{id}`. It does not prove the behavior of the `/api/plan/v3` route, and documentation is not qualification ([ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md)).

## Decision

- The adapter maps a Retrieve request to one `GET {endpoint}/responses/{reference}` with no retry and no body. A terminal response maps like a Create response, with the declared targets of the original Create. A response that is not terminal, a server error, a timeout, and a dropped connection are OutcomeUnknown. A rejection keeps its HTTP status and native code. A claimed Retrieve sends nothing again.
- The Wire Payload Projection of a Retrieve is the canonical JSON of its method and path. It has no cache setting.
- A streamed Create commits its response reference at the first report, through the stream sink of the Model Gateway, before the next stream commit. This commit has the same fence check as each stream commit. After a restart, the re-observation of the Attempt reports no reference, and the retrieval subject uses the committed reference. [S4-04e](https://github.com/FrankQDWang/StoryOS/issues/1133) added this rule.
- The adapter reports each reference as retrievable with declared read-only bounds. The Host admits a Retrieve only in two cases. The Model Capability Profile of the pinned Registration qualifies retrieval. Or the Attempt has `runtime_qualification = qualification_authorized` and the pinned Registration needs retrieval ([ADR 0049](0049-admit-an-author-authorized-runtime-qualification.md)). Otherwise the subject keeps `unsupported_retrieval`.
- The retrieval subject compares the reported mapping with the mapping of the pinned Registration.
- The work phase and the dispatch claim of a Retrieve check that the current assistance admission still admits the pinned route. Assistance must be available, and the binding, the Registration, and the qualification must be current. A changed binding or destination keeps the subject unknown with `destination_mismatch`, and no Retrieve leaves the Host.
- The requirement `RECOVERY_REQUIREMENT` adds retrieval to the compaction requirement. Registration `018f0000-0000-7001-8000-00000000fa12` (profile v11, retrieval pending) and the head Registration `018f0000-0000-7001-8000-00000000fa13` (profile v12, retrieval qualified) follow `fa11`. The abort entry stays unknown.
- The Stage 3 decision of S3-23 decides each retrieved result. A Retrieve has its own Destination Attempt, Wire Payload Projection, and Outbound Disclosure Event, and the Host admits it once. It never repeats the create, resumes the stream, or proves create idempotency. An unknown or incomplete result keeps the unknown outcome and the reservation.
- No author view shows a reference, a Retrieve, or a recovery classification ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)).

## Abort on the route

On 2026-10-13, [S4-04b: Qualify the Abort Behavior of the Real Route](https://github.com/FrankQDWang/StoryOS/issues/1130) added this section. The author asked for a probe of an undocumented cancel operation before the decision.

- The route has no usable cancel or abort operation. The public Responses API of the route lists no cancel operation. Four probes of `POST /responses/{reference}/cancel` returned `http_400:InvalidParameter`. They used a stored, an unknown, a just closed, and an in-progress response. The in-progress response then continued to its terminal event. The fourth probe is an approved exception to the limit of 3 probes ([evidence record](../research/agent-plan-recovery-qualification-2026-10-13.md)).
- The adapter refuses each Abort before its dispatch claim. No request leaves the Host, so the Abort has no Destination Attempt and no Outbound Disclosure Event.
- The refusal records `provider_abort = unsupported` and `provider_confirmed = false` on the Model Attempt Cancellation of the cancelled Attempt. The other cancellation duties continue, so the fenced retrieval of a late result still occurs. Before this decision, the refusal ended all cancellation duties of the Run.
- The closed stream at the cancellation, a local timeout, and the refused Abort never prove that the destination stopped. The cancelled Attempt keeps its unknown outcome and its reservation until a retrieved result reconciles its usage as evidence only.

## Considered options

- A raw GET outside the Model Gateway was rejected. It has no admission or disclosure evidence (ADR 0049).
- A retrieval admission from the adapter report alone was rejected. Documentation is not qualification, so the profile decides.

## Consequences

- The live qualification sent 11 requests ([evidence record](../research/agent-plan-recovery-qualification-2026-10-13.md)). The route served a stored complete response, and the Host settled a selected Advisory from it.
- Route fact: retrieval works only for a response that finished and that the destination stored. A streamed response whose connection closed before the terminal event returned `http_404:ResourceNotFound` later, also after 8.5 minutes. Such an Attempt keeps its unknown outcome and goes only to the one bounded successor of [S4-04c](https://github.com/FrankQDWang/StoryOS/issues/1131).
- Route fact: the route does not recognize a cancel operation. `POST /responses/{reference}/cancel` returned `http_400:InvalidParameter` for each of four references, also for an in-progress response. The section "Abort on the route" records the abort decision.
- The committed reference makes a finished response retrievable after a Worker crash between the end of the stream and the record. Without it, only the observation carries the reference, and the crash loses it.
- [S4-04c](https://github.com/FrankQDWang/StoryOS/issues/1131) settles a retrieved result of each decision kind. Until then, only a selected Advisory settles from a retrieved result.
