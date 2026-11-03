# Exa MCP Discovery Record, 2026-11-03

This report records one live `Discover` exchange of the MCP client adapter `storyos-adapter-mcp-client` against the hosted Exa MCP server, for [S5-01a](https://github.com/FrankQDWang/StoryOS/issues/1395) under [ADR 0083](../adr/0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md). The record is an untrusted observation. It has no Registration identity, no enablement, and no execution authority. S5-01b ([#1396](https://github.com/FrankQDWang/StoryOS/issues/1396)) maps it to exact trusted ToolSpecs.

## Method

- Command: `storyos-worker --discover-exa` at commit `bd796455`, with `STORYOS_ENV_FILE` set to the worktree `.env` and `STORYOS_LOG=debug`. The Worker settings (ADR 0050) resolved the Credential Reference `settings:STORYOS_EXA_API_KEY`. The key was never printed.
- Transport: MCP revision `2025-11-25`, Streamable HTTP, `x-api-key` header. The exchange sent `initialize`, `notifications/initialized`, and `tools/list`. It sent no `tools/call`, so no search and no page read were metered.
- Plan: posted on [#1395](https://github.com/FrankQDWang/StoryOS/issues/1395#issuecomment-6605114175) before the first request: 1 exchange, at most 2 after a transport failure.

## Request log

| # | Start (UTC) | Result | Diagnostic lines |
| --- | --- | --- | --- |
| 1 | 2026-11-03T13:30:00Z | `not_submitted` after 6 s | `initialize`: 156 request bytes, 0 response bytes. The operation request did not leave. |
| 2 | 2026-11-03T13:31:34Z | `discovered` after 4 s | `initialize` 156 / 1,077 bytes, session tag `8f4eed72`; `notifications/initialized` 54 / 0 bytes; `tools/list` 58 / 6,976 bytes, one page. |

Exchange 1 was a transport failure before the operation request. The client folds a send failure and a 5xx status into `not_submitted` before the operation, so the diagnostic lines do not tell which one occurred. A GET of the same URL from the same host one minute later, with the same HTTP client stack and no key, returned `405 Method Not Allowed`, as the transport specification permits. The network of this host had other TLS timeouts and connection resets in the same hour. Thus exchange 1 is recorded as a transient transport failure. The total is 2 exchanges, within the plan.

## Discovery Record

- Server URL: `https://mcp.exa.ai/mcp`
- Server name and version (`initialize`): `exa-search-server` `3.5.0`
- Protocol revision: `2025-11-25`. The server accepted the pinned revision.
- The server gave a session identifier. The session tag above is the first 8 hex digits of its SHA-256.
- `tools/list` returned one page with three tools. No tool has an output schema except `agent_run`.

| Tool | Input schema digest | Required inputs | Annotations |
| --- | --- | --- | --- |
| `web_search_exa` | `sha256:ea70b6dbd4ca5363f0ac82944520016e45d810ecfa332c39aa1c6ece65f2e4df` | `query`, `objective`; optional `numResults` | read-only, not destructive, idempotent, open world |
| `web_fetch_exa` | `sha256:d72ee7687517df49ee4374713985811b9a87c252590b2ef26c9218786b0f805b` | `urls`; optional `maxCharacters` | read-only, not destructive, idempotent, open world |
| `agent_run` | `sha256:61e7fb95bcab527d9f232983ef97eafab683af7fbb65f8e1ab1d18c0fb1da5b0` | none; optional `budget`, `dataSources`, `effort`, `input`, `outputSchema`, `previousRunId`, `query`, `runId`, `systemPrompt` | read-only, not destructive, not idempotent, open world |

The digest is the SHA-256 of the canonical JSON (sorted keys, no white space, UTF-8 text not escaped) of the `inputSchema`. A separate Python canonicalization of the recorded schemas gave the same three digests.

### Canonical input schemas of the two approved operations

`web_search_exa`:

```json
{"$schema":"http://json-schema.org/draft-07/schema#","additionalProperties":false,"properties":{"numResults":{"description":"Number of search results to return (default: 10).","type":"number"},"objective":{"description":"Goal for this search turn; say which documents should rank first, which should be excluded, and what specific facts or figures to pull from them.","maxLength":4096,"minLength":1,"type":"string"},"query":{"description":"Natural language search query. Should be a semantically rich description of the ideal page, not just keywords. Optionally include category:<type> (company, people) to focus results — e.g. 'category:people John Doe software engineer'.","minLength":1,"type":"string"}},"required":["query","objective"],"type":"object"}
```

`web_fetch_exa`:

```json
{"$schema":"http://json-schema.org/draft-07/schema#","additionalProperties":false,"properties":{"maxCharacters":{"description":"Maximum characters to extract per page (default: 3000)","minimum":1,"type":"number"},"urls":{"description":"URLs to read. Batch multiple URLs in one call.","items":{"type":"string"},"type":"array"}},"required":["urls"],"type":"object"}
```

## Findings for the next tickets

1. **The default tool set has three tools, not two.** ADR 0083 states that `agent_run` is optional and needs a query parameter at the URL. The live server lists `agent_run` with no query parameter. ADR 0083 still holds: only `web_search_exa` and `web_fetch_exa` get a StoryOS ToolSpec, and `agent_run` stays uncallable. S5-01b must refuse it explicitly, and the ADR 0083 context text needs a correction.
2. **`web_search_exa` requires `objective`.** The search operation needs two model inputs, `query` and `objective`, and accepts `numResults`. The ToolSpec of S5-01b and the input validation of S5-03 must pin both required inputs.
3. **The schemas are JSON Schema draft-07.** Each input schema declares `$schema` draft-07 and `additionalProperties: false`. The MCP default of 2020-12 does not apply.
4. **No output schema for the approved operations.** Their results are text blocks only. The shape check of the adapter (text blocks, `isError`, 256 KiB, UTF-8) is the only structural check before S5-03.
5. **The schema descriptions are part of the digest.** A change of description text by Exa changes the digest. S5-01b classifies such drift as descriptive or incompatible.
