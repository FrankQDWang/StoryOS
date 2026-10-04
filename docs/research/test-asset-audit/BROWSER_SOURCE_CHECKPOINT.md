# Browser source checkpoint

Source review is partial: 77 runtime cases in 30 test files. No mutation samples selected.

| Reviewed file | Cases | Verdict counts |
|---|---|---|
| apps/web/test/browser-source/accept-block-proposal.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/acceptance-journal.integration.test.ts | 1 | {'MERGE': 1} |
| apps/web/test/browser-source/activity-reorder.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/activity-resync.integration.test.ts | 17 | {'DELETE': 3, 'KEEP': 14} |
| apps/web/test/browser-source/activity-stream-consume.integration.test.ts | 4 | {'KEEP': 3, 'MERGE': 1} |
| apps/web/test/browser-source/archive-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/author-edit-outcome.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/author-edit-rate-limit.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/author-undo-rate-limit.integration.test.ts | 7 | {'KEEP': 6, 'MERGE': 1} |
| apps/web/test/browser-source/browser-command-foundation.test.ts | 2 | {'DELETE': 1, 'MOVE': 1} |
| apps/web/test/browser-source/chapter-navigation.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/create-historical-acknowledgement.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/current-chapter-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/delete-historical-acknowledgement.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/draft-undo-lifetime.integration.test.ts | 4 | {'DELETE': 1, 'KEEP': 3} |
| apps/web/test/browser-source/journal-gc-fenced.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/journal-gc.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/journal-working-set.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/list-open.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/local-edit-journal-append-drift.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/local-edit-journal-append-projection.integration.test.ts | 5 | {'KEEP': 5} |
| apps/web/test/browser-source/local-recovery-panel.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/manuscript-tiptap-adapter.integration.test.ts | 8 | {'KEEP': 6, 'DELETE': 2} |
| apps/web/test/browser-source/project-entry.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/readable-export-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/refused-edit-discard.integration.test.ts | 2 | {'KEEP': 2} |
| apps/web/test/browser-source/rename-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/takeover-late-result.integration.test.ts | 1 | {'MERGE': 1} |
| apps/web/test/browser-source/undo-historical-acknowledgement.integration.test.ts | 1 | {'KEEP': 1} |
| apps/web/test/browser-source/update-historical-acknowledgement.integration.test.ts | 2 | {'KEEP': 2} |

## Pending files

- apps/web/test/browser-source/acknowledgement-loss.integration.test.ts
- apps/web/test/browser-source/editor-session.integration.test.ts
- apps/web/test/browser-source/manual-input.integration.test.ts
- apps/web/test/browser-source/reload-recovery.integration.test.ts
- apps/web/test/browser-source/scenario.ts

## Evidence and execution dependencies

- BS001-BS011 cover eight historical-acknowledgement files. Unlike the shared Server structural replay reader, each browser command has its own retry map/catch and each form has a separate error-state owner. Keep actual UI/no-retry behaviors; mocked 409 responses do not prove database history handling.
- Archive historical acknowledgement calls the wrapper directly. Its title claims an explanation, but it does not mount UI or assert text. Retained protection is one PUT/Challenge and historical error classification. Delete Volume preserves the Project title but does not explicitly compare the Volume row.
- Chapter creation holds its request while another creation is attempted, then checks the same read-only input/title/Volume remains connected. Ordinary creation success lacks this controlled interleaving.
- BS012 keeps execution of the generated outcome proof-header request: no nonce in URL/body, exact GET, same-origin credentials and required proof. CT010 names it as the executable owner; static contract shape is not equivalent.
- BS013 keeps Acceptance delivery classification: exact retry after unknown/unreadable output, competing decision refusal, recovered persisted in-flight Attempt, terminal pre-admission refusal, and later authentication failure that cannot resolve an earlier unknown delivery. IndexedDB/mocked delivery differs from real HTTP refusal storage.
- BS014 MERGE moves valid flight sequence/record/group and pending-projection assertions into BS013 before successful retry. Fabricated foreign/key arguments to createFlight are not stored-partition drift: both product callers construct them from the same workspace. Reconcile accepted persistence/seam contracts before any broader D5 inference.
- BS015 DELETE removes generic input/clipboard smoke. Packaged input at s2-input:284 invokes those commands and observes saved manuscript changes. Cookie action=set is used only by the foundation test; actual journeys use another session setup and invoke this command only to clear it. A mutation confined to that unused helper action is not claimed caught by the packaged journey. This D4 limitation remains in the random sample population.
- BS016 MOVE keeps malformed production-command rejection at the pure Node parser layer; proposed destination: apps/web/test/node-contract/browser-command-contract.test.ts. No destination was created. The privileged browser-command dispatcher invokes this parser; valid journeys do not cover malformed inputs.
- BS017-BS019 keep stale library-title replacement, pending Chapter text across inspection/query failure, and invalid URL entry before requests. Packaged library/navigation use matching live titles and empty Chapters. Ordinary heading/query-order/credential checks can be trimmed while preserving unique assertions.
- Browser declaration and citation starts checked against fixed source. Current totals: 16 KEEP, one DELETE, one MERGE, one MOVE; 40 immediate candidate lines. No runtime test, product edit, source mutation or database command occurred.

