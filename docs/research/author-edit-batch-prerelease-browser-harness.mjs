import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const policyPath = join(root, "docs/foundation/author-edit-batch-release-1-policy.json");
const evidencePath = join(root, "docs/research/author-edit-batch-prerelease-browser-evidence.json");
const policy = JSON.parse(readFileSync(policyPath, "utf8"));
const captured = JSON.parse(readFileSync(evidencePath, "utf8"));
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
    deleteByCut: "cut", deleteContentBackward: "deletion",
    insertReplacementText: "selection_replacement" };
  record(origins[event.inputType] ?? "explicit_editor_command", event.data ?? "");
});
function emit(type, init, advance = 0) {
  now += advance;
  const started = performance.now();
  const EventType = type.startsWith("composition") ? CompositionEvent : InputEvent;
  editor.dispatchEvent(new EventType(type, { bubbles: true, cancelable: true, ...init }));
  latencies.push(performance.now() - started);
}
function reset() { now = 0; nextSequence = 1; intents = []; latencies = []; }
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
  const groups = batch(records, idleMs, maxUnits);
  return { request_count: groups.length, max_units_per_request: Math.max(...groups.map(g => g.length)),
    max_payload_bytes: Math.max(...groups.map(g => bytes({ author_edit_units: g.map(x => x.unit) }))),
    journal_storage_bytes: bytes(records),
    submission_group_storage_bytes: bytes(groups.map(group => ({ ordered_sequences:
      group.map(item => item.sequence), units: group.map(item => item.unit) }))) };
}
function state() { return { receipt: null, revision: 0, head: 0, action: 0, activity: 0,
  checkpoint: 0, projection: 0, base: 0 }; }
function settle(kind) {
  const value = state();
  if (["refused", "conflicted", "no_effect"].includes(kind)) value.receipt = { result: kind };
  if (kind === "success") Object.assign(value, { receipt: { result: kind }, revision: 1, head: 1,
    action: 1, activity: 1, checkpoint: 1, projection: 1, base: 1 });
  return value;
}
const zeroAuthority = value => ["revision", "head", "action", "activity", "checkpoint",
  "projection", "base"].every(key => value[key] === 0);
function validCoverage(records, units, policyRevision) {
  return policyRevision === policy.revision && records.length === units.length
    && records.every((record, index) => record.sequence === index + 1
      && JSON.stringify(record.unit) === JSON.stringify(units[index]))
    && new Set(records.map(record => record.sequence)).size === records.length;
}

reset(); english(300, 25); const continuous = intents.slice();
reset(); english(20, 600); const slow = intents.slice();
reset(); english(16, 250); const boundary = intents.slice();
reset(); ime(); const imeIntents = intents.slice();
reset(); emit("input", { inputType: "insertFromPaste", data: "paste" }, 10);
emit("input", { inputType: "deleteContentBackward", data: null }, 10);
emit("input", { inputType: "insertReplacementText", data: "replace" }, 10);
const editing = intents.slice();
reset(); english(1024, 1); const pressure = intents.slice();

const scenarios = { continuous_english: continuous, slow_english: slow,
  boundary_english: boundary, chinese_ime_confirm_cancel: imeIntents,
  paste_delete_replace: editing, pressure };
