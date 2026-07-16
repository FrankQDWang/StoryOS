import { spawn } from "node:child_process";
import { rm } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";

import { authorTranscriptDigest } from "./model.mjs";

const sourceDirectory = dirname(fileURLToPath(import.meta.url));
const prototypeDirectory = resolve(sourceDirectory, "..");
const scratchDirectory = resolve(prototypeDirectory, ".scratch");
const hostOrigin = "http://127.0.0.1:4181";
const mcpOrigin = "http://127.0.0.1:4183";
const processes = { host: null, mcp: null };
const processLog = [];
let shuttingDown = false;

if (process.argv.includes("--matrix")) {
  try {
    await reset();
    const result = await runMatrix();
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    process.exitCode = result.passed === 10 ? 0 : 1;
  } finally {
    await stopAll();
  }
} else {
  await reset();
  await postHost("/api/bootstrap");
  await runInteractive();
}

async function runInteractive() {
  const input = createInterface({ input: process.stdin, output: process.stdout });
  const actions = {
    a: approveFirstWait,
    b: () => postHost("/api/bootstrap"),
    c: () => postHost("/api/compact"),
    d: resourceDriftAndFallback,
    f: () => postHost("/api/fork"),
    i: interruptScenario,
    l: liveReconnectScenario,
    m: toggleMcp,
    r: restartHost,
    u: unauthorizedScenario,
    v: runMatrix,
    w: requestAppEdit,
    y: acceptFirstProposal,
    z: resetForReview
  };

  const redraw = async (notice = "") => {
    const current = await safeSnapshot();
    process.stdout.write("\x1b[2J\x1b[H");
    process.stdout.write(`${bold("技术验证控制台 — 无需审阅")}\n`);
    process.stdout.write(`${dim("作者审阅面: http://localhost:4181")}${notice ? `\n${notice}` : ""}\n\n`);
    if (!current) {
      process.stdout.write("host unavailable\n");
    } else {
      renderState(current);
    }
    process.stdout.write(`\n${bold("Actions")}\n`);
    process.stdout.write("[b] bootstrap View  [r] restart host  [m] toggle MCP  [w] App edit request\n");
    process.stdout.write("[a] approve Wait    [y] accept Proposal [l] live reconnect [i] interrupt\n");
    process.stdout.write("[c] compact context [f] fork branch     [d] drift/fallback [u] deny RPC\n");
    process.stdout.write("[v] run 10 cases    [z] reset review   [q] quit\n\n> ");
  };

  input.on("line", async (line) => {
    const key = line.trim().toLowerCase();
    if (key === "q") {
      input.close();
      return;
    }
    const action = actions[key];
    if (!action) {
      await redraw(`unknown action: ${key || "(empty)"}`);
      return;
    }
    try {
      const result = await action();
      const notice = result?.summary ?? `${key} completed`;
      await redraw(notice);
    } catch (error) {
      await redraw(`ERROR: ${error.message}`);
    }
  });
  input.on("close", async () => {
    await stopAll();
    process.exit(0);
  });

  process.on("SIGINT", () => input.close());
  await redraw();
}

