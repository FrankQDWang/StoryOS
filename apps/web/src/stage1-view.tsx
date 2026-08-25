import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import { collectEligibleJournalPayload } from "./journal-payload-collection.ts";
import type {
  ControlledProjectState,
  EditorReadyState,
  PendingEditProjection,
  ProjectReadyState,
} from "./editor-types.ts";
import { attachManualInput } from "./manual-input.ts";

interface Stage1ViewProps {
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}

interface ProjectReadyViewProps extends Omit<Stage1ViewProps, "state"> {
  state: ProjectReadyState;
}

function ProjectReadyView({
  state, baseUrl, fetchImpl, cryptoImpl,
}: ProjectReadyViewProps) {
  const editorRef = useRef<HTMLTextAreaElement>(null);
  const initialBody = state.editor.kind === "editor-ready"
    ? state.editor.pending.body
    : state.chapter.chapter.current_revision.body;
  const [pending, setPending] = useState<PendingEditProjection | null>(
    state.editor.kind === "editor-ready" ? state.editor.pending : null,
  );
  const [saveState, setSaveState] = useState<PendingEditProjection["save_state"]>(
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
        (state.editor as EditorReadyState).pending = projection;
        setPending(projection);
        setSaveState(projection.save_state);
      },
      onFailure() {
        setReadOnly(true);
        setSaveState("needs_attention");
      },
    });
  }, []);

  return (
    <section>
      <h1>{state.project.project.title}</h1>
      <h2>{state.chapter.chapter.title}</h2>
      <textarea ref={editorRef} defaultValue={initialBody} readOnly={readOnly} />
      <small
        data-save-state={saveState}
        data-unsettled-intent-count={pending?.unsettled_intent_count ?? ""}
        data-authoritative-revision-id={pending?.authoritative_revision_id ?? ""}
      >
        {saveState}
      </small>
      <small>{`权威修订 ${state.chapter.chapter.current_revision.revision_id}`}</small>
    </section>
  );
}

function Stage1View({ state, baseUrl, fetchImpl, cryptoImpl }: Stage1ViewProps) {
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
  if (state.kind === "protected-ready") {
    return (
      <section>
        <h1>StoryOS</h1>
        <p>本地写作已就绪。</p>
      </section>
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

export function mountStage1View(root: HTMLElement, props: Stage1ViewProps): void {
  root.dataset.bootState = props.state.kind;
  createRoot(root).render(<Stage1View {...props} />);
}
