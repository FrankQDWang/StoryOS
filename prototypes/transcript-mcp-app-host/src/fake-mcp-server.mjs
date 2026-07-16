import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import { dirname, resolve } from "node:path";

import { loadState, saveState } from "./store.mjs";
import { RESOURCE_MIME, RESOURCE_URI, resourceHtml } from "./resource-fixture.mjs";

const sourceDirectory = dirname(fileURLToPath(import.meta.url));
const prototypeDirectory = resolve(sourceDirectory, "..");
const scratchPath = process.env.STORYOS_PROTOTYPE_MCP_STATE
  ?? resolve(prototypeDirectory, ".scratch/fake-mcp.json");
const port = Number(process.env.STORYOS_PROTOTYPE_MCP_PORT ?? 4183);

async function loadMcpState() {
  const state = await loadState(scratchPath);
  if (state.schema === "storyos.transcript-mcp-app-host.prototype/v1") {
    const initial = { variant: "v1", invocationCount: 0 };
    await saveState(scratchPath, initial);
    return initial;
  }
  return state;
}

async function handleRpc(rpc) {
  if (rpc?.jsonrpc !== "2.0" || rpc.id === undefined || typeof rpc.method !== "string") {
    return rpcError(rpc?.id ?? null, -32600, "invalid JSON-RPC request");
  }

  if (rpc.method === "tools/call" && rpc.params?.name === "story.library.summary") {
    const state = await loadMcpState();
    state.invocationCount += 1;
    await saveState(scratchPath, state);
    return {
      jsonrpc: "2.0",
      id: rpc.id,
      result: {
        content: [{ type: "text", text: "Three indexed story sources are available." }],
        structuredContent: {
          sources: ["manuscript", "character-notes", "research-library"],
          count: 3
        },
        _meta: { ui: { resourceUri: RESOURCE_URI }, invocationCount: state.invocationCount }
      }
    };
  }

  if (rpc.method === "resources/read" && rpc.params?.uri === RESOURCE_URI) {
    const state = await loadMcpState();
    return {
      jsonrpc: "2.0",
      id: rpc.id,
      result: {
        contents: [{
          uri: RESOURCE_URI,
          mimeType: RESOURCE_MIME,
          text: resourceHtml(state.variant)
        }]
      }
    };
  }

  return rpcError(rpc.id, -32601, "method or tool not found");
}

const server = createServer(async (request, response) => {
  try {
    if (request.method === "GET" && request.url === "/health") {
      return json(response, 200, { ok: true });
    }
    if (request.method === "GET" && request.url === "/state") {
      return json(response, 200, await loadMcpState());
    }
    if (request.method === "POST" && request.url === "/control/variant") {
      const body = await readJson(request);
      if (!new Set(["v1", "v2"]).has(body.variant)) {
        return json(response, 400, { error: "variant must be v1 or v2" });
      }
      const state = await loadMcpState();
      state.variant = body.variant;
      await saveState(scratchPath, state);
      return json(response, 200, state);
    }
    if (request.method === "POST" && request.url === "/mcp") {
      return json(response, 200, await handleRpc(await readJson(request)));
    }
    return json(response, 404, { error: "not found" });
  } catch (error) {
    return json(response, 500, { error: error.message });
  }
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(`fake MCP listening on http://127.0.0.1:${port}\n`);
});

function rpcError(id, code, message) {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

async function readJson(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
}

function json(response, status, body) {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(`${JSON.stringify(body)}\n`);
}