function renderState(snapshot) {
  const state = snapshot.durable;
  const view = snapshot.projection.views[0];
  const toolLine = state.toolCalls.map((call) =>
    `${call.id}:${call.status}/${call.effectOutcome ?? "pending"}@${call.branchId}`
  ).join(" | ") || "none";
  const waitLine = state.waits.map((wait) => `${wait.id}:${wait.status}`).join(" | ") || "none";
  const proposalLine = state.proposals.map((proposal) => `${proposal.id}:${proposal.status}`).join(" | ") || "none";
  const denials = state.bridgeRequests.filter((request) => request.disposition === "denied").length;
  const evidence = state.evidence.map((item) => `${item.passed ? "PASS" : "FAIL"} ${item.case}`).join("\n  ") || "not run";

  field("processes", `host=${statusOf("host")} mcp=${statusOf("mcp")}`);
  field("durable revision", `${state.revision}; host generation=${state.hostGeneration}`);
  field("authoritative state", `${state.project.authoritativeRevision} :: ${state.project.authoritativeText}`);
  field("ToolCalls", toolLine);
  field("Run Waits", waitLine);
  field("Proposals", proposalLine);
  field("App View", view
    ? `${view.revision} :: ${view.renderMode} :: ${view.resourceDigest.slice(0, 12)}`
    : "none");
  field("model projection", `revision=${state.modelContext.revision}; compacted=${state.modelContext.compacted}`);
  field("author transcript", authorTranscriptDigest(state).slice(0, 16));
  field("branches", state.branches.map((branch) => `${branch.id}->${branch.inheritedViewRevision}`).join(" | "));
  field("bridge denials", String(denials));
  field("live overlay", snapshot.liveOverlay ? JSON.stringify(snapshot.liveOverlay) : "none");
  field("evidence", `\n  ${evidence}`);
  field("last event", state.events.at(-1)
    ? `${state.events.at(-1).seq}:${state.events.at(-1).type}`
    : "none");
  field("child log", processLog.slice(-2).join(" | ") || "none");
}

