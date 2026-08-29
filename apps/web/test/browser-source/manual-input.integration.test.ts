import { expect, it } from "vitest";

import type {
  DigestValue,
  GetEditorSessionResponse,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import {
  openEditorWorkspace,
  persistReplaceSelection,
  submitOnePendingAuthorEdit,
} from "../../src/editor-session.ts";
import type {
  EditorReadyState,
  JournalSubmissionGroup,
  PendingEditProjection,
  ReplaceSelectionEdit,
} from "../../src/editor-types.ts";
import {
  readJournalSnapshot,
  validateJournalSnapshot,
} from "../../src/local-edit-journal.ts";
import { attachManualInput } from "../../src/manual-input.ts";
import {
  applyImeComposition,
  applyTrustedInput,
  updateClipboardPermission,
} from "../support/browser-command-client.ts";
import {
  BLOCK,
  OWNER,
  PROJECT,
  SESSION,
  chapterRevision,
  createAppliedAuthorEditResponse,
  createBrowserScenario,
  deleteJournal,
  jsonResponse,
  requestHeaders,
  requireDigestValue,
  requireEditorReady,
  requireRequestBody,
  requestResult,
} from "./scenario.ts";

interface NativeEventTrace {
  readonly data: string | null;
  readonly inputType: string | null;
  readonly isComposing: boolean | null;
  readonly isTrusted: boolean;
  readonly type: string;
}

interface SubmissionTrace {
  readonly body: string;
  readonly pending: number;
}

it("settles trusted input, clipboard actions, and controlled Chrome IME in the Journal", async () => {
  document.body.innerHTML = `
    <label for="editor">Story editor</label>
    <textarea id="editor">Base</textarea>
    <label for="synthetic">Synthetic editor</label>
    <textarea id="synthetic">Base</textarea>
    <label for="silent">Silent editor</label>
    <textarea id="silent">Base</textarea>
  `;
  const editor = document.querySelector("#editor");
  const syntheticEditor = document.querySelector("#synthetic");
  const silentEditor = document.querySelector("#silent");
  if (!(editor instanceof HTMLTextAreaElement)
    || !(syntheticEditor instanceof HTMLTextAreaElement)
    || !(silentEditor instanceof HTMLTextAreaElement)) {
    throw new Error("the manual input editors are unavailable");
  }
  const scenario = createBrowserScenario();
  const trace = {
    authorEdits: [] as Array<{
      readonly idempotencyKey: string;
      readonly request: Record<string, unknown>;
    }>,
    challenges: [] as Record<string, unknown>[],
    failures: [] as string[],
    native: [] as NativeEventTrace[],
    persisted: [] as ReplaceSelectionEdit[],
    projections: [] as PendingEditProjection[],
    submissions: [] as SubmissionTrace[],
  };
  for (const type of [
    "beforeinput",
    "input",
    "compositionstart",
    "compositionupdate",
    "compositionend",
    "paste",
    "cut",
  ]) {
    editor.addEventListener(type, (event) => {
      trace.native.push({
        type: event.type,
        inputType: event instanceof InputEvent ? event.inputType : null,
        data: event instanceof InputEvent || event instanceof CompositionEvent ? event.data : null,
        isComposing: event instanceof InputEvent ? event.isComposing : null,
        isTrusted: event.isTrusted,
      });
    });
  }

  let canonicalSession: GetEditorSessionResponse = {
    ...scenario.session,
    schema_id: "storyos.query.editor-session.response.v1",
  };
  let settlementSequence = 0;
  let commandDigest: DigestValue | undefined;
  let workspace: EditorReadyState | undefined;
  const uuid = (value: number): string =>
    `018f0000-0000-7001-8000-${String(value).padStart(12, "0")}`;
  const sha256 = async (body: string): Promise<string> => {
    const digest = new Uint8Array(await crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(body),
    ));
    return [...digest].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  };
  const fetchImpl: typeof fetch = async (input, init) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      const request = requireRequestBody(init);
      trace.challenges.push(request);
      if (request.command_schema === "storyos.command.apply-author-edit.request.v1") {
        commandDigest = requireDigestValue(request.canonical_command_digest, "command digest");
      }
      return jsonResponse({
        nonce: "a".repeat(64),
        expires_at: "2026-08-13T08:05:00.000Z",
        limit_profile_revision: "storyos.foundation.absolute.v1",
      });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) return jsonResponse(canonicalSession);
    if (path.endsWith("/manuscript/author-edits")) {
      const request = requireRequestBody(init);
      const idempotencyKey = requestHeaders(init).get("idempotency-key");
      if (!workspace || !idempotencyKey || !commandDigest) {
        throw new Error("the admitted manual input identity is unavailable");
      }
      trace.authorEdits.push({ request, idempotencyKey });
      const body = workspace.pending.body;
      const priorRevision = canonicalSession.base_snapshot.authoritative_head_revision_id;
      settlementSequence += 1;
      const position = String(settlementSequence);
      const revisionId = uuid(100 + settlementSequence * 10 + 1);
      const commitId = uuid(100 + settlementSequence * 10 + 2);
      const commandId = uuid(100 + settlementSequence * 10 + 3);
      const admissionId = uuid(100 + settlementSequence * 10 + 4);
      const receiptId = uuid(100 + settlementSequence * 10 + 5);
      canonicalSession = {
        ...canonicalSession,
        correlation_id: uuid(100 + settlementSequence * 10 + 6),
        base_snapshot: {
          ...canonicalSession.base_snapshot,
          snapshot_id: uuid(100 + settlementSequence * 10 + 7),
          project_activity_position: position,
          authoritative_head_revision_id: revisionId,
          materialized_revision: chapterRevision(revisionId, body),
          materialized_payload_digest: {
            algorithm: "sha256",
            profile: "storyos.canonical-payload.sha256.v1",
            value_hex_lowercase: await sha256(body),
          },
          created_at: new Date(
            Date.parse("2026-08-15T08:00:00.000Z") + settlementSequence,
          ).toISOString(),
        },
      };
      return jsonResponse(createAppliedAuthorEditResponse({
        request,
        commandDigest,
        idempotencyKey,
        commandId,
        authorCommandAdmissionId: admissionId,
        receiptId,
        authoritativeRevisionId: revisionId,
        authoritativeCommitId: commitId,
        projectActivityPosition: position,
        priorRevisionId: priorRevision,
        body,
      }));
    }
    throw new Error(`unexpected request: ${path}`);
  };

  await deleteJournal(scenario.journalName);
  const persistIntent = async (
    currentWorkspace: Parameters<typeof persistReplaceSelection>[0],
    edit: ReplaceSelectionEdit,
    cryptoImpl?: Crypto,
  ): Promise<PendingEditProjection> => {
    const projection = await persistReplaceSelection(currentWorkspace, edit, cryptoImpl);
    trace.persisted.push(structuredClone(edit));
    return projection;
  };
  const submitGroup = async (
    options: Parameters<typeof submitOnePendingAuthorEdit>[0],
  ): Promise<PendingEditProjection> => {
    if (!workspace) throw new Error("the Editor Workspace is unavailable");
    const pending = workspace.pending.unsettled_intent_count;
    const projection = await submitOnePendingAuthorEdit(options);
    trace.submissions.push({ body: projection.body, pending });
    return projection;
  };
  let manualNow = Date.parse("2026-08-15T12:00:00.000Z");
  let timerSequence = 0;
  const delayedTimers = new Map<number, () => void>();
  let controller: ReturnType<typeof attachManualInput> | undefined;
  const syntheticTrace = { failures: [] as string[], persisted: [] as ReplaceSelectionEdit[] };
  let syntheticController: ReturnType<typeof attachManualInput> | undefined;
  const silentTrace = { failures: [] as string[], persisted: [] as ReplaceSelectionEdit[] };
  let silentController: ReturnType<typeof attachManualInput> | undefined;

  const focusAt = (target: HTMLTextAreaElement, start: number, end = start): void => {
    target.focus();
    target.setSelectionRange(start, end);
  };
  const trustedInput = async (
    request: Parameters<typeof applyTrustedInput>[0],
  ): Promise<void> => {
    await applyTrustedInput(request);
    if (!controller) throw new Error("the manual input controller is unavailable");
    await controller.whenIdle();
  };

  try {
    const state = await openEditorWorkspace({
      baseUrl: location.origin,
      project: scenario.project,
      chapter: scenario.chapter,
      profile: scenario.profile,
      fetchImpl,
      indexedDBImpl: indexedDB,
      cryptoImpl: crypto,
    });
    requireEditorReady(state);
    workspace = state;
    controller = attachManualInput({
      editor,
      workspace,
      persistIntent,
      submitGroup,
      baseUrl: location.origin,
      fetchImpl,
      cryptoImpl: crypto,
      onProjection: (projection) => trace.projections.push(projection),
      onFailure: (error) => trace.failures.push(
        error instanceof Error ? error.message : String(error),
      ),
      nowImpl: () => manualNow,
      setTimeoutImpl: (callback) => {
        if (typeof callback !== "function") throw new Error("string timers are unsupported");
        timerSequence += 1;
        delayedTimers.set(timerSequence, callback);
        return timerSequence;
      },
      clearTimeoutImpl: (timer) => {
        if (typeof timer !== "number") throw new Error("the browser timer ID is invalid");
        delayedTimers.delete(timer);
      },
      isTrustedEvent: (event) => event.isTrusted
        || event.type.startsWith("composition")
        || (event instanceof InputEvent && event.inputType === "insertCompositionText")
        || (event instanceof InputEvent
          && event.inputType === "insertText"
          && event.data === "中文"),
    });
    syntheticController = attachManualInput({
      editor: syntheticEditor,
      workspace,
      baseUrl: location.origin,
      persistIntent: async (_currentWorkspace, edit) => {
        syntheticTrace.persisted.push(edit);
        return state.pending;
      },
      submitGroup,
      onFailure: (error) => syntheticTrace.failures.push(
        error instanceof Error ? error.message : String(error),
      ),
    });
    silentController = attachManualInput({
      editor: silentEditor,
      workspace,
      baseUrl: location.origin,
      persistIntent: async (_currentWorkspace, edit) => {
        silentTrace.persisted.push(edit);
        return state.pending;
      },
      submitGroup,
      onFailure: (error) => silentTrace.failures.push(
        error instanceof Error ? error.message : String(error),
      ),
    });
    syntheticEditor.value = "Injected";
    syntheticEditor.dispatchEvent(new InputEvent("input", {
      inputType: "insertText",
      data: "Injected",
    }));
    expect(syntheticTrace).toEqual({
      persisted: [],
      failures: ["Manual input lost its trusted input boundary"],
    });
    expect(syntheticEditor.readOnly).toBe(true);

    silentEditor.value = "Injected";
    focusAt(silentEditor, 8);
    await applyTrustedInput({ operation: "insert_text", text: "!" });
    await silentController.whenIdle();
    expect(silentTrace).toEqual({
      persisted: [],
      failures: ["Manual input lost its trusted beforeinput boundary"],
    });

    focusAt(editor, 4);
    await trustedInput({ operation: "insert_text", text: "A" });
    manualNow += 251;
    focusAt(editor, 0, 4);
    await trustedInput({ operation: "insert_text", text: "X" });
    await controller.flush();
    focusAt(editor, 2);
    await trustedInput({ operation: "backspace" });

    await updateClipboardPermission({ action: "grant" });
    await navigator.clipboard.writeText("P");
    focusAt(editor, 1);
    await trustedInput({ operation: "paste" });
    await trustedInput({ operation: "insert_text", text: "Q" });
    focusAt(editor, 1, 3);
    await trustedInput({ operation: "cut" });

    focusAt(editor, 1);
    await applyImeComposition({
      text: "取消",
      replacementStart: 1,
      replacementEnd: 1,
      selectionStart: 2,
      selectionEnd: 2,
    });
    await controller.whenIdle();
    expect(trace.persisted).toHaveLength(6);
    await applyImeComposition({
      text: "",
      replacementStart: 1,
      replacementEnd: 1,
      selectionStart: 0,
      selectionEnd: 0,
    });
    await controller.whenIdle();
    expect(editor.value).toBe("X");
    expect(trace.persisted).toHaveLength(6);

    await applyImeComposition({
      text: "中文",
      replacementStart: 1,
      replacementEnd: 1,
      selectionStart: 2,
      selectionEnd: 2,
    });
    expect(trace.persisted).toHaveLength(6);
    await trustedInput({ operation: "insert_text", text: "中文" });
    const redundantCommit = { bubbles: true, inputType: "insertText", data: "中文" };
    editor.dispatchEvent(new InputEvent("beforeinput", redundantCommit));
    editor.dispatchEvent(new InputEvent("input", redundantCommit));
    await controller.whenIdle();
    expect(trace.persisted).toHaveLength(7);

    await trustedInput({ operation: "insert_text", text: "!" });
    await controller.flush();
    await trustedInput({ operation: "backspace" });
    await controller.flush();
    focusAt(editor, 3);
    await applyImeComposition({
      text: "draft",
      replacementStart: 3,
      replacementEnd: 3,
      selectionStart: 5,
      selectionEnd: 5,
    });
    expect(trace.persisted).toHaveLength(9);
    await trustedInput({ operation: "insert_text", text: "word" });
    focusAt(editor, 0, editor.value.length);
    await trustedInput({ operation: "insert_text", text: "aaaa" });
    await controller.flush();
    focusAt(editor, 1, 2);
    await trustedInput({ operation: "backspace" });
    await controller.flush();
    focusAt(editor, 0, editor.value.length);
    await trustedInput({ operation: "insert_text", text: "😀" });
    await controller.flush();
    focusAt(editor, 0, 2);
    await trustedInput({ operation: "insert_text", text: "😃" });
    await controller.flush();

    expect(trace.persisted.map(({ undoGroupId: _undo, createdAt: _created, ...edit }) => edit))
      .toEqual([{
        from: 4,
        to: 4,
        text: "A",
        resultingBody: "BaseA",
        inputOrigin: "typing",
      }, {
        from: 0,
        to: 4,
        text: "X",
        resultingBody: "XA",
        inputOrigin: "selection_replacement",
      }, {
        from: 1,
        to: 2,
        text: "",
        resultingBody: "X",
        inputOrigin: "deletion",
      }, {
        from: 1,
        to: 1,
        text: "P",
        resultingBody: "XP",
        inputOrigin: "paste",
      }, {
        from: 2,
        to: 2,
        text: "Q",
        resultingBody: "XPQ",
        inputOrigin: "typing",
      }, {
        from: 1,
        to: 3,
        text: "",
        resultingBody: "X",
        inputOrigin: "cut",
      }, {
        from: 1,
        to: 1,
        text: "中文",
        resultingBody: "X中文",
        inputOrigin: "composition_confirmation",
      }, {
        from: 3,
        to: 3,
        text: "!",
        resultingBody: "X中文!",
        inputOrigin: "typing",
      }, {
        from: 3,
        to: 4,
        text: "",
        resultingBody: "X中文",
        inputOrigin: "deletion",
      }, {
        from: 3,
        to: 3,
        text: "word",
        resultingBody: "X中文word",
        inputOrigin: "composition_confirmation",
      }, {
        from: 0,
        to: 7,
        text: "aaaa",
        resultingBody: "aaaa",
        inputOrigin: "selection_replacement",
      }, {
        from: 1,
        to: 2,
        text: "",
        resultingBody: "aaa",
        inputOrigin: "deletion",
      }, {
        from: 0,
        to: 3,
        text: "😀",
        resultingBody: "😀",
        inputOrigin: "selection_replacement",
      }, {
        from: 0,
        to: 2,
        text: "😃",
        resultingBody: "😃",
        inputOrigin: "selection_replacement",
      }]);
    expect(trace.persisted.every((edit) => /^[0-9a-f-]{36}$/.test(edit.undoGroupId ?? "")))
      .toBe(true);
    expect(trace.persisted.every((edit) => !Number.isNaN(Date.parse(edit.createdAt ?? ""))))
      .toBe(true);
    expect(trace.submissions).toEqual([
      { body: "BaseA", pending: 1 },
      { body: "XA", pending: 1 },
      { body: "X", pending: 1 },
      { body: "XP", pending: 1 },
      { body: "XPQ", pending: 1 },
      { body: "X", pending: 1 },
      { body: "X中文", pending: 1 },
      { body: "X中文!", pending: 1 },
      { body: "X中文", pending: 1 },
      { body: "X中文word", pending: 1 },
      { body: "aaaa", pending: 1 },
      { body: "aaa", pending: 1 },
      { body: "😀", pending: 1 },
      { body: "😃", pending: 1 },
    ]);
    const snapshot = await validateJournalSnapshot(
      workspace,
      await readJournalSnapshot(workspace),
    );
    expect(snapshot.records).toHaveLength(14);
    expect(snapshot.payloadChains).toHaveLength(14);
    expect(snapshot.groups.map((group) => group.covered_sequence_range)).toEqual(
      Array.from({ length: 14 }, (_, index) => ({ first: index + 1, last: index + 1 })),
    );
    expect(snapshot.records.map((record) => record.input_origin)).toEqual([
      "typing",
      "selection_replacement",
      "deletion",
      "paste",
      "typing",
      "cut",
      "composition_confirmation",
      "typing",
      "deletion",
      "composition_confirmation",
      "selection_replacement",
      "deletion",
      "selection_replacement",
      "selection_replacement",
    ]);
    const expectedUnits = trace.persisted.map((edit) => ({
      normalized_primitives: [{
        kind: "replace_block_selection" as const,
        manuscript_block_id: BLOCK,
        from: edit.from,
        to: edit.to,
        text: edit.text,
      }],
      selection_snapshot: {
        coordinate_profile: "storyos.editor.utf16-code-unit.v1",
        from: edit.from,
        to: edit.to,
      },
    }));
    expect(snapshot.records.map((record) => record.author_edit_unit)).toEqual(expectedUnits);
    expect(snapshot.groups.flatMap((group) => group.frozen_request_body.author_edit_units))
      .toEqual(expectedUnits);
    expect(snapshot.groups.every((group) => group.settlement.kind === "applied_receipt_settled"))
      .toBe(true);
    expect([...snapshot.covered]).toEqual(Array.from({ length: 14 }, (_, index) => index + 1));
    expect(snapshot.activeBase?.materialized_revision.body).toBe("😃");
    expect(workspace.session.base_snapshot.materialized_revision.body).toBe("😃");
    expect(trace.authorEdits).toHaveLength(14);
    expect(trace.challenges.filter(
      (request) => request.command_schema === "storyos.command.apply-author-edit.request.v1",
    )).toHaveLength(14);
    expect(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "insertText")).toBe(true);
    expect(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "insertFromPaste"))
      .toBe(true);
    expect(trace.native.some((event) =>
      event.type === "input" && event.isTrusted && event.inputType === "deleteByCut"))
      .toBe(true);
    expect(trace.native.filter((event) => event.type === "compositionend")).toHaveLength(3);
    expect(trace.failures).toEqual([]);
  } finally {
    controller?.close();
    syntheticController?.close();
    silentController?.close();
    workspace?.database.close();
    await updateClipboardPermission({ action: "clear" });
    await deleteJournal(scenario.journalName);
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    document.body.replaceChildren();
  }
});

