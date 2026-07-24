import { createHash, randomUUID } from "node:crypto";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import path from "node:path";
import {
  CORE_SCHEMA,
  DEFAULT_CHAPTER_ID,
  PROJECT_ID,
  stableJson,
} from "../shared/contract.mjs";

function digest(value) {
  return createHash("sha256").update(value).digest("hex");
}

function initialState(initialText = "") {
  return {
    schema: CORE_SCHEMA,
    project_id: PROJECT_ID,
    writer_generation: 0,
    active_session_id: null,
    heads: {
      [DEFAULT_CHAPTER_ID]: "revision-0",
      "chapter-beta": "revision-beta-0",
    },
    documents: {
      [DEFAULT_CHAPTER_ID]: initialText,
      "chapter-beta": "",
    },
    receipts: {},
    activities: [],
    author_actions: [],
    next_activity_position: 1,
    next_author_action_seq: 1,
    replay_floor: 0,
    config: {
      activity_order: "http-first",
      activity_duplicate: false,
      activity_gap: false,
      ack_loss: false,
      core_crash: "none",
      response_delay_ms: 0,
      activity_delay_ms: 0,
      outcome: "authoritative",
    },
  };
}

export class CoreStore {
  constructor(statePath) {
    this.statePath = statePath;
    this.state = initialState();
    this.writeChain = Promise.resolve();
  }

  async load() {
    try {
      this.state = JSON.parse(await readFile(this.statePath, "utf8"));
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
      await this.persist();
    }
    return this.state;
  }

  async persist() {
    this.writeChain = this.writeChain.then(async () => {
      await mkdir(path.dirname(this.statePath), { recursive: true });
      const temporaryPath = `${this.statePath}.next`;
      await writeFile(
        temporaryPath,
        `${JSON.stringify(this.state, null, 2)}\n`,
        "utf8",
      );
      await rename(temporaryPath, this.statePath);
    });
    await this.writeChain;
  }

  async reset({ initial_text = "", config = {} } = {}) {
    this.state = initialState(initial_text);
    this.state.config = { ...this.state.config, ...config };
    await this.persist();
    return this.snapshot();
  }

  async configure(config = {}) {
    this.state.config = { ...this.state.config, ...config };
    await this.persist();
    return this.snapshot();
  }

  snapshot() {
    return structuredClone({
      schema: this.state.schema,
      project_id: this.state.project_id,
      writer_generation: this.state.writer_generation,
      active_session_id: this.state.active_session_id,
      heads: this.state.heads,
      documents: this.state.documents,
      receipts: this.state.receipts,
      author_actions: this.state.author_actions,
      latest_activity_position: this.state.next_activity_position - 1,
      replay_floor: this.state.replay_floor,
      config: this.state.config,
    });
  }

  async openSession({ editor_session_id, request_writer = true }) {
    if (!request_writer) {
      return {
        editor_session_id,
        mode: "read_only",
        writer_generation: null,
        active_writer_session_id: this.state.active_session_id,
      };
    }
    if (this.state.active_session_id === null) {
      this.state.writer_generation += 1;
      this.state.active_session_id = editor_session_id;
      await this.persist();
      return {
        editor_session_id,
        mode: "writer",
        writer_generation: this.state.writer_generation,
      };
    }
    if (this.state.active_session_id === editor_session_id) {
      return {
        editor_session_id,
        mode: "writer",
        writer_generation: this.state.writer_generation,
      };
    }
    return {
      editor_session_id,
      mode: "read_only",
      writer_generation: null,
      active_writer_session_id: this.state.active_session_id,
      active_writer_generation: this.state.writer_generation,
    };
  }

  async takeover({ editor_session_id }) {
    const fenced_session_id = this.state.active_session_id;
    const fenced_writer_generation = this.state.writer_generation;
    this.state.writer_generation += 1;
    this.state.active_session_id = editor_session_id;
    await this.persist();
    return {
      editor_session_id,
      mode: "writer",
      writer_generation: this.state.writer_generation,
      fenced_session_id,
      fenced_writer_generation,
      snapshot: this.snapshot(),
    };
  }

  receipt(idempotencyKey) {
    return this.state.receipts[idempotencyKey] ?? null;
  }

  async setReplayFloor(position) {
    this.state.replay_floor = Math.max(0, Number(position));
    await this.persist();
    return this.snapshot();
  }

  activityAfter(after) {
    const numericAfter = Number(after);
    if (numericAfter < this.state.replay_floor) {
      return {
        replay_floor_miss: true,
        replay_floor: this.state.replay_floor,
        latest_activity_position: this.state.next_activity_position - 1,
      };
    }
    const now = Date.now();
    let events = this.state.activities.filter(
      (event) =>
        event.position > numericAfter &&
        (event.visible_at_epoch_ms ?? 0) <= now,
    );
    if (this.state.config.activity_gap && events.length > 0) {
      const first = events[0];
      events = [
        {
          type: "ProjectActivityGapProbe",
          position: first.position + 1,
          project_id: PROJECT_ID,
          receipt_id: null,
          outcome: null,
          resulting_head: first.resulting_head,
        },
      ];
    } else if (this.state.config.activity_duplicate && events.length > 0) {
      events = [events[0], events[0], ...events.slice(1)];
    }
    return {
      replay_floor_miss: false,
      replay_floor: this.state.replay_floor,
      latest_activity_position: this.state.next_activity_position - 1,
      events: structuredClone(events),
    };
  }

