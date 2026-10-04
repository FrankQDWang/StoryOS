# Contract questions and model corrections

All observations use product base `479224809cdaae997cda51cb8853e3fafa242b65`. A question is not a guessed expectation.

| ID | Contract question | Small replay and observed evidence | Owning source |
| --- | --- | --- | --- |
| A-001 | Archived Project tree-query visibility is not specified. | Seed 10, structure case `archiveProject:authoritative_applied`: create, archive, read Project and tree. Project is readable; tree returns 404. | ADR 0028 lifecycle; `crates/storyos-adapter-postgres/src/manuscript_tree.rs:24-28`. |
| A-002 | Initial Tree/Project/Assistance revision numbers are not stated as a public initialization contract. Project query does not expose Project Revision. | Bootstrap seed 1 reads initial Tree Revision; structure Update Project seeds probe schema-valid positive 1. Proposal seed 300 queries missing assistance, then uses schema-valid 0 to initialize. | ADR 0025, ADR 0028, generated request and Project/Assistance query schemas. |
| A-003 | Priority between empty Project and invalid live Chapter join is not stated. | Seed 10, structure case `setCurrentChapter:refused:empty_project`: create the last Chapter, delete it, attempt to select it. Result is invalid_chapter_join. | Current Chapter contract; Core `set_current_chapter.rs:56-64`. |
| A-004 | Generated revision/order syntax permits zero; Server adds a positive-only parser. The status/result of a schema-valid zero is not clear from the schema alone. | Structure seed 1, case `zero-revision`: create Project, Create Volume with expected Tree Revision 0. HTTP 400. | Generated Create Volume request numeric pattern; `structure_admission.rs:302-307`. |
| A-005 | Direct-edit eligibility after a later compensation is not specified with the precision used for UndoAcceptance. | Edits seed 220: four edits, compensate newest, then Undo the published older frontier using the new current Head. Result is wrong_target_head. | State Machine section 10.1 versus exact-head rules in section 10.2; Core `undo_latest_author_action.rs:134`. |
| A-006 | Ordinary Proposal edit resets validation to pending, but its next public read already contains a fresh valid Validation Receipt. Public timing between edit and separate validation is not specified. | Proposal seed 300 or 310, case `edit`: AgentRun, candidate edit, query. The validation identity changes and the candidate is valid. | State Machine sections 6.4, 7.2, 7.4; Postgres `author_edit_proposal.rs:431-502`. |
| A-008 | Acceptance NoEffect exists in the foundation's exhaustive result but is absent from the generated public effect and reason enums. No public trigger is defined. | Static minimum: compare section 8.1 with `accept-proposal-response.schema.json/$defs/AcceptProposalEffect`. Seed 310 case `accept` supplies a normal successful trace; it cannot establish a missing NoEffect trigger. | State Machine sections 8.1 and 9 allocation matrix; generated Accept Proposal response; Core `accept_proposal.rs:23-28`. |

A-009: the allowed prose sources define an optional Current Chapter and explicit selection, but do not specify all automatic choices after Chapter creation/removal. The model does not guess that policy from Core. Minimal observations: bootstrap seed 1 creates the first Chapter and observes it selected; structure seed 10, case `setCurrentChapter:refused:empty_project`, deletes the last Chapter and observes Empty Project. Owning contract: `GLOSSARY.md:55-57`, storage contract section 0.4. Implementation choices are in `crates/storyos-core/src/create_chapter.rs:115-117` and `delete_chapter.rs:93-114`. Explicit Set Current Chapter outcomes and preservation of tree revision remain checked.

A-007 in early progress was an incorrect inference that Replan had no registered Undo handler. Section 10.1 permits registered exact handlers. It is retired as a model error. The empty authoritative response in that same observation is tracked separately as D-004.

A-008 has no seed capable of selecting its missing branch: inventing one would invent a contract. Its replay is the deterministic schema/prose comparison, with the normal Acceptance trace as context. The benchmark explicitly records this wire reachability limit.

## Corrected model errors

- M-001: assumed initial Tree Revision zero. Read the initial query as an opaque control value and compare later increments.
- M-002: submitted update inputs without required title/order. Supply complete schema-defined input.
- M-003: assumed whitespace title invalid. The contract constrains byte length, not trimming.
- M-004: used an absent random Head to model a stale same-Scope Head. Retain a real prior Head.
- M-005: expected 200 for Create AgentRun. The closed route requires 202.
- M-006: assumed Replan Undo was a Barrier. A registered Proposal compensation handler exists.
- M-007: queried a Refused Edit Draft through the closure route prefix. Use the catalog's Refused Edit query route.
- M-008: treated an omitted optional empty frontier as a required null.
- M-009: sent a Proposal target without its required expected Proposal Head. This fails Admission and cannot test a Core ownership conflict.

Earlier evidence is retained with its original failures. Counted evidence is selected explicitly in `evidence-manifest.json`; old model errors do not become unexplained product differences.

## A-010: Rejection Undo handler availability

Section 10.1 names rejection-to-reopen routing and permits Barrier when no exact handler is registered. It does not expose a versioned per-action registration policy. Seed 355, case `rejection-undo`, rejects one pending Operation and submits Undo with the published exact frontier and Head. It returns unavailable/barrier and preserves rejection. This is recorded as an availability ambiguity, not a guessed mandatory compensation failure. The fallback is in `crates/storyos-adapter-postgres/src/undo_frontier.rs:185-203`; the supported handler lookup in `author_edit_proposal.rs:539-592` requires a parent Revision. The generated query/response exposes no handler registration field. Evidence: `rejection-undo-355.json.gz`.

Additional corrected probes: the ordered-source conflict assertion omitted its required current Head; a withdrawn Operation still held its unresolved reservation; a structured Draft-derived candidate did not support the legacy plain candidate-edit shape. Later probes use complete response objects, rejection to release reservations, and withdrawal compensation to change the derived Head.