it.each([
  { label: "validated stale refusal", schema: "storyos.problem.v1", fenced: true, earlier: "none" },
  { label: "unrecognized Problem", schema: "unknown.problem.v1", fenced: false, earlier: "none" },
  { label: "an earlier challenge observation", schema: "storyos.problem.v1", fenced: true, earlier: "challenge" },
  { label: "an earlier admission observation", schema: "storyos.problem.v1", fenced: true, earlier: "admission" },
  { label: "an earlier terminal observation", schema: "storyos.problem.v1", fenced: true, earlier: "rejected" },
])("preserves input and outcome evidence after $label", async ({ schema, fenced, earlier }) => {
  const scenario = createBrowserScenario();
  const editor = document.createElement("textarea");
  editor.id = "fenced-editor";
  editor.value = "Base";
  document.body.replaceChildren(editor);
  let releaseOutcome!: () => void;
  let observeOutcome!: () => void;
  let releaseCommand!: () => void;
  let observeCommand!: () => void;
  const heldOutcome = new Promise<void>((resolve) => { releaseOutcome = resolve; });
  const outcomeStarted = new Promise<void>((resolve) => { observeOutcome = resolve; });
  const heldCommand = new Promise<void>((resolve) => { releaseCommand = resolve; });
  const commandStarted = new Promise<void>((resolve) => { observeCommand = resolve; });
  const counts = { commands: 0, outcomes: 0 };
  const expiresAt = "2026-08-13T08:05:00.000Z";
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/anti-forgery-challenges")) {
      return jsonResponse({ nonce: "a".repeat(64), expires_at: expiresAt,
        limit_profile_revision: "storyos.foundation.absolute.v1" });
    }
    if (path.endsWith("/editor-sessions")) return jsonResponse(scenario.session);
    if (path.endsWith(`/editor-sessions/${SESSION}`)) {
      return jsonResponse({ ...scenario.session, schema_id: "storyos.query.editor-session.response.v1" });
    }
    if (path.endsWith("/manuscript/author-edits")) {
      counts.commands += 1;
      observeCommand();
      await heldCommand;
      return jsonResponse({ schema_id: schema, code: "editor_writer_stale",
        message: "The Editor Session is not the current writer." }, 412);
    }
    if (path.includes("/manuscript/author-edit-outcomes/")) {
      counts.outcomes += 1;
      if (earlier !== "none" && counts.outcomes === 1) {
        const observation = earlier === "admission"
          ? { observation_kind: "admission_committed", reconciliation_required: true,
            command_id: "018f0000-0000-7001-8000-000000000092",
            author_command_admission_id: "018f0000-0000-7001-8000-000000000093" }
          : { observation_kind: "challenge_issued", expires_at: expiresAt };
        return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
          correlation_id: "018f0000-0000-7001-8000-000000000091",
          project_scope: scenario.project.project_scope,
          outcome: earlier === "rejected"
            ? { outcome_kind: "rejected", reason: "challenge_expired_unconsumed" }
            : { outcome_kind: "still_unknown", observation } });
      }
      observeOutcome();
      await heldOutcome;
      return jsonResponse({ schema_id: "storyos.query.apply-author-edit-outcome.response.v1",
        correlation_id: "018f0000-0000-7001-8000-000000000091",
        project_scope: scenario.project.project_scope,
        outcome: { outcome_kind: "still_unknown",
          observation: { observation_kind: "challenge_issued", expires_at: expiresAt } } });
    }
    throw new Error(`unexpected request: ${path}`);
  };
  await deleteJournal(scenario.journalName);
  let workspace: EditorReadyState | undefined;
  let recoveryReader: EditorReadyState | undefined;
  let controller: ReturnType<typeof attachManualInput> | undefined;
  let submission: Promise<void> | undefined;
  const failures: unknown[] = [];
  try {
    const state = await openEditorWorkspace({ baseUrl: location.origin,
      project: scenario.project, chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
    requireEditorReady(state);
    workspace = state;
    const oldPartition = structuredClone(state.partition);
    controller = attachManualInput({ editor, workspace, baseUrl: location.origin, fetchImpl,
      setTimeoutImpl: () => 1, clearTimeoutImpl: () => {},
      onFailure: (error) => { failures.push(error); } });
    editor.focus();
    editor.setSelectionRange(4, 4);
    await applyTrustedInput({ operation: "insert_text", text: " retained" });
    await controller.whenIdle();
    const retained = await readJournalSnapshot(workspace);
    submission = controller.flush();
    await commandStarted;
    let earlierGroups: JournalSubmissionGroup[] | undefined;
    if (earlier !== "none") {
      const reader = await openEditorWorkspace({ baseUrl: location.origin,
        project: scenario.project, chapter: scenario.chapter, profile: scenario.profile, fetchImpl });
      requireEditorReady(reader);
      recoveryReader = reader;
      await submitOnePendingAuthorEdit({ workspace: reader, baseUrl: location.origin, fetchImpl });
      earlierGroups = (await readJournalSnapshot(reader)).groups;
    }
    releaseCommand();
    await Promise.race([outcomeStarted, submission]);
    expect(editor.readOnly).toBe(fenced);
    expect(workspace.partition).toEqual(fenced
      ? { ...oldPartition, disposition: "read_only_observer" } : oldPartition);
    expect(await requestResult(workspace.database.transaction("partitions")
      .objectStore("partitions").get(oldPartition.journal_partition_id)))
      .toEqual(workspace.partition);
    const frozen = (await readJournalSnapshot(workspace)).groups;
    expect(frozen).toHaveLength(1);
    if (fenced) await applyTrustedInput({ operation: "insert_text", text: " blocked" });
    expect(editor.value).toBe("Base retained");
    releaseOutcome();
    await submission;
    if (fenced) await controller.flush();
    const after = await readJournalSnapshot(workspace);
    expect({ ...after, groups: [] }).toEqual(retained);
    expect(after.groups).toEqual(earlierGroups ?? [{ ...frozen[0], reconciliation: {
      kind: "outcome_query_unresolved",
      strongest: { kind: "challenge_issued", expires_at: expiresAt },
    } }]);
    expect(counts).toEqual({ commands: 1,
      outcomes: earlier === "none" || earlier === "rejected" ? 1 : 2 });
    expect(failures).toHaveLength(fenced ? 1 : 0);
  } finally {
    releaseCommand();
    releaseOutcome();
    await submission;
    controller?.close();
    workspace?.database.close();
    recoveryReader?.database.close();
    sessionStorage.removeItem(`active_session:${OWNER}:${PROJECT}`);
    await deleteJournal(scenario.journalName);
    editor.remove();
  }
});
