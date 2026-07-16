import { createHash } from "node:crypto";
import { createServer } from "node:http";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { renderMode, transcriptProjection } from "./model.mjs";
import { hostPage, proxyPage } from "./pages.mjs";
import { RESOURCE_MIME, RESOURCE_URI } from "./resource-fixture.mjs";
import { dispatch, loadState } from "./store.mjs";

const sourceDirectory = dirname(fileURLToPath(import.meta.url));
const prototypeDirectory = resolve(sourceDirectory, "..");
const statePath = process.env.STORYOS_PROTOTYPE_STATE
  ?? resolve(prototypeDirectory, ".scratch/storyos-state.json");
const hostPort = Number(process.env.STORYOS_PROTOTYPE_HOST_PORT ?? 4181);
const sandboxPort = Number(process.env.STORYOS_PROTOTYPE_SANDBOX_PORT ?? 4182);
const mcpPort = Number(process.env.STORYOS_PROTOTYPE_MCP_PORT ?? 4183);
const hostOrigin = `http://localhost:${hostPort}`;
const sandboxOrigin = `http://127.0.0.1:${sandboxPort}`;
const mcpOrigin = `http://127.0.0.1:${mcpPort}`;

let liveOverlay = null;
let writeQueue = Promise.resolve();
const sseClients = new Set();

await apply({ type: "host_started", processId: process.pid });

const hostServer = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, hostOrigin);
    if (request.method === "GET" && url.pathname === "/") {
      return html(response, 200, hostPage({ hostOrigin, sandboxOrigin }), {
        "content-security-policy": `default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; connect-src 'self'; frame-src ${sandboxOrigin} data:`,
        "permissions-policy": "camera=(), microphone=(), geolocation=(), clipboard-read=(), clipboard-write=()"
      });
    }
    if (request.method === "GET" && url.pathname === "/health") {
      return json(response, 200, { ok: true, processId: process.pid });
    }
    if (request.method === "GET" && url.pathname === "/api/snapshot") {
      return json(response, 200, await snapshot());
    }
    if (request.method === "GET" && url.pathname === "/api/events") {
      response.writeHead(200, {
        "cache-control": "no-cache",
        connection: "keep-alive",
        "content-type": "text/event-stream"
      });
      sseClients.add(response);
      response.write(`data: ${JSON.stringify(await snapshot())}\n\n`);
      request.on("close", () => sseClients.delete(response));
      return;
    }
    if (request.method === "POST" && url.pathname === "/api/bootstrap") {
      return json(response, 200, await bootstrap());
    }
    if (request.method === "POST" && url.pathname === "/api/live/start") {
      await apply({ type: "start_live_call" });
      liveOverlay = { toolCallId: "tool-live-1", chunks: ["partial: one"], status: "streaming" };
      await broadcast();
      return json(response, 200, await snapshot());
    }
    if (request.method === "POST" && url.pathname === "/api/live/append") {
      if (!liveOverlay) return json(response, 409, { error: "no live overlay" });
      liveOverlay.chunks.push("partial: two");
      await broadcast();
      return json(response, 200, await snapshot());
    }
    if (request.method === "POST" && url.pathname === "/api/live/finish") {
      await apply({ type: "finish_live_call" });
      liveOverlay = null;
      await broadcast();
      return json(response, 200, await snapshot());
    }
    if (request.method === "POST" && url.pathname === "/api/wait") {
      const body = await readJson(request);
      const requestId = body.requestId ?? "crash-wait-request-1";
      const waitId = body.waitId ?? `wait-${requestId}`;
      const state = await apply({
        type: "create_app_edit_wait",
        requestId,
        waitId,
        requestedEdit: body.requestedEdit ?? "The lighthouse waited beyond the salt marsh."
      });
      return json(response, 200, { waitId, state });
    }
    if (request.method === "POST" && url.pathname === "/api/approve") {
      const body = await readJson(request);
      return json(response, 200, await apply({
        type: "resolve_wait_to_proposal",
        waitId: body.waitId
      }));
    }
    if (request.method === "POST" && url.pathname === "/api/accept") {
      const body = await readJson(request);
      return json(response, 200, await apply({
        type: "accept_proposal",
        proposalId: body.proposalId
      }));
    }
    if (request.method === "POST" && url.pathname === "/api/interrupt/start") {
      return json(response, 200, await apply({ type: "start_interruptible_call" }));
    }
    if (request.method === "POST" && url.pathname === "/api/interrupt") {
      const body = await readJson(request);
      return json(response, 200, await apply({
        type: "interrupt_call",
        effectOutcome: body.effectOutcome ?? "unknown_external_effect"
      }));
    }
    if (request.method === "POST" && url.pathname === "/api/compact") {
      return json(response, 200, await apply({ type: "compact_model_context" }));
    }
    if (request.method === "POST" && url.pathname === "/api/fork") {
      return json(response, 200, await apply({ type: "fork_branch" }));
    }
    if (request.method === "POST" && url.pathname === "/api/resource/loss") {
      return json(response, 200, await apply({ type: "simulate_resource_loss" }));
    }
    if (request.method === "POST" && url.pathname === "/api/bridge") {
      return json(response, 200, await mediateBridge(await readJson(request)));
    }
    if (request.method === "POST" && url.pathname === "/api/audit") {
      const body = await readJson(request);
      const state = await apply({
        type: "record_bridge_denial",
        requestId: body.requestId,
        method: body.method,
        reason: body.reason
      });
      return json(response, 200, state);
    }
    if (request.method === "POST" && url.pathname === "/api/evidence") {
      const body = await readJson(request);
      return json(response, 200, await apply({
        type: "record_evidence",
        case: body.case,
        passed: body.passed,
        detail: body.detail
      }));
    }
    return json(response, 404, { error: "not found" });
  } catch (error) {
    return json(response, 500, { error: error.message, stack: error.stack });
  }
});