  async applyAuthorEdit(command) {
    const existing = this.receipt(command.idempotency_key);
    if (existing) {
      return { receipt: structuredClone(existing), duplicate: true };
    }

    const failure = this.validateCommand(command);
    if (failure) {
      const receipt = this.createNonAuthoritativeReceipt(command, failure);
      this.state.receipts[command.idempotency_key] = receipt;
      await this.persist();
      return { receipt: structuredClone(receipt), duplicate: false };
    }

    const configuredOutcome = this.state.config.outcome;
    if (configuredOutcome !== "authoritative") {
      const receipt = this.createNonAuthoritativeReceipt(
        command,
        configuredOutcome,
      );
      this.state.receipts[command.idempotency_key] = receipt;
      await this.persist();
      return { receipt: structuredClone(receipt), duplicate: false };
    }

    const chapterId = command.chapter_id;
    if (command.result_text === this.state.documents[chapterId]) {
      const receipt = this.createNonAuthoritativeReceipt(command, "no_effect");
      this.state.receipts[command.idempotency_key] = receipt;
      await this.persist();
      return { receipt: structuredClone(receipt), duplicate: false };
    }

    const authorActionSeq = this.state.next_author_action_seq;
    const activityPosition = this.state.next_activity_position;
    const resultingHead = `revision-${digest(
      stableJson({
        command_id: command.command_id,
        expected_head: command.expected_head,
        result_text: command.result_text,
      }),
    ).slice(0, 16)}`;
    const receipt = {
      receipt_id: randomUUID(),
      idempotency_key: command.idempotency_key,
      command_id: command.command_id,
      outcome: "authoritative",
      expected_head: command.expected_head,
      resulting_head: resultingHead,
      author_action_seq: authorActionSeq,
      activity_position: activityPosition,
      draft_preserved: false,
      payload_digest: command.payload_digest,
    };
    const action = {
      author_action_seq: authorActionSeq,
      command_id: command.command_id,
      chapter_id: chapterId,
      before_text: this.state.documents[chapterId],
      after_text: command.result_text,
      before_head: command.expected_head,
      after_head: resultingHead,
      undo_group: command.undo_group,
    };
    const visibleAt =
      Date.now() +
      (this.state.config.activity_order === "http-first"
        ? this.state.config.activity_delay_ms
        : 0);
    const activity = {
      type: "ManuscriptAuthorEditSettled",
      position: activityPosition,
      project_id: command.project_id,
      chapter_id: chapterId,
      command_id: command.command_id,
      receipt_id: receipt.receipt_id,
      outcome: receipt.outcome,
      resulting_head: resultingHead,
      author_action_seq: authorActionSeq,
      visible_at_epoch_ms: visibleAt,
    };

    this.state.documents[chapterId] = command.result_text;
    this.state.heads[chapterId] = resultingHead;
    this.state.receipts[command.idempotency_key] = receipt;
    this.state.author_actions.push(action);
    this.state.activities.push(activity);
    this.state.next_author_action_seq += 1;
    this.state.next_activity_position += 1;
    await this.persist();
    return { receipt: structuredClone(receipt), duplicate: false };
  }

  validateCommand(command) {
    if (command.editor_session_id !== this.state.active_session_id) {
      return { outcome: "refused", reason_code: "stale_writer_session" };
    }
    if (command.writer_generation !== this.state.writer_generation) {
      return { outcome: "refused", reason_code: "stale_writer_generation" };
    }
    if (command.expected_head !== this.state.heads[command.chapter_id]) {
      return { outcome: "conflicted", reason_code: "expected_head_mismatch" };
    }
    if (Date.parse(command.admission_expires_at) <= Date.now()) {
      return { outcome: "refused", reason_code: "admission_expired" };
    }
    if (command.admission_id !== "admission-direct-edit") {
      return { outcome: "refused", reason_code: "admission_mismatch" };
    }
    return null;
  }

  createNonAuthoritativeReceipt(command, outcome) {
    const normalized =
      typeof outcome === "string"
        ? { outcome, reason_code: `configured_${outcome}` }
        : outcome;
    return {
      receipt_id: randomUUID(),
      idempotency_key: command.idempotency_key,
      command_id: command.command_id,
      outcome: normalized.outcome,
      reason_code: normalized.reason_code,
      expected_head: command.expected_head,
      resulting_head: this.state.heads[command.chapter_id],
      author_action_seq: null,
      activity_position: null,
      draft_preserved: true,
      payload_digest: command.payload_digest,
    };
  }
}
