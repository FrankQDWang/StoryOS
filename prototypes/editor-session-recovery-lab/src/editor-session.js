import {
  DEFAULT_CHAPTER_ID,
  DEFAULT_TARGET_ID,
  EDITOR_CONTRACT,
  HARD_BOUNDARY_KINDS,
  PROJECT_ID,
  bindingFingerprint,
  sha256,
  stableJson,
} from "../shared/contract.mjs";
import { LocalEditJournal } from "./journal.js";
import { partitionIntents } from "./policies.js";
import { TraceRecorder } from "./trace.js";

const COALESCIBLE_KINDS = new Set([
  "typing",
  "delete_backward",
  "delete_forward",
  "selection_replace",
]);

function uuid() {
  return crypto.randomUUID();
}

function futureAdmissionExpiry() {
  return new Date(Date.now() + 60 * 60 * 1_000).toISOString();
}

async function jsonFetch(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      "content-type": "application/json",
      ...(options.headers ?? {}),
    },
  });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const error = new Error(
      body.error ?? `http_${response.status}:${response.statusText}`,
    );
    error.status = response.status;
    error.body = body;
    throw error;
  }
  return body;
}

function sleep(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

export class EditorSessionLab {
  constructor({
    coreUrl,
    runId,
    scenarioId,
    strategy,
    editorSessionId,
    requestWriter,
    applyEditorText,
    readEditorText,
    onState,
  }) {
    this.coreUrl = coreUrl;
    this.runId = runId;
    this.scenarioId = scenarioId;
    this.strategy = strategy;
    this.editorSessionId = editorSessionId;
    this.requestWriter = requestWriter;
    this.applyEditorText = applyEditorText;
    this.readEditorText = readEditorText;
    this.onState = onState;
    this.journal = null;
    this.intentSequence = 0;
    this.localOrder = 0;
    this.pendingBatch = [];
    this.batchTimer = null;
    this.submissionChain = Promise.resolve();
    this.intents = [];
    this.commands = new Map();
    this.receipts = new Map();
    this.seenActivityKeys = new Set();
    this.recoveryDrafts = [];
    this.metrics = {
      commands: 0,
      core_commits: 0,
      author_actions: 0,
      payload_bytes: 0,
      journal_durability_ms: [],
      settlement_ms: [],
      convergence_ms: [],
      recovery_ms: [],
    };
    this.state = {
      status: "initializing",
      mode: "read_only",
      project_id: PROJECT_ID,
      chapter_id: DEFAULT_CHAPTER_ID,
      target_id: DEFAULT_TARGET_ID,
      ownership: "authoritative",
      writer_generation: null,
      expected_head: null,
      admission_id: "admission-direct-edit",
      admission_expires_at: futureAdmissionExpiry(),
      editor_contract: EDITOR_CONTRACT,
      projection_status: "loading",
      projection_position: 0,
      authority_text: "",
      projection_text: "",
      submission_paused: false,
      pause_reason: null,
    };
    this.trace = new TraceRecorder({
      runId,
      scenarioId,
      context: () => ({
        chapter_id: this.state.chapter_id,
        target_id: this.state.target_id,
        editor_session_id: this.editorSessionId,
        writer_generation: this.state.writer_generation,
        expected_head: this.state.expected_head,
        admission_id: this.state.admission_id,
        admission_expires_at: this.state.admission_expires_at,
        projection_position: this.state.projection_position,
      }),
    });
  }

  async initialize() {
    try {
      this.journal = await LocalEditJournal.open();
      await this.trace.record("journal", "journal_verified", {
        details: { schema: "storyos.issue69.local-edit-journal.v1" },
      });
    } catch (error) {
      this.state.status = "recovery_only";
      this.state.projection_status = "needs_attention";
      this.state.pause_reason = "journal_unavailable";
      await this.trace.record("journal", "journal_unavailable", {
        details: { message: error.message },
      });
      this.emit();
      return;
    }

    const session = await jsonFetch(`${this.coreUrl}/sessions/open`, {
      method: "POST",
      body: JSON.stringify({
        editor_session_id: this.editorSessionId,
        request_writer: this.requestWriter,
      }),
    });
    const snapshot = await jsonFetch(`${this.coreUrl}/snapshot`);
    this.state.mode = session.mode;
    this.state.writer_generation = session.writer_generation;
    this.state.expected_head = snapshot.heads[this.state.chapter_id];
    this.state.authority_text = snapshot.documents[this.state.chapter_id];
    this.state.projection_text = this.state.authority_text;
    this.state.projection_position = snapshot.latest_activity_position;
    this.state.status = "reconciling";
    this.state.projection_status =
      session.mode === "writer" ? "saved" : "read_only";
    await this.trace.record("recovery", "snapshot_loaded", {
      resulting_head: this.state.expected_head,
      activity_position: snapshot.latest_activity_position,
      text: this.state.authority_text,
      details: {
        replay_floor: snapshot.replay_floor,
        session_mode: session.mode,
      },
    });

    const entries = await this.journal.list();
    this.localOrder = entries.at(-1)?.local_order ?? 0;
    await this.reconcileEntries(entries);
    this.state.status = "ready";
    this.applyEditorText(this.state.projection_text);
    this.emit();
  }

  emit() {
    this.onState?.(this.publicState());
  }

  publicState() {
    return structuredClone({
      ...this.state,
      editor_session_id: this.editorSessionId,
      strategy: this.strategy,
      recovery_drafts: this.recoveryDrafts,
      pending_batch: this.pendingBatch.map((item) => item.entry.journal_id),
      intents: this.intents.length,
      commands: this.metrics.commands,
    });
  }

  binding(undoGroup) {
    return {
      project_id: this.state.project_id,
      chapter_id: this.state.chapter_id,
      target_id: this.state.target_id,
      ownership: this.state.ownership,
      expected_head: this.state.expected_head,
      writer_generation: this.state.writer_generation,
      admission_id: this.state.admission_id,
      admission_expires_at: this.state.admission_expires_at,
      editor_contract: this.state.editor_contract,
      undo_group: undoGroup,
    };
  }

  async completeIntent({
    kind,
    beforeText,
    afterText,
    undoGroup,
    transactionCount = 1,
    historyOnlyTransactions = 0,
    evidenceSource = "tiptap-dom",
    details = {},
  }) {
    if (this.state.mode !== "writer" || this.state.submission_paused) {
      const draft = {
        reason:
          this.state.mode !== "writer"
            ? "read_only_session"
            : this.state.pause_reason,
        text: afterText,
        kind,
      };
      this.recoveryDrafts.push(draft);
      await this.trace.record("recovery", "recovery_draft_preserved", {
        intent_kind: kind,
        text: afterText,
        details: draft,
      });
      this.emit();
      return { disposition: "recovery_draft" };
    }

    if (HARD_BOUNDARY_KINDS.has(kind)) {
      await this.flush();
    }

    const intentId = `intent-${++this.intentSequence}`;
    const binding = this.binding(undoGroup);
    const intent = {
      intent_id: intentId,
      kind,
      at_ms: Date.now(),
      before_text: beforeText,
      after_text: afterText,
      transaction_count: transactionCount,
      history_only_transactions: historyOnlyTransactions,
      evidence_source: evidenceSource,
      binding,
      details,
    };
    this.intents.push(intent);
    await this.trace.record("browser", "intent_completed", {
      intent_id: intentId,
      intent_kind: kind,
      undo_group: undoGroup,
      text: afterText,
      details: {
        evidence_source: evidenceSource,
        transaction_count: transactionCount,
        history_only_transactions: historyOnlyTransactions,
        ...details,
      },
    });

    const journalId = uuid();
    let localOrder = null;
    let entry = {
      schema: "storyos.issue69.local-edit-journal.v1",
      journal_id: journalId,
      local_order: null,
      project_id: binding.project_id,
      chapter_id: binding.chapter_id,
      target_id: binding.target_id,
      editor_session_id: this.editorSessionId,
      writer_generation: binding.writer_generation,
      ownership: binding.ownership,
      expected_head: binding.expected_head,
      admission_id: binding.admission_id,
      admission_expires_at: binding.admission_expires_at,
      editor_contract: binding.editor_contract,
      undo_group: binding.undo_group,
      intent_id: intentId,
      intent_kind: kind,
      before_text: beforeText,
      after_text: afterText,
      intent_digest: await sha256({
        kind,
        before_text: beforeText,
        after_text: afterText,
        binding,
      }),
      command_id: null,
      idempotency_key: null,
      payload_digest: null,
      receipt_id: null,
      activity_position: null,
      disposition: "persisting",
      created_at: new Date().toISOString(),
    };
    const durabilityStarted = performance.now();
    await this.trace.record("journal", "journal_persist_started", {
      intent_id: intentId,
      intent_kind: kind,
      undo_group: undoGroup,
      journal_id: journalId,
      local_order: localOrder,
    });
    entry = await this.journal.append(entry);
    localOrder = entry.local_order;
    this.localOrder = Math.max(this.localOrder, localOrder);
    entry.disposition = "durable_pending";
    await this.journal.update(journalId, {
      disposition: entry.disposition,
      durable_at: new Date().toISOString(),
    });
    const durabilityMs = performance.now() - durabilityStarted;
    this.metrics.journal_durability_ms.push(durabilityMs);
    await this.trace.record("journal", "journal_durable", {
      intent_id: intentId,
      intent_kind: kind,
      undo_group: undoGroup,
      journal_id: journalId,
      local_order: localOrder,
      text: afterText,
      details: { duration_ms: Number(durabilityMs.toFixed(3)) },
    });

    this.state.projection_text = afterText;
    this.state.projection_status = "saving";
    await this.trace.record("projection", "pending_projection_applied", {
      intent_id: intentId,
      intent_kind: kind,
      undo_group: undoGroup,
      journal_id: journalId,
      local_order: localOrder,
      text: afterText,
      details: {
        authority_text_sha256: await sha256(this.state.authority_text),
        pending_is_authority: false,
      },
    });
    this.emit();

    await this.queueForSubmission({ intent, entry });
    return { disposition: "journal_durable", intent, entry };
  }

  async queueForSubmission(item) {
    const hard = HARD_BOUNDARY_KINDS.has(item.intent.kind);
    const previous = this.pendingBatch.at(-1);
    const canCoalesce =
      this.strategy === "bounded-idle" &&
      previous &&
      COALESCIBLE_KINDS.has(previous.intent.kind) &&
      COALESCIBLE_KINDS.has(item.intent.kind) &&
      bindingFingerprint(previous.intent.binding) ===
        bindingFingerprint(item.intent.binding);

    if (
      this.strategy === "semantic-intent" ||
      this.strategy === "transaction" ||
      hard
    ) {
      await this.flush();
      this.pendingBatch.push(item);
      await this.flush();
      return;
    }

    if (this.strategy === "bounded-idle" && previous && !canCoalesce) {
      await this.flush();
    }
    this.pendingBatch.push(item);
    clearTimeout(this.batchTimer);
    const delay = this.strategy === "fixed-window" ? 500 : 250;
    this.batchTimer = setTimeout(() => void this.flush(), delay);
  }

  async flush() {
    clearTimeout(this.batchTimer);
    this.batchTimer = null;
    if (this.pendingBatch.length === 0) {
      await this.submissionChain;
      return;
    }
    const batch = this.pendingBatch.splice(0);
    this.submissionChain = this.submissionChain.then(() =>
      this.submitBatch(batch),
    );
    await this.submissionChain;
  }

  async submitBatch(batch) {
    if (this.state.submission_paused) return;
    const first = batch[0];
    const last = batch.at(-1);
    const commandId = uuid();
    const idempotencyKey = uuid();
    const command = {
      command_id: commandId,
      idempotency_key: idempotencyKey,
      project_id: first.entry.project_id,
      chapter_id: first.entry.chapter_id,
      target_id: first.entry.target_id,
      editor_session_id: first.entry.editor_session_id,
      writer_generation: first.entry.writer_generation,
      expected_head: first.entry.expected_head,
      admission_id: first.entry.admission_id,
      admission_expires_at: first.entry.admission_expires_at,
      editor_contract: first.entry.editor_contract,
      ownership: first.entry.ownership,
      undo_group: first.entry.undo_group,
      intent_ids: batch.map((item) => item.intent.intent_id),
      intent_kinds: batch.map((item) => item.intent.kind),
      base_text: first.entry.before_text,
      result_text: last.entry.after_text,
    };
    command.payload_digest = await sha256(command);
    const serializedBytes = new TextEncoder().encode(
      stableJson(command),
    ).byteLength;
    this.metrics.commands += 1;
    this.metrics.payload_bytes += serializedBytes;
    this.commands.set(commandId, { command, batch });

    for (const { entry } of batch) {
      await this.journal.update(entry.journal_id, {
        command_id: commandId,
        idempotency_key: idempotencyKey,
        payload_digest: command.payload_digest,
        command_envelope: command,
        disposition: "submission_bound",
      });
      await this.trace.record("journal", "journal_submission_bound", {
        intent_id: entry.intent_id,
        intent_kind: entry.intent_kind,
        undo_group: entry.undo_group,
        journal_id: entry.journal_id,
        local_order: entry.local_order,
        command_id: commandId,
        idempotency_key: idempotencyKey,
        payload_digest: command.payload_digest,
      });
    }

    const submissionStarted = performance.now();
    for (const { entry } of batch) {
      await this.journal.update(entry.journal_id, {
        disposition: "submission_started",
      });
    }
    await this.trace.record("transport", "submission_started", {
      intent_id: first.entry.intent_id,
      intent_kind: first.entry.intent_kind,
      undo_group: first.entry.undo_group,
      journal_id: first.entry.journal_id,
      local_order: first.entry.local_order,
      command_id: commandId,
      idempotency_key: idempotencyKey,
      payload_digest: command.payload_digest,
      text: command.result_text,
      details: {
        intent_ids: command.intent_ids,
        payload_bytes: serializedBytes,
      },
    });

    const activityPromise = this.pollForActivity(commandId, 3_000);
    let receipt;
    try {
      const result = await jsonFetch(`${this.coreUrl}/author-edits`, {
        method: "POST",
        body: JSON.stringify(command),
      });
      receipt = result.receipt;
      await this.settleReceipt(receipt, batch, {
        duplicate: result.duplicate,
        settlementStarted: submissionStarted,
      });
    } catch (error) {
      for (const { entry } of batch) {
        await this.journal.update(entry.journal_id, {
          disposition: "outcome_unknown",
        });
      }
      this.state.projection_status = "needs_attention";
      await this.trace.record("transport", "outcome_unknown", {
        command_id: commandId,
        idempotency_key: idempotencyKey,
        payload_digest: command.payload_digest,
        text: command.result_text,
        details: { message: error.message, blind_retry: false },
      });
      this.emit();
    }

    const activityResult = await activityPromise;
    if (receipt?.outcome === "authoritative") {
      if (activityResult?.event?.command_id === commandId) {
        await this.converge(receipt, batch, activityResult.event);
      } else if (activityResult?.resynced) {
        await this.convergeFromSnapshot(receipt, batch);
      }
    } else if (receipt) {
      await this.preserveNonAuthoritativeDraft(receipt, batch);
    }
  }

  async settleReceipt(receipt, batch, { duplicate, settlementStarted }) {
    const alreadyObserved = this.receipts.has(receipt.command_id);
    this.receipts.set(receipt.command_id, receipt);
    const duration = performance.now() - settlementStarted;
    this.metrics.settlement_ms.push(duration);
    if (receipt.outcome === "authoritative") {
      this.metrics.core_commits += alreadyObserved ? 0 : 1;
      this.metrics.author_actions +=
        alreadyObserved || receipt.author_action_seq === null ? 0 : 1;
    }
    for (const { entry } of batch) {
      await this.journal.update(entry.journal_id, {
        receipt_id: receipt.receipt_id,
        activity_position: receipt.activity_position,
        resulting_head: receipt.resulting_head,
        outcome: receipt.outcome,
        author_action_seq: receipt.author_action_seq,
        disposition:
          receipt.outcome === "authoritative"
            ? "receipt_settled"
            : "settled_draft_preserved",
      });
    }
    await this.trace.record("core", "core_committed", {
      command_id: receipt.command_id,
      idempotency_key: receipt.idempotency_key,
      payload_digest: receipt.payload_digest,
      resulting_head: receipt.resulting_head,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      author_action_seq: receipt.author_action_seq,
      activity_position: receipt.activity_position,
      details: {
        duplicate,
        atomic_transition: true,
        reason_code: receipt.reason_code ?? null,
      },
    });
    await this.trace.record("transport", "receipt_settled", {
      command_id: receipt.command_id,
      idempotency_key: receipt.idempotency_key,
      payload_digest: receipt.payload_digest,
      resulting_head: receipt.resulting_head,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      author_action_seq: receipt.author_action_seq,
      activity_position: receipt.activity_position,
      details: {
        duplicate,
        reason_code: receipt.reason_code ?? null,
        duration_ms: Number(duration.toFixed(3)),
      },
    });
  }

  async pollForActivity(commandId, timeoutMs) {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      try {
        const result = await jsonFetch(
          `${this.coreUrl}/activity?after=${this.state.projection_position}`,
        );
        let matchingEvent = null;
        for (const event of result.events) {
          const key = `${event.position}:${event.receipt_id ?? event.type}`;
          if (this.seenActivityKeys.has(key)) {
            await this.trace.record("activity", "activity_duplicate_ignored", {
              command_id: event.command_id ?? null,
              receipt_id: event.receipt_id ?? null,
              activity_position: event.position,
            });
            continue;
          }
          if (event.position > this.state.projection_position + 1) {
            await this.trace.record("activity", "activity_sequence_gap", {
              activity_position: event.position,
              details: {
                expected_position: this.state.projection_position + 1,
              },
            });
            await this.pauseAndResync("activity_sequence_gap");
            return { resynced: true };
          }
          this.seenActivityKeys.add(key);
          this.state.projection_position = Math.max(
            this.state.projection_position,
            event.position,
          );
          await this.trace.record("activity", "activity_observed", {
            command_id: event.command_id ?? null,
            receipt_id: event.receipt_id ?? null,
            outcome: event.outcome ?? null,
            resulting_head: event.resulting_head ?? null,
            author_action_seq: event.author_action_seq ?? null,
            activity_position: event.position,
          });
          if (event.command_id === commandId) matchingEvent = event;
        }
        if (matchingEvent) return { event: matchingEvent };
      } catch (error) {
        if (error.status === 409 && error.body?.replay_floor_miss) {
          await this.trace.record("activity", "activity_replay_floor_miss", {
            details: error.body,
          });
          await this.pauseAndResync("activity_replay_floor_miss");
          return { resynced: true };
        }
      }
      await sleep(20);
    }
    return { timed_out: true };
  }

  async converge(receipt, batch, event) {
    const started = performance.now();
    this.state.expected_head = receipt.resulting_head;
    this.state.authority_text = batch.at(-1).entry.after_text;
    this.state.projection_text = this.state.authority_text;
    this.state.projection_position = Math.max(
      this.state.projection_position,
      event.position,
    );
    this.state.projection_status = "saved";
    await this.trace.record("projection", "projection_converged", {
      command_id: receipt.command_id,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      resulting_head: receipt.resulting_head,
      author_action_seq: receipt.author_action_seq,
      activity_position: event.position,
      text: this.state.projection_text,
      details: { authority_equals_projection: true },
    });
    await this.gcBatch(receipt, batch);
    this.metrics.convergence_ms.push(performance.now() - started);
    this.emit();
  }

  async convergeFromSnapshot(receipt, batch) {
    const snapshot = await jsonFetch(`${this.coreUrl}/snapshot`);
    const text = snapshot.documents[this.state.chapter_id];
    if (
      snapshot.heads[this.state.chapter_id] !== receipt.resulting_head ||
      text !== batch.at(-1).entry.after_text
    ) {
      this.state.projection_status = "needs_attention";
      await this.trace.record("recovery", "recovery_draft_preserved", {
        command_id: receipt.command_id,
        receipt_id: receipt.receipt_id,
        text: batch.at(-1).entry.after_text,
        details: { reason: "snapshot_did_not_confirm_receipt" },
      });
      return;
    }
    this.state.projection_position = snapshot.latest_activity_position;
    await this.converge(receipt, batch, {
      position: snapshot.latest_activity_position,
    });
  }

  async preserveNonAuthoritativeDraft(receipt, batch) {
    const text = batch.at(-1).entry.after_text;
    const draft = {
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      text,
      command_id: receipt.command_id,
    };
    this.recoveryDrafts.push(draft);
    this.state.projection_status = "needs_attention";
    await this.trace.record("recovery", "recovery_draft_preserved", {
      command_id: receipt.command_id,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      resulting_head: receipt.resulting_head,
      text,
      details: { complete_draft: true, gc_allowed: false },
    });
    await this.trace.record("projection", "projection_converged", {
      command_id: receipt.command_id,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      resulting_head: receipt.resulting_head,
      text: this.state.authority_text,
      details: {
        authority_equals_pending: false,
        draft_preserved: true,
      },
    });
    this.emit();
  }

  async gcBatch(receipt, batch) {
    await this.trace.record("journal", "journal_gc_eligible", {
      command_id: receipt.command_id,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      activity_position: receipt.activity_position,
      details: {
        durable_settlement: true,
        projection_converged: true,
      },
    });
    for (const { entry } of batch) {
      await this.journal.update(entry.journal_id, {
        disposition: "settled_authoritative",
      });
      await this.journal.remove(entry.journal_id);
      await this.trace.record("journal", "journal_gc_completed", {
        journal_id: entry.journal_id,
        local_order: entry.local_order,
        command_id: receipt.command_id,
        receipt_id: receipt.receipt_id,
      });
    }
  }

  async pauseAndResync(reason) {
    const started = performance.now();
    this.state.submission_paused = true;
    this.state.pause_reason = reason;
    await this.trace.record("recovery", "submission_paused", {
      details: { reason },
    });
    const snapshot = await jsonFetch(`${this.coreUrl}/snapshot`);
    this.state.expected_head = snapshot.heads[this.state.chapter_id];
    this.state.authority_text = snapshot.documents[this.state.chapter_id];
    this.state.projection_position = snapshot.latest_activity_position;
    await this.trace.record("recovery", "snapshot_loaded", {
      resulting_head: this.state.expected_head,
      activity_position: snapshot.latest_activity_position,
      text: this.state.authority_text,
      details: { reason, replay_floor: snapshot.replay_floor },
    });
    this.state.submission_paused = false;
    this.state.pause_reason = null;
    this.metrics.recovery_ms.push(performance.now() - started);
    await this.trace.record("recovery", "journal_reconciled", {
      details: { reason, inference_used: false },
    });
    this.emit();
  }

  async reconcileEntries(entries) {
    const reconciledCommands = new Set();
    for (const entry of entries) {
      if (entry.disposition === "gc_completed") continue;
      const sameScope =
        entry.project_id === this.state.project_id &&
        entry.chapter_id === this.state.chapter_id &&
        entry.target_id === this.state.target_id &&
        entry.ownership === this.state.ownership &&
        entry.admission_id === this.state.admission_id &&
        entry.editor_contract === this.state.editor_contract &&
        Date.parse(entry.admission_expires_at) > Date.now();
      const sameBinding =
        sameScope &&
        entry.expected_head === this.state.expected_head &&
        entry.writer_generation === this.state.writer_generation;

      if (
        entry.idempotency_key &&
        !reconciledCommands.has(entry.command_id)
      ) {
        reconciledCommands.add(entry.command_id);
        await this.trace.record("recovery", "journal_reconciled", {
          journal_id: entry.journal_id,
          local_order: entry.local_order,
          command_id: entry.command_id,
          idempotency_key: entry.idempotency_key,
          payload_digest: entry.payload_digest,
          text: entry.after_text,
          details: {
            receipt_lookup_before_binding_decision: true,
            scope_equal: sameScope,
          },
        });
        const reconciled = await this.reconcileOutcome(entry);
        if (reconciled.status === "settled") continue;
      }

      if (!sameBinding || this.state.mode !== "writer") {
        this.recoveryDrafts.push({
          reason: "recovery_binding_mismatch",
          text: entry.after_text,
          journal_id: entry.journal_id,
        });
        await this.trace.record("recovery", "recovery_draft_preserved", {
          journal_id: entry.journal_id,
          local_order: entry.local_order,
          command_id: entry.command_id,
          idempotency_key: entry.idempotency_key,
          text: entry.after_text,
          details: {
            reason: "recovery_binding_mismatch",
            automatic_resubmit: false,
          },
        });
        continue;
      }
      this.state.projection_text = entry.after_text;
      this.state.projection_status = "saving";
      await this.trace.record("recovery", "journal_reconciled", {
        journal_id: entry.journal_id,
        local_order: entry.local_order,
        command_id: entry.command_id,
        idempotency_key: entry.idempotency_key,
        text: entry.after_text,
        details: { bindings_equal: true },
      });
      if (entry.disposition === "durable_pending" && !entry.command_id) {
        const intent = {
          intent_id: entry.intent_id,
          kind: entry.intent_kind,
          at_ms: Date.parse(entry.created_at),
          before_text: entry.before_text,
          after_text: entry.after_text,
          transaction_count: 1,
          history_only_transactions: 0,
          evidence_source: "indexeddb-recovery",
          binding: {
            project_id: entry.project_id,
            chapter_id: entry.chapter_id,
            target_id: entry.target_id,
            ownership: entry.ownership,
            expected_head: entry.expected_head,
            writer_generation: entry.writer_generation,
            admission_id: entry.admission_id,
            admission_expires_at: entry.admission_expires_at,
            editor_contract: entry.editor_contract,
            undo_group: entry.undo_group,
          },
          details: { recovered_from_durable_journal: true },
        };
        this.intents.push(intent);
        await this.trace.record("recovery", "submission_reauthorized", {
          intent_id: entry.intent_id,
          intent_kind: entry.intent_kind,
          undo_group: entry.undo_group,
          journal_id: entry.journal_id,
          local_order: entry.local_order,
          text: entry.after_text,
          details: {
            all_bindings_equal: true,
            network_was_never_attempted: true,
          },
        });
        await this.queueForSubmission({ intent, entry });
        continue;
      }
    }
  }

  async reconcileOutcome(entry, { reauthorize = false } = {}) {
    const started = performance.now();
    try {
      const result = await jsonFetch(
        `${this.coreUrl}/receipts/${encodeURIComponent(
          entry.idempotency_key,
        )}`,
      );
      if (result.receipt) {
        if (result.receipt.payload_digest !== entry.payload_digest) {
          await this.trace.record("recovery", "recovery_draft_preserved", {
            journal_id: entry.journal_id,
            command_id: entry.command_id,
            idempotency_key: entry.idempotency_key,
            payload_digest: entry.payload_digest,
            receipt_id: result.receipt.receipt_id,
            text: entry.after_text,
            details: { reason: "receipt_payload_digest_mismatch" },
          });
          return { status: "receipt_mismatch" };
        }
        const matchingEntries = (await this.journal.list()).filter(
          (candidate) => candidate.command_id === entry.command_id,
        );
        const batch = matchingEntries.map((candidate) => ({
          entry: candidate,
          intent: this.intents.find(
            (intent) => intent.intent_id === candidate.intent_id,
          ) ?? {
            intent_id: candidate.intent_id,
            kind: candidate.intent_kind,
          },
        }));
        await this.settleReceipt(result.receipt, batch, {
          duplicate: true,
          settlementStarted: started,
        });
        await this.convergeFromSnapshot(result.receipt, batch);
        return { status: "settled" };
      }
    } catch (error) {
      if (error.status !== 404) throw error;
    }

    if (!reauthorize) {
      await this.trace.record("recovery", "outcome_unknown", {
        journal_id: entry.journal_id,
        command_id: entry.command_id,
        idempotency_key: entry.idempotency_key,
        text: entry.after_text,
        details: {
          receipt_found: false,
          blind_retry: false,
          requires_reauthorization: true,
        },
      });
      return { status: "requires_reauthorization" };
    }

    const sameBinding =
      entry.expected_head === this.state.expected_head &&
      entry.writer_generation === this.state.writer_generation &&
      entry.admission_id === this.state.admission_id &&
      entry.editor_contract === this.state.editor_contract &&
      Date.parse(entry.admission_expires_at) > Date.now();
    if (!sameBinding) {
      await this.trace.record("recovery", "recovery_draft_preserved", {
        journal_id: entry.journal_id,
        command_id: entry.command_id,
        idempotency_key: entry.idempotency_key,
        text: entry.after_text,
        details: { reason: "reauthorization_binding_mismatch" },
      });
      return { status: "recovery_draft" };
    }
    await this.trace.record("recovery", "submission_reauthorized", {
      journal_id: entry.journal_id,
      command_id: entry.command_id,
      idempotency_key: entry.idempotency_key,
      details: { all_bindings_equal: true },
    });
    let stored = this.commands.get(entry.command_id);
    if (!stored && entry.command_envelope) {
      const entries = (await this.journal.list()).filter(
        (candidate) => candidate.command_id === entry.command_id,
      );
      stored = {
        command: entry.command_envelope,
        batch: entries.map((candidate) => ({
          entry: candidate,
          intent: {
            intent_id: candidate.intent_id,
            kind: candidate.intent_kind,
          },
        })),
      };
      this.commands.set(entry.command_id, stored);
    }
    if (!stored) {
      return { status: "command_envelope_unavailable" };
    }
    const result = await jsonFetch(`${this.coreUrl}/author-edits`, {
      method: "POST",
      body: JSON.stringify(stored.command),
    });
    await this.settleReceipt(result.receipt, stored.batch, {
      duplicate: result.duplicate,
      settlementStarted: started,
    });
    await this.convergeFromSnapshot(result.receipt, stored.batch);
    return { status: "settled_after_reauthorization" };
  }

  async reconcileAll({ reauthorize = false } = {}) {
    await this.waitForCore();
    const entries = await this.journal.list();
    const results = [];
    for (const entry of entries) {
      if (entry.disposition === "outcome_unknown") {
        results.push(await this.reconcileOutcome(entry, { reauthorize }));
      }
    }
    return results;
  }

  async waitForCore(timeoutMs = 5_000) {
    const deadline = performance.now() + timeoutMs;
    while (performance.now() < deadline) {
      try {
        await jsonFetch(`${this.coreUrl}/health`);
        return;
      } catch {
        await sleep(50);
      }
    }
    throw new Error("fake_core_restart_timeout");
  }

  async duplicateLastSubmission() {
    const stored = [...this.commands.values()].at(-1);
    if (!stored) throw new Error("no_command_to_duplicate");
    const result = await jsonFetch(`${this.coreUrl}/author-edits`, {
      method: "POST",
      body: JSON.stringify(stored.command),
    });
    await this.trace.record("transport", "duplicate_submission_observed", {
      command_id: result.receipt.command_id,
      idempotency_key: result.receipt.idempotency_key,
      receipt_id: result.receipt.receipt_id,
      author_action_seq: result.receipt.author_action_seq,
      outcome: result.receipt.outcome,
      details: { duplicate: result.duplicate },
    });
    return result;
  }

  async takeover() {
    const oldProjection = this.readEditorText();
    const result = await jsonFetch(`${this.coreUrl}/sessions/takeover`, {
      method: "POST",
      body: JSON.stringify({ editor_session_id: this.editorSessionId }),
    });
    this.state.mode = "writer";
    this.state.writer_generation = result.writer_generation;
    this.state.expected_head =
      result.snapshot.heads[this.state.chapter_id];
    this.state.authority_text =
      result.snapshot.documents[this.state.chapter_id];
    this.state.projection_text = this.state.authority_text;
    this.state.projection_position =
      result.snapshot.latest_activity_position;
    if (oldProjection !== this.state.authority_text) {
      this.recoveryDrafts.push({
        reason: "takeover_reconciled_old_projection",
        text: oldProjection,
      });
    }
    await this.trace.record("recovery", "writer_takeover_completed", {
      text: this.state.authority_text,
      details: {
        fenced_session_id: result.fenced_session_id,
        fenced_writer_generation: result.fenced_writer_generation,
        new_writer_generation: result.writer_generation,
      },
    });
    this.applyEditorText(this.state.projection_text);
    this.emit();
    return result;
  }

  async switchChapter(chapterId, targetId) {
    await this.flush();
    await this.trace.record("browser", "chapter_switch_started", {
      details: {
        from_chapter_id: this.state.chapter_id,
        to_chapter_id: chapterId,
        local_state_persisted_first: true,
      },
    });
    this.state.chapter_id = chapterId;
    this.state.target_id = targetId;
    const snapshot = await jsonFetch(`${this.coreUrl}/snapshot`);
    this.state.expected_head = snapshot.heads[chapterId] ?? "revision-0";
    this.state.authority_text = snapshot.documents[chapterId] ?? "";
    this.state.projection_text = this.state.authority_text;
    this.applyEditorText(this.state.projection_text);
    this.emit();
  }

  compareCandidates() {
    return [
      "transaction",
      "semantic-intent",
      "bounded-idle",
      "fixed-window",
    ].map((strategy) => partitionIntents(this.intents, strategy));
  }

  async report() {
    await this.flush();
    return {
      state: this.publicState(),
      trace: this.trace.export(),
      intents: structuredClone(this.intents),
      candidate_comparison: this.compareCandidates(),
      journal: await this.journal.list(),
      journal_metrics: await this.journal.metrics(),
      measurements: structuredClone(this.metrics),
      core: await jsonFetch(`${this.coreUrl}/snapshot`),
    };
  }
}
