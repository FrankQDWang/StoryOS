import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import {
  createProject,
  createProjectChallenge,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { openControlledProject } from "./boot.ts";
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

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

function uuidV7(cryptoImpl: Crypto, now = Date.now()): string {
  const bytes = cryptoImpl.getRandomValues(new Uint8Array(16));
  for (let offset = 5; offset >= 0; offset -= 1) {
    bytes[offset] = now & 0xff;
    now = Math.floor(now / 256);
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
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

async function createEmptyProject(
  title: string,
  props: Omit<Stage1ViewProps, "state">,
): Promise<ControlledProjectState> {
  try {
    const idempotencyKey = uuidV7(props.cryptoImpl);
    const createProjectInput = {
      title,
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY_REVISION,
      correlation_id: uuidV7(props.cryptoImpl),
    };
    const challenge = await createProjectChallenge({
      baseUrl: props.baseUrl,
      fetchImpl: props.fetchImpl,
      request: {
        command_schema: "storyos.command.create-project.request.v1",
        create_project_input: createProjectInput,
        idempotency_key: idempotencyKey,
      },
    });
    const created = await createProject({
      baseUrl: props.baseUrl,
      fetchImpl: props.fetchImpl,
      idempotencyKey,
      antiForgery: challenge.nonce,
      request: {
        command_schema: "storyos.command.create-project.request.v1",
        prospective_project_id: challenge.prospective_project_id,
        create_project_input: createProjectInput,
      },
    });
    return await openControlledProject({
      baseUrl: props.baseUrl,
      projectId: created.project_scope.project_id,
      fetchImpl: props.fetchImpl,
      cryptoImpl: props.cryptoImpl,
    });
  } catch {
    return {
      kind: "project-blocked",
      code: "project_unavailable",
      heading: "StoryOS 无法打开项目",
      message: "无法读取这个受控项目或其当前章节。",
    };
  }
}

function Stage1View({
  state, baseUrl, fetchImpl, cryptoImpl, setBootState,
}: Stage1ViewProps & { setBootState: (kind: string) => void }) {
  const [current, setCurrent] = useState(state);
  useEffect(() => { setBootState(current.kind); }, [current, setBootState]);
  if (current.kind === "project-ready") {
    return (
      <ProjectReadyView
        state={current}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
      />
    );
  }
  if (current.kind === "empty-project-ready") {
    return (
      <section>
        <h1>{current.project.project.title}</h1>
        <p>空工作区</p>
      </section>
    );
  }
  if (current.kind === "protected-ready") {
    return (
      <section>
        <h1>StoryOS</h1>
        <p>本地写作已就绪。</p>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            const title = String(new FormData(event.currentTarget).get("title") ?? "").trim();
            if (!title) return;
            void createEmptyProject(title, { baseUrl, fetchImpl, cryptoImpl }).then(setCurrent);
          }}
        >
          <label>
            项目标题
            <input name="title" required maxLength={1024} />
          </label>
          <button type="submit">创建项目</button>
        </form>
      </section>
    );
  }
  return (
    <section role="alert">
      <h1>{current.heading}</h1>
      <p>{current.message}</p>
      <pre>{JSON.stringify({ code: current.code, details: current.details }, null, 2)}</pre>
    </section>
  );
}

export function mountStage1View(root: HTMLElement, props: Stage1ViewProps): void {
  root.dataset.bootState = props.state.kind;
  createRoot(root).render(
    <Stage1View {...props} setBootState={(kind) => { root.dataset.bootState = kind; }} />,
  );
}
