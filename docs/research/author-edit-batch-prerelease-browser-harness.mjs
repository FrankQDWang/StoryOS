import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const policyPath = join(root, "docs/foundation/author-edit-batch-release-1-policy.json");
const evidencePath = join(root, "docs/research/author-edit-batch-prerelease-browser-evidence.json");
const candidatesPath = join(root, "docs/research/author-edit-batch-prerelease-browser-candidates.jsonl");
const policy = JSON.parse(readFileSync(policyPath, "utf8"));
const captured = JSON.parse(readFileSync(evidencePath, "utf8"));
const capturedCandidates = readFileSync(candidatesPath, "utf8").trim().split("\n").map(JSON.parse);
const chrome = [
  process.env.CHROME_BIN,
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/usr/bin/google-chrome",
  "/usr/bin/chromium",
].filter(Boolean).find(existsSync);

if (!chrome) throw new Error("Chrome or Chromium is required for the prerelease browser harness");

const page = `<!doctype html><meta charset="utf-8"><textarea id="editor"></textarea>
<pre id="result">running</pre><script>
window.addEventListener("error", event => {
  document.querySelector("#result").textContent = "ERROR:" + event.message + ":" + event.lineno;
});
const policy = ${JSON.stringify(policy)};
const encoder = new TextEncoder();
const editor = document.querySelector("#editor");
let now = 0;
let composing = false;
let nextSequence = 1;
let intents = [];
let latencies = [];
const hardOrigins = new Set(["composition_confirmation", "paste", "cut", "drop",
  "split_block", "join_blocks", "move_block", "retype_block", "explicit_editor_command",
  "policy_identity_change", "shared_binding_change"]);
const record = (origin, text = "x", from = 0, to = 0) => intents.push({
  sequence: nextSequence++, at_ms: now, origin,
  coverage: { first: nextSequence - 1, last: nextSequence - 1 },
  unit: { kind: "replace_selection", selection: { from, to }, text },
});
editor.addEventListener("compositionstart", () => { composing = true; });
editor.addEventListener("compositionupdate", () => {});
editor.addEventListener("compositionend", (event) => {
  composing = false;
  if (event.data) record("composition_confirmation", event.data);
});
editor.addEventListener("input", (event) => {
  if (composing || event.isComposing || event.inputType === "insertCompositionText") return;
  const origins = { insertText: "typing", insertFromPaste: "paste",
    insertFromDrop: "drop", deleteByCut: "cut", deleteContentBackward: "deletion",
    insertReplacementText: "selection_replacement" };
  record(origins[event.inputType] ?? "explicit_editor_command", event.data ?? "");
});
editor.addEventListener("storyos-boundary", event => record(event.detail));
function emit(type, init, advance = 0) {
  now += advance;
  const started = performance.now();
  const EventType = type.startsWith("composition") ? CompositionEvent : InputEvent;
  editor.dispatchEvent(new EventType(type, { bubbles: true, cancelable: true, ...init }));
  latencies.push(performance.now() - started);
}
function reset() { now = 0; nextSequence = 1; intents = []; latencies = []; }
function emitBoundary(origin) {
  if (origin === "composition_confirmation") {
    emit("compositionstart", { data: "" }, 10);
    emit("compositionend", { data: "你" }, 10);
    return;
  }
  const inputTypes = { paste: "insertFromPaste", cut: "deleteByCut", drop: "insertFromDrop" };
  if (inputTypes[origin]) {
    emit("input", { inputType: inputTypes[origin], data: origin }, 10);
    return;
  }
  now += 10;
  const started = performance.now();
  editor.dispatchEvent(new CustomEvent("storyos-boundary", { detail: origin }));
  latencies.push(performance.now() - started);
}
function snapshot(action) {
  reset(); action();
  const sorted = latencies.slice().sort((a, b) => a - b);
  return { records: intents.slice(), browser_input_latency_ms: { max: Math.max(...latencies),
    p95: sorted[Math.floor(sorted.length * 0.95)] ?? 0, classification: "advisory" } };
}
function english(count, gap) {
  for (let i = 0; i < count; i += 1) emit("input", { inputType: "insertText", data: "a" }, gap);
}
function ime() {
  emit("compositionstart", { data: "" }, 20);
  emit("compositionupdate", { data: "ni" }, 20);
  emit("input", { inputType: "insertCompositionText", data: "你", isComposing: true }, 20);
  emit("compositionend", { data: "你" }, 20);
  emit("compositionstart", { data: "" }, 20);
  emit("compositionupdate", { data: "hao" }, 20);
  emit("compositionend", { data: "" }, 20);
}
function batch(records, idleMs, maxUnits) {
  const groups = [];
  let group = [];
  const flush = () => { if (group.length) groups.push(group); group = []; };
  for (const item of records) {
    const prior = group.at(-1);
    if (hardOrigins.has(item.origin)) { flush(); groups.push([item]); continue; }
    if (prior && (item.at_ms - prior.at_ms > idleMs || group.length >= maxUnits)) flush();
    group.push(item);
  }
  flush();
  return groups;
}
function bytes(value) { return encoder.encode(JSON.stringify(value)).byteLength; }
function metrics(records, idleMs, maxUnits) {
  const started = performance.now();
  const groups = batch(records, idleMs, maxUnits);
  return { request_count: groups.length, max_units_per_request: Math.max(...groups.map(g => g.length)),
    max_payload_bytes: Math.max(...groups.map(g => bytes({ author_edit_units: g.map(x => x.unit) }))),
    journal_storage_bytes: bytes(records),
    submission_group_storage_bytes: bytes(groups.map(group => ({ ordered_sequences:
      group.map(item => item.sequence), units: group.map(item => item.unit) }))),
    grouping_evaluation_latency_ms: performance.now() - started };
}
function state(classification = "pending") { return { classification, receipt: null, revision: 0,
  head: 0, action: 0, activity: 0, checkpoint: 0, projection: 0, base: 0,
  retry_reused: false }; }
const zeroAuthority = value => ["revision", "head", "action", "activity", "checkpoint",
  "projection", "base"].every(key => value[key] === 0);
function executeTrace(trace) {
  let value = state();
  let admitted = false;
  let coreStarted = false;
  for (const step of trace) {
    if (step.kind === "admit") {
      const refused = step.policy_revision !== policy.revision
        || step.unit_count > policy.selected.max_author_edit_units
        || step.primitive_count > policy.selected.max_normalized_primitives
        || step.body_bytes > policy.selected.max_public_json_command_body_bytes;
      if (refused) return state("pre_admission_refusal");
      admitted = true;
    } else if (step.kind === "core_start" && admitted) coreStarted = true;
    else if (step.kind === "unit" && coreStarted && !step.valid) {
      value = state("refused"); value.receipt = { result: "refused" }; return value;
    } else if (step.kind === "result" && coreStarted) {
      value = state(step.outcome); value.receipt = { result: step.outcome }; return value;
    } else if (step.kind === "fault" && coreStarted && step.point === "CFP-CORE-BEFORE-COMMIT") {
      return state("transaction_failure_before_commit");
    } else if (step.kind === "commit" && coreStarted) {
      value = state("success"); Object.assign(value, { receipt: { result: "success" }, revision: 1,
        head: 1, action: 1, activity: 1, checkpoint: 1, projection: 1, base: 1 });
    } else if (step.kind === "retry" && value.classification === "success") value.retry_reused = true;
  }
  return value;
}
function validOracleState(value) {
  if (["pre_admission_refusal", "transaction_failure_before_commit"].includes(value.classification))
    return value.receipt === null && zeroAuthority(value);
  if (["refused", "conflicted", "no_effect"].includes(value.classification))
    return value.receipt?.result === value.classification && zeroAuthority(value);
  return value.classification === "success" && value.receipt?.result === "success"
    && ["revision", "head", "action", "activity", "checkpoint", "projection", "base"]
      .every(key => value[key] === 1);
}
function validCoverage(records, units, policyRevision) {
  return policyRevision === policy.revision && records.length === units.length
    && records.every((record, index) => record.sequence === index + 1
      && JSON.stringify(record.unit) === JSON.stringify(units[index]))
    && new Set(records.map(record => record.sequence)).size === records.length
    && records.every((record, index) => index === 0
      || records[index - 1].coverage.last < record.coverage.first);
}

const scenarios = {
  continuous_english: snapshot(() => english(300, 25)),
  slow_english: snapshot(() => english(20, 600)),
  boundary_english: snapshot(() => english(16, 250)),
  chinese_ime_confirm_cancel: snapshot(ime),
  paste_delete_replace: snapshot(() => {
    emit("input", { inputType: "insertFromPaste", data: "paste" }, 10);
    emit("input", { inputType: "deleteContentBackward", data: null }, 10);
    emit("input", { inputType: "insertReplacementText", data: "replace" }, 10);
  }),
  hard_boundaries: snapshot(() => [...hardOrigins].forEach(emitBoundary)),
  pressure: snapshot(() => english(1024, 1)),
};
const candidates = [];
for (const idleMs of policy.candidates.adjacent_completed_intent_idle_ms) {
  for (const maxUnits of policy.candidates.max_author_edit_units) {
    candidates.push({ idle_ms: idleMs, max_units: maxUnits,
      scenarios: Object.fromEntries(Object.entries(scenarios)
        .map(([name, scenario]) => [name, { ...metrics(scenario.records, idleMs, maxUnits),
          browser_input_latency_ms: scenario.browser_input_latency_ms }])) });
  }
}
const admitted = { kind: "admit", policy_revision: policy.revision, unit_count: 2,
  primitive_count: 2, body_bytes: 200 };
const run = (...steps) => executeTrace([admitted, { kind: "core_start" }, ...steps]);
const over = field => executeTrace([{ ...admitted, [field]: 1048577 }]);
const preAdmission = over("unit_count");
const transactionFailure = run({ kind: "unit", valid: true },
  { kind: "fault", point: "CFP-CORE-BEFORE-COMMIT" });
const refused = run({ kind: "result", outcome: "refused" });
const conflicted = run({ kind: "result", outcome: "conflicted" });
const noEffect = run({ kind: "result", outcome: "no_effect" });
const success = run({ kind: "unit", valid: true }, { kind: "unit", valid: true },
  { kind: "commit" }, { kind: "retry" });
const coverage = scenarios.continuous_english.records.slice(0, 3)
  .map((record, index) => ({ ...record, sequence: index + 1,
    coverage: { first: index + 1, last: index + 1 } }));
const coverageUnits = coverage.map(record => record.unit);
const invalidLaterUnit = run({ kind: "unit", valid: true }, { kind: "unit", valid: false });
const hardGroups = batch(scenarios.hard_boundaries.records, 750, 256);
const hardBoundaryOracle = Object.fromEntries([...hardOrigins].map(origin => [
  "hard_boundary_" + origin + "_isolated",
  hardGroups.some(group => group.length === 1 && group[0].origin === origin),
]));
const semanticOracle = {
  ime_update_and_cancel_not_durable: scenarios.chinese_ime_confirm_cancel.records.length === 1,
  ...hardBoundaryOracle,
  complete_local_coverage_accepted: validCoverage(coverage, coverageUnits, policy.revision),
  duplicate_local_coverage_rejected: !validCoverage([coverage[0], coverage[0], coverage[2]], coverageUnits, policy.revision),
  skipped_local_coverage_rejected: !validCoverage([coverage[0], coverage[2]], coverageUnits, policy.revision),
  reordered_local_coverage_rejected: !validCoverage([coverage[1], coverage[0], coverage[2]], coverageUnits, policy.revision),
  overlapping_local_coverage_rejected: !validCoverage([coverage[0],
    { ...coverage[1], coverage: { first: 1, last: 2 } }, coverage[2]], coverageUnits, policy.revision),
  policy_mismatched_coverage_rejected: !validCoverage(coverage, coverageUnits, "wrong-policy"),
  unit_limit_refused_before_admission: validOracleState(preAdmission),
  primitive_limit_refused_before_admission: validOracleState(over("primitive_count")),
  body_limit_refused_before_admission: validOracleState(over("body_bytes")),
  policy_mismatch_refused_before_admission: validOracleState(executeTrace([
    { ...admitted, policy_revision: "wrong-policy" }])),
  admitted_refused_typed_zero_authority: validOracleState(refused),
  admitted_conflicted_typed_zero_authority: validOracleState(conflicted),
  admitted_no_effect_typed_zero_authority: validOracleState(noEffect),
  invalid_later_unit_typed_zero_authority: validOracleState(invalidLaterUnit),
  pre_commit_transaction_failure_receipt_free: validOracleState(transactionFailure),
  whole_batch_has_one_final_settlement: validOracleState(success),
  no_prefix_authority: invalidLaterUnit.revision === 0 && success.revision === 1,
  contradictory_prefix_state_rejected: !validOracleState({ ...invalidLaterUnit, revision: 1 }),
  exact_retry_has_no_second_effect: success.retry_reused && success.revision === 1,
  grouping_never_exceeds_selected_limit: metrics(scenarios.pressure.records, 750, 240)
    .max_units_per_request <= 240,
};
if (!Object.values(semanticOracle).every(Boolean)) throw new Error("semantic oracle failed "
  + JSON.stringify(semanticOracle));
const result = { schema: "storyos.author-edit-batch-prerelease-browser-evidence.v1",
  policy_revision: policy.revision, synthetic_only: true, real_user_content: false,
  selected: policy.selected, candidates, semantic_oracle: semanticOracle };
document.querySelector("#result").textContent = btoa(unescape(encodeURIComponent(JSON.stringify(result))));
</script>`;

