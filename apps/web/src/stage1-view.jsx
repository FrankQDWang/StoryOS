import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import { collectEligibleJournalPayload } from "./journal-payload-collection.mjs";
import { attachManualInput } from "./manual-input.mjs";

function ProjectReadyView({ state, baseUrl, fetchImpl, cryptoImpl }) {
  const editorRef = useRef(null);
  const initialBody = state.editor.kind === "editor-ready"
    ? state.editor.pending.body
    : state.chapter.chapter.current_revision.body;
  const [saveState, setSaveState] = useState(
    state.editor.kind === "editor-ready" ? state.editor.pending.save_state : "needs_attention",
  );
  const [readOnly, setReadOnly] = useState(state.editor.kind !== "editor-ready");

  useEffect(() => {
    const editor = editorRef.current;
    if (state.editor.kind !== "editor-ready" || !editor) return;
    attachManualInput({
      editor,
      workspace: state.editor,
      baseUrl,
      fetchImpl,
      cryptoImpl,
      afterAppliedSettlement: collectEligibleJournalPayload,
      onProjection(projection) {
        state.editor.pending = projection;
        setSaveState(projection.save_state);
      },
      onFailure() {
        setReadOnly(true);
        setSaveState("needs_attention");
      },
    });
    // Attach once on mount, matching the DOM renderer in app.mjs.
  }, []);

  return (
    <section>
      <h1>{state.project.project.title}</h1>
      <h2>{state.chapter.chapter.title}</h2>
      <textarea ref={editorRef} defaultValue={initialBody} readOnly={readOnly} />
      <small data-save-state={saveState}>{saveState}</small>
      <small>{`权威修订 ${state.chapter.chapter.current_revision.revision_id}`}</small>
    </section>
  );
}

function Stage1View({ state, baseUrl, fetchImpl, cryptoImpl }) {
  if (state.kind === "project-ready") {
    return (
      <ProjectReadyView
        state={state}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
      />
    );
  }
  return (
    <section role="alert">
      <h1>{state.heading}</h1>
      <p>{state.message}</p>
      <pre>{JSON.stringify({ code: state.code, details: state.details }, null, 2)}</pre>
    </section>
  );
}

export function mountStage1View(root, props) {
  root.dataset.bootState = props.state.kind;
  createRoot(root).render(<Stage1View {...props} />);
}
