import { TRACE_SCHEMA, sha256, utf8Bytes } from "../shared/contract.mjs";

const MAX_RECORDS = 5_000;

export class TraceRecorder {
  constructor({ runId, scenarioId, context }) {
    this.runId = runId;
    this.scenarioId = scenarioId;
    this.context = context;
    const storageKey = `storyos.issue69.trace:${runId}:${scenarioId}`;
    const restored = JSON.parse(sessionStorage.getItem(storageKey) ?? "null");
    this.storageKey = storageKey;
    this.startedAt = restored?.started_at_epoch_ms ?? Date.now();
    this.records = restored?.records ?? [];
    this.sequence = this.records.at(-1)?.seq ?? 0;
  }

  async record(actor, stage, fields = {}) {
    const context = this.context?.() ?? {};
    const text = fields.text;
    const record = {
      trace_schema: TRACE_SCHEMA,
      run_id: this.runId,
      scenario_id: this.scenarioId,
      seq: ++this.sequence,
      at_ms: Date.now() - this.startedAt,
      actor,
      stage,
      chapter_id: null,
      target_id: null,
      editor_session_id: null,
      writer_generation: null,
      intent_id: null,
      intent_kind: null,
      undo_group: null,
      journal_id: null,
      local_order: null,
      command_id: null,
      idempotency_key: null,
      payload_digest: null,
      expected_head: null,
      resulting_head: null,
      admission_id: null,
      admission_expires_at: null,
      receipt_id: null,
      outcome: null,
      author_action_seq: null,
      activity_position: null,
      projection_position: null,
      text_sha256: text === undefined ? null : await sha256(text),
      utf8_bytes: text === undefined ? null : utf8Bytes(text),
      details: {},
      ...context,
      ...fields,
    };
    delete record.text;
    if (this.records.length >= MAX_RECORDS) {
      if (!this.records.some((item) => item.stage === "trace_truncated")) {
        this.records.push({
          ...record,
          seq: ++this.sequence,
          actor: "browser",
          stage: "trace_truncated",
          details: { max_records: MAX_RECORDS },
        });
      }
      return record;
    }
    this.records.push(record);
    this.persist();
    return record;
  }

  persist() {
    sessionStorage.setItem(
      this.storageKey,
      JSON.stringify({
        started_at_epoch_ms: this.startedAt,
        records: this.records,
      }),
    );
  }

  export() {
    return structuredClone(this.records);
  }
}