## Challenge waits, Activity, Takeover and Draft lifecycle

- BS020-BS056 add 37 runtime cases across seven files: Author Edit rate-limit two, Author Undo rate-limit seven, Activity reorder one, Activity resync 17, Activity SSE consumption four, Draft Undo lifetime four, and Discard two. JSON expands all test.each parameters; internal loops remain in their owning runtime test.
- Author Edit and Undo use the same ChallengeAdmissionWait helper but own separate persistence, projection and cancellation consumers. Keep consumer-level saving/retry-key and close behavior. MERGE repeated Ctrl/Cmd+Z exclusion into the successful Undo wait case before its Challenge release and during its retry wait.
- Keep the three Undo timing cuts: new input during an existing timer; successful Challenge returning after abandonment; and an already sent Undo returning after new input. A late 429 after workspace replacement must schedule no new timer or saving state and must release the keypress latch.
- Keep Activity gap holding/duplicate convergence separately from SSE cursor consumption. DELETE activity-resync:48: the retained consumer invokes the same helper with the same generation/position transition and requires unchanged Journal plus correct resumed ingest. Its no-Activity-request assertion describes the helper step, not a separate public outcome.
- Takeover valid, valid_other_session and valid_activity_advance differ: same-Session versus retained-other-Session binding reconstruction, and a canonical/Chapter position later than the writer base. The latter returns early after ingest/reload checks and does not prove the full old-evidence/new-input sequence of the first two.
- DELETE position_before_base and position_invalid. Both also disagree with the Chapter position, so they can still return read-only if the intended lower-bound/overflow guard is removed. position_after_chapter owns that observed mismatch. A discriminating lower-bound test would set both canonical and Chapter positions below base; this input is currently absent. No overflow-only kill is claimed.
- Keep independent Session scope, locator scope, requested Session identity, canonical generation, retained binding, reused base, payload digest, returned Snapshot identity, expiry and second-read drift inputs. The expiry check runs during staged atomic ingest installation. All compare unchanged stored data after refusal.
- MERGE the Activity foreign-locator scenario into stream-consume:105 with a specific Canonical Snapshot invalid assertion and complete downstream fixture responses. Its existing broad rejects.toThrow can pass on an unconfigured Snapshot request after the intended scope guard is removed. This is an oracle defect, not proved product isolation.
- Keep unknown Activity rejection and unchanged Last-Event-ID. The positive Assistance/Run setup overlaps the packaged input journey, but the latter never emits an unknown kind. This source test proves another edit can be persisted, not that an actual rendered control remains enabled.
- DELETE the Draft Undo unmount parameter in favor of workspace replacement: both invalidate the same lifetime effect while a source GET is held; replacement additionally protects a live successor. Ordinary Undo's unmount parameter separately owns timer cleanup. Keep Draft Undo schema drift and stored secret contamination because they reach distinct Journal phases/parsers.
- Discard tests retain nonce-free freeze before Challenge, digest-time abandonment, exact public-event reconciliation, monotone settled observation despite stale unresolved observations, and Receipt-backed refusal/conflict settlement. Database Discard tests cannot observe these browser Journal decisions.
- Current browser-source total: 56 cases / 22 files; 47 KEEP, five DELETE, three MERGE, one MOVE; 188 immediate candidate lines. All declaration lines and parameter multiplicities checked. No runtime execution, mutation, product/test/generated changes or active process.

## Journal append, retirement and mounted input

- BS057-BS077 add 21 cases in eight files. Two more support files have dispositions in SUPPORT.md. The digest budget wraps workspace validation Crypto, not the separate Crypto parameter used for new input hashes; it proves bounded validation work, not measured latency.
- Keep the digest-time changed-history refusal and Project allocator advancement separately. The latter must link sequence 3 to this partition's sequence 1 and freeze noncontiguous coverage correctly.
- Keep mixed Draft Copy/Discard scope changes, mixed IME cancellation/confirmation, the first render after earlier Edit settlement, and background refresh while a new append is still hashing. Each has a different captured-input or component-lifetime gap.
- Keep active and fenced partition collection plus the 2400-intent retirement/migration case. The latter checks actual working-index retirement and atomic aborts, not merely a long happy path. The v3 helper builds an older layout from current records.
- MERGE takeover-late-result:37 into journal-gc-fenced:43 before collection, transferring raw partition and full validated group/coverage assertions. Both use the same acknowledgement-loss/new-writer Outcome sequence; no separate race barrier distinguishes them.
- DELETE adapter Backspace join in favor of the packaged split/join journey and simple captured suffix insertion in favor of mounted append projection. Keep middle split, selected-text split, Shift+Enter, unsupported transactions, backward mixed heading selection and open-Slice/CRLF paste. Packaged end-of-text split cannot detect losing an existing right suffix.
- Keep both local recovery panel parameters: authoritative two-Block text and Proposal candidate text have different projection owners. Copy is a mocked destination; explicit continuation and reopened IndexedDB state are observed. No later server write is claimed.
- Current browser-source total: 77 cases / 30 files; 65 KEEP, seven DELETE, four MERGE, one MOVE; 232 immediate candidate lines. All declaration lines and parameter multiplicities checked. No runtime execution, mutation or product/test/generated change.