const candidates = [];
for (const idleMs of policy.candidates.adjacent_completed_intent_idle_ms) {
  for (const maxUnits of policy.candidates.max_author_edit_units) {
    candidates.push({ idle_ms: idleMs, max_units: maxUnits,
      scenarios: Object.fromEntries(Object.entries(scenarios)
        .map(([name, records]) => [name, metrics(records, idleMs, maxUnits)])) });
  }
}
const preAdmission = settle("pre_admission_refusal");
const transactionFailure = settle("transaction_failure_before_commit");
const refused = settle("refused");
const conflicted = settle("conflicted");
const noEffect = settle("no_effect");
const success = settle("success");
const retry = structuredClone(success);
const coverage = continuous.slice(0, 3).map((record, index) => ({ ...record, sequence: index + 1 }));
const coverageUnits = coverage.map(record => record.unit);
const invalidLaterUnit = settle("refused");
const semanticOracle = {
  ime_update_and_cancel_not_durable: imeIntents.length === 1 && imeIntents[0].origin === "composition_confirmation",
  hard_boundaries_isolated: batch(editing, 750, 256)
    .filter(group => hardOrigins.has(group[0].origin)).every(group => group.length === 1),
  complete_local_coverage_accepted: validCoverage(coverage, coverageUnits, policy.revision),
  duplicate_local_coverage_rejected: !validCoverage([coverage[0], coverage[0], coverage[2]], coverageUnits, policy.revision),
  skipped_local_coverage_rejected: !validCoverage([coverage[0], coverage[2]], coverageUnits, policy.revision),
  reordered_local_coverage_rejected: !validCoverage([coverage[1], coverage[0], coverage[2]], coverageUnits, policy.revision),
  policy_mismatched_coverage_rejected: !validCoverage(coverage, coverageUnits, "wrong-policy"),
  pre_admission_refusal_receipt_free: preAdmission.receipt === null && zeroAuthority(preAdmission),
  over_limit_command_refused_before_admission: pressure.slice(0, 241).length === 241
    && preAdmission.receipt === null && zeroAuthority(preAdmission),
  primitive_limit_refused_before_admission: pressure.slice(0, 241)
    .reduce(count => count + 1, 0) === 241 && preAdmission.receipt === null,
  body_limit_refused_before_admission: bytes({ body: "x".repeat(1048577) }) > 1048576
    && preAdmission.receipt === null && zeroAuthority(preAdmission),
  admitted_refused_typed_zero_authority: refused.receipt?.result === "refused" && zeroAuthority(refused),
  admitted_conflicted_typed_zero_authority: conflicted.receipt?.result === "conflicted" && zeroAuthority(conflicted),
  admitted_no_effect_typed_zero_authority: noEffect.receipt?.result === "no_effect" && zeroAuthority(noEffect),
  invalid_later_unit_typed_zero_authority: invalidLaterUnit.receipt?.result === "refused"
    && zeroAuthority(invalidLaterUnit),
  pre_commit_transaction_failure_receipt_free: transactionFailure.receipt === null && zeroAuthority(transactionFailure),
  whole_batch_has_one_final_settlement: success.receipt?.result === "success" && success.revision === 1
    && success.head === 1 && success.action === 1 && success.activity === 1,
  no_prefix_authority: invalidLaterUnit.revision === 0 && success.revision === 1,
  contradictory_prefix_state_rejected: !zeroAuthority({ ...invalidLaterUnit, revision: 1 }),
  exact_retry_has_no_second_effect: JSON.stringify(retry) === JSON.stringify(success),
  grouping_never_exceeds_selected_limit: metrics(pressure, 750, 240).max_units_per_request <= 240,
};
if (!Object.values(semanticOracle).every(Boolean)) throw new Error("semantic oracle failed "
  + JSON.stringify(semanticOracle));
const sorted = latencies.slice().sort((a, b) => a - b);
const result = { schema: "storyos.author-edit-batch-prerelease-browser-evidence.v1",
  policy_revision: policy.revision, synthetic_only: true, real_user_content: false,
  selected: policy.selected, candidates, semantic_oracle: semanticOracle,
  browser_input_latency_ms: { max: Math.max(...latencies),
    p95: sorted[Math.floor(sorted.length * 0.95)] ?? 0, classification: "advisory" } };
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
  const at = (idleMs, maxUnits) => result.candidates.find(candidate =>
    candidate.idle_ms === idleMs && candidate.max_units === maxUnits).scenarios;
  const comparison = captured.deterministic_comparison;
  assert.deepEqual(comparison.continuous_english_300_request_count_by_max_units,
    policy.candidates.max_author_edit_units.map(units => at(250, units).continuous_english.request_count));
  assert.deepEqual(comparison.pressure_1024_request_count_by_max_units,
    policy.candidates.max_author_edit_units.map(units => at(250, units).pressure.request_count));
  assert.deepEqual(comparison.slow_english_20_request_count_by_idle_ms,
    policy.candidates.adjacent_completed_intent_idle_ms.map(idle => at(idle, 240).slow_english.request_count));
  assert.deepEqual(comparison.max_payload_bytes_by_max_units,
    policy.candidates.max_author_edit_units.map(units => at(250, units).continuous_english.max_payload_bytes));
  const selected = at(policy.selected.adjacent_completed_intent_idle_ms,
    policy.selected.max_author_edit_units);
  assert.deepEqual(comparison.journal_storage_bytes, {
    continuous_english_300: selected.continuous_english.journal_storage_bytes,
    slow_english_20: selected.slow_english.journal_storage_bytes,
    boundary_english_16: selected.boundary_english.journal_storage_bytes,
    chinese_ime_confirm_cancel: selected.chinese_ime_confirm_cancel.journal_storage_bytes,
    paste_delete_replace: selected.paste_delete_replace.journal_storage_bytes,
    pressure_1024: selected.pressure.journal_storage_bytes,
  });
  assert.deepEqual(comparison.selected_250ms_240_units, {
    continuous_english_300_request_count: selected.continuous_english.request_count,
    pressure_1024_request_count: selected.pressure.request_count,
    slow_english_20_request_count: selected.slow_english.request_count,
    max_units_per_request: selected.pressure.max_units_per_request,
    max_payload_bytes: selected.pressure.max_payload_bytes,
    payload_fraction_of_1mib: selected.pressure.max_payload_bytes
      / policy.selected.max_public_json_command_body_bytes,
    continuous_submission_group_storage_bytes: selected.continuous_english.submission_group_storage_bytes,
    pressure_submission_group_storage_bytes: selected.pressure.submission_group_storage_bytes,
  });
  assert.deepEqual(captured.semantic_oracle, result.semantic_oracle);
  if (process.argv.includes("--json")) process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  else console.log(`verified ${result.policy_revision} in a real browser with ${result.candidates.length} candidate runs`);
} finally {
  rmSync(directory, { recursive: true, force: true });
}
