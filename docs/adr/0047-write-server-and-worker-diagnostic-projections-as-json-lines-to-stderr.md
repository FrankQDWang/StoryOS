---
status: accepted
---

# Write Server and Worker Diagnostic Projections as JSON Lines to Stderr

On 2026-10-09 the author accepted this decision for [Decide the Observability of the Server and the Worker Before the Tracing Rule Applies](https://github.com/FrankQDWang/StoryOS/issues/1053). The Server and the Worker use `tracing`. Each binary installs one `tracing-subscriber` formatter that writes JSON lines to stderr, and no exporter. Spans go only on the boundary functions in this record. Each span field and each event field is a safe identifier, a static category, or a counter. The Observability rule of `CODING_STANDARDS.md` stays, and this record gives its scope.

## Context

At `main` `2f2c0127`, no workspace crate depends on `tracing`, and no binary installs a subscriber. `tracing` is in `Cargo.lock` only through axum and tower. The Worker drops each claim error with `Err(_) => {}`. The Server drops the cause of each `503 Service Unavailable` response. Thus an operator has no record of a failure.

These contracts already apply:

- Local logs and tracing are Diagnostic Projections (retention contract, section 9). They contain only safe correlation identifiers, categories, reason codes, times, counters, and availability facts. They contain no prose, prompt, raw Provider payload, Credential, or credential value digest.
- Telemetry sent beyond the StoryOS Controlled Processing Boundary is a Telemetry Disclosure. A loopback address does not put a processor inside that boundary.
- AP-11 of the threat model turns off SQL binds, HTTP bodies, prompts, authorization headers, secret-bearing URLs, and Provider wire payloads by default. Debug mode cannot weaken these rules. A canary corpus is its verifiable evidence.

Most domain types derive `Debug`, and many hold author text, for example `ManuscriptBlock.text`, `CreateRequest.author_message`, and `ProjectCommandEnvelope.canonical_command_bytes`. The default argument capture of `#[tracing::instrument]` records each argument with `Debug`. Thus the default capture leaks author text.

## Decision

### Destination

- Each of the two binaries, `storyos-server` and `storyos-worker`, installs one formatter that writes JSON lines to stderr.
- Stdout keeps only the `STORYOS_SERVER_URL=` startup line. Tests and scripts find the Server through this line.
- StoryOS installs no OTLP exporter, no collector, and no hosted vendor. A later exporter is a Telemetry Disclosure and needs its own ADR.
- On the Linux VPS of [ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md), the process manager keeps stderr on the host. That process manager owns the Diagnostic Projection retention. The `PER-008` tuning item owns the window, and deployment preparation sets it.
- A configuration refusal at startup keeps its current stderr text. A runtime error that stops a binary writes one `error` event with the stage and the SQLSTATE code, and no error text.

### Configuration and levels

- The `STORYOS_LOG` environment variable selects one level: `off`, `error`, `warn`, `info`, or `debug`. The default is `info`. A binary refuses to start when the value is not in this set.
- The filter passes events of `storyos` targets only. It turns off each third-party target, for example axum, hyper, and tower. StoryOS installs no `log` bridge, so `tokio-postgres` writes nothing.
- `info`: one close event for each HTTP request span, command sequence span, Worker claim span, and Model Gateway dispatch span. The close event includes the duration.
- `warn`: a failure that StoryOS handles, for example a Worker retry or a `503` response.
- `error`: an exit of the Worker loop and a panic.
- `debug`: the spans of the port implementations.
- StoryOS does not sample.

### Boundary functions

Only these async functions get `#[tracing::instrument(skip_all, ...)]`:

1. **HTTP request.** One axum middleware function makes one span for each request. It records the route template from `MatchedPath`, or `unmatched`. It never records the raw path or the query, because a search query can contain author text.
2. **Command sequence.** The entry functions of `command_sequence` in `storyos-adapter-postgres`: settle, admit, settle the admitted command, and replay.
3. **Worker claims.** The Worker step, the completion of a readable export and of an archive export, and `complete_agent_run`.
4. **Model Gateway.** `Gateway::dispatch` at `info`. At `debug`, the implementations of `prepare`, `exchange`, `commit_dispatch_claim`, `record`, and the append of Model Stream Events.

The other async functions get no span. The 466 internal functions of `storyos-adapter-postgres` are in this group.

### Fields

Each layer records only the fields in this table:

| Layer | Fields |
| --- | --- |
| HTTP request | `method`, `route`, `status` |
| Command sequence | `command_kind`, `project_id`, `command_id`, `author_command_admission_id`, `correlation_id`, `outcome` |
| Worker claim | `project_id`, `run_id` or `export_id`, `fence_token`, `attempt`, `outcome` |
| Model Gateway | `project_id`, `run_id`, `request_kind`, `adapter`, `observation`, usage counters |

- `outcome` is a static category: the name of the applied variant, `replayed`, or a refusal reason code.
- `correlation_id` is the client UUID that the Server validates.
- No span records `owner_user_id`. `project_id` identifies the Project Scope.
- No span records a digest, an idempotency key, a nonce, a Credential Reference, or a credential value.
- A new field changes this table and the rule in `CODING_STANDARDS.md`.

### Error events

- At each place that drops an error today, StoryOS writes one `warn` or `error` event.
- The event records only a reason code: the static category of the error variant. For a PostgreSQL error, it also records the SQLSTATE code.
- No event records the `Display` or `Debug` text of an error. A `tokio-postgres` message can contain bind values, for example the detail of a unique violation.
- The Server watches the handle of its in-process Worker loop. When the loop exits or panics, the Server writes one `error` event.
- Each binary installs a panic hook in place of the default hook. The hook writes one `error` event with the source location only. It does not write the panic message, because the message of `unwrap` contains the `Debug` text of the error.

### Enforcement

- Each field value implements one trait, `DiagnosticField`. Only identifiers, static categories, counters, and SQLSTATE codes implement it. Thus the compiler refuses a field of a type that holds author text.
- A static guard in `make verify-policy` refuses `#[tracing::instrument]` without `skip_all`. It also refuses a `?` Debug capture in a `tracing` macro, and a field value that is not a literal or a `DiagnosticField` value.
- A canary corpus test starts the packaged Server and Worker at `debug` and keeps all stderr. Unique canary values go into manuscript text, an `author_message`, a search query, the challenge secret, and the database URL. The requests go through success, refusal, exact retry, and Worker claim paths. The test asserts that no canary value is in stderr. It also asserts that the expected spans are in stderr, so that an empty output cannot pass.

### Tests and verification

- Rust tests install no subscriber. Spans do nothing there, and the test output does not change.
- Node and shell tests start the packaged binaries without `STORYOS_LOG`. Thus they run at `info`. The existing `STORYOS_SERVER_URL=` matches do not change.
- Only the canary corpus test uses `debug`. It is in the existing `node-postgresql` suite. No new verification stage is necessary.
- Verification observation under [ADR 0037](0037-keep-verification-execution-local-and-observe-cost-separately.md) does not read product logs.

## Considered options

- OTLP to a collector on the host was rejected. The collector is a Telemetry Disclosure, even on a loopback address. It needs manifest-before-egress evidence and a vendor or collector decision. A single author does not need it now.
- A human-readable format for local development was rejected. It adds a configuration contract, and `jq` can read JSON lines.
- A span on each of the 566 async functions was rejected. It is a large mechanical change that conflicts with parallel work, and it adds output without information.
- An open field set that review alone controls was rejected. Review cannot prove that a new field holds no author text.
- A sanitized error message beside the reason code was rejected. The sanitizer is a leak risk, and no test can prove it safe.
- Removal of the rule from `CODING_STANDARDS.md` was rejected. The author selected the rule and its application on 2026-10-09.

## Consequences

- The workspace gets direct `tracing` and `tracing-subscriber` dependencies. No package gets an OpenTelemetry dependency.
- The Observability rule of `CODING_STANDARDS.md` names the boundary functions, `skip_all`, and `DiagnosticField`. A Standards review applies it after this specification merges.
- The Provider credential is not in the canary corpus, because no adapter resolves a credential yet. The ticket that adds the Volcengine adapter or a credential resolver adds that canary.
- The `PER-008` retention window and the process manager configuration on the VPS stay with deployment preparation.