async function runMatrix() {
  await reset();
  const results = [];

  await postHost("/api/bootstrap");
  const first = await snapshot();
  const originalDigest = first.projection.views[0].resourceDigest;
  await stop("mcp");
  await restartHost();
  const offline = await snapshot();
  results.push(await evidence(
    "completed offline replay",
    offline.projection.views[0].resourceDigest === originalDigest
      && offline.durable.toolCalls.find((call) => call.id === "tool-library-1").invocationCount === 1
      && offline.projection.views[0].renderMode === "interactive_snapshot",
    "host restarted with MCP offline; exact stored resource/result replayed; invocation count stayed 1"
  ));
  await startMcp();

  results.push(await liveReconnectScenario());

  const crashWait = await postHost("/api/wait", {
    requestId: "crash-wait-1",
    waitId: "wait-crash-1",
    requestedEdit: "The lighthouse waited beyond the salt marsh."
  });
  await restartHost();
  await postHost("/api/approve", { waitId: crashWait.waitId });
  await postHost("/api/approve", { waitId: crashWait.waitId });
  const afterWait = await snapshot();
  results.push(await evidence(
    "crash during approval/wait",
    afterWait.durable.waits.find((wait) => wait.id === crashWait.waitId)?.status === "resolved"
      && afterWait.durable.waitResolutions.filter((item) => item.waitId === crashWait.waitId).length === 1,
    "durable unresolved Wait survived host-process loss and duplicate resolution settled once"
  ));

  results.push(await interruptScenario());

  const resultProjection = await snapshot();
  const message = resultProjection.durable.messages[0];
  const view = resultProjection.durable.appViews[0];
  results.push(await evidence(
    "result projection",
    message?.toolCallId === view?.originatingToolCallId
      && message?.appViewRevision === view?.revision
      && view?.resultRevision === "tool-result-library-r1"
      && Boolean(view?.staticFallback),
    "one Message links one terminal ToolCall result to one exact App View Artifact revision and fallback"
  ));

  const beforeCompaction = await snapshot();
  const beforeTranscriptDigest = authorTranscriptDigest(beforeCompaction.durable);
  const beforeModelRevision = beforeCompaction.durable.modelContext.revision;
  await postHost("/api/compact");
  const afterCompaction = await snapshot();
  results.push(await evidence(
    "context compaction",
    authorTranscriptDigest(afterCompaction.durable) === beforeTranscriptDigest
      && afterCompaction.durable.modelContext.revision === beforeModelRevision + 1,
    "model projection changed while author-visible Message/Tool/App/Wait projection remained byte-stable"
  ));

  await postHost("/api/fork");
  const forked = await snapshot();
  results.push(await evidence(
    "branch replay",
    forked.durable.branches.every((branch) => branch.inheritedViewRevision === "app-view-library-r1")
      && forked.durable.toolCalls.find((call) => call.id === "tool-branch-action-1")?.branchId === "branch-fork"
      && forked.durable.appViews.length === 1,
    "source and fork share the immutable historical View revision; branch action has new ToolCall lineage"
  ));

  results.push(await resourceDriftAndFallback(originalDigest));

  const authoritativeBefore = (await snapshot()).durable.project.authoritativeRevision;
  const request = bridgeRequest("app-edit-matrix-1", "story.request_edit", {
    replacement: "The lighthouse burned beyond the salt marsh."
  });
  await postHost("/api/bridge", request);
  await postHost("/api/bridge", request);
  const waiting = await snapshot();
  const editWait = waiting.durable.waits.find((wait) => wait.requestId === "app-edit-matrix-1");
  await postHost("/api/approve", { waitId: editWait.id });
  const proposed = await snapshot();
  const proposal = proposed.durable.proposals.find((item) => item.id === `proposal-${editWait.id}`);
  const unchangedBeforeAcceptance = proposed.durable.project.authoritativeRevision === authoritativeBefore;
  await postHost("/api/accept", { proposalId: proposal.id });
  const accepted = await snapshot();

  const rejectRequest = bridgeRequest("app-edit-matrix-reject-1", "story.request_edit", {
    replacement: "The lighthouse dimmed beyond the salt marsh."
  });
  await postHost("/api/bridge", rejectRequest);
  const waitingForRejection = await snapshot();
  const rejectedWait = waitingForRejection.durable.waits.find(
    (wait) => wait.requestId === "app-edit-matrix-reject-1"
  );
  await postHost("/api/reject", { waitId: rejectedWait.id });
  await postHost("/api/reject", { waitId: rejectedWait.id });
  const rejected = await snapshot();
  const rejectedWithoutMutation = rejected.durable.project.authoritativeRevision
    === accepted.durable.project.authoritativeRevision;
  results.push(await evidence(
    "authoritative edit attempt",
    waiting.durable.waits.filter((wait) => wait.requestId === "app-edit-matrix-1").length === 1
      && unchangedBeforeAcceptance
      && accepted.durable.project.authoritativeRevision !== authoritativeBefore
      && accepted.durable.acceptances.at(-1)?.actor === "author"
      && rejectedWithoutMutation
      && rejected.durable.waitResolutions.filter(
        (resolution) => resolution.waitId === rejectedWait.id
          && resolution.decision === "rejected_by_author"
      ).length === 1
      && !rejected.durable.proposals.some(
        (candidate) => candidate.id === `proposal-${rejectedWait.id}`
      ),
    "App requests became one Wait each; Approval made only a Proposal, author Acceptance changed truth, and rejection made no Proposal or edit"
  ));

  results.push(await unauthorizedScenario());

  const final = await snapshot();
  return {
    passed: final.durable.evidence.filter((item) => item.passed).length,
    total: 10,
    evidence: final.durable.evidence,
    hostGeneration: final.durable.hostGeneration,
    originatingToolInvocationCount: final.durable.toolCalls.find((call) => call.id === "tool-library-1")?.invocationCount,
    finalRenderMode: final.projection.views[0]?.renderMode
  };
}

async function liveReconnectScenario() {
  await postHost("/api/live/start");
  const firstConnection = await readSseSnapshot();
  await postHost("/api/live/append");
  const secondConnection = await readSseSnapshot();
  await postHost("/api/live/finish");
  const completed = await snapshot();
  return evidence(
    "live reconnect",
    firstConnection.liveOverlay?.chunks.length === 1
      && secondConnection.liveOverlay?.chunks.length === 2
      && completed.durable.toolCalls.filter((call) => call.id === "tool-live-1").length === 1
      && completed.durable.toolCalls.find((call) => call.id === "tool-live-1")?.status === "completed",
    "two separate SSE clients converged persisted history plus one live overlay into one terminal ToolCall"
  );
}

