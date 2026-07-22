# Versioned HTTP, SSE, schema, digest, and MCP protocol primary sources

- Audited: 2026-07-22
- Scope: external protocol facts needed by [Specify the Versioned Command, Query, Artifact, and Event Protocol](https://github.com/FrankQDWang/StoryOS/issues/58)
- Evidence policy: official standards, official specifications, and first-party
  protocol repositories only
- Authority: research evidence only; this note does not choose StoryOS domain
  semantics or override `CONTEXT.md` or a foundation specification

## Facts

### 1. HTTP methods, conditions, acknowledgement, and idempotency

1. HTTP defines a method as idempotent when multiple identical requests have
   the same **intended effect** as one request. Safe methods, `PUT`, and
   `DELETE` are idempotent; `POST` is not inherently idempotent. HTTP also says
   clients should not automatically retry a non-idempotent request unless they
   know its semantics are idempotent or can determine that the original request
   was not applied. Server-side logging or revision history may still record
   each attempt. [RFC 9110, section 9.2.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.2)
   (accessed 2026-07-21).

2. HTTP conditional requests can guard state-changing methods. `If-Match`
   uses strong entity-tag comparison and is explicitly described as a way to
   prevent lost updates; a false condition can produce `412 Precondition
   Failed`. `If-None-Match: *` can condition a write on the absence of a current
   representation. These preconditions concern the selected HTTP
   representation; HTTP does not define a StoryOS aggregate Head or a
   multi-object domain revision. [RFC 9110, sections 13.1 and 13.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-13)
   (accessed 2026-07-21).

3. `428 Precondition Required` lets a server require a request to be
   conditional, typically to avoid lost updates. Its use is optional, and the
   standard says clients cannot rely on it being used. [RFC 6585, section 3](https://www.rfc-editor.org/rfc/rfc6585.html#section-3)
   (accessed 2026-07-21).

4. HTTP separates acknowledgement from completion. `201 Created` identifies
   a newly created resource. `202 Accepted` says processing was accepted but is
   incomplete and might never occur; HTTP has no facility to resend a later
   status code on that response. The `202` representation ought to describe
   current status and point to or embed a status monitor. `204 No Content`
   reports successful completion without response content. [RFC 9110, sections
   15.3.2-15.3.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.3.2)
   (accessed 2026-07-21).

5. Standard status codes provide useful coarse distinctions, not an
   application error taxonomy: `409` is a conflict with current resource state;
   `412` is a failed request-header precondition; `413` is content too large;
   and `422` means syntactically valid content whose instructions could not be
   processed. [RFC 9110, section 15.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5)
   (accessed 2026-07-21). `428` requires a precondition and `429` represents
   rate limiting, optionally with `Retry-After`. [RFC 6585, sections 3-4](https://www.rfc-editor.org/rfc/rfc6585.html#section-3)
   (accessed 2026-07-21). None of these standards sets an application's byte,
   item, time, or work budget.

6. HTTP permits an origin server to answer `404 Not Found` when it is unwilling
   to disclose whether a forbidden target exists. Thus non-oracular
   not-found/denied behavior is compatible with HTTP, but the uniform policy
   and response body remain application decisions. [RFC 9110, sections 15.5.4-15.5.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.4)
   (accessed 2026-07-21).

7. There is no published RFC for `Idempotency-Key` as of the audit date. The
   latest IETF working-group text is expired Internet-Draft revision 07, so it
   is work in progress rather than a standard. It proposes a client-generated
   structured-header string, forbids reuse with a different payload, permits a
   server-side fingerprint, replays a completed result, and distinguishes a
   concurrent duplicate from a completed duplicate. [IETF Datatracker draft
   status and text](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/)
   (accessed 2026-07-21); [revision history showing expiry on 2026-04-18](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/history/)
   (accessed 2026-07-21).

8. Consequently, HTTP method idempotency is standardized, but the scope,
   retention, request-digest binding, in-flight behavior, stored result, and
   mismatch behavior of an application idempotency key are not currently fixed
   by an HTTP RFC. This is a standards-scope conclusion from RFC 9110 and the
   expired draft above, not a claim that the draft is normative.

### 2. Server-Sent Events and reconnect

1. The EventSource wire media type is `text/event-stream`, always UTF-8. A
   blank line dispatches an event. Recognized fields are `event`, `data`, `id`,
   and `retry`; unknown fields are ignored. Multiple `data` lines are joined
   with line feeds, an `id` value updates the last-event-ID buffer, and an
   all-digit `retry` value changes the reconnection time. [WHATWG HTML, event
   stream parsing and interpretation](https://html.spec.whatwg.org/multipage/server-sent-events.html#parsing-an-event-stream)
   (accessed 2026-07-21).

2. EventSource reconnects after eligible connection loss. If its last event ID
   is non-empty, the user agent sets `Last-Event-ID` on the reconnect request.
   The value is an opaque UTF-8 string excluding NUL, LF, and CR. A server can
   use HTTP `204 No Content` to tell the user agent to stop reconnecting.
   [WHATWG HTML, processing model and `Last-Event-ID`](https://html.spec.whatwg.org/multipage/server-sent-events.html#the-last-event-id-header)
   (accessed 2026-07-21).

3. The native `EventSource` constructor exposes a URL and an optional
   `withCredentials` flag; it does not expose a general custom-request-header
   map. Reconnection time begins with an implementation-defined value, and a
   user agent may add backoff beyond the server-provided delay. [WHATWG HTML,
   `EventSource` interface and reconnection algorithm](https://html.spec.whatwg.org/multipage/server-sent-events.html#the-eventsource-interface)
   (accessed 2026-07-21).

4. The WHATWG format defines one opaque last-event-ID string and reconnect
   mechanics. It does **not** define Project or User scope, stream identity,
   global or per-stream sequence semantics, authorization on reconnect,
   snapshot format, replay floor, cursor-too-old response, compaction, branch
   identity, or retention duration. Those concepts do not appear in the
   normative wire grammar or processing algorithms. [WHATWG HTML, complete
   Server-Sent Events section](https://html.spec.whatwg.org/multipage/server-sent-events.html)
   (accessed 2026-07-21).

### 3. OpenAPI 3.1 and JSON Schema 2020-12

1. OpenAPI 3.1 Schema Objects inherit JSON Schema Draft 2020-12 parsing and
   support the OAS base dialect. An OpenAPI description can drive documentation,
   client/server generation, and testing, but OpenAPI describes an HTTP API; it
   does not itself define application settlement, authorization, or storage
   semantics. [OpenAPI 3.1.2, parsing and Schema Object](https://spec.openapis.org/oas/v3.1.2.html#schema-object)
   (accessed 2026-07-21).

2. In JSON Schema, `additionalProperties` applies a subschema to object
   properties not covered by `properties` or `patternProperties`; omitting it
   has the same assertion behavior as an empty schema, so extra instance
   properties are accepted by default. `unevaluatedProperties` can constrain
   properties left unevaluated after adjacent composition. [JSON Schema
   2020-12 Core, object applicators](https://json-schema.org/draft/2020-12/json-schema-core#name-additionalproperties)
   (accessed 2026-07-21); [unevaluated properties](https://json-schema.org/draft/2020-12/json-schema-core#name-unevaluatedproperties)
   (accessed 2026-07-21).

3. `anyOf` succeeds when at least one subschema succeeds; `oneOf` requires
   exactly one; `allOf` requires all. OpenAPI's `discriminator` can help select
   or deserialize an alternative but must not change the JSON Schema validation
   result. [JSON Schema 2020-12 Core, logical applicators](https://json-schema.org/draft/2020-12/json-schema-core#name-keywords-for-applying-subschem)
   (accessed 2026-07-21); [OpenAPI 3.1.2, Discriminator Object](https://spec.openapis.org/oas/v3.1.2.html#discriminator-object)
   (accessed 2026-07-21).

4. `enum` accepts only values equal to one of its listed elements. Therefore a
   newly added enum value is not automatically accepted by an older validator.
   [JSON Schema 2020-12 Validation, `enum`](https://json-schema.org/draft/2020-12/json-schema-validation#name-enum)
   (accessed 2026-07-21).

5. Unknown **schema keywords** should be treated as annotations; this is
   distinct from unknown properties in a payload instance. JSON Schema also
   treats lexical formatting as outside its data model, and its behavior for a
   JSON object with duplicate property names is undefined. [JSON Schema
   2020-12 Core, schema keywords and instance data model](https://json-schema.org/draft/2020-12/json-schema-core#name-json-schema-objects-and-keyw)
   (accessed 2026-07-21).

6. OpenAPI's own version string identifies the OAS feature set. `3.1.*` patch
   versions are clarifications/errata and tooling should treat all `3.1.*`
   versions compatibly. This says nothing about the version policy of an API
   described by OpenAPI. [OpenAPI 3.1.2, Versions](https://spec.openapis.org/oas/v3.1.2.html#versions)
   (accessed 2026-07-21).

### 4. JSON canonicalization and digests

1. RFC 8785 defines the JSON Canonicalization Scheme (JCS), an Informational
   RFC rather than an Internet Standards Track specification. JCS produces an
   invariant byte representation using I-JSON constraints, ECMAScript primitive
   serialization, no emitted whitespace, and recursive lexicographic property
   sorting. Inputs must not contain duplicate property names, and JSON numbers
   must be representable as IEEE 754 double precision. [RFC 8785, sections 2-3](https://www.rfc-editor.org/rfc/rfc8785.html#section-2)
   (accessed 2026-07-21).

2. JCS explicitly allows original JSON to travel on the wire while signatures
   or hashes use a canonicalized counterpart. It normalizes property order,
   whitespace, escapes, and numeric serialization, so a JCS digest is a digest
   of canonical semantic JSON bytes, not proof of the exact original wire
   serialization. [RFC 8785, abstract and section 3](https://www.rfc-editor.org/rfc/rfc8785.html)
   (accessed 2026-07-21).

3. RFC 9530 standardizes HTTP `Content-Digest` over actual message content and
   `Repr-Digest` over selected representation data, with named hash algorithms
   and algorithm agility. It does not provide authentication, authorization,
   privacy, object identity, idempotency, or canonical JSON semantics.
   [RFC 9530, sections 1-5](https://www.rfc-editor.org/rfc/rfc9530.html#section-1.2)
   (accessed 2026-07-21).

4. Neither RFC selects StoryOS's internal digest profile. A protocol that needs
   both exact historical wire evidence and content-addressed semantic identity
   must say which bytes each digest covers and retain the original bytes when
   exact re-emission or forensic comparison is required. This follows from the
   different byte domains defined by JCS and RFC 9530; it is not an additional
   requirement imposed by either RFC.

### 5. Problem Details and error semantics

1. RFC 9457 defines `application/problem+json` with standard members `type`,
   `status`, `title`, `detail`, and `instance`, plus problem-specific extension
   members. The `type` URI is the primary machine identifier; `status` is
   advisory and must match the actual HTTP status generated; clients should not
   parse human-facing `detail`; and clients must ignore unrecognized extension
   members. [RFC 9457, sections 3-3.2](https://www.rfc-editor.org/rfc/rfc9457.html#section-3)
   (accessed 2026-07-21).

2. A defined problem type must document its type URI, title, and associated
   HTTP status and may specify `Retry-After`. Problem Details is a carrier for
   application error information, not a replacement for domain-specific
   semantics. [RFC 9457, section 4](https://www.rfc-editor.org/rfc/rfc9457.html#section-4)
   (accessed 2026-07-21).

3. Problem details must be vetted against information disclosure: the RFC
   warns against exposing implementation internals, data, stack dumps, or other
   exploitable details. Combined with HTTP's permission to hide forbidden
   resources behind `404`, this supports but does not fully specify a
   non-oracular StoryOS error policy. [RFC 9457, section 5](https://www.rfc-editor.org/rfc/rfc9457.html#section-5)
   (accessed 2026-07-21); [RFC 9110, sections 15.5.4-15.5.5](https://www.rfc-editor.org/rfc/rfc9110.html#section-15.5.4)
   (accessed 2026-07-21).

4. RFC 9457 does not define retryability, settlement state, an
   `OutcomeUnknown` condition, idempotency-key mismatch, replay-floor failure,
   lease/fence failure, or a StoryOS conflict taxonomy. A response lost after a
   non-idempotent request cannot be made unambiguous by Problem Details because
   no response was received; HTTP instead cautions against automatically
   retrying such requests. [RFC 9110, section 9.2.2](https://www.rfc-editor.org/rfc/rfc9110.html#section-9.2.2)
   (accessed 2026-07-21).

### 6. Version compatibility and historical wire bytes

1. The reviewed standards version their own formats or protocol revisions, but
   none defines a universal application rule for additive versus breaking
   changes, N/N-1 support, or simultaneous API/envelope/payload/event-schema
   versions. OpenAPI explicitly versions the **OpenAPI specification**, while
   JSON Schema validates an instance against a selected schema. [OpenAPI 3.1.2,
   Versions](https://spec.openapis.org/oas/v3.1.2.html#versions)
   (accessed 2026-07-21); [JSON Schema 2020-12 Core, dialects and vocabularies](https://json-schema.org/draft/2020-12/json-schema-core#name-schema-vocabularies)
   (accessed 2026-07-21).

2. JSON Schema's default acceptance of extra object properties can support
   additive evolution, while `additionalProperties: false`,
   `unevaluatedProperties: false`, exact `enum`, and ambiguous `oneOf` branches
   can make changes breaking. The standards define validation outcomes; they do
   not declare which StoryOS surfaces should be strict or tolerant.

3. JSON Schema intentionally ignores lexical JSON differences, and JCS
   intentionally canonicalizes them. Neither requires retaining received
   historical wire bytes after an upgrade. Exact wire-byte retention, schema
   lookup for historical records, re-decoding policy, and whether old bytes are
   served unchanged are StoryOS storage and compatibility contracts. [JSON
   Schema 2020-12 Core, instance data model](https://json-schema.org/draft/2020-12/json-schema-core#name-instance-data-model)
   (accessed 2026-07-21); [RFC 8785](https://www.rfc-editor.org/rfc/rfc8785.html)
   (accessed 2026-07-21).

### 7. MCP and MCP Apps facts

1. As of the audit date, the official MCP site marks protocol revision
   `2025-11-25` as latest. Its initialization handshake exchanges one requested
   protocol version and capabilities; the server returns the same supported
   version or another it supports, and an incompatible client should
   disconnect. Subsequent HTTP requests carry `MCP-Protocol-Version`.
   [MCP 2025-11-25 Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
   (accessed 2026-07-21).

2. MCP Streamable HTTP defines a more specific SSE resumption profile than the
   generic WHATWG format: event IDs must be unique across the relevant session
   or client streams, should encode originating-stream information, resumption
   uses GET plus `Last-Event-ID`, and a server must not replay messages from a
   different stream. This is an MCP transport contract, not a generic SSE rule
   and not automatically the StoryOS Client/Server event contract. [MCP
   2025-11-25 Transports, Resumability and Redelivery](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports#resumability-and-redelivery)
   (accessed 2026-07-21).

3. The stable MCP Apps profile is `2026-01-26` with extension identifier
   `io.modelcontextprotocol/ui`. It uses JSON-RPC 2.0 over `postMessage` between
   View and Host, requires an MCP-like `ui/initialize` /
   `ui/notifications/initialized` handshake, exchanges app/host capabilities,
   and requires the Host to validate incoming View messages. A web Host must
   interpose a different-origin Sandbox proxy and cannot send View messages
   before initialization completes. [MCP Apps stable specification at its
   first-party pinned commit](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#communication-protocol)
   (accessed 2026-07-21).

4. JSON-RPC request `id` correlates a response to a request; a server must echo
   the same value. Notifications omit `id`, receive no response, and are not
   confirmable. JSON-RPC IDs therefore provide RPC correlation, not Project
   authorization, durable command identity, idempotency, or acceptance proof.
   [JSON-RPC 2.0, Request, Notification, and Response Objects](https://www.jsonrpc.org/specification#request_object)
   (accessed 2026-07-21).

5. MCP/MCP Apps negotiate external protocol revision and capabilities, but the
   cited specifications do not bind a message to StoryOS's exact requester,
   Project Scope, AgentRun/RunStep, Capability/Approval, Credential Reference,
   App instance, Artifact revision, or Proposal/Acceptance. Nor do they define
   how a stored StoryOS record migrates when an MCP server, App profile, Tool
   schema, or Provider API changes. Those bindings and compatibility records
   remain Host/Core adapter contracts.

## Non-standard decisions StoryOS must own

The following are not settled by the cited standards and must be explicit in
the StoryOS protocol. This list identifies ownership; it does not choose values
that the foundation ticket has not accepted.

1. **Surface boundaries:** which envelopes are stable Client-to-Server public
   protocol, Core contracts, internal worker/adapter contracts, and untrusted
   Provider/Tool/MCP/App boundary messages.
2. **Identity and authorization binding:** exact requester and server-derived
   User, exact Project Scope, selected Project, Run/Step, command/query/event
   and Artifact identity, correlation/causation, and which identifiers are
   non-authoritative client claims.
3. **Command settlement:** synchronous durable acknowledgement versus eventual
   settlement, status-resource or query shape, receipt identity, and how the
   client distinguishes rejected, pending, committed, failed, and
   `OutcomeUnknown`.
4. **Idempotency:** key namespace and scope, entropy and length bounds, expiry,
   digest profile and covered fields, duplicate-in-flight behavior, completed
   result replay, mismatch response, and behavior after retention or upgrade.
5. **Concurrency:** how domain `expected_head` / `expected_revision` relates to
   HTTP validators, which failures are `412`, `409`, or application problem
   types, and how leases, fence generations, Attempts, and late results appear
   on the wire.
6. **Query consistency:** projection watermark, Snapshot identity, pagination
   ordering, page-size bounds, redaction, and cross-scope non-oracle behavior.
7. **SSE cursor profile:** exact cursor scope and encoding, Event ID and stream
   sequence relationship, reconnect re-authorization, replay floor,
   cursor-too-old response, Snapshot handoff, and compaction/branch/retention
   ownership.
8. **Artifact and payload profile:** logical identity versus revision versus
   digest, inline-versus-reference threshold, media type and schema identity,
   immutable payload semantics, and hard byte/item/depth bounds.
9. **Error vocabulary:** stable machine code or problem `type`, human-safe
   detail, retryability, non-oracular equivalence classes, occurrence identity,
   and explicit `OutcomeUnknown` and reconciliation instructions.
10. **Schema evolution:** independent API/envelope/payload/event versions,
    strict versus tolerant fields and enums per surface, additive and breaking
    rules, N/N-1 window, downgrade behavior, and migration owner.
11. **Historical evidence:** whether to retain raw received/sent bytes alongside
    decoded typed records, the canonicalization and digest profile, historical
    schema registry, and post-upgrade verification/re-emission behavior.
12. **External compatibility:** negotiated MCP and MCP Apps revisions,
    capability snapshots, Tool schema/version, Provider/API/model revision,
    unsupported-version behavior, and the exact Host binding from untrusted
    external RPC IDs/messages to StoryOS scope, Capability, Approval, and
    durable records.

## Implications

These are evidence-backed constraints for the foundation protocol, not claims
that an external standard has selected the final StoryOS design.

1. A command response needs a StoryOS settlement contract in addition to an
   HTTP status. `202` can acknowledge asynchronous work, but the protocol must
   identify the durable command/receipt and the authoritative way to observe
   settlement; a later SSE event is a new message, not a delayed HTTP status.
2. `If-Match`/ETag is a useful HTTP concurrency primitive only where the ETag
   denotes the same state being guarded. Domain expected Heads still need an
   explicit mapping, especially for commands spanning more than one object.
3. A replay-safe `POST` needs a StoryOS-defined idempotency record. Treat the
   expired IETF draft as design input and fixtures, not as authority that fills
   in Project Scope, request digest, retention, or `OutcomeUnknown`.
4. An SSE `id` can carry or reference a cursor, but the server must validate its
   stream/Project binding and re-authorize every reconnect. The cursor must not
   become a bearer capability merely because browsers echo it as
   `Last-Event-ID`.
5. Snapshot, replay, and cursor-too-old must be explicit application messages or
   errors. Generic EventSource cannot infer them, and the retention ticket must
   later own the replay-floor duration without changing cursor scope semantics.
6. Schema strictness should be deliberate per direction. Closed command inputs
   can reject unknown fields, while compatible outputs/events may need additive
   fields; exact enums require an explicit unknown-value strategy because old
   schemas reject new values.
7. Canonical semantic digests and exact wire-byte evidence are different
   artifacts. Every digest should identify its algorithm and byte profile; if
   historical wire bytes matter, store them rather than expecting JCS or JSON
   Schema to reconstruct them.
8. `application/problem+json` can carry StoryOS errors without inventing a new
   generic HTTP error format, but StoryOS still needs stable problem types or
   extension codes for conflict, precondition, denial/not-found, retryability,
   cursor age, idempotency mismatch, fencing, and `OutcomeUnknown`.
9. MCP and MCP Apps version negotiation belongs at the external adapter/Host
   boundary. Record the negotiated revision and capabilities, but never treat
   an MCP session, JSON-RPC ID, App message, Tool visibility flag, or Provider
   version as StoryOS authorization or authoritative creative state.
10. OpenAPI/JSON Schema generation and a golden wire corpus can verify declared
    compatibility, but the standards cannot choose StoryOS's compatibility
    promise. The foundation spec must state the promise first, including hard
    bounds and adversarial fixtures, then generation and drift checks can enforce
    it.

## Primary source set

- [RFC 9110: HTTP Semantics](https://www.rfc-editor.org/rfc/rfc9110.html)
- [RFC 6585: Additional HTTP Status Codes](https://www.rfc-editor.org/rfc/rfc6585.html)
- [IETF HTTPAPI Idempotency-Key Internet-Draft](https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/)
- [WHATWG HTML: Server-Sent Events](https://html.spec.whatwg.org/multipage/server-sent-events.html)
- [OpenAPI Specification 3.1.2](https://spec.openapis.org/oas/v3.1.2.html)
- [JSON Schema Draft 2020-12 Core](https://json-schema.org/draft/2020-12/json-schema-core)
- [JSON Schema Draft 2020-12 Validation](https://json-schema.org/draft/2020-12/json-schema-validation)
- [RFC 8785: JSON Canonicalization Scheme](https://www.rfc-editor.org/rfc/rfc8785.html)
- [RFC 9530: Digest Fields](https://www.rfc-editor.org/rfc/rfc9530.html)
- [RFC 9457: Problem Details for HTTP APIs](https://www.rfc-editor.org/rfc/rfc9457.html)
- [JSON-RPC 2.0 Specification](https://www.jsonrpc.org/specification)
- [MCP 2025-11-25 Lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle)
- [MCP 2025-11-25 Transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
- [MCP Apps stable profile 2026-01-26](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx)

All sources in this index were accessed on 2026-07-21.
