import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import {
  createProject,
  createProjectChallenge,
  getManuscriptTree,
  listProjects,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { GetManuscriptTreeResponse, GetProjectResponse, ProjectListItem } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { openControlledProject } from "./boot.ts";
import { collectEligibleJournalPayload } from "./journal-payload-collection.ts";
import type {
  ControlledProjectState,
  EditorReadyState,
  PendingEditProjection,
  ProjectReadyState,
} from "./editor-types.ts";
import { archiveOwnedProject } from "./archive-project.ts";
import { createOwnedChapter } from "./create-chapter.ts";
import { createOwnedVolume } from "./create-volume.ts";
import { attachManualInput } from "./manual-input.ts";
import { renameOwnedProject } from "./rename-project.ts";

interface Stage1ViewProps {
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}

interface ProjectReadyViewProps extends Omit<Stage1ViewProps, "state"> {
  state: ProjectReadyState;
  onArchived: () => void;
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
  state, baseUrl, fetchImpl, cryptoImpl, onArchived,
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
  const [title, setTitle] = useState(state.project.project.title);
  const [revision, setRevision] = useState<string>();
  const [lifecycle, setLifecycle] = useState<"active" | "archived">();
  const [tree, setTree] = useState<GetManuscriptTreeResponse>();

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
  useEffect(() => {
    void listProjects({ baseUrl, fetchImpl }).then((response) => {
      const item = response.projects.find((entry) =>
        entry.project_scope.project_id === state.project.project.project_id);
      if (item === undefined) return;
      setRevision(item.revision);
      setLifecycle(item.lifecycle.kind);
    }).catch(() => {});
  }, [baseUrl, fetchImpl, state.project.project.project_id]);
  useEffect(() => {
    void getManuscriptTree({
      baseUrl,
      projectId: state.project.project.project_id,
      fetchImpl,
    }).then(setTree).catch(() => {});
  }, [baseUrl, fetchImpl, state.project.project.project_id]);

  const archived = lifecycle === "archived";
  return (
    <section>
      <h1>{title}</h1>
      {archived ? null : (
        <>
          <RenameProjectForm
            projectId={state.project.project.project_id}
            revision={revision}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            onRenamed={(nextTitle, nextRevision) => {
              setTitle(nextTitle);
              setRevision(nextRevision);
            }}
          />
          <ArchiveProjectForm
            projectId={state.project.project.project_id}
            revision={revision}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            onArchived={onArchived}
          />
        </>
      )}
      {tree === undefined ? null : (
        <ManuscriptTree
          projectId={state.project.project.project_id}
          tree={tree}
          baseUrl={baseUrl}
          fetchImpl={fetchImpl}
          cryptoImpl={cryptoImpl}
          onChapterCreated={() => {
            void getManuscriptTree({
              baseUrl,
              projectId: state.project.project.project_id,
              fetchImpl,
            }).then(setTree).catch(() => {});
          }}
        />
      )}
      <h2>{state.chapter.chapter.title}</h2>
      <textarea ref={editorRef} defaultValue={initialBody} readOnly={readOnly || archived} />
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

function RenameProjectForm({
  projectId,
  revision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onRenamed,
}: {
  projectId: string;
  revision: string | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onRenamed: (title: string, revision: string) => void;
}) {
  return (
    <form
      data-rename={projectId}
      onSubmit={(event) => {
        event.preventDefault();
        if (revision === undefined) return;
        const title = String(new FormData(event.currentTarget).get("rename-title") ?? "").trim();
        if (!title) return;
        void renameOwnedProject({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId,
          title,
          expectedProjectRevision: revision,
        }).then((updated) => {
          if (updated.effect.kind !== "authoritative_applied") return;
          onRenamed(updated.project.title, updated.effect.revision);
        }).catch(() => {});
      }}
    >
      <label>
        项目标题
        <input name="rename-title" required maxLength={1024} disabled={revision === undefined} />
      </label>
      <button type="submit" disabled={revision === undefined}>重命名</button>
    </form>
  );
}

function ArchiveProjectForm({
  projectId,
  revision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onArchived,
}: {
  projectId: string;
  revision: string | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onArchived: () => void;
}) {
  return (
    <form
      data-archive={projectId}
      onSubmit={(event) => {
        event.preventDefault();
        if (revision === undefined) return;
        void archiveOwnedProject({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId,
          expectedProjectRevision: revision,
        }).then((archived) => {
          if (archived.effect.kind === "authoritative_applied"
            || (archived.effect.kind === "no_effect"
              && archived.effect.reason === "already_archived")) {
            onArchived();
          }
        }).catch(() => {});
      }}
    >
      <button type="submit" disabled={revision === undefined}>归档</button>
    </form>
  );
}

function CreateChapterForm({
  projectId,
  volumeId,
  treeRevision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onCreated,
}: {
  projectId: string;
  volumeId: string;
  treeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onCreated: () => void;
}) {
  return (
    <form
      data-create-chapter={volumeId}
      onSubmit={(event) => {
        event.preventDefault();
        const title = String(new FormData(event.currentTarget).get("chapter-title") ?? "").trim();
        if (!title) return;
        void createOwnedChapter({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId,
          volumeId,
          title,
          expectedTreeRevision: treeRevision,
        }).then((created) => {
          if (created.effect.kind !== "authoritative_applied") return;
          onCreated();
        }).catch(() => {});
      }}
    >
      <label>
        章标题
        <input name="chapter-title" required maxLength={1024} />
      </label>
      <button type="submit">创建章</button>
    </form>
  );
}

function ManuscriptTree({
  projectId,
  tree,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onChapterCreated,
}: {
  projectId: string;
  tree: GetManuscriptTreeResponse;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onChapterCreated: () => void;
}) {
  return (
    <nav aria-label="稿件目录">
      <ul>
        {tree.volumes.map((volume) => (
          <li key={volume.volume_id}>
            {volume.title}
            <ul>
              {volume.chapters.map((chapter) => (
                <li key={chapter.chapter_id}>{chapter.title}</li>
              ))}
            </ul>
            <CreateChapterForm
              projectId={projectId}
              volumeId={volume.volume_id}
              treeRevision={tree.tree_revision}
              baseUrl={baseUrl}
              fetchImpl={fetchImpl}
              cryptoImpl={cryptoImpl}
              onCreated={onChapterCreated}
            />
          </li>
        ))}
      </ul>
    </nav>
  );
}

function CreateVolumeForm({
  projectId,
  treeRevision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onCreated,
}: {
  projectId: string;
  treeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onCreated: () => void;
}) {
  return (
    <form
      data-create-volume={projectId}
      onSubmit={(event) => {
        event.preventDefault();
        const title = String(new FormData(event.currentTarget).get("volume-title") ?? "").trim();
        if (!title) return;
        void createOwnedVolume({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId,
          title,
          expectedTreeRevision: treeRevision,
        }).then((created) => {
          if (created.effect.kind !== "authoritative_applied") return;
          onCreated();
        }).catch(() => {});
      }}
    >
      <label>
        卷标题
        <input name="volume-title" required maxLength={1024} />
      </label>
      <button type="submit">创建卷</button>
    </form>
  );
}

function EmptyProjectReadyView({
  project,
  tree,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onArchived,
  onVolumeCreated,
  onChapterCreated,
}: {
  project: GetProjectResponse;
  tree: GetManuscriptTreeResponse;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onArchived: () => void;
  onVolumeCreated: () => void;
  onChapterCreated: () => void;
}) {
  const [title, setTitle] = useState(project.project.title);
  const [revision, setRevision] = useState<string>();
  useEffect(() => {
    void listProjects({ baseUrl, fetchImpl }).then((response) => {
      const item = response.projects.find((entry) =>
        entry.project_scope.project_id === project.project.project_id);
      if (item === undefined) return;
      setRevision(item.revision);
    }).catch(() => {});
  }, [baseUrl, fetchImpl, project.project.project_id]);
  return (
    <section>
      <h1>{title}</h1>
      <p>空工作区</p>
      <ManuscriptTree
        projectId={project.project.project_id}
        tree={tree}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        onChapterCreated={onChapterCreated}
      />
      <CreateVolumeForm
        projectId={project.project.project_id}
        treeRevision={tree.tree_revision}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        onCreated={onVolumeCreated}
      />
      <RenameProjectForm
        projectId={project.project.project_id}
        revision={revision}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        onRenamed={(nextTitle, nextRevision) => {
          setTitle(nextTitle);
          setRevision(nextRevision);
        }}
      />
      <ArchiveProjectForm
        projectId={project.project.project_id}
        revision={revision}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        onArchived={onArchived}
      />
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

function ProtectedReadyView({
  baseUrl, fetchImpl, cryptoImpl, setCurrent,
}: Omit<Stage1ViewProps, "state"> & { setCurrent: (state: ControlledProjectState) => void }) {
  const [library, setLibrary] = useState<ProjectListItem[] | null>(null);
  useEffect(() => {
    void listProjects({ baseUrl, fetchImpl })
      .then((response) => {
        setLibrary(response.projects);
      })
      .catch(() => {
        setLibrary([]);
      });
  }, [baseUrl, fetchImpl]);
  const refreshLibrary = () => {
    void listProjects({ baseUrl, fetchImpl })
      .then((response) => {
        setLibrary(response.projects);
      })
      .catch(() => {});
  };
  return (
    <section>
      <h1>StoryOS</h1>
      <p>本地写作已就绪。</p>
      {library !== null && library.length > 0 ? (
        <ul>
          {library.map((item) => {
            const archived = item.lifecycle.kind === "archived";
            return (
              <li key={item.project_scope.project_id}>
                <button
                  type="button"
                  data-project-id={item.project_scope.project_id}
                  data-open={item.open.kind}
                  data-lifecycle={item.lifecycle.kind}
                  data-revision={item.revision}
                  disabled={archived}
                  onClick={() => {
                    if (archived) return;
                    void openControlledProject({
                      baseUrl,
                      projectId: item.project_scope.project_id,
                      fetchImpl,
                      cryptoImpl,
                    }).then(setCurrent);
                  }}
                >
                  {item.title}
                </button>
                {archived ? null : (
                  <>
                    <RenameProjectForm
                      projectId={item.project_scope.project_id}
                      revision={item.revision}
                      baseUrl={baseUrl}
                      fetchImpl={fetchImpl}
                      cryptoImpl={cryptoImpl}
                      onRenamed={refreshLibrary}
                    />
                    <ArchiveProjectForm
                      projectId={item.project_scope.project_id}
                      revision={item.revision}
                      baseUrl={baseUrl}
                      fetchImpl={fetchImpl}
                      cryptoImpl={cryptoImpl}
                      onArchived={refreshLibrary}
                    />
                  </>
                )}
              </li>
            );
          })}
        </ul>
      ) : null}
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
        onArchived={() => {
          setCurrent({ kind: "protected-ready", profile: current.profile });
        }}
      />
    );
  }
  if (current.kind === "empty-project-ready") {
    return (
      <EmptyProjectReadyView
        project={current.project}
        tree={current.tree}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        onArchived={() => {
          setCurrent({ kind: "protected-ready", profile: current.profile });
        }}
        onVolumeCreated={() => {
          void openControlledProject({
            baseUrl,
            projectId: current.project.project.project_id,
            fetchImpl,
            cryptoImpl,
          }).then(setCurrent);
        }}
        onChapterCreated={() => {
          void openControlledProject({
            baseUrl,
            projectId: current.project.project.project_id,
            fetchImpl,
            cryptoImpl,
          }).then(setCurrent);
        }}
      />
    );
  }
  if (current.kind === "protected-ready") {
    return (
      <ProtectedReadyView
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        setCurrent={setCurrent}
      />
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
