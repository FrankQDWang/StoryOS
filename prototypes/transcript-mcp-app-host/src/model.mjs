import { createHash } from "node:crypto";

const PROTOCOL_VERSION = "2026-01-26";

export function createInitialState() {
  return {
    schema: "storyos.transcript-mcp-app-host.prototype/v1",
    revision: 0,
    hostGeneration: 0,
    project: {
      authoritativeRevision: "manuscript-r1",
      authoritativeText: "The lighthouse stood beyond the salt marsh."
    },
    runs: [],
    steps: [],
    toolCalls: [],
    waits: [],
    waitResolutions: [],
    proposals: [],
    acceptances: [],
    appViews: [],
    messages: [],
    branches: [{ id: "branch-main", inheritedViewRevision: null }],
    bridgeRequests: [],
    modelContext: {
      revision: 1,
      compacted: false,
      visibleMessageIds: []
    },
    evidence: [],
    events: []
  };
}

export function reduce(current, action) {
  const state = structuredClone(current);
  const handlers = {
    accept_proposal: acceptProposal,
    bootstrap_completed: bootstrapCompleted,
    compact_model_context: compactModelContext,
    create_app_edit_wait: createAppEditWait,
    finish_live_call: finishLiveCall,
    fork_branch: forkBranch,
    host_started: hostStarted,
    interrupt_call: interruptCall,
    record_bridge_denial: recordBridgeDenial,
    record_evidence: recordEvidence,
    resolve_wait_to_proposal: resolveWaitToProposal,
    simulate_resource_loss: simulateResourceLoss,
    start_interruptible_call: startInterruptibleCall,
    start_live_call: startLiveCall
  };
  const handler = handlers[action.type];
  if (!handler) {
    throw new Error(`unknown prototype action: ${action.type}`);
  }
  handler(state, action);
  state.revision += 1;
  return state;
}

export function transcriptProjection(state, liveOverlay = null) {
  const messages = state.messages.map((message) => ({
    id: message.id,
    role: message.role,
    appViewRevision: message.appViewRevision,
    toolCallId: message.toolCallId
  }));
  const toolActivity = state.toolCalls.map((call) => ({
    id: call.id,
    branchId: call.branchId,
    status: call.status,
    resultRevision: call.resultRevision ?? null,
    effectOutcome: call.effectOutcome ?? null
  }));
  const waits = state.waits.map((wait) => ({
    id: wait.id,
    status: wait.status,
    resolutionId: wait.resolutionId ?? null
  }));
  const views = state.appViews.map((view) => ({
    id: view.id,
    revision: view.revision,
    resourceDigest: view.resource.digest,
    renderMode: renderMode(view),
    inputRevision: view.inputRevision,
    resultRevision: view.resultRevision
  }));
  return { messages, toolActivity, waits, views, liveOverlay };
}

export function authorTranscriptDigest(state) {
  return digest(JSON.stringify(transcriptProjection(state, null)));
}

export function renderMode(view) {
  if (!view.resource.html) {
    return "static_fallback";
  }
  return digest(view.resource.html) === view.resource.digest
    ? "interactive_snapshot"
    : "static_fallback";
}

function hostStarted(state, action) {
  state.hostGeneration += 1;
  appendEvent(state, "host.started", {
    generation: state.hostGeneration,
    processId: action.processId
  });
}

