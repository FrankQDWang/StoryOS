import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import path from "node:path";
import { CoreStore } from "./core-store.mjs";

const ownDirectory = path.dirname(fileURLToPath(import.meta.url));
const labRoot = path.resolve(ownDirectory, "..");
const port = Number(process.env.STORYOS_LAB_CORE_PORT ?? 41770);
const statePath =
  process.env.STORYOS_LAB_CORE_STATE ??
  path.join(labRoot, ".lab-state", "core-state.json");
const store = new CoreStore(statePath);
await store.load();

function json(response, status, value) {
  response.writeHead(status, {
    "access-control-allow-origin": "*",
    "access-control-allow-headers": "content-type",
    "access-control-allow-methods": "GET,POST,OPTIONS",
    "content-type": "application/json; charset=utf-8",
  });
  response.end(`${JSON.stringify(value)}\n`);
}

async function readJson(request) {
  const chunks = [];
  for await (const chunk of request) chunks.push(chunk);
  const body = Buffer.concat(chunks).toString("utf8");
  return body === "" ? {} : JSON.parse(body);
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

const server = createServer(async (request, response) => {
  if (request.method === "OPTIONS") {
    json(response, 204, {});
    return;
  }

  const url = new URL(request.url, `http://${request.headers.host}`);
  try {
    if (request.method === "GET" && url.pathname === "/health") {
      json(response, 200, { ok: true, pid: process.pid, snapshot: store.snapshot() });
      return;
    }
    if (request.method === "POST" && url.pathname === "/admin/reset") {
      json(response, 200, await store.reset(await readJson(request)));
      return;
    }
    if (request.method === "POST" && url.pathname === "/admin/config") {
      json(response, 200, await store.configure(await readJson(request)));
      return;
    }
    if (request.method === "POST" && url.pathname === "/admin/replay-floor") {
      const body = await readJson(request);
      json(response, 200, await store.setReplayFloor(body.position));
      return;
    }
    if (request.method === "POST" && url.pathname === "/sessions/open") {
      json(response, 200, await store.openSession(await readJson(request)));
      return;
    }
    if (request.method === "POST" && url.pathname === "/sessions/takeover") {
      json(response, 200, await store.takeover(await readJson(request)));
      return;
    }
    if (request.method === "GET" && url.pathname === "/snapshot") {
      json(response, 200, store.snapshot());
      return;
    }
    if (request.method === "GET" && url.pathname === "/activity") {
      const result = store.activityAfter(url.searchParams.get("after") ?? 0);
      json(response, result.replay_floor_miss ? 409 : 200, result);
      return;
    }
    if (
      request.method === "GET" &&
      url.pathname.startsWith("/receipts/")
    ) {
      const key = decodeURIComponent(url.pathname.slice("/receipts/".length));
      const receipt = store.receipt(key);
      json(response, receipt ? 200 : 404, { receipt });
      return;
    }
    if (request.method === "POST" && url.pathname === "/author-edits") {
      const command = await readJson(request);
      const config = store.state.config;
      if (config.core_crash === "before_commit") {
        response.destroy();
        setTimeout(() => process.exit(86), 10);
        return;
      }
      const result = await store.applyAuthorEdit(command);
      if (config.core_crash === "after_commit_before_response") {
        response.destroy();
        setTimeout(() => process.exit(87), 10);
        return;
      }
      if (config.ack_loss) {
        response.destroy();
        return;
      }
      if (config.activity_order === "event-first") {
        await delay(Math.max(25, config.response_delay_ms));
      } else if (config.response_delay_ms > 0) {
        await delay(config.response_delay_ms);
      }
      json(response, 200, result);
      return;
    }
    json(response, 404, { error: "not_found", path: url.pathname });
  } catch (error) {
    json(response, 500, {
      error: "internal_error",
      message: error instanceof Error ? error.message : String(error),
    });
  }
});

server.listen(port, "127.0.0.1", () => {
  process.stdout.write(
    `${JSON.stringify({
      event: "fake_core_ready",
      pid: process.pid,
      port,
      state_path: statePath,
    })}\n`,
  );
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close(() => process.exit(0)));
}
