---
status: accepted
---

# Pin a Dated Provider Model Version in Each Model Registration

On 2026-10-14, the author selected the identifier rule of a Model Registration. The decision was made on [Pin the Served Doubao Model Version and Record Its Context Window](https://github.com/FrankQDWang/StoryOS/issues/1143).

## Context

S4-01 ([#392](https://github.com/FrankQDWang/StoryOS/issues/392)) required one exact model identifier, and a later model change was to be a new Registration or Compatibility Decision. The Agent Plan Registrations `fa02` to `fa13` send `doubao-seed-2.1-pro`. On 2026-10-14, the route served the version `doubao-seed-2-1-pro-260915` for this identifier ([evidence record](../research/agent-plan-over-long-input-rejection-2026-10-14.md)). The Ark model list gives this version a window of 1024K tokens, and the earlier version `doubao-seed-2-1-pro-260628` a window of 256K tokens. Thus the provider changed the served version under the same identifier, and StoryOS did not see it. The qualification evidence of the earlier Registrations does not state the served version.

## Decision

- When the provider publishes dated versions of a model, the provider model identifier of a Registration is a dated version identifier. An undated name is an alias, as `latest` is ([glossary](../glossary/model-gateway.md)).
- Registration `018f0000-0000-7001-8000-00000000fa14` pins `doubao-seed-2-1-pro-260915` with the pending profile v1 of that version. The head Registration `018f0000-0000-7001-8000-00000000fa15` pins the same identifier with the qualified profile v2.
- The evidence of the undated identifier does not apply to the dated version. The qualified capabilities of the dated version have their own live evidence ([evidence record](../research/agent-plan-pinned-version-qualification-2026-10-14.md)).
- The profile of a dated version records the documented window of that version. For `doubao-seed-2-1-pro-260915`, it is 1024K, 1,048,576 tokens ([ADR 0056](0056-compact-at-80-percent-of-the-model-window-after-the-output-bound.md)).
- A later provider version is a new Registration that the author approves. Its profile gets new evidence before it qualifies.
- An AgentRun that pinned an earlier Registration refuses with Registration drift, as before. Registration `fa13` keeps the undated identifier and the window of 262,144 tokens.

## Considered options

- Keep the undated identifier and accept provider version changes. Rejected: a version change then changes the window and the behavior without a visible new Registration.
- Compare the version that a response reports with the Registration, and fail on a difference. Not in this decision: with a dated identifier, the route reported the same version in each response of the qualification.

## Consequences

- At the Claim of #1143, one live request confirmed that the route accepts the exact dated identifier.
- The compaction threshold of the head Registration is 812,646 tokens (ADR 0056).
- A provider that removes a dated version makes the Registration refuse at the destination. Then the author approves a new Registration.