const sandboxServer = createServer((request, response) => {
  const url = new URL(request.url, sandboxOrigin);
  if (request.method === "GET" && url.pathname === "/health") {
    return json(response, 200, { ok: true, processId: process.pid });
  }
  if (request.method === "GET" && url.pathname === "/proxy.html") {
    return html(response, 200, proxyPage({ hostOrigin }), {
      "content-security-policy": "default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; frame-src 'self' data: blob:",
      "permissions-policy": "camera=(), microphone=(), geolocation=(), clipboard-read=(), clipboard-write=()",
      "cross-origin-resource-policy": "cross-origin"
    });
  }
  return json(response, 404, { error: "not found" });
});

hostServer.listen(hostPort, "127.0.0.1", () => {
  process.stdout.write(`StoryOS host listening on ${hostOrigin}\n`);
});
sandboxServer.listen(sandboxPort, "127.0.0.1", () => {
  process.stdout.write(`Sandbox proxy listening on ${sandboxOrigin}\n`);
});

async function bootstrap() {
  const state = await loadState(statePath);
  if (state.toolCalls.some((call) => call.id === "tool-library-1")) {
    await apply({ type: "bootstrap_completed" });
    return snapshot();
  }

  const input = { query: "available story sources" };
  const toolResponse = await callMcp({
    jsonrpc: "2.0",
    id: "tool-library-rpc-1",
    method: "tools/call",
    params: { name: "story.library.summary", arguments: input }
  });
  if (toolResponse.error) throw new Error(toolResponse.error.message);

  const resourceResponse = await callMcp({
    jsonrpc: "2.0",
    id: "resource-library-rpc-1",
    method: "resources/read",
    params: { uri: RESOURCE_URI }
  });
  if (resourceResponse.error) throw new Error(resourceResponse.error.message);
  const resource = validateResource(resourceResponse.result?.contents?.[0]);
  await apply({
    type: "bootstrap_completed",
    input,
    result: toolResponse.result,
    invocationCount: toolResponse.result._meta.invocationCount,
    resource
  });
  return snapshot();
}

async function mediateBridge(message) {
  const requestId = message?.rpc?.id ?? "malformed-bridge-request";
  const deny = async (reason, code = -32600) => {
    await apply({
      type: "record_bridge_denial",
      requestId,
      method: message?.rpc?.method,
      reason
    });
    return { jsonrpc: "2.0", id: requestId, error: { code, message: reason } };
  };

  if (message?.appInstanceId !== "app-instance-library-1"
    || message?.viewId !== "app-view-library") {
    return deny("app_instance_or_view_mismatch", -32001);
  }
  const rpc = message.rpc;
  if (!rpc || rpc.jsonrpc !== "2.0" || rpc.id === undefined || rpc.method !== "tools/call") {
    return deny("malformed_or_unsupported_jsonrpc");
  }
  if (rpc.params?.name !== "story.request_edit"
    || typeof rpc.params?.arguments?.replacement !== "string") {
    return deny("tool_not_exposed_or_schema_invalid", -32002);
  }

  const waitId = `wait-${rpc.id}`;
  await apply({
    type: "create_app_edit_wait",
    requestId: rpc.id,
    waitId,
    requestedEdit: rpc.params.arguments.replacement
  });
  return {
    jsonrpc: "2.0",
    id: rpc.id,
    result: { status: "approval_required", waitId }
  };
}

function validateResource(candidate) {
  if (candidate?.uri !== RESOURCE_URI) throw new Error("resource URI mismatch");
  if (candidate?.mimeType !== RESOURCE_MIME) throw new Error("resource MIME mismatch");
  if (typeof candidate.text !== "string" || Buffer.byteLength(candidate.text) > 100_000) {
    throw new Error("resource missing or oversized");
  }
  return {
    uri: candidate.uri,
    mimeType: candidate.mimeType,
    html: candidate.text,
    digest: digest(candidate.text)
  };
}

async function callMcp(rpc) {
  const response = await fetch(`${mcpOrigin}/mcp`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(rpc)
  });
  if (!response.ok) throw new Error(`fake MCP unavailable: ${response.status}`);
  return response.json();
}

async function apply(action) {
  const operation = writeQueue.then(() => dispatch(statePath, action));
  writeQueue = operation.catch(() => undefined);
  return operation;
}

async function snapshot() {
  const durable = await loadState(statePath);
  const projection = transcriptProjection(durable, liveOverlay);
  projection.views = projection.views.map((view) => ({
    ...view,
    renderMode: renderMode(durable.appViews.find((candidate) => candidate.id === view.id))
  }));
  return { durable, projection, liveOverlay };
}

async function broadcast() {
  const message = `data: ${JSON.stringify(await snapshot())}\n\n`;
  for (const client of sseClients) client.write(message);
}

async function readJson(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  return JSON.parse(Buffer.concat(chunks).toString("utf8") || "{}");
}

function html(response, status, body, headers = {}) {
  response.writeHead(status, { "content-type": "text/html; charset=utf-8", ...headers });
  response.end(body);
}

function json(response, status, body) {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(`${JSON.stringify(body)}\n`);
}

function digest(value) {
  return createHash("sha256").update(value).digest("hex");
}