async function interruptScenario() {
  await postHost("/api/interrupt/start");
  await postHost("/api/interrupt", { effectOutcome: "unknown_external_effect" });
  const interrupted = await snapshot();
  const call = interrupted.durable.toolCalls.find((item) => item.id === "tool-interrupt-1");
  const eventTypes = interrupted.durable.events.map((event) => event.type);
  return evidence(
    "interrupt",
    call?.status === "cancelled"
      && call?.effectOutcome === "unknown_external_effect"
      && eventTypes.includes("tool.interrupt_requested")
      && eventTypes.includes("tool.cancelled")
      && !call?.resultRevision,
    "interrupt intent, cancelled terminal state, and unknown external effect remain distinct; no fake result"
  );
}

async function resourceDriftAndFallback(expectedDigest = null) {
  const before = await snapshot();
  const digest = expectedDigest ?? before.projection.views[0]?.resourceDigest;
  await postMcp("/control/variant", { variant: "v2" });
  await restartHost();
  const drifted = await snapshot();
  const historicalDigestPreserved = drifted.projection.views[0]?.resourceDigest === digest;
  await postHost("/api/resource/loss");
  await restartHost();
  const missing = await snapshot();
  return evidence(
    "resource drift/missing resource",
    historicalDigestPreserved
      && missing.projection.views[0]?.renderMode === "static_fallback"
      && Boolean(missing.durable.appViews[0]?.staticFallback),
    "server drift did not replace the historical digest; missing stored bytes failed closed to StoryOS fallback"
  );
}

async function unauthorizedScenario() {
  const before = await snapshot();
  const revision = before.durable.project.authoritativeRevision;
  const unauthorized = bridgeRequest("app-denied-matrix-1", "shell.exec", { command: "whoami" });
  await postHost("/api/bridge", unauthorized, { allowRpcError: true });
  await postHost("/api/bridge", unauthorized, { allowRpcError: true });
  await postHost("/api/bridge", {
    appInstanceId: "wrong-instance",
    viewId: "app-view-library",
    rpc: { id: "malformed-matrix-1" }
  }, { allowRpcError: true });
  const after = await snapshot();
  return evidence(
    "malformed/unauthorized bridge call",
    after.durable.project.authoritativeRevision === revision
      && after.durable.bridgeRequests.filter((request) => request.id === "app-denied-matrix-1").length === 1
      && after.durable.bridgeRequests.some((request) => request.id === "malformed-matrix-1" && request.disposition === "denied"),
    "ungranted, replayed, malformed, and wrong-instance requests were denied idempotently without domain mutation"
  );
}

async function requestAppEdit() {
  return postHost("/api/bridge", bridgeRequest(
    "app-edit-manual-1",
    "story.request_edit",
    { replacement: "The lighthouse burned beyond the salt marsh." }
  ));
}

async function approveFirstWait() {
  const state = await snapshot();
  const wait = state.durable.waits.find((candidate) => candidate.status === "unresolved");
  if (!wait) throw new Error("no unresolved Run Wait");
  return postHost("/api/approve", { waitId: wait.id });
}

async function acceptFirstProposal() {
  const state = await snapshot();
  const proposal = state.durable.proposals.find((candidate) => candidate.status === "awaiting_author_acceptance");
  if (!proposal) throw new Error("no Proposal awaiting author Acceptance");
  return postHost("/api/accept", { proposalId: proposal.id });
}

function bridgeRequest(id, name, argumentsValue) {
  return {
    appInstanceId: "app-instance-library-1",
    viewId: "app-view-library",
    rpc: {
      jsonrpc: "2.0",
      id,
      method: "tools/call",
      params: { name, arguments: argumentsValue }
    }
  };
}

async function evidence(caseName, passed, detail) {
  await postHost("/api/evidence", { case: caseName, passed, detail });
  return { case: caseName, passed, detail };
}

async function reset() {
  await stopAll();
  await rm(scratchDirectory, { recursive: true, force: true });
  await startMcp();
  await startHost();
  return { summary: "prototype scratch wiped; fresh host and MCP processes started" };
}

