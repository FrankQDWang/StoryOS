# Protected Web Client security-boundary primary sources

- Audited: 2026-07-24
- Scope: primary-source evidence for the Release 1 Protected Web Client gaps in
  [Threat-Model the StoryOS Service, Client, and External Trust Boundaries](https://github.com/FrankQDWang/StoryOS/issues/57)
- Evidence policy: authoritative Web specifications, official browser/platform
  documentation, and directly necessary official package-ecosystem
  documentation only
- Authority: research evidence only; this note neither changes StoryOS domain
  classifications nor selects a frontend implementation, browser support
  matrix, service-worker strategy, dependency manager, or endpoint-management
  policy

## Evidence boundary and existing material reused

This note uses four labels:

- **External fact** means the cited source specifies or documents the behavior.
- **StoryOS fact** means a current tracked contract already fixes the behavior.
- **Research inference** means a credible source-to-sink consequence follows
  from the external and StoryOS facts, but is not itself a Web-platform rule.
- **Unselected choice** means primary sources cannot choose the StoryOS product
  or deployment policy.

The current
[service/client/external threat model](../foundation/storyos-service-client-external-trust-boundaries-threat-model.md)
already covers Origin/Host/CSRF, exact Project Scope, command-digest admission,
Tool/MCP/App and external-disclosure boundaries, credentials, replay,
`OutcomeUnknown`, and durable non-browser authority. This note does not
duplicate those analyses.

The retained primary-source research for
[Research Trustworthy Browser Author-Intent Attestation Boundaries](https://github.com/FrankQDWang/StoryOS/issues/67)
already establishes the physical-human limit:
[Browser author-intent attestation boundaries at retained commit `74ddfda`](https://github.com/FrankQDWang/StoryOS/blob/74ddfda4295ceef39aa82e687810cb623e513edd/docs/research/browser-author-intent-attestation-boundaries.md).
It shows why user activation, a session, an exact digest, WebCrypto, or plain
WebAuthn cannot by themselves prove that a physical person saw and approved
the semantic command when the first-party page is compromised. The present
note reuses that conclusion in PC-06 and does not repeat its full mechanism
survey.

Current contracts already assign the consumer boundary to
[Author Command Admission](../foundation/author-command-admission.md),
[Web Editor Session](../foundation/web-editor-session-synchronization-and-recovery-semantics.md),
and the
[Deterministic Verification and Failure-Recovery Gates](../foundation/deterministic-verification-and-failure-recovery-gates.md).
The fresh evidence is limited to XSS, third-party/dependency compromise,
extensions, cache/stale-build behavior, stale tabs, and takeover.

## Executive finding

**Research inference.** A first-party Origin or valid browser session is not an
integrity statement about the JavaScript constructing a StoryOS command. CSP,
Trusted Types, SRI, dependency locking, provenance, and browser updates reduce
specific entry paths for unreviewed or stale code; none proves accepted code is
benign. Extensions can share the DOM or inject into the page, and old tabs can
retain loaded code and state. Server-side exact-build/policy/session/writer and
command fencing is therefore structural; browser-local state is evidence, not
authority.

The strongest Release 1 statement remains exactly the current StoryOS one:

> The Protected Web Client submitted the exact digest-covered command that the Server admitted for its Server-derived User and Project Scope.

It must not be expanded into proof of a physical author gesture, trusted
display, user presence, or user verification.

## Source-to-claim matrix

| Evidence | Source fact the root threat model may rely on | Limitation that must remain explicit |
| --- | --- | --- |
| **[W-CSP]** | Enforced CSP restricts resource loading, inline script, dynamic code execution, and other policy-controlled behavior; Report-Only monitors but does not enforce. CSP calls itself defense in depth rather than a replacement for correct input handling. | A CSP-authorized malicious script is still authorized. `strict-dynamic` lets a nonce/hash-authorized script load non-parser-inserted dependencies, and CSP warns that an attacker-controlled loader URL can then load arbitrary script. |
| **[W-TT]** | Trusted Types can require typed values at defined DOM XSS injection sinks and concentrate creation in named application policies. | Its stated non-goals include malicious application JavaScript, server-generated injection, general subresource control, and data-exfiltration confinement. It cannot establish that an accepted bundle is trustworthy. |
| **[W-SRI]** | SRI compares fetched `script`/`link` resource bytes with author-supplied cryptographic integrity metadata and turns mismatch into a network error; cross-origin use requires CORS. | It authenticates bytes against the supplied expectation, not code behavior or reviewer intent. It does not cover a malicious dependency already bundled into an accepted first-party asset, a resource loaded without integrity metadata, or a compromised process that changes both bytes and the expected digest. |
| **[B-THIRD]** | Mozilla's platform documentation states that a third-party script included directly with `script` can access the page's scripts and data regardless of where it is hosted; it is effectively first-party code. | Hosting origin, CORS, or a “third-party” label does not sandbox a directly included script. An isolated cross-origin iframe is a different boundary and must not be conflated with direct script inclusion. |
| **[N-LOCK] [N-CI]** | npm's lockfile records the exact generated dependency tree and artifact integrity values; `npm ci` requires the lock, fails package/lock mismatch, and does not rewrite it. | Reproducibility and artifact integrity do not prove that a pinned package is non-malicious. These npm facts apply only if the eventual StoryOS frontend selects and pins the corresponding npm toolchain. |
| **[N-PROV]** | npm provenance links a published package to source/build information and a signed transparency record. npm explicitly says provenance does not guarantee absence of malicious code. | Provenance supports traceability and review; it is not a behavioral security verdict or a StoryOS release decision. |
| **[B-EXT]** | Chrome content scripts can access and change the shared page DOM, message more privileged extension components, and inject on matching hosts. Chrome also permits an authorized extension to execute in the DOM's `MAIN` world, shared with page JavaScript. | “Isolated world” separates JavaScript variables, not the DOM. Page CSP does not prove that no authorized extension content script exists; Chrome documents a separate CSP for isolated extension worlds. |
| **[W-SW] [B-SWL]** | A service-worker registration persists and can have installing, waiting, and active workers. By default an update waits for the old worker to control no clients; `skipWaiting()` can activate while clients still use the registration. Chrome warns this can combine an old page with a new worker and split one page's fetches across versions. | Browser update discovery and activation are not an atomic StoryOS client-contract migration. A worker/cache/version name or “latest” observation is not authority. |
| **[I-CACHE] [W-SW-CACHE]** | HTTP caches can reuse fresh responses and, in allowed cases, stale ones; `no-cache` requires validation and `no-store` prohibits intentional storage. Service Worker CacheStorage is a script-managed request/response store shared by documents/workers in its storage partition, with explicit open/delete operations. | HTTP `no-store` itself warns it is not reliable against a malicious or compromised cache. HTTP directives do not govern a malicious service-worker response strategy, and cache presence, age, or deletion is not proof of the active StoryOS build. |
| **[W-CSD]** | A network-delivered `Clear-Site-Data` response can request clearing cache/storage and reloading execution contexts; service-worker-produced responses cannot trigger it. | Clearing is broad and best-effort, and the specification does not turn it into version attestation or command authority. It is an emergency invalidation mechanism, not ordinary admission evidence. |
| **[W-LOCK] [W-BC]** | Web Locks is a cooperative same-storage-bucket coordination primitive explicitly motivated by a document editor with multiple tabs; BroadcastChannel sends notifications among same-origin browsing contexts. A lock query is only a possibly stale snapshot. | Both are controlled by origin script and local user-agent state. Neither fences a Server command, revokes a Client Session, proves the current writer generation, or reaches another browser/device. |
| **[S-HUMAN]** | Existing retained StoryOS research establishes that current portable pure-Web mechanisms cannot prove compromised-page-resistant semantic confirmation of an arbitrary StoryOS command without a separately trusted confirmation surface. | The allowed Release 1 claim is protected-client submission of exact admitted bytes, not physical-human gesture, trusted display, user presence, or user verification. |

## Credible Protected Web Client attack paths

### PC-01: attacker-controlled data reaches a DOM XSS or script-loading sink

**Source to sink.** Author prose, imported material, Tool/MCP output, research
content, URL state, `postMessage`, restored browser data, or a server error is
rendered by the first-party client. A string reaches a DOM XSS injection sink,
an inline/eval-like execution sink, or an attacker-controlled dynamic script
loader. Executing code can read or alter the page's displayed state, construct
requests with the browser-held session, substitute the command before digest
submission, or disclose page-visible data to an allowed destination.
[W-CSP] [W-TT]

**Structural mitigation pressure, not a new contract.**

- Use an **enforced response-header policy**, not Report-Only alone, with a
  closed default and explicitly justified destinations. Authorize only the
  exact reviewed first-party script set; broad sources, `unsafe-inline`,
  `unsafe-eval`, or an attacker-influenced `strict-dynamic` loader break that
  claim.
- When supported by the selected browser profile, enforce Trusted Types with
  narrowly reviewed policy factories; support and policy names remain a
  versioned security-policy fact.
- The Server still recomputes the digest and validates every tracked
  build/policy/session/scope/writer/action/replay/lifetime binding. CSP and
  Trusted Types violations are diagnostics, never admission or settlement.

**Residual risk.** A malicious script already accepted by a nonce/hash, a
compromised Trusted Types policy factory, a browser defect, or a fully
compromised client can act with the privileges intentionally left to the
client. The Server can reject invalid or stale bindings, but it cannot infer
what the author saw.

**Verification handoff.** Exercise representative HTML/script/URL/navigation/
worker sinks, loaders, `postMessage`, imported/error/journal text, and effective
headers; prove every build/policy substitution fails before admission or
disclosure. This consumes DVG-01 through DVG-03, not a new gate.

### PC-02: dependency, registry, build, CDN, or third-party script compromise

**Source to sink.** A dependency maintainer/registry account, lifecycle script,
build runner, artifact store, deployment step, CDN response, or directly
included third-party script supplies malicious code. The code becomes part of
the accepted first-party bundle or executes directly in the document and can
reach the same session, command-construction, display, journal, and disclosure
sinks as PC-01. [B-THIRD] [N-LOCK] [N-CI]

**Structural mitigation pressure, not a package-manager selection.**

- Admit no ambient third-party script. Any selected external `script`/`link`
  needs CSP, exact SRI bytes, and compatible CORS; direct script inclusion is
  otherwise first-party execution. [W-SRI] [B-THIRD]
- Give the dependency graph, toolchain/configuration, generated contracts,
  asset manifest/digests, and security-policy revision reviewable identities.
  Frozen install, integrity, provenance, and signatures establish repeatability
  or origin—not benign behavior. [N-LOCK] [N-CI] [N-PROV]
- Refuse build/contract/policy identities outside the accepted set even for a
  valid Origin and session. Cache hits, prior acceptance, and compatible
  package names cannot satisfy exact equality.

**Residual risk.** A deliberately accepted malicious version, compromised
reviewer/build signer, or compromised build process that legitimately produces
the recorded asset digest passes byte-integrity controls. Preventing a
privileged maintainer or controlled deployment operator from approving
malicious code is beyond a pure browser mechanism.

**Verification handoff.** Detect lock/build/contract/manifest drift, undeclared
runtime scripts, and missing or mismatched SRI. DVG-01 proves the active
build/corpus identity; DVG-02 refuses stale or substituted identity.

### PC-03: an authorized or compromised browser extension crosses the page boundary

**Source to sink.** An extension with a matching host permission or temporary
`activeTab` grant injects a content script. Even in Chrome's isolated world it
shares the DOM, can observe or alter editor content and visible controls, and
can relay data to extension components. An extension with scripting permission
can also inject into the page's `MAIN` world. Modified displayed state or
intercepted page/extension communication can then influence command bytes,
journal contents, disclosure, or what the author believes they approved.
[B-EXT]

**Structural mitigation pressure.**

- DOM/event/extension/local/journal state is untrusted at the Server/Core
  boundary and cannot mint scope, action, admission, or settlement. Exact
  digest and server bindings contain replay/stale/scope effects but do not
  prove the display was untouched.
- A managed extension-free browser profile could narrow endpoint risk, but is
  an **unselected choice**. Nor does an extension/native bridge become a
  trusted confirmation surface without independently specified display,
  input, signature, freshness, and settlement semantics. [S-HUMAN]

**Residual risk.** A browser extension intentionally authorized to the
StoryOS Origin may possess observation or modification powers the Web
application cannot independently revoke or attest away. CSP, SRI, and Trusted
Types do not prove extension absence.

**Verification handoff.** Simulate isolated- and `MAIN`-world DOM mutation,
forged page/extension messages, and altered journal/command input; prove the
server-side binding refusals. Passing cannot attest universal extension
absence.

### PC-04: HTTP cache, service worker, or stale build creates a mixed client

**Source to sink.** A long-lived tab retains old JavaScript in memory; an HTTP
cache reuses an older response; a service worker serves cached application
assets; or a new worker activates while an old page remains loaded. The page,
worker, generated command contract, security policy, or asset set can then
come from different releases. A stale/mixed client may form a command with
obsolete schema, omit a new binding, misrender a refusal/recovery condition,
or try to reuse an old session/writer/admission path. [I-CACHE] [W-SW]
[B-SWL]

**Structural mitigation pressure.**

- Select either **no service worker** or one exact versioned
  worker/cache-activation protocol. If selected, worker, cache, asset manifest,
  client contract, and policy share a compatible release identity;
  `skipWaiting()` is not an atomic upgrade. [B-SWL]
- Select an explicit page-shell and immutable-asset HTTP cache strategy;
  `no-cache` and `no-store` differ, and neither is admission evidence.
  [I-CACHE]
- Revalidate build, contract, policy, Client Session, and writer generation on
  every command. Refuse a stale build before Core invocation through the typed,
  non-oracular reload/recovery path.
- Worker/cache/browser state and timestamps are non-authoritative. They cannot
  reconcile `OutcomeUnknown`, prove no Receipt, or authorize retry.
  `Clear-Site-Data`, if selected, is emergency invalidation—not normal
  settlement, deletion proof, or trust proof. [W-CSD]

**Residual risk.** An old page can continue displaying obsolete information
until reloaded, and forced reload can disrupt unsaved local work. Server-side
release floors can prevent stale mutation but cannot make an old display
current. The Web Editor Session recovery path must preserve recoverable text
without treating it as authority.

**Verification handoff.** Combine old/new page, worker, cache, contract, policy,
session, writer, Snapshot, and command identities; prove exact activation,
refusal, resync/reload, Draft preservation, no blind retry, and no Core effect
from an incompatible mix under DVG-01 through DVG-03.

### PC-05: a stale tab submits after another tab takes over

**Source to sink.** Two same-origin tabs can hold independent in-memory
documents and communicate or cooperate through browser primitives. The former
writer can retain its session handle, old writer generation, journal, pending
projection, and queued network work after a second tab takes over. If the
Server trusts BroadcastChannel, Web Locks, focus/visibility, timestamps, or a
locally cached “active writer” flag, the old tab can still reach Author Command
Admission. [W-LOCK] [W-BC]

**Structural mitigation pressure.**

- Browser coordination can improve immediate read-only UX, but Server writer
  generation is the takeover fence; a local lock is not a Server lease.
- Each editor-bound admission/Core invocation revalidates Editor Session and
  writer generation. Takeover advances the generation; stale local
  journal/projection follows Recovery Draft and cannot overwrite authority.
- Focus, visibility, closure, heartbeat, Web Locks queries, and
  BroadcastChannel messages are race-prone observations, not send prevention.

**Residual risk.** A stale tab may continue to display and locally edit old
state, and browser-local coordination can fail or be bypassed by compromised
same-origin script. Server fencing protects authority; it cannot guarantee
that both tabs present a current or unspoofed display.

**Verification handoff.** DVG-03 exercises second-tab read-only/takeover,
stale-writer refusal, reordering, resync, and prior-tab text preservation,
including simultaneous submit, queued delivery, lock-holder crash, missing
BroadcastChannel messages, and old-tab reload.

### PC-06: a real gesture or credential ceremony is repurposed by compromised client code

**Source to sink.** Compromised page or extension code waits for a real click,
keystroke, or credential ceremony, but displays one meaning and submits another
digest-covered command. The valid session, Origin, user activation, command
digest, or even plain WebAuthn ceremony can reach a server admission without a
trusted surface ever displaying the StoryOS command semantics. [S-HUMAN]

**Boundary retained from #67.** Release 1 may claim only that the exact
Protected Web Client build/session/policy/writer/action/digest/nonce/
idempotency/lifetime bindings matched and that the Server admitted that
command for its Server-derived User and Project Scope. It may not claim:

- one physical human gesture or browser-event cardinality;
- a trusted display of command semantics;
- user presence;
- user verification; or
- proof that the physical Project Author saw, understood, or approved the
  exact command.

A stronger compromised-page-resistant claim requires an external trusted
confirmation surface that independently displays meaningful command data and
binds confirmation to fresh signed command-specific evidence. StoryOS has not
selected such a Release 1 surface.

## Cross-cutting non-authority and recovery consequences

The browser evidence above reinforces, but does not create, these existing
StoryOS rules:

- Browser cache/storage/worker/lock/message/journal/projection/DOM/process and
  timing state is never authority. Presence cannot prove submission, Core
  invocation, Receipt, settlement, or external effect; absence cannot disprove
  them.
- Missing acknowledgement remains `OutcomeUnknown`, blocks blind retry, and
  resolves only through authoritative read-only reconciliation and typed
  settlement. Duplicate recovery requires the same unexpired, fully matching
  admission; changed or unverifiable bindings require the existing refusal or
  reconfirmation path.
- Security reports, CSP violations, asset/provenance evidence, and extension
  observations do not reclassify Operational Records, Draft Artifacts, or
  Proposal Conflict.

## Owner handoff without a new map

| Existing owner | Evidence this note hands off |
| --- | --- |
| Threat model, Issue #57 | PC-01 through PC-06 source-to-sink paths, structural mitigations, and residual client/platform risks |
| [Author Command Admission](../foundation/author-command-admission.md) | exact protected-client/build/policy/session/writer/action/digest/nonce/idempotency/lifetime claim and the physical-human disclaimer; no ownership change |
| [Versioned Command, Query, Artifact, and Event Protocol](../foundation/versioned-command-query-artifact-event-protocol.md) | concrete CSP/cache headers, build/contract/policy identities, typed stale-client refusal, and non-oracular wire behavior |
| [Web Editor Session](../foundation/web-editor-session-synchronization-and-recovery-semantics.md) | stale-tab UX, writer takeover, local journal/Draft preservation, reload/resync, and mixed-version recovery |
| [Deterministic Verification and Failure-Recovery Gates](../foundation/deterministic-verification-and-failure-recovery-gates.md) | hostile DOM/script/extension inputs, effective-header checks, dependency/asset drift, old/new page-worker-cache matrices, stale-tab races, acknowledgement loss, and no-blind-retry proof |
| Repository/release governance | exact chosen dependency/build inputs, lock/install policy, provenance/signature verification, emitted asset manifest, and reviewable release diff |

This evidence does not justify a second security map or a new implementation
issue.

## Unselected implementation and deployment choices

Primary sources do not decide:

- the supported browser/version matrix for Trusted Types or other controls;
- the exact CSP directives, nonce/hash strategy, or allowed resource
  destinations;
- whether the protected Release 1 profile has no service worker or a versioned
  service-worker/cache design;
- the eventual production package manager and install-script policy;
- whether a managed/extension-free endpoint profile is required or browser
  extensions remain an explicitly accepted endpoint residual risk; or
- whether StoryOS later adds an independent trusted confirmation surface.

The owning contracts may choose and version these during implementation
handoff. Until then, verification must not assume the strongest variant.

## Primary source ledger

All external sources below were accessed on 2026-07-24.

- **[W-CSP]** W3C,
  [Content Security Policy Level 3 introduction and defense-in-depth boundary](https://www.w3.org/TR/CSP3/#intro),
  [Report-Only header](https://www.w3.org/TR/CSP3/#header-content-security-policy-report-only),
  [`script-src`](https://www.w3.org/TR/CSP3/#directive-script-src), and
  [`strict-dynamic` caveat](https://www.w3.org/TR/CSP3/#strict-dynamic-usage).
- **[W-TT]** W3C,
  [Trusted Types introduction](https://www.w3.org/TR/trusted-types/#introduction),
  [non-goals](https://www.w3.org/TR/trusted-types/#non-goals), and
  [`require-trusted-types-for`](https://www.w3.org/TR/trusted-types/#require-trusted-types-for-csp-directive).
- **[W-SRI]** W3C,
  [Subresource Integrity resource-integrity use case](https://www.w3.org/TR/sri/#resource-integrity),
  [integrity metadata](https://www.w3.org/TR/sri/#integrity-metadata-description),
  and
  [integrity-violation handling](https://www.w3.org/TR/sri/#handling-integrity-violations).
- **[B-THIRD]** Mozilla,
  [Privacy on the Web: carefully manage third-party resources](https://developer.mozilla.org/en-US/docs/Web/Privacy#carefully_manage_third-party_resources).
- **[N-LOCK]** npm,
  [`package-lock.json` exact-tree and integrity fields](https://docs.npmjs.com/cli/v11/configuring-npm/package-lock-json/).
- **[N-CI]** npm,
  [`npm ci` frozen-install behavior and install-script controls](https://docs.npmjs.com/cli/v11/commands/npm-ci/).
- **[N-PROV]** npm,
  [Generating provenance statements, including explicit malicious-code limitation](https://docs.npmjs.com/generating-provenance-statements/#provenance-limitations).
- **[B-EXT]** Chrome,
  [content-script capabilities, isolated worlds, shared DOM, host permissions, and CSP](https://developer.chrome.com/docs/extensions/develop/concepts/content-scripts)
  and
  [`chrome.scripting` permissions and `ExecutionWorld`](https://developer.chrome.com/docs/extensions/reference/api/scripting#type-ExecutionWorld).
- **[W-SW]** W3C,
  [Service Worker registration lifetime and active/waiting workers](https://www.w3.org/TR/service-workers/#service-worker-registration-concept),
  [`skipWaiting()`](https://www.w3.org/TR/service-workers/#dom-serviceworkerglobalscope-skipwaiting),
  and
  [CacheStorage](https://www.w3.org/TR/service-workers/#cachestorage-interface).
- **[B-SWL]** Chrome/web.dev,
  [The service worker lifecycle](https://web.dev/articles/service-worker-lifecycle),
  especially the update/waiting and `skipWaiting()` mixed-version warning.
- **[I-CACHE]** IETF,
  [RFC 9111 HTTP cache reuse and stale-response rules](https://www.rfc-editor.org/rfc/rfc9111.html#section-4)
  and
  [`no-cache`/`no-store` response directives and limitations](https://www.rfc-editor.org/rfc/rfc9111.html#section-5.2.2).
- **[W-SW-CACHE]** W3C,
  [Service Worker `Cache` and `CacheStorage` shared stores](https://www.w3.org/TR/service-workers/#cache-objects)
  and
  [`CacheStorage.delete`](https://www.w3.org/TR/service-workers/#cache-storage-delete).
- **[W-CSD]** W3C,
  [Clear Site Data clearing model](https://www.w3.org/TR/clear-site-data/#clearing),
  [service-worker restriction](https://www.w3.org/TR/clear-site-data/#service-workers),
  and
  [privacy/best-effort limitation](https://www.w3.org/TR/clear-site-data/#privacy).
- **[W-LOCK]** W3C,
  [Web Locks document-editor/multiple-tab use case](https://www.w3.org/TR/web-locks/#motivations),
  [lock-manager query snapshot](https://www.w3.org/TR/web-locks/#api-lock-manager-query),
  and
  [lock scope](https://www.w3.org/TR/web-locks/#security-scope).
- **[W-BC]** WHATWG,
  [Broadcasting to other browsing contexts](https://html.spec.whatwg.org/multipage/web-messaging.html#broadcasting-to-other-browsing-contexts).
- **[S-HUMAN]** StoryOS retained primary-source research,
  [Browser author-intent attestation boundaries](https://github.com/FrankQDWang/StoryOS/blob/74ddfda4295ceef39aa82e687810cb623e513edd/docs/research/browser-author-intent-attestation-boundaries.md),
  audited 2026-07-23.