function bootstrapCompleted(state, action) {
  const existing = state.toolCalls.find((call) => call.id === "tool-library-1");
  if (existing) {
    appendEvent(state, "bootstrap.replayed_without_tool_execution", {
      toolCallId: existing.id
    });
    return;
  }

  state.runs.push({ id: "run-root-1", lifecycle: "terminal", outcome: "succeeded" });
  state.steps.push({ id: "step-root-1", runId: "run-root-1", status: "settled" });
  state.toolCalls.push({
    id: "tool-library-1",
    branchId: "branch-main",
    runId: "run-root-1",
    stepId: "step-root-1",
    tool: "story.library.summary",
    status: "completed",
    invocationCount: action.invocationCount,
    inputRevision: "tool-input-library-r1",
    resultRevision: "tool-result-library-r1",
    effectOutcome: "confirmed_read_only"
  });
  state.appViews.push({
    id: "app-view-library",
    revision: "app-view-library-r1",
    originatingToolCallId: "tool-library-1",
    appInstanceId: "app-instance-library-1",
    protocolVersion: PROTOCOL_VERSION,
    requestedCapabilities: ["tools/call"],
    effectiveCapabilities: ["tools/call:story.request_edit"],
    inputRevision: "tool-input-library-r1",
    resultRevision: "tool-result-library-r1",
    hostContext: { theme: "light", locale: "en", displayMode: "inline" },
    resource: {
      uri: action.resource.uri,
      mimeType: action.resource.mimeType,
      digest: action.resource.digest,
      html: action.resource.html
    },
    structuredResult: action.result.structuredContent,
    staticFallback: {
      title: "Library summary",
      text: action.result.content[0].text,
      provenance: "tool-library-1 / tool-result-library-r1"
    }
  });
  state.messages.push({
    id: "message-agent-library-1",
    role: "agent",
    toolCallId: "tool-library-1",
    appViewRevision: "app-view-library-r1"
  });
  state.modelContext.visibleMessageIds = ["message-agent-library-1"];
  state.branches[0].inheritedViewRevision = "app-view-library-r1";
  appendEvent(state, "tool.completed", { toolCallId: "tool-library-1" });
  appendEvent(state, "app_view.recorded", { revision: "app-view-library-r1" });
}

function startLiveCall(state) {
  if (state.toolCalls.some((call) => call.id === "tool-live-1")) {
    return;
  }
  state.toolCalls.push({
    id: "tool-live-1",
    branchId: "branch-main",
    runId: "run-root-live",
    stepId: "step-live-1",
    tool: "story.live.summary",
    status: "running",
    invocationCount: 1,
    inputRevision: "tool-input-live-r1"
  });
  appendEvent(state, "tool.started", { toolCallId: "tool-live-1" });
}

function finishLiveCall(state) {
  const call = requireToolCall(state, "tool-live-1");
  if (call.status === "completed") {
    return;
  }
  call.status = "completed";
  call.resultRevision = "tool-result-live-r1";
  call.effectOutcome = "confirmed_read_only";
  appendEvent(state, "tool.completed", { toolCallId: call.id });
}

function createAppEditWait(state, action) {
  const priorRequest = state.bridgeRequests.find((request) => request.id === action.requestId);
  if (priorRequest) {
    return;
  }
  const waitId = action.waitId;
  state.bridgeRequests.push({
    id: action.requestId,
    method: "tools/call",
    disposition: "approval_required",
    waitId
  });
  state.waits.push({
    id: waitId,
    kind: "approval",
    requestId: action.requestId,
    requestedEdit: action.requestedEdit,
    status: "unresolved"
  });
  appendEvent(state, "run_wait.created", { waitId, requestId: action.requestId });
}

function resolveWaitToProposal(state, action) {
  const wait = state.waits.find((candidate) => candidate.id === action.waitId);
  if (!wait) {
    throw new Error(`missing Run Wait: ${action.waitId}`);
  }
  if (wait.status === "resolved") {
    return;
  }
  const resolutionId = `resolution-${wait.id}`;
  const proposalId = `proposal-${wait.id}`;
  wait.status = "resolved";
  wait.resolutionId = resolutionId;
  state.waitResolutions.push({
    id: resolutionId,
    waitId: wait.id,
    decision: "approved_for_proposal_only"
  });
  state.proposals.push({
    id: proposalId,
    source: "mcp_app",
    requestedEdit: wait.requestedEdit,
    baseAuthoritativeRevision: state.project.authoritativeRevision,
    status: "awaiting_author_acceptance"
  });
  appendEvent(state, "run_wait.resolved", { waitId: wait.id, resolutionId });
  appendEvent(state, "proposal.created", { proposalId });
}

