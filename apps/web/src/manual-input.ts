import { submitOnePendingAuthorEdit } from "./author-edit-submission.ts";
import {
  AUTHOR_EDIT_BATCH_IDLE_MS,
  AUTHOR_EDIT_MAX_UNITS,
  createJournalUuid,
  persistReplaceSelection,
} from "./local-edit-journal.ts";
import type {
  EditorReadyState,
  EditorWorkspace,
  InputOrigin,
  PendingEditProjection,
  ReplaceSelectionEdit,
} from "./editor-types.ts";

interface BeforeInputObservation {
  body: string;
  from: number;
  to: number;
  inputType: string;
  forcedOrigin: InputOrigin | undefined;
  createdAt: string;
  redundantCompositionCommit: boolean | undefined;
}

interface CompositionObservation {
  baseBody: string;
  from: number;
  to: number;
}

export interface ManualInputController {
  flush(): Promise<void>;
  whenIdle(): Promise<void>;
  hasIncompleteSemanticIntent(): boolean;
  close(): void;
}

type TimerHandle = number | ReturnType<typeof globalThis.setTimeout>;

function isUtf16Boundary(body: string, offset: number): boolean {
  if (!Number.isSafeInteger(offset) || offset < 0 || offset > body.length) return false;
  if (offset === 0 || offset === body.length) return true;
  const prior = body.charCodeAt(offset - 1);
  const next = body.charCodeAt(offset);
  return !(prior >= 0xd800 && prior <= 0xdbff && next >= 0xdc00 && next <= 0xdfff);
}

function exactSelectionEdit(
  before: string,
  after: string,
  from: number,
  to: number,
): ReplaceSelectionEdit {
  const suffixLength = before.length - to;
  if (!isUtf16Boundary(before, from)
    || !isUtf16Boundary(before, to)
    || to < from
    || suffixLength < 0
    || after.length < from + suffixLength
    || after.slice(0, from) !== before.slice(0, from)
    || after.slice(after.length - suffixLength) !== before.slice(to)) {
    throw new Error("Manual input does not match its trusted selection");
  }
  const text = after.slice(from, after.length - suffixLength);
  if (`${before.slice(0, from)}${text}${before.slice(to)}` !== after) {
    throw new Error("Manual input does not match its trusted selection");
  }
  return { from, to, text, resultingBody: after };
}

function editFromBeforeInput(
  observation: BeforeInputObservation,
  after: string,
  selectionStart: number,
): ReplaceSelectionEdit {
  let { from, to } = observation;
  if (from === to && observation.inputType?.startsWith("delete")) {
    if (observation.inputType.endsWith("Backward")) from = selectionStart;
    else if (observation.inputType.endsWith("Forward")) {
      to += observation.body.length - after.length;
    } else {
      throw new Error("Manual deletion does not have a supported direction");
    }
  }
  return exactSelectionEdit(observation.body, after, from, to);
}

function inputOrigin(
  event: InputEvent | CompositionEvent,
  edit: ReplaceSelectionEdit,
  clipboardOrigin: InputOrigin | undefined,
): InputOrigin {
  const eventInputType = (event as InputEvent).inputType;
  if (clipboardOrigin) return clipboardOrigin;
  if (eventInputType === "insertFromPaste") return "paste";
  if (eventInputType === "deleteByCut") return "cut";
  if (eventInputType?.startsWith("delete")) return "deletion";
  if (eventInputType === "insertReplacementText" || edit.to > edit.from) {
    return "selection_replacement";
  }
  return "typing";
}

const SUPPORTED_INPUT_TYPES = new Set<string>([
  "insertText",
  "insertReplacementText",
  "insertFromPaste",
  "insertLineBreak",
  "insertParagraph",
  "deleteByCut",
  "deleteContentBackward",
  "deleteContentForward",
  "deleteWordBackward",
  "deleteWordForward",
  "deleteSoftLineBackward",
  "deleteSoftLineForward",
  "deleteHardLineBackward",
  "deleteHardLineForward",
]);

