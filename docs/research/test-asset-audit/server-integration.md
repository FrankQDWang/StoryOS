# Server integration tests

Scope: all of `crates/storyos-server/tests` at the fixed baseline. Two cases in one file. Source review complete; mutation confirmation pending.

Reason codes: [METHOD.md](METHOD.md). Public boundary: paired Server process startup.

## crates/storyos-server/tests/public_origin_startup.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering test |
|---|---|---|---|---|---|
| SI001 | 14 — invalid_public_origin_refuses_before_storage | DELETE | D1 | The packaged Node process test executes the same main entry, supplies the same Origin and bind arguments, and asserts refusal before Storage Activation with no readiness marker. It additionally uses a real paired Web root. This Cargo process test uses a missing root but asserts the same Origin error; it adds no input or observation. | apps/web/test/node-contract/protocol-http.integration.test.ts:127 |
| SI002 | 25 — public_origin_refuses_a_non_loopback_listen_before_storage | DELETE | D1 | The packaged Node process test executes the same main entry, supplies the same Origin and bind arguments, and asserts refusal before Storage Activation with no readiness marker. It additionally uses a real paired Web root. This Cargo process test uses a missing root but asserts the same Origin error; it adds no input or observation. | apps/web/test/node-contract/protocol-http.integration.test.ts:137 |

Immediate recommendation: remove 33 lines (whole file, including the helper). The two rows share this span; count it once. Before removal, update test inventory ownership as required by the verification guide. No test has been changed.
