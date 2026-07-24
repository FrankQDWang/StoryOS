import { execFileSync } from "node:child_process";
import { mkdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { chromium } from "playwright";
import { labRoot, startLab } from "./lab-processes.mjs";
import { partitionIntents } from "../src/policies.js";

const runId = `issue69-${new Date().toISOString().replaceAll(/[:.]/g, "-")}`;
const outputDirectory = path.join(labRoot, "artifacts", "latest");
const statePath = path.join(
  labRoot,
  ".lab-state",
  `evidence-core-${process.pid}.json`,
);
await rm(outputDirectory, { recursive: true, force: true });
await rm(statePath, { force: true });
await mkdir(outputDirectory, { recursive: true });

const lab = await startLab({ coreStatePath: statePath });
let browser;
const results = [];
const trace = [];
const measurements = [];
const consoleErrors = [];
const scenarioFilter = new Set(
  (process.env.STORYOS_LAB_SCENARIOS ?? "")
    .split(",")
    .map((value) => value.trim())
    .filter(Boolean),
);

function check(condition, message, evidence = null) {
  if (!condition) {
    const error = new Error(message);
    error.evidence = evidence;
    throw error;
  }
}

async function corePost(pathname, body = {}) {
  const response = await fetch(`${lab.coreUrl}${pathname}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  const value = await response.json();
  if (!response.ok) {
    throw new Error(`core_admin_error:${response.status}:${JSON.stringify(value)}`);
  }
  return value;
}

async function resetCore(config = {}, initialText = "") {
  return corePost("/admin/reset", {
    initial_text: initialText,
    config,
  });
}

async function newContext() {
  return browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
    viewport: { width: 1280, height: 900 },
  });
}

async function openPage(
  context,
  scenarioId,
  {
    strategy = "bounded-idle",
    session = "",
    writer = true,
  } = {},
) {
  const page = await context.newPage();
  page.on("console", (message) => {
    if (message.type() === "error") {
      consoleErrors.push({
        scenario_id: scenarioId,
        text: message.text(),
      });
    }
  });
  page.on("pageerror", (error) => {
    consoleErrors.push({
      scenario_id: scenarioId,
      text: error.stack ?? error.message,
    });
  });
  const query = new URLSearchParams({
    run: runId,
    scenario: scenarioId,
    strategy,
    writer: writer ? "1" : "0",
  });
  if (session) query.set("session", session);
  await page.goto(`${lab.editorUrl}/?${query}`);
  await page.waitForFunction(() => Boolean(window.storyosLab), null, {
    timeout: 10_000,
  });
  await page.evaluate(() => window.storyosLab.ready());
  return page;
}

async function report(page) {
  return page.evaluate(() => window.storyosLab.report());
}

function stages(value, stage) {
  return value.trace.filter((record) => record.stage === stage);
}

function stageIndex(value, stage) {
  return value.trace.findIndex((record) => record.stage === stage);
}

function addReport(scenarioId, value) {
  trace.push(...value.trace);
  const metric = value.measurements;
  measurements.push({
    scenario_id: scenarioId,
    commands: metric.commands,
    core_commits: metric.core_commits,
    author_actions: metric.author_actions,
    payload_bytes: metric.payload_bytes,
    journal_records: value.journal_metrics.records,
    journal_bytes: value.journal_metrics.bytes,
    mean_journal_durability_ms: mean(metric.journal_durability_ms),
    mean_settlement_ms: mean(metric.settlement_ms),
    mean_convergence_ms: mean(metric.convergence_ms),
    mean_recovery_ms: mean(metric.recovery_ms),
  });
}

async function scenario(id, operation) {
  if (scenarioFilter.size > 0 && !scenarioFilter.has(id)) return;
  const started = performance.now();
  try {
    const evidence = await operation();
    results.push({
      scenario_id: id,
      status: "passed",
      duration_ms: Number((performance.now() - started).toFixed(3)),
      evidence,
    });
    process.stdout.write(`[scenario] PASS ${id}\n`);
  } catch (error) {
    results.push({
      scenario_id: id,
      status: "failed",
      duration_ms: Number((performance.now() - started).toFixed(3)),
      error: error.stack ?? error.message,
      evidence: error.evidence ?? null,
    });
    process.stderr.write(`[scenario] FAIL ${id}: ${error.message}\n`);
  }
}

function mean(values) {
  if (values.length === 0) return "";
  return Number(
    (values.reduce((sum, value) => sum + value, 0) / values.length).toFixed(3),
  );
}

async function waitForStage(page, stage, timeoutMs = 5_000) {
  await page.waitForFunction(
    (wanted) =>
      window.storyosLab
        ?.trace()
        .some((item) => item.stage === wanted),
    stage,
    { timeout: timeoutMs },
  );
}

try {
  browser = await chromium.launch({
    channel: "chrome",
    headless: true,
  });

  await scenario("candidate-policy-boundaries", async () => {
    await resetCore({ activity_delay_ms: 40 });
    const context = await newContext();
    const page = await openPage(context, "candidate-policy-boundaries");
    await page.evaluate(() => window.storyosLab.insertText("A"));
    await page.evaluate(() => window.storyosLab.insertText("B"));
    await page.evaluate(() => window.storyosLab.syntheticComposition("中文"));
    await page.evaluate(() => window.storyosLab.insertText("C"));
    await page.evaluate(() => window.storyosLab.splitBlock());
    await page.evaluate(() => window.storyosLab.insertText("D"));
    await page.evaluate(() => window.storyosLab.flush());
    const value = await report(page);
    const byStrategy = Object.fromEntries(
      value.candidate_comparison.map((candidate) => [
        candidate.strategy,
        candidate,
      ]),
    );
    check(
      byStrategy.transaction.violations.some(
        (item) => item.code === "composition_fragmented",
      ),
      "transaction baseline did not expose composition fragmentation",
      byStrategy.transaction,
    );
    check(
      byStrategy["fixed-window"].violations.some(
        (item) => item.code === "fixed_window_crossed_contract_boundary",
      ),
      "fixed-window control did not cross a frozen boundary",
      byStrategy["fixed-window"],
    );
    check(
      byStrategy["semantic-intent"].violations.length === 0,
      "semantic-intent baseline violated a boundary",
    );
    check(
      byStrategy["bounded-idle"].violations.length === 0,
      "bounded-idle candidate violated a boundary",
    );
    check(
      byStrategy["bounded-idle"].groups.length <
        byStrategy["semantic-intent"].groups.length,
      "bounded-idle did not reduce command units",
    );
    addReport("candidate-policy-boundaries", value);
    await context.close();
    return {
      groups: Object.fromEntries(
        value.candidate_comparison.map((candidate) => [
          candidate.strategy,
          candidate.groups.length,
        ]),
      ),
      violations: Object.fromEntries(
        value.candidate_comparison.map((candidate) => [
          candidate.strategy,
          candidate.violations.map((item) => item.code),
        ]),
      ),
    };
  });

  await scenario("binding-boundary-matrix", async () => {
    const baseBinding = {
      project_id: "project-issue-69",
      chapter_id: "chapter-alpha",
      target_id: "manuscript:chapter-alpha",
      ownership: "authoritative",
      expected_head: "revision-0",
      writer_generation: 1,
      admission_id: "admission-direct-edit",
      admission_expires_at: "2030-01-01T00:00:00.000Z",
      editor_contract: "storyos.web-editor.session-semantics.v1",
      undo_group: "undo-1",
    };
    const boundaryMutations = {
      project: { project_id: "project-other" },
      chapter: {
        chapter_id: "chapter-beta",
        target_id: "manuscript:chapter-beta",
      },
      target: { target_id: "manuscript:other-target" },
      ownership: { ownership: "proposal" },
      expected_head: { expected_head: "revision-1" },
      writer_generation: { writer_generation: 2 },
      admission_id: { admission_id: "admission-other" },
      admission_expiry: {
        admission_expires_at: "2030-01-01T00:01:00.000Z",
      },
      editor_contract: { editor_contract: "editor-contract-other" },
      undo_group: { undo_group: "undo-2" },
    };
    const checks = [];
    for (const [name, mutation] of Object.entries(boundaryMutations)) {
      const intents = [
        {
          intent_id: `${name}-left`,
          kind: "typing",
          at_ms: 0,
          before_text: "",
          after_text: "a",
          binding: baseBinding,
        },
        {
          intent_id: `${name}-right`,
          kind: "typing",
          at_ms: 10,
          before_text: "a",
          after_text: "ab",
          binding: { ...baseBinding, ...mutation },
        },
      ];
      const bounded = partitionIntents(intents, "bounded-idle");
      const fixed = partitionIntents(intents, "fixed-window");
      check(
        bounded.groups.length === 2,
        `bounded-idle crossed ${name}`,
        bounded,
      );
      check(
        fixed.violations.some(
          (item) => item.code === "fixed_window_crossed_contract_boundary",
        ),
        `fixed-window did not expose ${name} crossing`,
        fixed,
      );
      checks.push(name);
    }
    for (const kind of [
      "composition",
      "paste",
      "cut",
      "drop",
      "structural",
      "explicit_command",
    ]) {
      const intents = [
        {
          intent_id: `${kind}-left`,
          kind: "typing",
          at_ms: 0,
          before_text: "",
          after_text: "a",
          binding: baseBinding,
        },
        {
          intent_id: `${kind}-right`,
          kind,
          at_ms: 10,
          before_text: "a",
          after_text: "ab",
          binding: baseBinding,
        },
      ];
      check(
        partitionIntents(intents, "bounded-idle").groups.length === 2,
        `bounded-idle crossed ${kind}`,
      );
      check(
        partitionIntents(intents, "fixed-window").violations.length >= 1,
        `fixed-window did not expose ${kind} crossing`,
      );
      checks.push(kind);
    }
    return { verified_boundaries: checks };
  });

  await scenario("native-input-clipboard-drop-undo", async () => {
    await resetCore({ activity_delay_ms: 30 });
    const context = await newContext();
    const page = await openPage(context, "native-input-clipboard-drop-undo");
    const editor = page.locator(".ProseMirror");
    await editor.click();
    await page.keyboard.type("English typing");
    await page.waitForTimeout(280);
    await page.evaluate(() => window.storyosLab.flush());
    await page.keyboard.press("Backspace");
    await page.waitForTimeout(280);
    await page.evaluate(() => window.storyosLab.flush());
    await page.evaluate(() => window.storyosLab.setCursor(1));
    await page.keyboard.press("Delete");
    await page.waitForTimeout(280);
    await page.evaluate(() => window.storyosLab.flush());
    await editor.click();
    await page.keyboard.press("Meta+A");
    await page.waitForFunction(
      () => window.storyosLab.selection().empty === false,
    );
    await page.keyboard.type("Selection replaced");
    await page.waitForTimeout(280);
    await page.evaluate(() => window.storyosLab.flush());
    await page.evaluate(() => navigator.clipboard.writeText(" pasted"));
    await page.keyboard.press("Meta+V");
    await page.waitForTimeout(80);
    await page.evaluate(() => window.storyosLab.flush());
    await page.keyboard.down("Shift");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.up("Shift");
    await page.keyboard.press("Meta+X");
    await page.waitForTimeout(80);
    await page.evaluate(() => window.storyosLab.flush());
    await page.locator('[data-testid="drag-source"]').dragTo(editor);
    await page.waitForTimeout(100);
    await page.evaluate(() => window.storyosLab.flush());
    await page.keyboard.press("Meta+Z");
    await page.waitForTimeout(100);
    await page.evaluate(() => window.storyosLab.flush());
    await page.keyboard.press("Meta+Shift+Z");
    await page.waitForTimeout(100);
    await page.evaluate(() => window.storyosLab.flush());
    const value = await report(page);
    addReport("native-input-clipboard-drop-undo", value);
    const sources = new Set(
      value.intents.map((intent) => intent.evidence_source),
    );
    const kinds = new Set(value.intents.map((intent) => intent.kind));
    check(sources.has("native-beforeinput"), "native beforeinput was not used");
    check(sources.has("native-clipboard"), "native clipboard path was not used");
    check(kinds.has("selection_replace"), "selection replace was not classified");
    check(kinds.has("delete_backward"), "backward delete was not classified");
    check(kinds.has("delete_forward"), "forward delete was not classified");
    check(kinds.has("cut"), "native cut was not classified");
    check(kinds.has("drop"), "native drop was not classified");
    check(
      value.core.documents["chapter-alpha"] ===
        (await page.evaluate(() => window.storyosLab.editorText())),
      "Core and editor text diverged after native paths",
    );
    check(
      stages(value, "journal_gc_completed").length > 0,
      "settled native intents did not reach journal GC",
    );
    await context.close();
    return {
      intent_kinds: [...kinds],
      evidence_sources: [...sources],
      final_text_sha256: stages(value, "projection_converged").at(-1)
        ?.text_sha256,
    };
  });

  for (const order of ["http-first", "event-first"]) {
    await scenario(`delivery-order-${order}`, async () => {
      await resetCore({
        activity_order: order,
        activity_delay_ms: order === "http-first" ? 90 : 0,
        response_delay_ms: order === "event-first" ? 90 : 0,
      });
      const context = await newContext();
      const page = await openPage(context, `delivery-order-${order}`, {
        strategy: "semantic-intent",
      });
      await page.evaluate(() => window.storyosLab.insertText("ordered"));
      const value = await report(page);
      const receipt = stageIndex(value, "receipt_settled");
      const activity = stageIndex(value, "activity_observed");
      check(receipt >= 0 && activity >= 0, "missing receipt or Activity");
      check(
        order === "http-first" ? receipt < activity : activity < receipt,
        `observed order did not match ${order}`,
        { receipt, activity },
      );
      check(
        value.core.author_actions.length === 1,
        "delivery order produced duplicate/missing Author Action",
      );
      addReport(`delivery-order-${order}`, value);
      await context.close();
      return { receipt_trace_index: receipt, activity_trace_index: activity };
    });
  }

  await scenario("duplicate-delivery-and-idempotency", async () => {
    await resetCore({
      activity_duplicate: true,
      activity_delay_ms: 20,
    });
    const context = await newContext();
    const page = await openPage(context, "duplicate-delivery-and-idempotency", {
      strategy: "semantic-intent",
    });
    await page.evaluate(() => window.storyosLab.insertText("once"));
    const duplicate = await page.evaluate(() =>
      window.storyosLab.duplicateLastSubmission(),
    );
    const value = await report(page);
    check(duplicate.duplicate === true, "duplicate command was not idempotent");
    check(
      stages(value, "activity_duplicate_ignored").length >= 1,
      "duplicate Activity was not observed and ignored",
    );
    check(
      value.core.author_actions.length === 1,
      "duplicate delivery allocated another Author Action",
    );
    addReport("duplicate-delivery-and-idempotency", value);
    await context.close();
    return {
      receipt_id: duplicate.receipt.receipt_id,
      author_actions: value.core.author_actions.length,
    };
  });

  await scenario("ack-loss-reconciliation", async () => {
    await resetCore({ ack_loss: true });
    const context = await newContext();
    const page = await openPage(context, "ack-loss-reconciliation", {
      strategy: "semantic-intent",
    });
    await page.evaluate(() => window.storyosLab.insertText("ack lost"));
    await corePost("/admin/config", { ack_loss: false });
    const reconciliation = await page.evaluate(() =>
      window.storyosLab.reconcileAll(),
    );
    const value = await report(page);
    check(
      stages(value, "outcome_unknown").length >= 1,
      "ack loss did not enter OutcomeUnknown",
    );
    check(
      value.core.author_actions.length === 1,
      "ack reconciliation duplicated the Author Action",
    );
    check(
      value.journal_metrics.records === 0,
      "reconciled settled journal was not collected",
    );
    addReport("ack-loss-reconciliation", value);
    await context.close();
    return { reconciliation };
  });

  for (const gapKind of ["sequence-gap", "replay-floor"]) {
    await scenario(gapKind, async () => {
      await resetCore(
        gapKind === "sequence-gap" ? { activity_gap: true } : {},
      );
      if (gapKind === "replay-floor") {
        await corePost("/admin/replay-floor", { position: 1 });
      }
      const context = await newContext();
      const page = await openPage(context, gapKind, {
        strategy: "semantic-intent",
      });
      await page.evaluate(
        (text) => window.storyosLab.insertText(text),
        gapKind,
      );
      const value = await report(page);
      const expectedStage =
        gapKind === "sequence-gap"
          ? "activity_sequence_gap"
          : "activity_replay_floor_miss";
      check(stages(value, expectedStage).length >= 1, `${gapKind} not observed`);
      check(
        stages(value, "submission_paused").length >= 1,
        `${gapKind} did not pause submission`,
      );
      check(
        stages(value, "snapshot_loaded").length >= 2,
        `${gapKind} did not obtain a resync Snapshot`,
      );
      check(
        value.state.projection_text === value.core.documents["chapter-alpha"],
        `${gapKind} did not converge from Snapshot`,
      );
      addReport(gapKind, value);
      await context.close();
      return {
        projection_position: value.state.projection_position,
        head: value.state.expected_head,
      };
    });
  }

  for (const crashPoint of [
    "before_commit",
    "after_commit_before_response",
  ]) {
    await scenario(`core-crash-${crashPoint}`, async () => {
      await resetCore({ core_crash: crashPoint });
      const context = await newContext();
      const page = await openPage(context, `core-crash-${crashPoint}`, {
        strategy: "semantic-intent",
      });
      await page.evaluate(() => window.storyosLab.insertText("crash safe"));
      await page.evaluate(() => window.storyosLab.reconcileAll()).catch(() => {});
      await page.waitForTimeout(250);
      await corePost("/admin/config", { core_crash: "none" });
      const first = await page.evaluate(() =>
        window.storyosLab.reconcileAll({ reauthorize: false }),
      );
      let second = [];
      if (crashPoint === "before_commit") {
        second = await page.evaluate(() =>
          window.storyosLab.reconcileAll({ reauthorize: true }),
        );
      }
      const value = await report(page);
      check(
        stages(value, "outcome_unknown").length >= 1,
        "Core crash did not preserve OutcomeUnknown",
      );
      check(
        value.core.author_actions.length === 1,
        "Core crash recovery did not produce exactly one Author Action",
      );
      check(
        value.core.documents["chapter-alpha"] === "crash safe",
        "Core crash recovery lost the edit",
      );
      if (crashPoint === "before_commit") {
        check(
          stages(value, "submission_reauthorized").length >= 1,
          "not-committed command was retried without explicit reauthorization",
        );
      }
      addReport(`core-crash-${crashPoint}`, value);
      await context.close();
      return { first_reconciliation: first, second_reconciliation: second };
    });
  }

  await scenario("reload-after-journal-durable", async () => {
    await resetCore({});
    const context = await newContext();
    let page = await openPage(context, "reload-after-journal-durable", {
      strategy: "fixed-window",
      session: "reload-writer",
    });
    await page.evaluate(() => window.storyosLab.insertText("reload exact"));
    const before = await page.evaluate(() => window.storyosLab.journalMetrics());
    check(before.records === 1, "journal was not durable before reload", before);
    await page.reload();
    await page.waitForFunction(() => Boolean(window.storyosLab));
    await page.evaluate(() => window.storyosLab.ready());
    await page.evaluate(() => window.storyosLab.flush());
    const value = await report(page);
    check(
      value.core.documents["chapter-alpha"] === "reload exact",
      "reload did not recover exact durable journal text",
    );
    check(
      stages(value, "submission_reauthorized").some(
        (item) => item.details.network_was_never_attempted,
      ),
      "never-submitted durable entry was not explicitly reauthorized",
    );
    addReport("reload-after-journal-durable", value);
    await context.close();
    return { journal_before_reload: before };
  });

  await scenario("reload-binding-mismatch-preserves-draft", async () => {
    await resetCore({});
    const context = await newContext();
    const original = await openPage(
      context,
      "reload-binding-mismatch-preserves-draft",
      {
        strategy: "fixed-window",
        session: "original-writer",
      },
    );
    await original.evaluate(() =>
      window.storyosLab.insertText("unsettled exact draft"),
    );
    const successor = await openPage(
      context,
      "reload-binding-mismatch-preserves-draft",
      { session: "successor-writer" },
    );
    await successor.evaluate(() => window.storyosLab.takeover());
    await original.reload();
    await original.waitForFunction(() => Boolean(window.storyosLab));
    await original.evaluate(() => window.storyosLab.ready());
    const value = await report(original);
    check(
      value.state.mode === "read_only",
      "fenced original session reopened as a writer",
    );
    check(
      value.state.recovery_drafts.some(
        (draft) => draft.text === "unsettled exact draft",
      ),
      "binding mismatch did not preserve the exact Draft",
      value.state.recovery_drafts,
    );
    check(
      !stages(value, "submission_reauthorized").some(
        (record) => record.details.network_was_never_attempted,
      ),
      "binding-mismatched journal entry was automatically resubmitted",
    );
    check(
      value.core.documents["chapter-alpha"] === "",
      "binding-mismatched Draft changed Core authority",
    );
    addReport("reload-binding-mismatch-preserves-draft", value);
    await context.close();
    return {
      recovery_draft_sha256: stages(
        value,
        "recovery_draft_preserved",
      ).at(-1)?.text_sha256,
      active_writer_generation: value.core.writer_generation,
    };
  });

  await scenario("reload-after-receipt-before-activity", async () => {
    await resetCore({
      activity_order: "http-first",
      activity_delay_ms: 2_000,
    });
    const context = await newContext();
    const page = await openPage(
      context,
      "reload-after-receipt-before-activity",
      {
        strategy: "semantic-intent",
        session: "receipt-reload-writer",
      },
    );
    await page.evaluate(() => {
      void window.storyosLab.insertText("receipt durable");
    });
    await waitForStage(page, "receipt_settled");
    await page.reload();
    await page.waitForFunction(() => Boolean(window.storyosLab));
    await page.evaluate(() => window.storyosLab.ready());
    const value = await report(page);
    addReport("reload-after-receipt-before-activity", value);
    check(
      value.core.documents["chapter-alpha"] === "receipt durable",
      "receipt-before-activity reload lost Core authority",
    );
    check(
      stages(value, "journal_reconciled").length >= 1,
      "receipt-before-activity journal was not reconciled",
    );
    check(
      value.journal_metrics.records === 0,
      "reconciled receipt journal did not GC after Snapshot convergence",
    );
    await context.close();
    return {
      author_actions: value.core.author_actions.length,
      projection_position: value.state.projection_position,
    };
  });

  await scenario("secondary-tab-takeover-stale-writer", async () => {
    await resetCore({});
    const context = await newContext();
    const writer = await openPage(
      context,
      "secondary-tab-takeover-stale-writer",
      { session: "writer-a" },
    );
    const secondary = await openPage(
      context,
      "secondary-tab-takeover-stale-writer",
      { session: "writer-b" },
    );
    const secondaryBefore = await secondary.evaluate(
      () => window.storyosLab.state(),
    );
    check(
      secondaryBefore.mode === "read_only",
      "secondary tab was not read-only",
      secondaryBefore,
    );
    await secondary.evaluate(() => window.storyosLab.takeover());
    await writer.evaluate(() => window.storyosLab.insertText("stale draft"));
    await writer.evaluate(() => window.storyosLab.flush());
    await secondary.evaluate(() => window.storyosLab.insertText("new writer"));
    await secondary.evaluate(() => window.storyosLab.flush());
    const staleValue = await report(writer);
    const value = await report(secondary);
    check(
      staleValue.state.recovery_drafts.some(
        (draft) => draft.outcome === "refused",
      ),
      "stale writer text was not preserved as a refused Draft",
      staleValue.recovery_drafts,
    );
    check(
      value.core.documents["chapter-alpha"] === "new writer",
      "takeover writer did not own authority",
    );
    check(
      value.core.author_actions.length === 1,
      "stale writer allocated an Author Action",
    );
    trace.push(...staleValue.trace);
    addReport("secondary-tab-takeover-stale-writer", value);
    await context.close();
    return {
      fenced_outcome: staleValue.state.recovery_drafts.at(-1)?.outcome,
      active_writer_generation: value.core.writer_generation,
    };
  });

  for (const outcome of [
    "authoritative",
    "proposal",
    "refused",
    "conflicted",
    "no_effect",
  ]) {
    await scenario(`typed-outcome-${outcome}`, async () => {
      await resetCore({ outcome });
      const context = await newContext();
      const page = await openPage(context, `typed-outcome-${outcome}`, {
        strategy: "semantic-intent",
      });
      await page.evaluate(
        (text) => window.storyosLab.insertText(text),
        `draft-${outcome}`,
      );
      const value = await report(page);
      const receipt = stages(value, "receipt_settled").at(-1);
      check(receipt?.outcome === outcome, `wrong typed outcome for ${outcome}`);
      check(
        value.core.author_actions.length === (outcome === "authoritative" ? 1 : 0),
        `${outcome} allocated the wrong Author Action count`,
      );
      if (outcome !== "authoritative") {
        check(
          value.state.recovery_drafts.at(-1)?.text === `draft-${outcome}`,
          `${outcome} did not preserve the complete Draft`,
        );
        check(
          value.journal_metrics.records === 1,
          `${outcome} journal was collected before Draft resolution`,
        );
      }
      addReport(`typed-outcome-${outcome}`, value);
      await context.close();
      return {
        receipt_id: receipt.receipt_id,
        outcome,
        author_action_seq: receipt.author_action_seq,
      };
    });
  }

  await scenario("chapter-switch-persistence", async () => {
    await resetCore({});
    const context = await newContext();
    const page = await openPage(context, "chapter-switch-persistence", {
      strategy: "fixed-window",
    });
    await page.evaluate(() => window.storyosLab.insertText("alpha pending"));
    await page.evaluate(() =>
      window.storyosLab.switchChapter(
        "chapter-beta",
        "manuscript:chapter-beta",
      ),
    );
    await page.evaluate(() => window.storyosLab.insertText("beta text"));
    await page.evaluate(() => window.storyosLab.flush());
    const value = await report(page);
    check(
      value.core.documents["chapter-alpha"] === "alpha pending",
      "chapter switch did not persist current chapter first",
    );
    check(
      value.core.documents["chapter-beta"] === "beta text",
      "new chapter did not settle independently",
    );
    check(
      stages(value, "chapter_switch_started").at(-1)?.details
        .local_state_persisted_first === true,
      "chapter switch trace did not expose persistence ordering",
    );
    addReport("chapter-switch-persistence", value);
    await context.close();
    return { heads: value.core.heads };
  });

  await scenario("long-session-journal-growth", async () => {
    await resetCore({});
    const context = await newContext();
    const page = await openPage(context, "long-session-journal-growth");
    const samples = [];
    for (let index = 1; index <= 240; index += 1) {
      await page.evaluate(() => window.storyosLab.insertText("x"));
      if ([1, 40, 80, 120, 180, 240].includes(index)) {
        samples.push({
          intents: index,
          ...(await page.evaluate(() => window.storyosLab.journalMetrics())),
        });
      }
    }
    await page.evaluate(() => window.storyosLab.flush());
    const value = await report(page);
    check(
      value.core.documents["chapter-alpha"] === "x".repeat(240),
      "long session lost or reordered characters",
    );
    check(
      value.measurements.commands < 240,
      "bounded-idle did not reduce long-session commands",
    );
    check(
      value.journal_metrics.records === 0,
      "long-session settled journal did not GC",
    );
    addReport("long-session-journal-growth", value);
    await context.close();
    return {
      intents: 240,
      commands: value.measurements.commands,
      growth_samples: samples,
    };
  });

  await scenario("synthetic-composition-boundary-only", async () => {
    await resetCore({});
    const context = await newContext();
    const page = await openPage(context, "synthetic-composition-boundary-only");
    await page.evaluate(() => window.storyosLab.syntheticComposition("中文输入"));
    const value = await report(page);
    const intent = value.intents.at(-1);
    check(intent.kind === "composition", "composition was not one semantic intent");
    check(
      intent.evidence_source === "synthetic-boundary-only",
      "synthetic composition was mislabeled",
    );
    check(
      intent.details.real_os_ime_evidence === false,
      "synthetic composition was mislabeled as real OS IME",
    );
    check(
      value.core.documents["chapter-alpha"] === "中文输入",
      "synthetic boundary probe was not byte-exact",
    );
    addReport("synthetic-composition-boundary-only", value);
    await context.close();
    return {
      transaction_count: intent.transaction_count,
      real_os_ime_evidence: false,
    };
  });
} finally {
  if (browser) await browser.close();
  await lab.stop();
}

if (scenarioFilter.size === 0) {
  const started = performance.now();
  try {
    validateLifecycleTrace(trace);
    results.push({
      scenario_id: "global-lifecycle-invariants",
      status: "passed",
      duration_ms: Number((performance.now() - started).toFixed(3)),
      evidence: {
        trace_records: trace.length,
        submission_durability_order: "verified",
        gc_settlement_convergence_order: "verified",
        bounded_trace: "verified",
      },
    });
    process.stdout.write("[scenario] PASS global-lifecycle-invariants\n");
  } catch (error) {
    results.push({
      scenario_id: "global-lifecycle-invariants",
      status: "failed",
      duration_ms: Number((performance.now() - started).toFixed(3)),
      error: error.stack ?? error.message,
      evidence: error.evidence ?? null,
    });
    process.stderr.write(
      `[scenario] FAIL global-lifecycle-invariants: ${error.message}\n`,
    );
  }
}

const failed = results.filter((result) => result.status !== "passed");
const expectedConsoleErrors = consoleErrors.filter(isExpectedConsoleError);
const unexpectedConsoleErrors = consoleErrors.filter(
  (entry) => !isExpectedConsoleError(entry),
);
const environment = {
  run_id: runId,
  generated_at: new Date().toISOString(),
  node: process.version,
  platform: process.platform,
  architecture: process.arch,
  os_release: os.release(),
  chrome: browser?.version() ?? null,
  trace_schema: "storyos.issue69.trace.v1",
  core_schema: "storyos.issue69.fake-core.v1",
  journal_schema: "storyos.issue69.local-edit-journal.v1",
  source_head: execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: labRoot,
    encoding: "utf8",
  }).trim(),
  scenario_count: results.length,
  passed: results.length - failed.length,
  failed: failed.length,
  expected_fault_console_errors: expectedConsoleErrors,
  unexpected_console_errors: unexpectedConsoleErrors,
};

await writeFile(
  path.join(outputDirectory, "trace.jsonl"),
  `${trace.map((record) => JSON.stringify(record)).join("\n")}\n`,
);
await writeFile(
  path.join(outputDirectory, "scenario-results.json"),
  `${JSON.stringify({ run_id: runId, results }, null, 2)}\n`,
);
await writeFile(
  path.join(outputDirectory, "environment.json"),
  `${JSON.stringify(environment, null, 2)}\n`,
);
await writeFile(
  path.join(outputDirectory, "measurements.csv"),
  toCsv(measurements),
);

process.stdout.write(
  `${JSON.stringify({
    event: "evidence_complete",
    output_directory: outputDirectory,
    scenarios: results.length,
    failed: failed.length,
  })}\n`,
);

if (failed.length > 0 || unexpectedConsoleErrors.length > 0) {
  process.exitCode = 1;
}

function toCsv(rows) {
  const fields = [
    "scenario_id",
    "commands",
    "core_commits",
    "author_actions",
    "payload_bytes",
    "journal_records",
    "journal_bytes",
    "mean_journal_durability_ms",
    "mean_settlement_ms",
    "mean_convergence_ms",
    "mean_recovery_ms",
  ];
  return `${fields.join(",")}\n${rows
    .map((row) =>
      fields
        .map((field) => JSON.stringify(row[field] ?? ""))
        .join(","),
    )
    .join("\n")}\n`;
}

function validateLifecycleTrace(records) {
  check(
    !records.some((record) => record.stage === "trace_truncated"),
    "a scenario exceeded the bounded trace limit",
  );
  const streams = new Map();
  for (const record of records) {
    const key = `${record.scenario_id}:${record.editor_session_id}`;
    const stream = streams.get(key) ?? [];
    stream.push(record);
    streams.set(key, stream);
  }
  for (const [streamId, stream] of streams) {
    for (const submission of stream.filter(
      (record) => record.stage === "submission_started",
    )) {
      for (const intentId of submission.details.intent_ids ?? []) {
        check(
          stream.some(
            (record) =>
              record.stage === "journal_durable" &&
              record.intent_id === intentId &&
              record.seq < submission.seq,
          ),
          `network submission preceded journal durability in ${streamId}`,
          { submission, intent_id: intentId },
        );
      }
    }
    for (const collected of stream.filter(
      (record) => record.stage === "journal_gc_completed",
    )) {
      check(
        stream.some(
          (record) =>
            record.stage === "receipt_settled" &&
            record.command_id === collected.command_id &&
            record.seq < collected.seq,
        ),
        `journal GC preceded durable settlement in ${streamId}`,
        collected,
      );
      check(
        stream.some(
          (record) =>
            record.stage === "projection_converged" &&
            record.command_id === collected.command_id &&
            record.seq < collected.seq,
        ),
        `journal GC preceded projection convergence in ${streamId}`,
        collected,
      );
      check(
        stream.some(
          (record) =>
            record.stage === "journal_gc_eligible" &&
            record.command_id === collected.command_id &&
            record.seq < collected.seq,
        ),
        `journal GC lacked an explicit eligibility gate in ${streamId}`,
        collected,
      );
    }
  }
}

function isExpectedConsoleError(entry) {
  const expectedByScenario = {
    "ack-loss-reconciliation": ["ERR_EMPTY_RESPONSE"],
    "replay-floor": ["409 (Conflict)"],
    "core-crash-before_commit": [
      "ERR_EMPTY_RESPONSE",
      "ERR_CONNECTION_REFUSED",
      "404 (Not Found)",
    ],
    "core-crash-after_commit_before_response": [
      "ERR_EMPTY_RESPONSE",
      "ERR_CONNECTION_REFUSED",
      "404 (Not Found)",
    ],
  };
  return (expectedByScenario[entry.scenario_id] ?? []).some((fragment) =>
    entry.text.includes(fragment),
  );
}
