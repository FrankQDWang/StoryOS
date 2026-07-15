# MCP Apps host obligations source audit

Audited: 2026-07-14

Scope: source verification for
[`mcp-apps-host-obligations.md`](./mcp-apps-host-obligations.md) against
first-party MCP sources and the pinned OpenAI Codex source used as
non-normative implementation evidence.

This ledger verifies upstream facts. It does not promote StoryOS design
inferences into MCP protocol requirements.

## Verification ledger

### 1. SEP status

- **Claim:** SEP-1865 is the adopted MCP Apps extension record.
- **Status:** Verified.
- **Evidence:** [modelcontextprotocol/modelcontextprotocol#1865](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/1865)
  is closed and was merged on 2026-01-28 as commit
  `54111414d0a0f0864e6c98f77e015bd86dfe2931`.

### 2. Stable protocol identity

- **Claim:** The stable MCP Apps profile is dated `2026-01-26` and uses
  extension identifier `io.modelcontextprotocol/ui`.
- **Status:** Verified.
- **Evidence:** The pinned [stable specification](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1-L53)
  declares `Status: Stable (2026-01-26)`, the identifier, optional negotiation,
  and the initial `text/html;profile=mcp-app` profile.

### 3. Pinned repository and SDK version

- **Claim:** Commit `cf87f2a2c2581b2bc45ff4848aac9fa7e106a576` is a valid
  source snapshot for SDK `1.7.4`.
- **Status:** Verified; no drift at audit time.
- **Evidence:** On 2026-07-14 the official repository's `main` still resolved to
  that commit, the latest release remained [`v1.7.4`](https://github.com/modelcontextprotocol/ext-apps/releases/tag/v1.7.4),
  and the pinned [`package.json`](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/package.json)
  reported `1.7.4`.

### 4. Web sandbox obligations

- **Claim:** A web host must use a cross-origin Sandbox proxy, the required
  sandbox permissions and readiness exchange, and initialization ordering.
- **Status:** Verified.
- **Evidence:** The stable [Sandbox proxy rules](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L470-L508)
  specify the different-origin proxy, `allow-scripts` plus `allow-same-origin`,
  reserved readiness messages, CSP loading, transparent relay, and the ban on
  host-to-View requests or notifications before `initialized`.

### 5. Bridge surface and visibility

- **Claim:** The stable bridge includes mediated server tool/resource operations
  and host UI requests; visibility is not a StoryOS authorization substitute.
- **Status:** Verified, with the authorization conclusion correctly labeled as
  a stricter StoryOS obligation.
- **Evidence:** Stable [resource discovery](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L319-L402),
  [standard messages](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L489-L508),
  and [UI-specific messages](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L959-L1230)
  support the research note's protocol inventory.

### 6. Fallback and deferred lifecycle state

- **Claim:** Stable requires meaningful non-UI `content` and defers persistence,
  restoration, multiple UI resources, and View-to-View communication.
- **Status:** Verified.
- **Evidence:** Stable [graceful degradation](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1492-L1560)
  requires meaningful `content`; [Extensibility](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/2026-01-26/apps.mdx#L1562-L1576)
  explicitly defers the listed features. Durable replay and static fallback are
  therefore correctly identified as StoryOS host design.

### 7. Stable versus draft

- **Claim:** Downloads, View-requested teardown, broader capabilities,
  host-to-App tool calls, and App-provided tools are draft-only relative to the
  stable profile.
- **Status:** Verified.
- **Evidence:** The pinned [draft](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L1-L40)
  is explicitly `Status: Draft`; its [message additions](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L1026-L1511)
  and [App-provided tools](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/specification/draft/apps.mdx#L1738-L2176)
  are absent from the stable document.

### 8. SDK and protocol version separation

- **Claim:** SDK package version must not be treated as negotiated protocol
  version.
- **Status:** Verified.
- **Evidence:** The SDK keeps [`LATEST_PROTOCOL_VERSION = "2026-01-26"`](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/spec.types.ts#L20-L31)
  while the same source tree contains draft-only types. [`AppBridge`](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/app-bridge.ts#L1457-L1490)
  falls back from an unsupported requested version to its latest version.

### 9. SDK forwarding and pinned Codex evidence

- **Claim:** `AppBridge` can automatically forward raw MCP operations, while the
  pinned Codex snapshot stores MCP App context beside durable tool-call history.
- **Status:** Verified as implementation evidence, not normative protocol.
- **Evidence:** Pinned [`AppBridge.connect`](https://github.com/modelcontextprotocol/ext-apps/blob/cf87f2a2c2581b2bc45ff4848aac9fa7e106a576/src/app-bridge.ts#L1792-L1926)
  installs automatic forwarding. OpenAI Codex commit
  [`1f0566d3f59298d1bb88820a0d35294f1eeb07ea`](https://github.com/openai/codex/commit/1f0566d3f59298d1bb88820a0d35294f1eeb07ea)
  contains the cited feature gate, MCP App context, and history reconstruction.

## Errors or drift found

No substantive error or drift was found in the audited claims. The research
consistently separates stable protocol requirements, first-party implementation
evidence, and stricter StoryOS obligations.

## Wording cautions

- Say **stable profile `2026-01-26`**, not “SDK protocol `1.7.4`.”
- Preserve the distinction between MCP `MUST`/`SHOULD` language and StoryOS's
  fail-closed capability, approval, persistence, and replay policy.
- `ui/resource-teardown` is described upstream as a notification but carries a
  JSON-RPC request id and response; “request-shaped teardown message” is clearer.
- The stable document's deprecated flat metadata key says “before GA”; do not
  use that phrase to override the document's explicit `Stable` status.
- Scope “current `main`” and “latest release” observations to this audit date.
- SDK `1.7.4` contains draft-only APIs while its protocol constant remains
  `2026-01-26`; do not infer negotiated support from installed package surface.

## Conclusion

The existing host-obligations research is source-sound for downstream
architecture work. No factual correction is required before using its stable
baseline, mediated bridge, durable replay, static fallback, and fail-closed
boundaries.