export function attachManualInput({
  editor,
  workspace,
  baseUrl,
  fetchImpl = globalThis.fetch,
  cryptoImpl = globalThis.crypto,
  persistIntent = persistReplaceSelection,
  submitGroup = submitOnePendingAuthorEdit,
  afterAppliedSettlement,
  onProjection = () => {},
  onFailure = () => {},
  setTimeoutImpl = (callback, timeout) => globalThis.setTimeout(callback, timeout),
  clearTimeoutImpl = (timer) => globalThis.clearTimeout(timer),
  nowImpl = Date.now,
  isTrustedEvent = (event) => event.isTrusted,
}: {
  editor: HTMLTextAreaElement;
  workspace: EditorReadyState;
  baseUrl: string;
  fetchImpl?: typeof fetch;
  cryptoImpl?: Crypto;
  persistIntent?: (
    workspace: EditorWorkspace,
    edit: ReplaceSelectionEdit,
    cryptoImpl?: Crypto,
  ) => Promise<PendingEditProjection>;
  submitGroup?: (options: {
    workspace: EditorWorkspace;
    baseUrl: string;
    fetchImpl?: typeof fetch;
    cryptoImpl?: Crypto;
    onWriterFenced?: () => void;
  }) => Promise<PendingEditProjection>;
  afterAppliedSettlement?: (workspace: EditorWorkspace) => Promise<unknown> | unknown;
  onProjection?: (projection: PendingEditProjection) => void;
  onFailure?: (error: unknown) => void;
  setTimeoutImpl?: (callback: () => void, timeout: number) => TimerHandle;
  clearTimeoutImpl?: (timer: TimerHandle) => void;
  nowImpl?: () => number;
  isTrustedEvent?: (event: Event) => boolean;
}): ManualInputController {
  if (!editor || typeof editor.addEventListener !== "function") {
    throw new TypeError("Manual input requires one browser editor element");
  }
  let observedBody = editor.value;
  let pendingIntentCount = workspace.pending?.unsettled_intent_count ?? 0;
  let undoGroupId: string | undefined;
  let lastCompletedAt: number | undefined;
  let idleTimer: TimerHandle | undefined;
  let composition: CompositionObservation | undefined;
  let compositionFinishing = false;
  let postCompositionCommit: { body: string; data: string } | undefined;
  let beforeInput: BeforeInputObservation | undefined;
  let clipboardOrigin: InputOrigin | undefined;
  let stopped = false;
  let failed = false;
  let queuedOperations = 0;
  let queue: Promise<void> = Promise.resolve();

  const fail = (error: unknown): void => {
    if (!failed) onFailure(error);
    failed = true;
    editor.readOnly = true;
  };

  const enqueue = (operation: () => Promise<void>): Promise<void> => {
    queuedOperations += 1;
    if (queuedOperations > AUTHOR_EDIT_MAX_UNITS) {
      queuedOperations -= 1;
      fail(new Error("Manual input queue limit failed"));
      return queue;
    }
    queue = queue.then(async () => {
      if (stopped || failed) return;
      await operation();
    }).catch(fail).finally(() => { queuedOperations -= 1; });
    return queue;
  };

  const clearIdle = (): void => {
    if (idleTimer !== undefined) clearTimeoutImpl(idleTimer);
    idleTimer = undefined;
  };

  const submitPending = async (): Promise<void> => {
    clearIdle();
    if (pendingIntentCount === 0 || composition) return;
    const projection = await submitGroup({
      workspace, baseUrl, fetchImpl, cryptoImpl,
      onWriterFenced: () => fail(new Error("Editor Session is read only")),
    });
    workspace.pending = projection;
    pendingIntentCount = projection.unsettled_intent_count;
    onProjection(projection);
    undoGroupId = undefined;
    if (projection.save_state === "saved" && afterAppliedSettlement) {
      await afterAppliedSettlement(workspace);
    }
    if (projection.save_state === "needs_attention") {
      throw new Error("Author Edit requires attention");
    }
  };

  const scheduleIdle = (): void => {
    clearIdle();
    idleTimer = setTimeoutImpl(() => {
      idleTimer = undefined;
      enqueue(submitPending);
    }, AUTHOR_EDIT_BATCH_IDLE_MS);
  };

  const persist = async (
    edit: ReplaceSelectionEdit,
    origin: InputOrigin,
    createdAt: string,
  ): Promise<void> => {
    const hardBoundary = origin === "composition_confirmation"
      || origin === "paste" || origin === "cut";
    const completedAt = Date.parse(createdAt);
    const idleBoundary = lastCompletedAt !== undefined
      && completedAt - lastCompletedAt > AUTHOR_EDIT_BATCH_IDLE_MS;
    if (hardBoundary || idleBoundary) await submitPending();
    if (!undoGroupId) undoGroupId = createJournalUuid(cryptoImpl);
    const projection = await persistIntent(workspace, {
      ...edit, inputOrigin: origin, undoGroupId, createdAt,
    }, cryptoImpl);
    workspace.pending = projection;
    pendingIntentCount = projection.unsettled_intent_count;
    lastCompletedAt = completedAt;
    onProjection(projection);
    if (hardBoundary || pendingIntentCount >= AUTHOR_EDIT_MAX_UNITS) await submitPending();
    else scheduleIdle();
  };

  const capture = (
    edit: ReplaceSelectionEdit,
    originEvent: InputEvent | CompositionEvent,
    forcedOrigin: InputOrigin | undefined,
    createdAt: string,
  ): void => {
    const origin = forcedOrigin ?? inputOrigin(originEvent, edit, clipboardOrigin);
    clipboardOrigin = undefined;
    enqueue(() => persist(edit, origin, createdAt));
  };

  const markClipboardOrigin = (origin: "paste" | "cut"): void => {
    clipboardOrigin = origin;
    queueMicrotask(() => {
      if (clipboardOrigin === origin) clipboardOrigin = undefined;
    });
  };
  const onPaste = (): void => { markClipboardOrigin("paste"); };
  const onCut = (): void => { markClipboardOrigin("cut"); };
  const onBeforeInput = (event: InputEvent): void => {
    if (stopped || failed) return;
    if (!isTrustedEvent(event)) {
      fail(new Error("Manual input lost its trusted beforeinput boundary"));
      return;
    }
    if (composition || compositionFinishing || event.isComposing) return;
    const redundantCompositionCommit = postCompositionCommit
      && event.inputType === "insertText"
      && event.data === postCompositionCommit.data
      && editor.value === postCompositionCommit.body;
    if (postCompositionCommit) postCompositionCommit = undefined;
    if (!SUPPORTED_INPUT_TYPES.has(event.inputType) || editor.value !== observedBody) {
      fail(new Error("Manual input lost its trusted beforeinput boundary"));
      return;
    }
    beforeInput = {
      body: observedBody,
      from: editor.selectionStart,
      to: editor.selectionEnd,
      inputType: event.inputType,
      forcedOrigin: clipboardOrigin,
      createdAt: new Date(nowImpl()).toISOString(),
      redundantCompositionCommit,
    };
    clipboardOrigin = undefined;
  };
  const onCompositionStart = (event: CompositionEvent): void => {
    if (stopped || failed || composition) return;
    if (!isTrustedEvent(event) || editor.value !== observedBody) {
      fail(new Error("Manual composition lost its trusted start boundary"));
      return;
    }
    clearIdle();
    composition = {
      baseBody: observedBody,
      from: editor.selectionStart,
      to: editor.selectionEnd,
    };
    compositionFinishing = false;
    postCompositionCommit = undefined;
    beforeInput = undefined;
  };
  const onCompositionEnd = (event: CompositionEvent): void => {
    if (!composition || stopped || failed) return;
    if (!isTrustedEvent(event)) {
      fail(new Error("Manual composition lost its trusted confirmation boundary"));
      return;
    }
    const completed = composition;
    const confirmation = event.data;
    const createdAt = new Date(nowImpl()).toISOString();
    composition = undefined;
    compositionFinishing = true;
    queueMicrotask(() => {
      if (stopped || failed) return;
      const finalBody = editor.value;
      compositionFinishing = false;
      beforeInput = undefined;
      observedBody = finalBody;
      if (finalBody === completed.baseBody && !confirmation) {
        postCompositionCommit = undefined;
        if (pendingIntentCount > 0) scheduleIdle();
        return;
      }
      try {
        const edit = exactSelectionEdit(
          completed.baseBody, finalBody, completed.from, completed.to,
        );
        postCompositionCommit = { body: finalBody, data: edit.text };
        capture(
          edit,
          event,
          "composition_confirmation",
          createdAt,
        );
      } catch (error) {
        fail(error);
      }
    });
  };
  const onInput = (event: InputEvent): void => {
    if (stopped || failed) return;
    const nextBody = editor.value;
    if (!isTrustedEvent(event)) {
      fail(new Error("Manual input lost its trusted input boundary"));
      return;
    }
    if (composition || compositionFinishing || event.isComposing) {
      beforeInput = undefined;
      return;
    }
    if (nextBody === observedBody && !beforeInput) return;
    const observation = beforeInput;
    beforeInput = undefined;
    if (observation?.redundantCompositionCommit && nextBody === observedBody) return;
    if (!observation || observation.body !== observedBody) {
      fail(new Error("Manual input lost its trusted beforeinput boundary"));
      return;
    }
    try {
      const edit = editFromBeforeInput(observation, nextBody, editor.selectionStart);
      observedBody = nextBody;
      capture(edit, event, observation.forcedOrigin, observation.createdAt);
    } catch (error) {
      fail(error);
    }
  };

  editor.addEventListener("paste", onPaste);
  editor.addEventListener("cut", onCut);
  editor.addEventListener("beforeinput", onBeforeInput);
  editor.addEventListener("compositionstart", onCompositionStart);
  editor.addEventListener("compositionend", onCompositionEnd);
  editor.addEventListener("input", onInput);
  if (pendingIntentCount > 0) scheduleIdle();

  return {
    flush() {
      clearIdle();
      return enqueue(submitPending);
    },
    async whenIdle() {
      await Promise.resolve();
      await queue;
    },
    hasIncompleteSemanticIntent() {
      return composition !== undefined || compositionFinishing;
    },
    close() {
      stopped = true;
      clearIdle();
      editor.removeEventListener("paste", onPaste);
      editor.removeEventListener("cut", onCut);
      editor.removeEventListener("beforeinput", onBeforeInput);
      editor.removeEventListener("compositionstart", onCompositionStart);
      editor.removeEventListener("compositionend", onCompositionEnd);
      editor.removeEventListener("input", onInput);
    },
  };
}
