# Server source directory checkpoint

All 59 test functions in 11 source test files have manual verdicts. The inventory and verdict (path, name) sets match exactly. The Server integration directory was completed separately. No product or test changes were made.

Source verdicts: {'MOVE': 2, 'DELETE': 26, 'KEEP': 27, 'MERGE': 4}. Immediate candidate source lines: 677. Mutation review and cross-directory reconciliation remain pending.

| Test file | Cases | Verdict counts |
|---|---|---|
| crates/storyos-server/src/author_edit_outcome_tests.rs | 1 | {'MOVE': 1} |
| crates/storyos-server/src/author_edit_tests.rs | 3 | {'MOVE': 1, 'DELETE': 2} |
| crates/storyos-server/src/client_session_binding_tests.rs | 8 | {'DELETE': 5, 'KEEP': 1, 'MERGE': 2} |
| crates/storyos-server/src/contract_reason_tests.rs | 2 | {'DELETE': 2} |
| crates/storyos-server/src/editor_session_tests.rs | 1 | {'DELETE': 1} |
| crates/storyos-server/src/project_command_challenge_tests.rs | 11 | {'DELETE': 3, 'KEEP': 8} |
| crates/storyos-server/src/public_origin_tests.rs | 6 | {'DELETE': 1, 'KEEP': 5} |
| crates/storyos-server/src/request_origin_tests.rs | 12 | {'DELETE': 7, 'MERGE': 2, 'KEEP': 3} |
| crates/storyos-server/src/session_bootstrap_tests.rs | 3 | {'KEEP': 2, 'DELETE': 1} |
| crates/storyos-server/src/web_assets_tests.rs | 4 | {'DELETE': 1, 'KEEP': 3} |
| crates/storyos-server/src/web_host_tests.rs | 8 | {'KEEP': 5, 'DELETE': 3} |

## Module links and helpers

| Source file | Disposition |
|---|---|
| crates/storyos-server/src/author_edit.rs | KEEP product code and live test-module links; no separate removable lines counted. |
| crates/storyos-server/src/author_edit_outcome.rs | KEEP product code and live test-module links; no separate removable lines counted. |
| crates/storyos-server/src/contract_reason.rs | KEEP product code; remove the test-module link only after the all-DELETE file is removed. |
| crates/storyos-server/src/lib.rs | KEEP router and live test links; remove only the editor_session test-module link after that file is removed. |
| crates/storyos-server/src/public_origin.rs | KEEP product code and live test-module links; no separate removable lines counted. |
| crates/storyos-server/src/request_origin.rs | KEEP product code and live test-module links; no separate removable lines counted. |
| crates/storyos-server/src/session_bootstrap.rs | KEEP product code and live test-module links; no separate removable lines counted. |
| crates/storyos-server/src/web_assets.rs | KEEP product code and live test-module links; no separate removable lines counted. |
| crates/storyos-server/src/web_host.rs | KEEP product code and live test-module links; no separate removable lines counted. |

## Reconciliation notes

- Scheme-only Origin difference is a MERGE, not a DELETE: existing HTTP foreign Origin inputs change the host too. Add that independent input before removing the helper assertion.
- Keep exact Retry-After propagation: the HTTP rate test accepts any integer from 1 through 60 and cannot catch a wrong constant within that range.
- Move the unmatched Author Edit target and admitted-unsettled outcome to the public HTTP boundary. The current HTTP damaged Admission fixture resolves as RequiresReconfirmation, not StillUnknown/AdmissionCommitted. Do not cite it as existing coverage of the latter.
- Session binding D5 rows depend on the sole current main builder. Reconcile accepted security contracts before freezing DELETE rows; no claim is made that another test kills those unreachable internal checks.
- Public HTTPS transport is an accepted ADR 0023 contract. Keep its distinct Host/port, Secure-cookie and proxy-header cases. Local HTTP package tests do not prove them.
- Resource validation tests remain because packaged startup negatives can fail on unrelated prerequisites. Positive resource immutability is covered by actual HTTP after root relocation.
- Shared helpers in mixed files remain. Search found no cross-test-module helper imports from the two all-DELETE files. Only those two full files include helper savings.