function acceptProposal(state, action) {
  const proposal = state.proposals.find((candidate) => candidate.id === action.proposalId);
  if (!proposal) {
    throw new Error(`missing Proposal: ${action.proposalId}`);
  }
  if (proposal.status === "accepted") {
    return;
  }
  proposal.status = "accepted";
  state.project.authoritativeRevision = `manuscript-r${state.acceptances.length + 2}`;
  state.project.authoritativeText = proposal.requestedEdit;
  state.acceptances.push({
    id: `acceptance-${proposal.id}`,
    proposalId: proposal.id,
    resultingRevision: state.project.authoritativeRevision,
    actor: "author"
  });
  appendEvent(state, "proposal.accepted_by_author", { proposalId: proposal.id });
}

function startInterruptibleCall(state) {
  if (state.toolCalls.some((call) => call.id === "tool-interrupt-1")) {
    return;
  }
  state.toolCalls.push({
    id: "tool-interrupt-1",
    branchId: "branch-main",
    runId: "run-root-interrupt",
    stepId: "step-interrupt-1",
    tool: "external.write_like_operation",
    status: "running",
    invocationCount: 1,
    inputRevision: "tool-input-interrupt-r1"
  });
  appendEvent(state, "tool.started", { toolCallId: "tool-interrupt-1" });
}

function interruptCall(state, action) {
  const call = requireToolCall(state, "tool-interrupt-1");
  if (call.status !== "running") {
    return;
  }
  appendEvent(state, "tool.interrupt_requested", { toolCallId: call.id });
  call.status = "cancelled";
  call.effectOutcome = action.effectOutcome;
  appendEvent(state, "tool.cancelled", {
    toolCallId: call.id,
    effectOutcome: action.effectOutcome
  });
}

function compactModelContext(state) {
  state.modelContext.revision += 1;
  state.modelContext.compacted = true;
  state.modelContext.visibleMessageIds = ["model-compaction-summary-1"];
  appendEvent(state, "model_context.compacted", {
    modelContextRevision: state.modelContext.revision
  });
}

function forkBranch(state) {
  if (state.branches.some((branch) => branch.id === "branch-fork")) {
    return;
  }
  state.branches.push({
    id: "branch-fork",
    inheritedViewRevision: "app-view-library-r1"
  });
  state.toolCalls.push({
    id: "tool-branch-action-1",
    branchId: "branch-fork",
    runId: "run-branch-1",
    stepId: "step-branch-1",
    tool: "story.library.filter",
    status: "completed",
    invocationCount: 1,
    inputRevision: "tool-input-branch-r1",
    resultRevision: "tool-result-branch-r1",
    effectOutcome: "confirmed_read_only"
  });
  appendEvent(state, "branch.created", { branchId: "branch-fork" });
}

function simulateResourceLoss(state) {
  const view = requireView(state);
  view.resource.html = null;
  appendEvent(state, "app_view.resource_missing", { revision: view.revision });
}

function recordBridgeDenial(state, action) {
  if (state.bridgeRequests.some((request) => request.id === action.requestId)) {
    return;
  }
  state.bridgeRequests.push({
    id: action.requestId,
    method: action.method ?? "malformed",
    disposition: "denied",
    reason: action.reason
  });
  appendEvent(state, "app_bridge.denied", {
    requestId: action.requestId,
    reason: action.reason
  });
}

function recordEvidence(state, action) {
  const prior = state.evidence.find((item) => item.case === action.case);
  const value = {
    case: action.case,
    passed: action.passed,
    detail: action.detail
  };
  if (prior) {
    Object.assign(prior, value);
  } else {
    state.evidence.push(value);
  }
}

function appendEvent(state, type, data) {
  state.events.push({ seq: state.events.length + 1, type, data });
}

function requireToolCall(state, id) {
  const call = state.toolCalls.find((candidate) => candidate.id === id);
  if (!call) {
    throw new Error(`missing ToolCall: ${id}`);
  }
  return call;
}

function requireView(state) {
  const view = state.appViews.find((candidate) => candidate.id === "app-view-library");
  if (!view) {
    throw new Error("bootstrap the App View first");
  }
  return view;
}

function digest(value) {
  return createHash("sha256").update(value).digest("hex");
}
