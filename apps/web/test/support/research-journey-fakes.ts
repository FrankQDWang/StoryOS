import { readFileSync } from "node:fs";
import { createServer } from "node:http";
import type { IncomingMessage, Server, ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

/** The JSON body of one request that a fake received. */
export type ReceivedBody = Record<string, unknown>;

/** The SSE events of one Agent Plan response, made from the request body and the earlier bodies. */
export type ModelReply = (body: ReceivedBody, earlier: readonly ReceivedBody[]) => readonly unknown[];

export interface FakeDestination {
  readonly base: string;
  /** Each request body, in arrival order. */
  readonly bodies: ReceivedBody[];
  readonly close: () => Promise<void>;
}

async function readBody(request: IncomingMessage): Promise<ReceivedBody> {
  const chunks: Buffer[] = [];
  for await (const chunk of request) chunks.push(chunk as Buffer);
  const text = Buffer.concat(chunks).toString("utf8");
  return text.length === 0 ? {} : JSON.parse(text) as ReceivedBody;
}

async function listen(
  handle: (body: ReceivedBody, request: IncomingMessage, response: ServerResponse) => void,
  path: string,
): Promise<FakeDestination> {
  const bodies: ReceivedBody[] = [];
  const server: Server = createServer((request, response) => {
    void readBody(request).then((body) => {
      if (request.method !== "POST") {
        response.writeHead(405).end();
        return;
      }
      bodies.push(body);
      handle(body, request, response);
    }, () => response.writeHead(400).end());
  });
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  return {
    base: `http://127.0.0.1:${port}${path}`,
    bodies,
    close: () => new Promise((resolve) => {
      server.closeAllConnections();
      server.close(() => resolve());
    }),
  };
}

/** A fake Agent Plan Responses route that answers each request with the next reply, as the Postgres tests do. */
export function fakeAgentPlan(replies: readonly ModelReply[]): Promise<FakeDestination> {
  let next = 0;
  let destination: FakeDestination | undefined;
  const started = listen((body, _request, response) => {
    const reply = replies[next];
    next += 1;
    if (reply === undefined || destination === undefined) {
      response.writeHead(500).end();
      return;
    }
    response.writeHead(200, { "content-type": "text/event-stream" });
    for (const event of reply(body, destination.bodies.slice(0, -1))) response.write(`data: ${JSON.stringify(event)}\n\n`);
    response.end();
  }, "/api/plan/v3");
  return started.then((started) => {
    destination = started;
    return started;
  });
}

function completed(id: string, output: readonly unknown[]): unknown[] {
  return [
    { type: "response.created", response: { id, status: "in_progress", output: [] } },
    ...output.map((item, index) => ({ type: "response.output_item.done", output_index: index, item })),
    { type: "response.completed", response: { id, status: "completed", output,
      usage: { input_tokens: 90, output_tokens: 30 } } },
  ];
}

/** One response that requests each `[call_id, function name, arguments]` call in order. */
export function callsReply(id: string, calls: readonly (readonly [string, string, unknown])[]): ModelReply {
  return () => completed(id, calls.map(([callId, name, args], index) => ({
    id: `fc_${index}`, type: "function_call", status: "completed", call_id: callId, name, arguments: JSON.stringify(args),
  })));
}

/** One response that ends with one assistant message of this text. */
export function messageReply(id: string, text: (body: ReceivedBody, earlier: readonly ReceivedBody[]) => string): ModelReply {
  return (body, earlier) => completed(id, [{ id: "msg_01", type: "message", role: "assistant", status: "completed",
    content: [{ type: "output_text", text: text(body, earlier) }] }]);
}

/** The `function_call_output` items of a request body. */
export function functionOutputs(body: ReceivedBody): { call_id: string; output: string }[] {
  const input = Array.isArray(body.input) ? body.input as Record<string, unknown>[] : [];
  return input.filter((item) => item.type === "function_call_output")
    .map((item) => ({ call_id: String(item.call_id), output: String(item.output) }));
}

/** The identities that the `tool_sources` tags of the function outputs name after `tag`, for example `<source id="`. */
export function taggedIdentities(body: ReceivedBody, tag: string): string[] {
  return functionOutputs(body).flatMap(({ output }) => output.split(tag).slice(1).map((rest) => rest.split('"')[0]!));
}

// The Exa result shapes of the live records `exa-first-governed-call-2026-11-10.md` and
// `exa-result-reentry-2026-11-10.md`: titled search entries with a `---` separator, and page blocks
// with an error line for each URL that the server could not read.
export const SEARCH_QUERY = "Liverpool tide table 1890 high water";
export const PAGE_URL = "https://example.org/liverpool-tides-1890";
export const MISSING_URL = "https://example.org/missing-harbour-log";
export const SEARCH_TEXT = `Title: Liverpool tide table 1890\nURL: ${PAGE_URL}\nPublished: 1890-01-01T00:00:00.000Z\nAuthor: N/A\n`
  + "Highlights:\nHigh water at the Prince's Dock came at 10.42.\n\n---\n\n"
  + "Title: Mersey port log\nURL: https://example.org/mersey-port-log\nPublished: N/A\nAuthor: N/A\nHighlights:\n"
  + "The port log notes a calm morning.";
const PAGE_TEXT = `# Liverpool tide table 1890\nURL: ${PAGE_URL}\n\nHigh water at the Prince's Dock came at 10.42 in the morning.`;

/** The tool list of the Discovery Record appendix of `exa-mcp-discovery-2026-11-03.md`, as `tools/list` gives it. */
function recordedTools(repositoryRoot: string): unknown {
  const record = readFileSync(`${repositoryRoot}/docs/research/exa-mcp-discovery-2026-11-03.md`, "utf8");
  const appendix = record.slice(record.indexOf("## Appendix"));
  const start = appendix.indexOf("```json\n") + "```json\n".length;
  const recorded = JSON.parse(appendix.slice(start, appendix.indexOf("\n```", start))) as {
    tools: { name: string; description: string; input_schema: unknown; output_schema: unknown; annotations: unknown }[];
  };
  return { tools: recorded.tools.map((tool) => ({ name: tool.name, description: tool.description,
    inputSchema: tool.input_schema, annotations: tool.annotations,
    ...(tool.output_schema === null ? {} : { outputSchema: tool.output_schema }) })) };
}

/** The text of one `tools/call`: the search entries, or one page block for each URL. Only `PAGE_URL` can be read. */
function callText(params: { name: string; arguments: { urls?: string[] } }): string {
  if (params.name === "web_search_exa") return SEARCH_TEXT;
  return (params.arguments.urls ?? [])
    .map((url) => url === PAGE_URL ? PAGE_TEXT : `Error fetching ${url}: CRAWL_NOT_FOUND`).join("\n\n");
}

/** A fake Exa MCP server with the recorded tool list. It answers each JSON-RPC request by its method. */
export function fakeExaMcp(repositoryRoot: string): Promise<FakeDestination & { readonly calls: () => ReceivedBody[] }> {
  const tools = recordedTools(repositoryRoot);
  return listen((body, _request, response) => {
    const answer = (result: unknown) => response.writeHead(200, { "content-type": "application/json" })
      .end(JSON.stringify({ jsonrpc: "2.0", id: body.id, result }));
    switch (body.method) {
      case "initialize":
        answer({ protocolVersion: "2025-11-25", capabilities: { tools: {} },
          serverInfo: { name: "exa-search-server", version: "3.5.0" } });
        return;
      case "tools/list":
        answer(tools);
        return;
      case "tools/call":
        answer({ content: [{ type: "text", text: callText(body.params as Parameters<typeof callText>[0]) }], isError: false });
        return;
      default:
        response.writeHead(202).end();
    }
  }, "/mcp").then((started) => ({
    ...started,
    calls: () => started.bodies.filter((body) => body.method === "tools/call"),
  }));
}
