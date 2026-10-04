# server-integration test verdicts

Reviewed: 2 cases in 1 files. See PROGRESS.md for directory completion.

Reason codes: [METHOD.md](METHOD.md). Locations use the fixed audit baseline.

## crates/storyos-server/tests/public_origin_startup.rs

| ID | Line / test | Verdict | Reason | Regression and coverage comparison | Covering or compared test |
|---|---|---|---|---|---|---|
| SI001 | 14 — invalid_public_origin_refuses_before_storage | DELETE | D1 | The packaged Node process test executes the same main entry, supplies the same Origin and bind arguments, and asserts refusal before Storage Activation with no readiness marker. It additionally uses a real paired Web root. This Cargo process test uses a missing root but asserts the same Origin error; it adds no input or observation. | apps/web/test/node-contract/protocol-http.integration.test.ts:127 |
| SI002 | 25 — public_origin_refuses_a_non_loopback_listen_before_storage | DELETE | D1 | The packaged Node process test executes the same main entry, supplies the same Origin and bind arguments, and asserts refusal before Storage Activation with no readiness marker. It additionally uses a real paired Web root. This Cargo process test uses a missing root but asserts the same Origin error; it adds no input or observation. | apps/web/test/node-contract/protocol-http.integration.test.ts:137 |
