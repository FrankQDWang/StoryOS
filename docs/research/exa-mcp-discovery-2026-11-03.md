# Exa MCP Discovery Record, 2026-11-03

This report records one live `Discover` exchange of the MCP client adapter `storyos-adapter-mcp-client` against the remote Exa MCP server, for [S5-01a](https://github.com/FrankQDWang/StoryOS/issues/1395) under [ADR 0083](../adr/0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md). The record is an untrusted observation. It has no Registration identity, no enablement, and no execution authority. S5-01b ([#1396](https://github.com/FrankQDWang/StoryOS/issues/1396)) maps it to exact trusted ToolSpecs.

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

## Appendix: the complete Discovery Record

This is the stdout of exchange 2, as `storyos-worker --discover-exa` wrote it. It holds each tool description, each input schema, the output schema of `agent_run`, and the annotations. It holds no key.

```json
{
  "server_url": "https://mcp.exa.ai/mcp",
  "server_name": "exa-search-server",
  "server_version": "3.5.0",
  "protocol_revision": "2025-11-25",
  "tools": [
    {
      "name": "web_search_exa",
      "description": "Search the web for any topic and get clean, ready-to-use content.\n\n      Best for: Finding current information, news, facts, people, companies, or answering questions about any topic.\n      Returns: Clean text content from top search results.\n\n      Query tips:\n      describe the ideal page, not keywords. \"blog post comparing React and Vue performance\" not \"React vs Vue\".\n      Use category:people / category:company to search through Linkedin profiles / companies respectively.\n      If highlights are insufficient, follow up with web_fetch_exa on the best URLs.",
      "input_schema": {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "additionalProperties": false,
        "properties": {
          "numResults": {
            "description": "Number of search results to return (default: 10).",
            "type": "number"
          },
          "objective": {
            "description": "Goal for this search turn; say which documents should rank first, which should be excluded, and what specific facts or figures to pull from them.",
            "maxLength": 4096,
            "minLength": 1,
            "type": "string"
          },
          "query": {
            "description": "Natural language search query. Should be a semantically rich description of the ideal page, not just keywords. Optionally include category:<type> (company, people) to focus results — e.g. 'category:people John Doe software engineer'.",
            "minLength": 1,
            "type": "string"
          }
        },
        "required": [
          "query",
          "objective"
        ],
        "type": "object"
      },
      "input_schema_digest": "sha256:ea70b6dbd4ca5363f0ac82944520016e45d810ecfa332c39aa1c6ece65f2e4df",
      "output_schema": null,
      "annotations": {
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": true,
        "readOnlyHint": true
      }
    },
    {
      "name": "web_fetch_exa",
      "description": "Read a webpage's full content as clean markdown. Use after web_search_exa when highlights are insufficient or to read any URL.\n\nBest for: Extracting full content from known URLs. Batch multiple URLs in one call.\nReturns: Clean text content and metadata from the page(s).",
      "input_schema": {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "additionalProperties": false,
        "properties": {
          "maxCharacters": {
            "description": "Maximum characters to extract per page (default: 3000)",
            "minimum": 1,
            "type": "number"
          },
          "urls": {
            "description": "URLs to read. Batch multiple URLs in one call.",
            "items": {
              "type": "string"
            },
            "type": "array"
          }
        },
        "required": [
          "urls"
        ],
        "type": "object"
      },
      "input_schema_digest": "sha256:d72ee7687517df49ee4374713985811b9a87c252590b2ef26c9218786b0f805b",
      "output_schema": null,
      "annotations": {
        "destructiveHint": false,
        "idempotentHint": true,
        "openWorldHint": true,
        "readOnlyHint": true
      }
    },
    {
      "name": "agent_run",
      "description": "Start or resume an Exa Agent run for multi-step research, list-building, or enrichment. Use it instead of running many searches yourself when a task needs multiple searches, cross-source verification, or a structured table of results. Long-running: returns a run ID; resume with runId when the tool reports the run is still running. Not for a single question or lookup (use web_search_exa) or reading a known URL (use web_fetch_exa). An interrupted tool call is not an explicit cancellation request.",
      "input_schema": {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "additionalProperties": false,
        "properties": {
          "budget": {
            "additionalProperties": false,
            "description": "Per-run spend and duration limits for the metered auto and ultra efforts. Set it when the user names a cost or time limit.",
            "properties": {
              "maxCostDollars": {
                "description": "Maximum spend for the run in US dollars, $1–$100. Only for effort auto or ultra; defaults to $5 for auto and $20 for ultra.",
                "type": "number"
              },
              "maxDurationSeconds": {
                "description": "Soft wall-clock limit in seconds, 300–10800. Only for effort ultra; the run returns what it has found as it nears the limit.",
                "type": "integer"
              }
            },
            "type": "object"
          },
          "dataSources": {
            "description": "Exa Connect providers to enable for this run.",
            "items": {
              "additionalProperties": false,
              "properties": {
                "provider": {
                  "description": "Exa Connect provider ID. Self-serve providers: fiber, financial_datasets, similarweb, baselayer, affiliate, particle, jinko, polymarket, macrobond. Other providers work only when enabled for your Exa team.",
                  "minLength": 1,
                  "type": "string"
                }
              },
              "required": [
                "provider"
              ],
              "type": "object"
            },
            "maxItems": 5,
            "type": "array"
          },
          "effort": {
            "description": "Agent effort level. Defaults to low.",
            "enum": [
              "minimal",
              "low",
              "medium",
              "high",
              "xhigh",
              "ultra",
              "auto"
            ],
            "type": "string"
          },
          "input": {
            "additionalProperties": false,
            "properties": {
              "data": {
                "description": "Known rows/entities to enrich or process.",
                "items": {
                  "additionalProperties": {},
                  "type": "object"
                },
                "type": "array"
              },
              "exclusion": {
                "description": "Entities, rows, or records Agent should avoid returning again.",
                "items": {
                  "additionalProperties": {},
                  "type": "object"
                },
                "type": "array"
              }
            },
            "type": "object"
          },
          "outputSchema": {
            "additionalProperties": {},
            "description": "JSON Schema describing the desired research output. Prefer a top-level object with bounded arrays and source/evidence fields.",
            "type": "object"
          },
          "previousRunId": {
            "description": "ID of a completed prior run to use as context for a new run.",
            "pattern": "^agent_run_",
            "type": "string"
          },
          "query": {
            "description": "Natural-language research or enrichment objective. Provide query or runId, not both.",
            "minLength": 1,
            "type": "string"
          },
          "runId": {
            "description": "ID returned by an earlier call. Use it to check or continue waiting for the same run. Do not start a duplicate run.",
            "pattern": "^agent_run_",
            "type": "string"
          },
          "systemPrompt": {
            "description": "System-level guidance for the Agent.",
            "type": "string"
          }
        },
        "type": "object"
      },
      "input_schema_digest": "sha256:61e7fb95bcab527d9f232983ef97eafab683af7fbb65f8e1ab1d18c0fb1da5b0",
      "output_schema": {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "additionalProperties": false,
        "properties": {
          "costDollars": {
            "additionalProperties": {},
            "type": "object"
          },
          "error": {
            "additionalProperties": {},
            "type": "object"
          },
          "id": {
            "description": "agent_run_... ID. Pass as runId to resume a running run.",
            "type": "string"
          },
          "message": {
            "description": "Next-step guidance for running runs.",
            "type": "string"
          },
          "output": {
            "anyOf": [
              {
                "additionalProperties": true,
                "properties": {
                  "files": {
                    "items": {
                      "additionalProperties": {},
                      "type": "object"
                    },
                    "type": "array"
                  },
                  "grounding": {
                    "description": "Per-field citations and confidence."
                  },
                  "structured": {
                    "description": "Output matching the caller-supplied outputSchema, if any."
                  },
                  "text": {
                    "anyOf": [
                      {
                        "minLength": 0,
                        "type": "string"
                      },
                      {
                        "type": "null"
                      }
                    ],
                    "description": "Prose answer."
                  }
                },
                "type": "object"
              },
              {
                "type": "null"
              }
            ]
          },
          "outputReady": {
            "description": "True only when status is completed.",
            "type": "boolean"
          },
          "status": {
            "enum": [
              "completed",
              "running",
              "failed",
              "cancelled"
            ],
            "type": "string"
          },
          "stopReason": {
            "anyOf": [
              {
                "minLength": 0,
                "type": "string"
              },
              {
                "type": "null"
              }
            ],
            "description": "Why the run stopped, e.g. complete, cancelled, error."
          },
          "success": {
            "description": "True for completed and running runs; false for failed and cancelled.",
            "type": "boolean"
          },
          "usage": {
            "additionalProperties": {},
            "type": "object"
          }
        },
        "required": [
          "success",
          "id",
          "status",
          "outputReady"
        ],
        "type": "object"
      },
      "annotations": {
        "destructiveHint": false,
        "idempotentHint": false,
        "openWorldHint": true,
        "readOnlyHint": true
      }
    }
  ]
}
```
