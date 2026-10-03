---
status: accepted
---

# Show Assistance Results, Not Run Internals, to the Author

On 2026-10-03, before Stage 4 execution, the author accepted this decision. It applies to every author-facing view of the Protected Web Client.

After assistance, the author sees the result. For each change, the author sees where the change is, a link that opens that exact passage in the editor, and one plain-language sentence that tells what changed. The author can inspect, edit, Accept, or Reject each Proposal in the editor, as GOAL.md requires.

The author is asked only for decisions that the author must make:

- edit, Accept, or Reject a Proposal;
- approve one exact extra disclosure;
- approve a governed action when its owning contract requires author approval, for example a Tool Approval under [ADR 0034](0034-bound-provider-hosted-tool-operations.md);
- make a recovery or budget choice when work cannot continue without one.

When assistance stops, the author sees one plain-language sentence about the outcome and the available next action. When a missing capability or a budget limit stops the work, that sentence tells the author so.

No author-facing view shows Run internals. Run internals include Context Assembly sources and decisions, Working Target selection, manifests, wire payloads, sent deltas or full input, opaque references, cache behavior, compaction, recovery classification, successor and retrieval paths, capability gap details, and usage details.

StoryOS continues to record and verify all of these facts. Disclosure evidence, credential references, Attempts, fences, and usage classification stay required by S4-REQ-003, [ADR 0005](0005-require-ordered-context-assembly-before-destination-disclosure.md), [ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md), and ADR 0034. Context Inspect stays a read-only audit query for diagnosis and verification. The Protected Web Client does not show it to the author.

This ADR revises one consequence of ADR 0005. ADR 0005 gives the author "on-demand inspection and simple controls". After this ADR, the author keeps the zero-configuration experience and simple controls. On-demand inspection of context and Run evidence becomes a read-only audit query, not an author view.

The author stated this product decision during Stage 3 review. The approved concise result presentation (commit `80869849`) applied it to the Stage 3 assistance panel.

## Considered options

- A selected-Run details view that lists context sources and recovery reasons was rejected. Stage 3 had this view and removed it. It moves system complexity to the author. Discovery writing must not require technical judgement from the author.
- A separate author view, outside the writing workspace, for Run internals was rejected for the same reason.
- Removing the recorded evidence was rejected. Disclosure and credential evidence are trust requirements. Journey verification and diagnosis need them.
- Showing usage after each request was rejected. The author sees usage only when a budget limit stops work or needs an author choice.

## Consequences

- In [Deliver Stage 4: One Authorized Real-Model Journey](https://github.com/FrankQDWang/StoryOS/issues/362), the user stories that asked the author to inspect sources, disclosure, input, compaction, recovery paths, capability gaps, or usage change. StoryOS records and verifies those facts. The author sees results and required decisions only. Journey verification examines those facts through queries and tests, not through an author-facing view.
- Each Stage 4 child applies this ADR at its refresh gate before Claim.
- The Glossary entries for Context Inspect and Default Context Experience follow this ADR. The S4-JRN-001 journey in the AI-independent editor-first release baseline follows this ADR.
- `apps/web/src/assistant-run-details.tsx` and `apps/web/src/assistant-run-evidence.tsx`, which no view renders, leave the Protected Web Client.

## Pending downstream alignment

The Context owner must revise these contracts before the first Stage 4 child Claim:

- Section 11, "Author inspection and controls", of the Context Assembly, Retrieval, and Outbound Disclosure Semantics contract: Context Inspect is a read-only audit query, not an author view.
- Section 11, "Author inspection and history availability", and RET-012 of the Run Event, Mailbox, Snapshot, Retention, and Archival Semantics contract: the inspection Query serves audit and verification.
- The "author inspection/control semantics" responsibility of the Context, Memory, and Research module in the Modular Monolith and Repository Governance Boundaries contract.