const directory = mkdtempSync(join(tmpdir(), "storyos-author-edit-batch-"));
try {
  const html = join(directory, "harness.html");
  writeFileSync(html, page);
  const run = spawnSync(chrome, ["--headless=new", "--disable-gpu", "--no-sandbox",
    `--user-data-dir=${join(directory, "profile")}`, "--virtual-time-budget=10000", "--dump-dom",
    pathToFileURL(html).href], { encoding: "utf8", killSignal: "SIGKILL",
    maxBuffer: 16 * 1024 * 1024, timeout: 15000 });
  const encoded = run.stdout.match(/<pre id="result">([^<]+)<\/pre>/)?.[1];
  assert.ok(encoded && encoded !== "running",
    `browser harness did not produce a result: ${run.stderr}`);
  assert.ok(!encoded.startsWith("ERROR:"), encoded);
  const result = JSON.parse(Buffer.from(encoded, "base64").toString("utf8"));
  assert.equal(result.policy_revision, policy.revision);
  assert.ok(Object.values(result.semantic_oracle).every(Boolean));
  assert.ok(result.candidates.some(candidate => candidate.idle_ms === 250 && candidate.max_units === 240));
  if (!process.argv.includes("--capture")) {
    const deterministic = candidates => candidates.map(candidate => ({ ...candidate,
      scenarios: Object.fromEntries(Object.entries(candidate.scenarios).map(([name, metrics]) => {
        const { grouping_evaluation_latency_ms, browser_input_latency_ms, ...stable } = metrics;
        return [name, stable];
      })) }));
    assert.deepEqual(deterministic(capturedCandidates), deterministic(result.candidates));
    assert.ok(capturedCandidates.every(candidate => Object.values(candidate.scenarios).every(metrics =>
      Number.isFinite(metrics.grouping_evaluation_latency_ms)
      && Number.isFinite(metrics.browser_input_latency_ms.max)
      && Number.isFinite(metrics.browser_input_latency_ms.p95))));
    assert.deepEqual(captured.semantic_oracle, result.semantic_oracle);
  }
  if (process.argv.some(argument => ["--capture", "--json"].includes(argument)))
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  else console.log(`verified ${result.policy_revision} in a real browser with ${result.candidates.length} candidate runs`);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
