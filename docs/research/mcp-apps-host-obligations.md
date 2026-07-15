# StoryOS MCP Apps host obligations

Status: accepted research conclusion for [Establish StoryOS MCP Apps Host Obligations](https://github.com/FrankQDWang/StoryOS/issues/43), researched 2026-07-10 and independently reverified against first-party sources on 2026-07-14. See the [source audit](./mcp-apps-host-obligations-source-audit.md).

## Source and terminology boundary

The normative baseline in this note is the **stable MCP Apps specification dated 2026-01-26**, extension identifier `io.modelcontextprotocol/ui`, adopted through the merged SEP-1865 record. The official repository separately labels its moving `draft` specification as development; this note therefore treats draft-only SDK features as non-normative unless StoryOS later negotiates and versions them explicitly. The source snapshot inspected was `modelcontextprotocol/ext-apps` commit `cf87f2a2c2581b2bc45ff4848aac9fa7e106a576` (SDK package version `1.7.4`). [sep-1865] [stable-status] [upstream-status] [draft-status]

Labels used below:

- **Normative** means a `MUST`, `MUST NOT`, `SHOULD`, or protocol rule in the stable 2026-01-26 specification.
- **StoryOS obligation** means a design inference required by StoryOS's author-sovereignty, local-first, durable-run, and untrusted-extension invariants. It is intentionally stricter than interoperability alone.
- **Implementation evidence** means behavior observed in first-party SDK source, tests, or examples. It is useful evidence, not a protocol requirement. The upstream repository explicitly says that it does not ship a supported host implementation beyond `examples/basic-host`. [upstream-host-status]

## Executive conclusion

StoryOS should implement a **stable-2026-01-26-compatible, fail-closed web host** with these properties:

1. A tool's `ui://` declaration makes a View *eligible to render*; it grants no StoryOS capability.
2. Every View runs behind the specification's cross-origin sandbox proxy and communicates only through validated JSON-RPC over `postMessage`.
3. Every View-to-host request passes through StoryOS's own capability, schema, provenance, approval, and domain-command boundary. Do not give the SDK bridge a raw MCP client that can auto-forward calls.
4. StoryOS persists the tool call, immutable UI-resource snapshot and hash, security decision, protocol version, and fallback representation—not a live iframe, DOM, pending JSON-RPC connection, or hidden app-owned truth.
5. Reload and reconnect create a fresh sandbox and handshake, then replay already-recorded input/result events. Replay never repeats the original side effect.
6. If the resource or protocol cannot be rendered safely, the transcript remains useful through a StoryOS-rendered static fallback built from the tool's meaningful `content`, stored structured result, status, and provenance.

The stable specification deliberately defers state persistence and restoration, so points 4–6 are StoryOS design obligations rather than MCP Apps protocol behavior. [stable-deferred]

## 1. Normative interoperability baseline

### 1.1 Extension negotiation and versioning

**Normative**

- MCP Apps is optional and must be explicitly negotiated. The host advertises `io.modelcontextprotocol/ui` in MCP client capabilities and supplies a required `mimeTypes` array; the stable baseline MIME type is `text/html;profile=mcp-app`. [stable-status] [stable-negotiation]
- The View starts its own host-facing lifecycle with `ui/initialize`, including `appCapabilities` and a protocol version; the host returns the negotiated protocol version, its actual capabilities, host identity, and host context, after which the View sends `ui/notifications/initialized`. [stable-context] [stable-lifecycle]
- A host must not send requests or notifications to the View before receiving `ui/notifications/initialized`. [stable-sandbox]

**StoryOS obligation**

- Implement an explicit adapter keyed by extension identifier and UI protocol version. Initially advertise and select only the stable `2026-01-26` profile and only capabilities StoryOS actually enforces; continue only if the View completes initialization against that selected version.
- Preserve the selected version and capability snapshots with each durable View instance. If there is no explicitly supported version, fail closed. Unknown or draft-only methods are rejected; support must not be guessed from the installed JavaScript SDK.
- Treat `_meta.ui.resourceUri` as canonical. Any deprecated flat key accepted for compatibility must be normalized inside the MCP adapter, attributed as legacy input, and never become StoryOS's internal contract.
- The current SDK answers an unsupported requested version with its latest version, and its source already contains draft additions while the protocol constant remains `2026-01-26`. StoryOS should therefore perform its own compatibility check rather than assuming SDK package version equals negotiated protocol version. [sdk-version] [bridge-version]

### 1.2 Tool-to-resource linkage and resource validation

**Normative**

- A UI resource uses a `ui://` URI. For the stable HTML profile, `resources/read` must return `text/html;profile=mcp-app`, either `text` or base64 `blob`, containing valid HTML5. [stable-resource]
- A tool links to the resource with `_meta.ui.resourceUri`. The referenced resource must exist, and a supporting host must fetch it with `resources/read`; a host may prefetch and cache it. [stable-discovery]
- `_meta.ui.visibility` defaults to `['model', 'app']`. A host must omit app-only tools from the agent's model-visible tool list, reject App calls to tools without `app` visibility, and block cross-server App calls for app-only tools. [stable-discovery]

**StoryOS obligation**

- Resolve `resourceUri` only against the same MCP server/connector that declared the tool. Reject scheme confusion, mismatched returned URI, unsupported MIME type, missing/ambiguous matching content, invalid base64, oversized HTML, malformed metadata, or invalid HTML before creating an iframe.
- Snapshot the exact bytes and normalized metadata into the project's content-addressed artifact store before execution. Record the server/connector identity, server version, resource URI, MIME type, content hash, fetch time, and requested CSP/permissions.
- Interpret visibility as **eligibility**, not authorization. Effective callable tools are the intersection of same-server visibility, the local `ToolSpec`, the current Run's capabilities, project policy, and any required approval.
- A changed resource hash is a new executable UI version. Never silently replace the code behind an old transcript item with newly fetched HTML.

The first-party quickstart confirms the intended two-part shape: a tool points at a separately registered `ui://` resource while the tool still returns text content. [quickstart]

### 1.3 Static fallback

**Normative**

- If MCP Apps is unsupported, the tool remains a normal tool. Servers should provide text-only behavior, and tools **must** return a meaningful `content` array even when a UI is available. [stable-negotiation]
- The stable data-passing guidance separates `content` for model/text hosts, `structuredContent` for UI rendering, and `_meta` for non-model metadata. [stable-data]

**StoryOS obligation**

- Every transcript View has a StoryOS-owned fallback renderer that safely escapes and presents the stored `content`, execution status/error, source attribution, and—where a registered typed renderer exists—`structuredContent`.
- `_meta` and `structuredContent` must not enter model context merely because they are displayed. The ContextManifest records any explicit later promotion.
- Never fall back to unsandboxed HTML, an external URL, or a fresh tool execution. If no safe meaningful fallback exists, render a durable error card with provenance rather than an empty transcript slot.

## 2. Sandboxing, CSP, permissions, and origins

### 2.1 Required iframe architecture

**Normative**

- All View content must render in a sandboxed iframe with restricted permissions. For a web host, the host must use an intermediate Sandbox proxy on a different origin from the host. [stable-security] [stable-sandbox]
- The Sandbox proxy has `allow-scripts` and `allow-same-origin`, performs the reserved `sandbox-proxy-ready` / `sandbox-resource-ready` exchange, loads the raw View HTML under the enforced CSP, and transparently relays non-reserved JSON-RPC messages. It should not synthesize request IDs. [stable-sandbox]
- The sandbox must deny nested frames unless `frameDomains` are declared, deny dangerous plugin objects, use the specified base-URI policy, and apply restrictive defaults when metadata is absent. Browser permissions are requests that a host may decline. [stable-resource] [stable-sandbox]

**StoryOS obligation**

- Serve the proxy from a dedicated origin that has no StoryOS cookies, bearer token, local storage namespace, service workers, or DOM access. Do not share stateful sandbox storage across mutually untrusted MCP servers; partition by connector/server trust principal or use an equivalent browser-enforced partition.
- The inner and outer iframes receive only the sandbox tokens needed by the stable profile. Do not grant top navigation, popups, downloads, presentation, pointer lock, or additional forms/storage behavior by default.
- Treat `_meta.ui.domain` as a host-specific request, not a server-selected URL or implicit trust grant. A stable origin requires an explicit StoryOS mapping and security review.
- Render a visible host-owned boundary naming the MCP server/App and its permission state; View HTML cannot obscure that attribution.

The first-party proxy example validates both parent origin and inner-frame origin, while `PostMessageTransport` separately validates `event.source`. The end-to-end test demonstrates that sibling frames can deliver a `postMessage`, making source-window validation essential rather than optional defense in depth. [sandbox-example] [transport-source] [cross-app-test]

### 2.2 CSP and Permission Policy

**Normative**

- The host must construct and enforce CSP from resource metadata, must not allow undeclared domains, and should log the effective CSP. With no declared CSP, the stable specification requires a deny-by-default policy with no connections and only self/inline scripts and styles plus limited self/data images and media. [stable-resource]
- Undeclared frames are blocked with `frame-src 'none'`; dangerous objects are blocked; the host should warn when external access is required and may apply global allow/block lists. [stable-security]
- Requested camera, microphone, geolocation, and clipboard-write permissions may be reflected in iframe Permission Policy only if the host elects to grant them; Apps must tolerate denial. [stable-resource]

**StoryOS obligation**

- Parse requested domains as structured origins and build CSP from validated tokens; never interpolate raw server strings into a header. Reject directive-breaking characters, unsupported schemes, userinfo, and hosts disallowed by StoryOS network policy.
- Apply `min(spec declaration, StoryOS global policy, project grant, Run grant)` independently to connection, resource, frame, base URI, and browser-permission requests. Metadata requests do not self-authorize.
- Default all browser permissions and external origins to denied. New grants follow the existing human-approval boundary and are recorded with scope, reason, requester, and resource hash.
- Enforce CSP in an HTTP response header generated by the trusted proxy service, not a View-controlled `<meta>` element. The first-party host example makes the same header-level distinction and sanitizes CSP tokens, but it also enables broader development directives such as `unsafe-eval`; those broader example directives are not the stable normative default and must not be copied wholesale. [basic-host-csp]
- Bound CPU time, memory, message frequency, iframe dimensions, resource bytes, tool-result bytes, and outstanding RPC requests. The stable threat model explicitly calls out malicious resource consumption. [stable-security]

## 3. Bridge and mediated host operations

### 3.1 Transport validation

**Normative**

- View/host communication is JSON-RPC 2.0 over `postMessage`. The host must validate incoming messages, reject malformed messages, and log View-initiated RPC for security review. [stable-bridge] [stable-security]
- The stable standard-message subset is `tools/call`, `resources/read`, `notifications/message`, `ping`, and the UI lifecycle; UI-specific requests include open-link, chat message, display-mode, and model-context update. [stable-bridge] [stable-messages]
- A host may block forwarded non-`ui/` methods or subject them to further user approval. [stable-sandbox]

**StoryOS obligation**

- Validate both the expected source window and the appropriate origin at each relay boundary, then schema-validate the complete JSON-RPC envelope and method payload. Reject unknown methods, duplicate/invalid IDs, responses without an outstanding request, oversized payloads, and requests before initialization.
- Bind each bridge to one immutable `AppInstanceId`, one transcript item, one resource hash, one originating tool call, and one MCP server connection. Never route by a client-supplied server or connector name.
- Instantiate `AppBridge` without a raw MCP client and register explicit handlers that call StoryOS's capability gateway. The SDK's convenience path automatically proxies tools, resources, and prompts from server capabilities; that bypass is incompatible with StoryOS's rule that discovery is not authorization. [bridge-auto-forward]
- Store an auditable event for every accepted or denied View request: instance, method, sanitized argument summary/hash, capability decision, approval reference, result/error, duration, and byte counts. Secret-bearing raw payloads remain redacted or encrypted according to the owning domain.

### 3.2 Method policy

The following are **StoryOS obligations** built on the stable methods:

| View request | StoryOS host policy |
| --- | --- |
| `tools/call` | Resolve only a same-server, `app`-visible registered `ToolSpec`; validate input/output schemas; apply current Run/project capability and approval policy; execute through the unified tool gateway; never permit direct database or authoritative-state writes. |
| `resources/read` | Scope to explicitly granted resources on the originating server; do not expose the project filesystem, other servers, or StoryOS artifacts merely because the MCP server supports resources. |
| `ui/open-link` | Advertise only if implemented. Validate and normalize the URL, default to `https`, block local/file/data/javascript destinations and unsafe redirects, and require the appropriate author approval before the external side effect. |
| `ui/message` | Treat as an App-attributed request to submit a user message, never proof that the author spoke. Require a visible author gesture or confirmation, then create the normal typed transcript command with provenance. |
| `ui/update-model-context` | Advertise only when supported. Size-cap, sanitize, attribute, and store the latest accepted value as an instance-scoped context contribution. It is neither durable memory nor authoritative creative state and enters a future model call only through the normal ContextManifest policy. |
| `notifications/message` | Store as rate-limited, redacted diagnostic telemetry. It never enters model context or the author transcript by default. |
| `ui/request-display-mode` | Intersect the View's declared modes with host support; initially support `inline` and clamp dimensions. Never allow a View to overlay host approval controls. |
| `ui/notifications/size-changed` | Accept only finite positive dimensions within host limits; debounce and clamp before changing layout. |
| Unknown or draft-only method | Return method-not-found or policy-denied. Do not silently forward. |

The official transcript example shows why capability advertisement matters: it requests microphone and clipboard permission in resource metadata, checks the host capability before updating model context, and handles teardown. These are examples of negotiation—not authority granted by the App itself. [transcript-server] [transcript-view]

### 3.3 Domain-data ownership

**StoryOS obligation**

- The iframe is a sandboxed View/Controller over StoryOS-owned typed Artifacts. It never owns characters, relationships, timeline events, research sources, canon, author plans, or prose.
- The View receives immutable snapshots or tool results. App actions return through Host-mediated ToolCalls and may produce a `Candidate`, `Proposal`, or another non-authoritative Artifact. Any intended change to creative Authoritative State must be expressed as a StoryOS Core `Proposal` and pass through ordinary author inspection and `Acceptance`; an App interaction never qualifies as a `Direct Author Action` and cannot invoke a direct authoritative write path.
- App local state, DOM state, IndexedDB/localStorage, `ui/update-model-context`, and MCP `_meta` are never sources of domain truth.
- If an App submits an opaque UI-preference blob in a future StoryOS-specific contract, it remains non-authoritative, versioned by App/resource hash, size-limited, inspectable/deletable, and isolated from model context.

## 4. Lifecycle, reconnect, replay, and persistence

### 4.1 Stable lifecycle obligations

**Normative**

- Initialization may run in parallel with the originating tool call. After the View is initialized, the host sends the complete tool input at most once and before the tool result; partial input is optional and must stop when complete input is sent. [stable-messages] [stable-lifecycle]
- If the displayed tool completes, the host sends `ui/notifications/tool-result`; if it is cancelled, the host sends `ui/notifications/tool-cancelled`. [stable-messages]
- Before removing a View, the host sends the request-shaped `ui/resource-teardown` message and should wait for its response to reduce data loss. [stable-messages] [stable-lifecycle]
- The stable profile does **not** standardize state persistence/restoration, view-to-view communication, or multiple UI resources. [stable-deferred]

### 4.2 Durable StoryOS envelope

**StoryOS obligation**

Persist enough host-owned state to reproduce the transcript meaning without preserving executable runtime state:

- transcript item and App instance identifiers;
- originating Run, tool-call event, MCP server/connector, tool name, and ToolSpec version;
- references to the durable tool input, result/cancellation/error, and their schemas;
- resource URI, exact HTML/blob snapshot or content-addressed reference, MIME type, content hash, server version, and fetch time;
- normalized resource metadata plus requested and actually granted CSP domains and browser permissions;
- negotiated UI protocol version, host/View capability snapshots, and minimal host-context snapshot;
- lifecycle status, initialization/teardown timestamps, teardown reason, and whether shutdown was clean;
- the last accepted instance-scoped model-context contribution as an attributed event, if that capability was enabled;
- a safe static fallback payload and provenance.

Do **not** persist or restore the iframe DOM, JavaScript heap, pending JSON-RPC request IDs/promises, browser storage as authority, media handles, or App-registered draft tools.

Pinned Codex is useful only as non-normative evidence here: it persists MCP App context (`connectorId`, `linkId`, `resourceUri`, App/template/action identity) beside durable tool-call arguments/results and reconstructs it from event history, while MCP Apps itself remains feature-gated as under development. StoryOS should borrow the separation between durable transcript context and ephemeral rendering, not Codex's product-specific fields or runtime. [codex-item] [codex-history] [codex-feature]

### 4.3 Reconnect and replay

**StoryOS obligation**

1. Treat the iframe bridge and MCP transport as disposable projections, never the Run or transcript source of truth.
2. On reload/reconnect, create a new instance/transport and repeat extension negotiation, sandbox setup, `ui/initialize`, and `ui/notifications/initialized`.
3. Rehydrate from the immutable approved resource snapshot and replay the already-durable complete tool input followed by the recorded result, cancellation, or error. Do not replay partial-input deltas unless a future version gives them durable semantics.
4. Never re-execute the originating tool merely to redraw a transcript App. A refresh requiring live data is a new, visible, capability-checked tool call.
5. If the cached resource is absent, newly disallowed, fails integrity validation, or requires a no-longer-granted capability, render the static fallback. Refetching a changed resource creates a new resource version and approval decision; it does not mutate the historical View.
6. Teardown gets a bounded grace timeout. After timeout, record unclean teardown, close the transport, cancel outstanding requests, remove listeners, revoke object URLs/media, and destroy the iframe.
7. Pending App requests fail on disconnect and are never silently retried if they could have side effects. Safe retry semantics belong to the underlying ToolSpec/idempotency contract, not the bridge.

This replay model is a StoryOS extension above the stable profile, which explicitly leaves persistence/restoration for future work. It must therefore remain an internal host behavior rather than an advertised MCP Apps capability until a compatible extension is standardized. [stable-deferred]

## 5. Untrusted-server policy

The stable threat model assumes malicious HTML, sandbox escape attempts, unauthorized tool execution, exfiltration, phishing, and resource exhaustion. [stable-security]

Accordingly, **StoryOS must**:

- treat every MCP server, resource body, metadata field, tool annotation, tool result, and View message as untrusted input, including first-party-looking names;
- keep discovery, model visibility, project enablement, per-Run capability, browser permission, external-data disclosure, and actual execution as separate decisions;
- verify resource integrity and retain a hash-addressed review record before execution;
- present server/App identity and an unspoofable sandbox boundary around the View;
- use schema validation, byte/rate/time limits, cancellation, and per-instance outstanding-request limits at every bridge edge;
- prohibit ambient access to project data, secrets, host cookies, bearer tokens, filesystem paths, other MCP servers, other App instances, and internal localhost services;
- route all external fetches through the effective CSP plus StoryOS network/SSRF policy;
- route every requested change to creative Authoritative State into a StoryOS Core `Proposal` and ordinary author `Acceptance`; an App cannot invoke a `Direct Author Action` or any other direct authoritative write path;
- preserve denied requests and policy reasons in the inspectable Run timeline without exposing secrets;
- fail to the static transcript representation when any trust, integrity, version, or sandbox invariant is not satisfied.

HTML scanning, allowlists, and resource hashes are useful review inputs but are not substitutes for browser isolation and capability mediation. The specification recommends all three layers. [stable-security]

## 6. Stable profile versus current draft

The current draft and SDK source contain features that are not part of the stable 2026-01-26 contract, including host-mediated downloads, View-requested teardown, broader host capability fields, host-to-App tool calls, and App-provided tool registration/lifecycle. The draft itself remains marked `Draft`, and persistence/restoration is still deferred. [draft-status] [draft-messages] [draft-app-tools] [draft-deferred]

StoryOS's foundation host should therefore:

- implement the stable profile first;
- keep draft method handlers disabled and unadvertised;
- never persist App-provided tool registrations across sessions—even the draft explicitly forbids that;
- add future protocol versions through a reviewed adapter and migration, not by upgrading the npm package and inheriting behavior automatically.

## 7. Required prototype and contract tests

The downstream host prototype should demonstrate these observable cases:

1. valid `ui://` text and blob resources initialize under the exact stable MIME type;
2. wrong scheme/MIME, mismatched URI, malformed body, oversized resource, and CSP injection attempts fail closed;
3. host and Sandbox are cross-origin, the outer/inner sandboxes lack dangerous tokens, and undeclared network/frame/base destinations are blocked;
4. sibling-App message injection fails by source-window validation, and wrong-origin relay messages fail;
5. no host request reaches the View before `initialized`;
6. app-only tools never enter the model list; model-only, cross-server, unregistered, schema-invalid, or unauthorized App tool calls fail;
7. requested browser permissions remain denied until separately granted and are reflected accurately in host capabilities;
8. complete input arrives once before result; cancellation and bounded teardown are observable;
9. `ui/message`, `ui/open-link`, and `ui/update-model-context` cannot bypass author consent, context size/provenance, or external-effect policy;
10. refresh reconstructs the View from durable events without re-executing the tool;
11. changed/missing/untrusted UI resources produce the static fallback rather than replacing historical code;
12. static fallback remains readable with MCP Apps disabled or the server offline;
13. draft-only and unknown methods are rejected unless an explicit negotiated adapter is enabled;
14. View state never becomes authoritative StoryOS data. Any requested change to creative Authoritative State must instead become a StoryOS Core `Proposal` and pass through ordinary author `Acceptance`; an App interaction never qualifies as a `Direct Author Action`.

The upstream security suite is a useful starting point because it includes a non-vacuous sibling-frame injection test, but StoryOS needs the authorization, persistence, replay, and author-sovereignty cases above in its own public-boundary tests. [cross-app-test]

## 8. Decisions handed to downstream tickets

This research fixes the host obligations but intentionally leaves these contract names and schema shapes to their owning Wayfinder tickets:

- the exact `App View Artifact`/transcript representation and lifecycle states;
- the precise ToolSpec effect taxonomy and approval scopes;
- resource-size, message-rate, CPU/memory, and teardown timeout values;
- the project-storage tables and content-addressed artifact layout;
- the Command/Event names used for App requests, replay, and fallback;
- whether a future reviewed protocol version enables any draft-only feature.

Those tickets should preserve the boundaries established here: stable baseline, manual mediation, durable host-owned replay, immutable resource identity, and no App-owned authoritative state.

## Primary sources

[sep-1865]: https://github.com/modelcontextprotocol/modelcontextprotocol/pull/1865
[stable-status]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1-L53
[stable-resource]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L55-L287
[stable-discovery]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L319-L402
[stable-bridge]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L411-L508
[stable-sandbox]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L470-L508
[stable-context]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L510-L666
[stable-messages]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L960-L1230
[stable-lifecycle]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1272-L1490
[stable-data]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1391-L1490
[stable-negotiation]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1492-L1560
[stable-deferred]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1562-L1576
[stable-security]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1680-L1763
[upstream-status]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/README.md#L237-L254
[upstream-host-status]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/README.md#L132-L151
[draft-status]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L1-L40
[draft-messages]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L1026-L1511
[draft-app-tools]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L1738-L2176
[draft-deferred]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L2321-L2351
[sdk-version]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/spec.types.ts#L20-L31
[bridge-version]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/app-bridge.ts#L1457-L1490
[transport-source]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/message-transport.ts#L12-L142
[cross-app-test]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/tests/e2e/security.spec.ts#L305-L358
[sandbox-example]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/examples/basic-host/src/sandbox.ts#L1-L137
[basic-host-csp]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/examples/basic-host/serve.ts#L1-L125
[bridge-auto-forward]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/app-bridge.ts#L1792-L1926
[quickstart]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/examples/quickstart/server.ts#L21-L57
[transcript-server]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/examples/transcript-server/server.ts#L28-L82
[transcript-view]: https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/examples/transcript-server/src/mcp-app.ts#L57-L83
[codex-feature]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/features/src/lib.rs#L1058-L1067
[codex-item]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/v2/item.rs#L397-L409
[codex-history]: https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/app-server-protocol/src/protocol/thread_history.rs#L760-L846