async function resetForReview() {
  await reset();
  await postHost("/api/bootstrap");
  return { summary: "作者审阅场景已重置并准备好" };
}

async function restartHost() {
  await stop("host");
  await startHost();
  return { summary: "host process killed and restarted from durable scratch state" };
}

async function toggleMcp() {
  if (processes.mcp) {
    await stop("mcp");
    return { summary: "fake MCP stopped; transcript replay must remain available" };
  }
  await startMcp();
  return { summary: "fake MCP started" };
}

async function startHost() {
  if (processes.host) return;
  processes.host = startChild("host", resolve(sourceDirectory, "host-server.mjs"));
  await waitFor(`${hostOrigin}/health`);
}

async function startMcp() {
  if (processes.mcp) return;
  processes.mcp = startChild("mcp", resolve(sourceDirectory, "fake-mcp-server.mjs"));
  await waitFor(`${mcpOrigin}/health`);
}

function startChild(name, script) {
  const child = spawn(process.execPath, [script], {
    env: { ...process.env },
    stdio: ["ignore", "pipe", "pipe"]
  });
  const capture = (chunk) => {
    for (const line of chunk.toString("utf8").trim().split("\n")) {
      if (line) processLog.push(`${name}: ${line}`);
    }
  };
  child.stdout.on("data", capture);
  child.stderr.on("data", capture);
  child.once("exit", () => {
    if (processes[name] === child) processes[name] = null;
  });
  return child;
}

async function stop(name) {
  const child = processes[name];
  if (!child) return;
  processes[name] = null;
  child.kill("SIGTERM");
  await Promise.race([
    new Promise((resolveExit) => child.once("exit", resolveExit)),
    delay(1_000).then(() => child.kill("SIGKILL"))
  ]);
}

async function stopAll() {
  if (shuttingDown) return;
  shuttingDown = true;
  await Promise.all([stop("host"), stop("mcp")]);
  shuttingDown = false;
}

async function waitFor(url) {
  let lastError;
  for (let attempt = 0; attempt < 50; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await delay(50);
  }
  throw lastError ?? new Error(`timed out waiting for ${url}`);
}

async function readSseSnapshot() {
  const controller = new AbortController();
  const response = await fetch(`${hostOrigin}/api/events`, { signal: controller.signal });
  const reader = response.body.getReader();
  const { value } = await reader.read();
  controller.abort();
  const text = new TextDecoder().decode(value);
  const dataLine = text.split("\n").find((line) => line.startsWith("data: "));
  return JSON.parse(dataLine.slice(6));
}

async function safeSnapshot() {
  try {
    return await snapshot();
  } catch {
    return null;
  }
}

async function snapshot() {
  return fetch(`${hostOrigin}/api/snapshot`).then(requireJson);
}

async function postHost(path, body = {}, options = {}) {
  return post(`${hostOrigin}${path}`, body, options);
}

async function postMcp(path, body = {}) {
  return post(`${mcpOrigin}${path}`, body);
}

async function post(url, body, { allowRpcError = false } = {}) {
  const response = await fetch(url, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body)
  });
  const value = await response.json();
  if (!response.ok || (!allowRpcError && value.error)) {
    throw new Error(value.error?.message ?? value.error ?? `HTTP ${response.status}`);
  }
  return value;
}

async function requireJson(response) {
  const body = await response.json();
  if (!response.ok || body.error) {
    throw new Error(body.error?.message ?? body.error ?? `HTTP ${response.status}`);
  }
  return body;
}

function statusOf(name) {
  return processes[name] ? `pid:${processes[name].pid}` : "stopped";
}

function field(label, value) {
  process.stdout.write(`${bold(label.padEnd(20))} ${value}\n`);
}

function bold(value) {
  return `\x1b[1m${value}\x1b[0m`;
}

function dim(value) {
  return `\x1b[2m${value}\x1b[0m`;
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}
