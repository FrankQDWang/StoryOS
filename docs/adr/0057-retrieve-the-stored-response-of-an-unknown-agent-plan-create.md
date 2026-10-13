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
- The adapter reports each reference as retrievable with declared read-only bounds. The Host admits a Retrieve only in two cases. The Model Capability Profile of the pinned Registration qualifies retrieval. Or the Attempt has `runtime_qualification = qualification_authorized` and the pinned Registration needs retrieval ([ADR 0049](0049-admit-an-author-authorized-runtime-qualification.md)). Otherwise the subject keeps `unsupported_retrieval`.
- The retrieval subject compares the reported mapping with the mapping of the pinned Registration.
- The requirement `RECOVERY_REQUIREMENT` adds retrieval to the compaction requirement. Registration `018f0000-0000-7001-8000-00000000fa12` (profile v11, retrieval pending) and the head Registration `018f0000-0000-7001-8000-00000000fa13` (profile v12, retrieval qualified) follow `fa11`. The abort entry stays unknown.
- The Stage 3 decision of S3-23 decides each retrieved result. A Retrieve has its own Destination Attempt, Wire Payload Projection, and Outbound Disclosure Event, and the Host admits it once. It never repeats the create, resumes the stream, or proves create idempotency. An unknown or incomplete result keeps the unknown outcome and the reservation.
- No author view shows a reference, a Retrieve, or a recovery classification ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)).

## Considered options

- A raw GET outside the Model Gateway was rejected. It has no admission or disclosure evidence (ADR 0049).
- A retrieval admission from the adapter report alone was rejected. Documentation is not qualification, so the profile decides.

## Consequences

- The live qualification sent 11 requests ([evidence record](../research/agent-plan-recovery-qualification-2026-10-13.md)). The route served a stored complete response, and the Host settled a selected Advisory from it.
- Route fact: retrieval works only for a response that finished and that the destination stored. A streamed response whose connection closed before the terminal event returned `http_404:ResourceNotFound` later, also after 8.5 minutes. Such an Attempt keeps its unknown outcome and goes only to the one bounded successor of [S4-04c](https://github.com/FrankQDWang/StoryOS/issues/1131).
- Route fact: the route does not recognize a cancel operation. `POST /responses/{reference}/cancel` returned `http_400:InvalidParameter` for a stored reference and for an unknown reference. [S4-04b](https://github.com/FrankQDWang/StoryOS/issues/1130) records the abort decision in this ADR.
- Only the observation of the exchange carries the reference. Thus a Worker crash after the end of the stream and before the record still loses the reference of a finished response. [S4-04e](https://github.com/FrankQDWang/StoryOS/issues/1133) commits the reference at its first report, which makes this case retrievable.
- [S4-04c](https://github.com/FrankQDWang/StoryOS/issues/1131) settles a retrieved result of each decision kind. Until then, only a selected Advisory settles from a retrieved result.
