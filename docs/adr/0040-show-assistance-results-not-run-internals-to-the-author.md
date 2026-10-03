---
status: accepted
---

# Show Assistance Results, Not Run Internals, to the Author

The author surface shows the result of assistance. For each change, it shows where the change is, a link that opens that exact passage in the editor, and one plain-language sentence that tells what changed. It asks the author only for decisions that the author must make: edit, Accept, or Reject a Proposal; approve one exact extra disclosure; and make a recovery or budget choice when work cannot continue without one. When assistance stops, the author sees one plain-language sentence about the outcome and the available next action.

The author surface does not show Run internals. These include Context Assembly sources and decisions, Working Target selection, manifests, wire payloads, sent deltas or full input, opaque references, cache behavior, compaction, recovery classification, successor and retrieval paths, capability gaps, and usage details.

StoryOS continues to record and verify all of these facts. Disclosure evidence, credential references, Attempts, fences, and usage classification stay required by the Stage 4 contract and ADRs 0005, 0033, and 0034. Context Inspect stays a read-only audit query for diagnosis and verification. It is not part of the author writing surface.

The author stated this product decision during Stage 3 review, and the approved concise result presentation (commit `80869849`) applied it to the Stage 3 assistance panel.

## Considered options

- A selected-Run details view that lists context sources and recovery reasons was rejected. Stage 3 had this view and removed it. It moves system complexity to the author, and discovery writing must not require technical judgement from the author.
- Removing the recorded evidence was rejected. Disclosure and credential evidence are trust requirements, and journey verification and diagnosis need them.
- Showing usage after each request was rejected. The author sees usage only when a budget limit stops work or needs an author choice.

## Consequences

- In [Deliver Stage 4: One Authorized Real-Model Journey](https://github.com/FrankQDWang/StoryOS/issues/362), the user stories that asked the author to inspect sources, disclosure, input, compaction, recovery paths, capability gaps, or usage change. StoryOS records and verifies those facts, and the author sees results and required decisions only. Journey verification examines those facts through queries and tests, not through the author surface.
- Each Stage 4 child applies this ADR at its refresh gate before Claim.
- The Glossary entry for Context Inspect describes a read-only audit query, not an author view.
- The unrendered Stage 3 Run details components leave the Protected Web Client.
