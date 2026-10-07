# Glossary: Deterministic verification

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Deterministic Verification Boundary**:
The boundary of facts an implementation gate may establish reproducibly: given synthetic scoped inputs, a scripted fake destination, and a deterministic fault schedule, it proves only StoryOS-local admission, state transitions, transaction atomicity, scope and redaction enforcement, durable evidence, and recovery classification. A fake may simulate destination observations such as receipt, delay, duplicate delivery, timeout, or crash cut, but it never proves a Provider's internal handling, a model's semantic understanding or creative quality, or that an opaque external system will rerun identically. A real Provider call may create explicitly scoped advisory evidence, but it is not a deterministic implementation gate.
_Avoid_: Provider-internal proof, creative-quality assertion, real-provider CI gate, simulated receipt as external fact

**Deterministic Verification Oracle**:
A small, independently maintained executable state model that consumes a synthetic operation trace and deterministic fault schedule and defines the permitted durable StoryOS facts after execution or recovery. It models authoritative state, Receipts, Events, Attempts, fences, mailbox obligations, and explicit uncertainty only at their contract boundary; generated state-machine and property traces compare the recovered system against that permitted result set, while curated end-to-end scenarios remain readable examples. It does not mirror production storage, worker, cache, or adapter implementation, and it does not resolve opaque external truth hidden behind OutcomeUnknown.
_Avoid_: Mock call expectation, duplicated production runtime, happy-path-only suite, inferred Provider outcome

**OutcomeUnknown**:
The conservative durable settlement after StoryOS has crossed a local authoritative or external-dispatch boundary but cannot prove whether the corresponding external or authoritative effect occurred. It is neither success nor ordinary failure: timeout, lease expiry, process death, lost connection, and a fake destination's hidden state cannot collapse it. A verified pre-dispatch failure remains distinct; an unknown result may settle only when immutable reconciliation evidence enters through the ordinary applicable StoryOS boundary, and any successor requires the existing live revalidation, fresh Attempt, disclosure, and budget rules.
_Avoid_: Timeout as failure, harness-only fake receipt, inferred non-submission, blind retry, zero usage

**Contract Fault Point**:
A stable named deterministic test boundary at one durable or externally irreversible contract transition. Its declaration identifies the fault cut, the exact recoverable evidence boundary, and the permitted recovered state classification; a new durable commit, dispatch claim, seal or fence, replay handoff, or recovery-visibility boundary cannot become implementation-complete without one. It deliberately does not expose or prescribe individual SQL statements, function calls, queue internals, or storage write order.
_Avoid_: SQL-line crash hook, random process kill, implementation-coupled test, inferred recovery state

**Contract-Faithful Fake Destination**:
A deterministic destination that stands in for one External Processing Destination by replacing only nondeterministic transport and scripted observations. Stage 3 uses it as the product model path and deterministic gates use it as a proof fixture; it remains subject to the same Host admission, Context Assembly, Destination Context Manifest, Outbound Disclosure Manifest, Destination Attempt, Adapter mapping, and later-context boundaries, and a Worker dispatch claim creates an Outbound Disclosure Event without proving external-destination receipt.
_Avoid_: Provider, second author-facing route, Host Internal Processing, Core shortcut mock, direct Agent Decision injection, bypassed manifest, fake as authority, fake as no-disclosure, external-destination receipt claim, Provider-internal fact from a fake observation

**Multi-Scope Adversarial Verification World**:
A deterministic test world containing at least two distinct Users and Project Scopes, with independently scoped sessions, identities, Artifacts, Attempts, cursors, grants, Credential bindings, manifests, and disposable projections. Its generated traces deliberately substitute same-shaped foreign references at every scope-sensitive join and assert non-oracular denial, zero unauthorized effect, and absent cross-scope context or egress. A single-Project test may illustrate normal behavior but cannot prove Project Isolation.
_Avoid_: One-Project isolation proof, UUID uniqueness as scope, client-side filter test, positive-path-only corpus

**Negative Evidence Closure**:
The deterministic security-gate rule that a hostile or cross-Scope input proves more than a rejected request: it leaves no unauthorized authoritative change, external disclosure, Attempt, budget consumption, or other effect; its public result remains non-oracular; and its emitted logs, traces, wire records, archives, projections, and test evidence contain no secret or undeclared foreign-Scope identity. Source inspection alone cannot establish this closure; the gate examines the actual emitted test artifacts.
_Avoid_: Status-code-only denial, debug-log leak, source-scan-only proof, hidden foreign identifier

**Verification Evidence Bundle**:
The non-secret, reproducible output of one deterministic gate, containing its synthetic fixture or seed, exact contract and profile revisions, fake-destination script, Contract Fault Points, expected oracle classification, observed safe durable-fact digests, and sanitized egress and diagnostic summary. Both a pass and a failure validate this shape; a counterexample can be rerun from the Bundle without retaining real novel payload, Credential material, raw transport traffic, product-runtime telemetry, or an undeclared foreign-Scope identity. Declared synthetic fixture identities remain safe test inputs rather than product data.
_Avoid_: Passed-only log line, raw crash dump, unreproducible randomized failure, production telemetry record

**Foundation Contract Walk**:
A small synthetic end-to-end conformance path that crosses selected accepted StoryOS boundaries and compares its durable facts with the Deterministic Verification Oracle. A walk may cover editor input, Run and fake destination use, Proposal and Acceptance, replay, crash recovery, or adversarial denial, but it is not a realistic-product mega-test and does not choose the editor-first release stage, UI scope, or handoff criteria owned by the release baseline.
_Avoid_: Product-slice selection, monolithic UI journey, isolated unit test as end-to-end proof, real Provider dependency

**Fail-Closed Verification Gate**:
A required deterministic implementation gate whose oracle mismatch, missing or unreplayable Verification Evidence Bundle, unrun case, or unclassified Contract Fault Point is an unverified or failed result that blocks the claimed contract implementation. Repeated execution, quarantine, or an observational label cannot turn it green. Real-Provider observations, creative-quality assessment, and empirical performance work may remain advisory evidence, but none may substitute for a failed or missing gate.
_Avoid_: Flaky-test waiver, retry-until-green, silent skip, advisory result as conformance proof

**Deterministic Test Scheduler**:
The controlled virtual monotonic clock and explicit interleaving schedule used by deterministic state-machine, property, fault, mailbox, lease, fence, retry, and timeout gates. Time advances only through the recorded test schedule and is included in the Verification Evidence Bundle, so a seed reproduces the same recovery boundary. Real wall-clock sleeps, thread races, and network timing cannot determine a gate result; separately required recovery drills may measure their declared service profile without becoming synthetic performance thresholds.
_Avoid_: Sleep-based test, scheduler race, accidental timeout, throughput benchmark as semantic gate
