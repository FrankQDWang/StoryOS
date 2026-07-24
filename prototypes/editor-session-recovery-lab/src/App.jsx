import { useEffect, useMemo, useRef, useState } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { EditorSessionLab } from "./editor-session.js";

function classifyInputType(inputType) {
  if (inputType === "deleteContentBackward") return "delete_backward";
  if (inputType === "deleteContentForward") return "delete_forward";
  if (inputType === "insertFromPaste") return "paste";
  if (inputType === "deleteByCut") return "cut";
  if (inputType === "insertFromDrop") return "drop";
  if (inputType?.startsWith("format") || inputType === "insertParagraph") {
    return "structural";
  }
  return "typing";
}

function plainText(editor) {
  return editor?.getText({ blockSeparator: "\n" }) ?? "";
}

function parameter(name, fallback) {
  return new URLSearchParams(window.location.search).get(name) ?? fallback;
}

export function App() {
  const [sessionState, setSessionState] = useState({
    status: "booting",
    mode: "read_only",
  });
  const [lastError, setLastError] = useState(null);
  const sessionRef = useRef(null);
  const inputRef = useRef(null);
  const composingRef = useRef(false);
  const suppressTransactionsRef = useRef(false);
  const undoGroupRef = useRef(1);
  const settleChainRef = useRef(Promise.resolve());

  const configuration = useMemo(() => {
    const runId = parameter("run", `run-${Date.now()}`);
    const scenarioId = parameter("scenario", "interactive");
    const explicitSessionId = parameter("session", "");
    let editorSessionId = explicitSessionId || sessionStorage.getItem(
      "storyos.issue69.editor-session-id",
    );
    if (!editorSessionId) {
      editorSessionId = crypto.randomUUID();
      sessionStorage.setItem(
        "storyos.issue69.editor-session-id",
        editorSessionId,
      );
    }
    return {
      coreUrl: parameter("core", "http://127.0.0.1:41770"),
      runId,
      scenarioId,
      strategy: parameter("strategy", "bounded-idle"),
      editorSessionId,
      requestWriter: parameter("writer", "1") !== "0",
    };
  }, []);

  const editor = useEditor({
    extensions: [
      StarterKit.configure({
        heading: false,
        blockquote: false,
        codeBlock: false,
        horizontalRule: false,
      }),
    ],
    content: "<p></p>",
    editable: false,
    onTransaction({ editor: transactionEditor, transaction }) {
      if (!transaction.docChanged || suppressTransactionsRef.current) return;
      const afterText = plainText(transactionEditor);
      if (inputRef.current) {
        inputRef.current.afterText = afterText;
        inputRef.current.transactionCount += 1;
        return;
      }
      inputRef.current = {
        kind: "structural",
        beforeText: sessionRef.current?.publicState().projection_text ?? "",
        afterText,
        transactionCount: 1,
        evidenceSource: "tiptap-transaction-fallback",
        details: { fallback: true },
      };
      queueMicrotask(() => void finalizeIntent());
    },
  });

  function beginIntent(kind, evidenceSource, details = {}) {
    if (!editor || suppressTransactionsRef.current) return;
    if (inputRef.current && inputRef.current.kind !== kind) {
      void finalizeIntent();
    }
    if (!inputRef.current) {
      inputRef.current = {
        kind,
        beforeText: plainText(editor),
        afterText: plainText(editor),
        transactionCount: 0,
        evidenceSource,
        details,
      };
    }
  }

  async function finalizeIntent() {
    const pending = inputRef.current;
    if (!pending || !sessionRef.current) return;
    inputRef.current = null;
    pending.afterText = plainText(editor);
    const hardBoundary = [
      "composition",
      "paste",
      "cut",
      "drop",
      "structural",
      "explicit_command",
    ].includes(pending.kind);
    if (hardBoundary) undoGroupRef.current += 1;
    const undoGroup = `undo-${undoGroupRef.current}`;
    settleChainRef.current = settleChainRef.current
      .then(() =>
        sessionRef.current.completeIntent({
          kind: pending.kind,
          beforeText: pending.beforeText,
          afterText: pending.afterText,
          undoGroup,
          transactionCount: Math.max(1, pending.transactionCount),
          historyOnlyTransactions: pending.historyOnlyTransactions ?? 0,
          evidenceSource: pending.evidenceSource,
          details: pending.details,
        }),
      )
      .catch((error) => setLastError(error.stack ?? error.message));
    await settleChainRef.current;
  }

  async function performIntent(kind, operation, options = {}) {
    beginIntent(
      kind,
      options.evidenceSource ?? "tiptap-command-api",
      options.details,
    );
    operation();
    await new Promise((resolve) => queueMicrotask(resolve));
    await finalizeIntent();
    return sessionRef.current.publicState();
  }

  useEffect(() => {
    if (!editor || sessionRef.current) return;
    const session = new EditorSessionLab({
      ...configuration,
      applyEditorText(text) {
        suppressTransactionsRef.current = true;
        editor.commands.setContent(
          text === "" ? "<p></p>" : `<p>${escapeHtml(text)}</p>`,
          { emitUpdate: false },
        );
        suppressTransactionsRef.current = false;
      },
      readEditorText: () => plainText(editor),
      onState(next) {
        setSessionState(next);
        editor.setEditable(next.mode === "writer" && !next.submission_paused);
      },
    });
    sessionRef.current = session;
    session.initialize().catch((error) => {
      setLastError(error.stack ?? error.message);
      window.__STORYOS_LAB_INIT_ERROR__ = error;
    });
  }, [configuration, editor]);

  useEffect(() => {
    if (!editor) return;
    const element = editor.view.dom;

    const onBeforeInput = (event) => {
      if (composingRef.current) return;
      const selection = editor.view.state.selection;
      const classified = classifyInputType(event.inputType);
      const targetRangeReplacesSelection = [...event.getTargetRanges()].some(
        (range) =>
          range.startContainer !== range.endContainer ||
          range.startOffset !== range.endOffset,
      );
      const kind =
        classified === "typing" &&
        (!selection.empty || targetRangeReplacesSelection)
          ? "selection_replace"
          : classified;
      if (
        inputRef.current?.kind === "selection_replace" &&
        classified === "typing"
      ) {
        inputRef.current.details.input_type = event.inputType;
        inputRef.current.details.data = event.data;
        return;
      }
      beginIntent(kind, "native-beforeinput", {
        input_type: event.inputType,
        data: event.data,
      });
    };
    const onInput = () => {
      if (!composingRef.current) void finalizeIntent();
    };
    const onCompositionStart = (event) => {
      composingRef.current = true;
      beginIntent("composition", "os-composition-events", {
        composition_data_start: event.data,
      });
    };
    const onCompositionEnd = (event) => {
      if (inputRef.current) {
        inputRef.current.details.composition_data_end = event.data;
      }
      composingRef.current = false;
      setTimeout(() => void finalizeIntent(), 0);
    };
    const onPaste = (event) => {
      beginIntent("paste", "native-clipboard", {
        clipboard_types: [...event.clipboardData.types],
      });
    };
    const onCut = (event) => {
      beginIntent("cut", "native-clipboard", {
        clipboard_types: [...event.clipboardData.types],
      });
    };
    const onDrop = (event) => {
      beginIntent("drop", "native-drag-drop", {
        data_transfer_types: [...event.dataTransfer.types],
      });
      setTimeout(() => void finalizeIntent(), 0);
    };
    const onKeyDown = (event) => {
      const modifier = event.metaKey || event.ctrlKey;
      if (modifier && event.key.toLowerCase() === "z") {
        beginIntent("explicit_command", "native-keyboard", {
          command: event.shiftKey ? "redo" : "undo",
        });
        setTimeout(() => void finalizeIntent(), 0);
      } else if (
        !modifier &&
        event.key.length === 1 &&
        !editor.view.state.selection.empty
      ) {
        const { from, to } = editor.view.state.selection;
        beginIntent("selection_replace", "native-keyboard-selection", {
          key: event.key,
          selection_from: from,
          selection_to: to,
        });
      }
    };

    element.addEventListener("beforeinput", onBeforeInput, true);
    element.addEventListener("input", onInput, true);
    element.addEventListener("compositionstart", onCompositionStart, true);
    element.addEventListener("compositionend", onCompositionEnd, true);
    element.addEventListener("paste", onPaste, true);
    element.addEventListener("cut", onCut, true);
    element.addEventListener("drop", onDrop, true);
    element.addEventListener("keydown", onKeyDown, true);
    return () => {
      element.removeEventListener("beforeinput", onBeforeInput, true);
      element.removeEventListener("input", onInput, true);
      element.removeEventListener("compositionstart", onCompositionStart, true);
      element.removeEventListener("compositionend", onCompositionEnd, true);
      element.removeEventListener("paste", onPaste, true);
      element.removeEventListener("cut", onCut, true);
      element.removeEventListener("drop", onDrop, true);
      element.removeEventListener("keydown", onKeyDown, true);
    };
  }, [editor]);

  useEffect(() => {
    if (!editor) return;
    const api = {
      async ready() {
        while (
          !sessionRef.current ||
          !["ready", "recovery_only"].includes(
            sessionRef.current.publicState().status,
          )
        ) {
          if (window.__STORYOS_LAB_INIT_ERROR__) {
            throw window.__STORYOS_LAB_INIT_ERROR__;
          }
          await new Promise((resolve) => setTimeout(resolve, 10));
        }
        return sessionRef.current.publicState();
      },
      state: () => sessionRef.current?.publicState(),
      trace: () => sessionRef.current?.trace.export() ?? [],
      report: () => sessionRef.current.report(),
      flush: async () => {
        await finalizeIntent();
        await settleChainRef.current;
        await sessionRef.current.flush();
        return sessionRef.current.publicState();
      },
      insertText: (text, options = {}) =>
        performIntent(
          options.kind ?? "typing",
          () => editor.chain().focus().insertContent(text).run(),
          options,
        ),
      selectionReplace: (from, to, text) =>
        performIntent(
          "selection_replace",
          () =>
            editor
              .chain()
              .focus()
              .setTextSelection({ from, to })
              .insertContent(text)
              .run(),
          { evidenceSource: "tiptap-command-api" },
        ),
      syntheticComposition: (text) =>
        performIntent(
          "composition",
          () => {
            editor.chain().focus().run();
            for (const character of [...text]) {
              editor.commands.insertContent(character);
            }
          },
          {
            evidenceSource: "synthetic-boundary-only",
            details: { real_os_ime_evidence: false },
          },
        ),
      splitBlock: () =>
        performIntent(
          "structural",
          () => editor.chain().focus().splitBlock().run(),
          { evidenceSource: "tiptap-command-api" },
        ),
      undo: () =>
        performIntent(
          "explicit_command",
          () => editor.chain().focus().undo().run(),
          {
            evidenceSource: "tiptap-history-command",
            details: { command: "undo" },
          },
        ),
      redo: () =>
        performIntent(
          "explicit_command",
          () => editor.chain().focus().redo().run(),
          {
            evidenceSource: "tiptap-history-command",
            details: { command: "redo", durable_redo_claimed: false },
          },
        ),
      takeover: () => sessionRef.current.takeover(),
      duplicateLastSubmission: () =>
        sessionRef.current.duplicateLastSubmission(),
      reconcileAll: (options) => sessionRef.current.reconcileAll(options),
      switchChapter: (chapterId, targetId) =>
        sessionRef.current.switchChapter(chapterId, targetId),
      setBinding: (name, value) => {
        sessionRef.current.state[name] = value;
        sessionRef.current.emit();
      },
      editorText: () => plainText(editor),
      journalMetrics: () => sessionRef.current.journal.metrics(),
      focus: () => editor.chain().focus().run(),
      selectAll: () => editor.chain().focus().selectAll().run(),
      setCursor: (position) =>
        editor.chain().focus().setTextSelection(position).run(),
      selection: () => ({
        from: editor.state.selection.from,
        to: editor.state.selection.to,
        empty: editor.state.selection.empty,
      }),
    };
    window.storyosLab = api;
    return () => delete window.storyosLab;
  }, [editor]);

  const statusJson = JSON.stringify(sessionState, null, 2);
  const traceJson = JSON.stringify(
    sessionRef.current?.trace.export() ?? [],
    null,
    2,
  );
  return (
    <main>
      <header>
        <div>
          <p className="eyebrow">Disposable Issue #69 evidence harness</p>
          <h1>Editor session, synchronization, and recovery</h1>
        </div>
        <span
          className={`mode mode-${sessionState.mode}`}
          data-testid="session-mode"
        >
          {sessionState.mode}
        </span>
      </header>

      <section className="editor-panel">
        <div className="label-row">
          <label htmlFor="editor">Real Tiptap input surface</label>
          <span data-testid="projection-status">
            {sessionState.projection_status}
          </span>
        </div>
        <EditorContent id="editor" editor={editor} data-testid="editor" />
      </section>

      <section className="controls" aria-label="Lab controls">
        <button
          type="button"
          onClick={() => sessionRef.current?.flush()}
          disabled={sessionState.status !== "ready"}
        >
          Flush pending intent
        </button>
        <button
          type="button"
          onClick={() => sessionRef.current?.takeover()}
          disabled={sessionState.mode === "writer"}
        >
          Explicit takeover
        </button>
        <span
          draggable="true"
          data-testid="drag-source"
          onDragStart={(event) => {
            event.dataTransfer.setData("text/plain", " dropped");
            event.dataTransfer.effectAllowed = "copy";
          }}
        >
          Drag “ dropped” into the editor
        </span>
      </section>

      <section className="state-panel">
        <h2>Inspectable state</h2>
        <pre data-testid="state">{statusJson}</pre>
      </section>
      <details className="state-panel">
        <summary>Trace export</summary>
        <pre data-testid="trace-export">{traceJson}</pre>
      </details>
      {lastError ? (
        <section className="error" data-testid="error">
          <h2>Harness error</h2>
          <pre>{lastError}</pre>
        </section>
      ) : null}
    </main>
  );
}

function escapeHtml(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;")
    .replaceAll("\n", "</p><p>");
}
