# Outcome reachability

Counts come from public HTTP calls in the explicitly listed corpus, not source instrumentation. A reason with zero hits is not silently treated as covered.

| ID | Unreached outcome | Why this public setup cannot select it | Contract and owning code |
| --- | --- | --- | --- |
| R-001 | Structure invalid_title (four commands) | Empty/over-limit titles are refused by the Server before Core; Server and Core use the same byte limits. Public refusal is sampled. | Generated title constraints; Server `structure_admission.rs:310-316`; Core structure title classifiers. |
| R-002 | Set Current Chapter empty_project | An empty Project has no live same-Scope Chapter target. Invalid join is tested before the empty-current branch. | A-003; Core `set_current_chapter.rs:56-64`. |
| R-003 | Undo source_unavailable | Requires missing/unusable prior Acceptance evidence or a no-longer-retained Draft source. The permitted fresh HTTP histories retain those immutable records. No implemented author purge endpoint can remove them, and the harness does not corrupt storage. | State Machine sections 4 and 10.2; Core `undo_latest_author_action.rs:121-130`; Postgres `undo_draft_close.rs:55-58`, `undo_acceptance.rs:170`. |
| R-004 | Proposal wrong_scope and wrong_admission (six commands) | The Server creates Admission and the scoped load rejects foreign/missing records before Core. Clients cannot supply an Admission object. The adapter passes true only after these proofs. | Admission section 3; State Machine section 2.2; Postgres `accept_proposal.rs:245-269` and corresponding lifecycle admission owners. |
| R-005 | Accept altered_candidate | Candidate content is immutable per Revision. Public edits create a new Revision and fresh validation. The candidate/receipt mismatch requires corruption or a producer violating that immutable contract, not a legal author command sequence. | State Machine sections 4, 7.2, 7.4; Postgres `accept_proposal.rs:277-288`. |
| R-006 | Withdraw/Reopen terminal_supersession | The planning catalog names Supersede Proposal, but this fixed Server/OpenAPI has no implemented Supersede author route. The named command set cannot create the terminal state. | State Machine sections 6.4 and 7.6; catalog `supersedeProposal`; current generated OpenAPI; Postgres `withdraw_proposal.rs:155` and `reopen_withdrawn_proposal.rs:91`. |
| R-007 | Reopen closure_not_withdrawn | A matching withdrawal event must belong to the current Revision. With that event, the Revision is withdrawn; reopening appends a new Revision, so another request fails the event match first. | New-Revision reopen contract; Postgres `reopen_withdrawn_proposal.rs:168-174`; Core `reopen_withdrawn_proposal.rs:71-78`. |
| R-008 | Writer takeover_compare_failed (two reasons) | Current PostgreSQL implementation admits and applies takeover in one serialized transaction. Stale generation/current-requester checks refuse before Admission; no public pause point selects a post-Admission compare failure. | Editor session contract section 2 writer takeover; Postgres `takeover.rs:15-45,60-137`. |

These are reachability limits at the fixed product base and permitted public setup, not statements that the enum cases can never occur in a later implementation. No unsupported SQL writes, test hooks, clock overrides, or extra credentials were used to force a count.

The foundation also declares Acceptance NoEffect while the generated HTTP effect lacks it. A-008 records this contract contradiction; there is no defined input that an independent model can invent to select the missing wire result.

A ProposalRevised StructuralReshapeConflict is a further foundation-level sub-outcome. Current explicit Proposal targets admit only a single ReplaceSelection; the versioned structured-edit path has no ProposalRevised result. Its reachability probe and source limit are recorded separately in the final ledger. This does not turn a plain candidate edit into a structural reshape.

Support commands initialize assistance, Editor Sessions, and retained Drafts. Their used positive paths and exact retries are retained. The primary denominator is the command set in the requested four stages, expanded to every generated effect and typed reason; it is not a claim about every other Release 1 command or the Cartesian product of all optional DraftRetry provenance metadata.
