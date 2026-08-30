import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";

import {
  createProject,
  createProjectChallenge,
  getManuscriptTree,
  listProjects,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetChapterResponse,
  GetManuscriptTreeResponse,
  GetProjectResponse,
  ProjectListItem,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { openControlledProject } from "./boot.ts";
import {
  chapterSwitchRecoveryMessage,
  completeJournalOrRefuse,
  openSelectedChapter,
  selectedChapterSurface,
} from "./chapter-navigation.ts";
import type {
  ControlledProjectState,
  EditorReadyState,
  PendingEditProjection,
  ProjectReadyState,
} from "./editor-types.ts";
import { rebuildPendingProjection, reconfirmLegacyReplaceSelection } from "./editor-session.ts";
import { archiveOwnedProject } from "./archive-project.ts";
import type { ManualInputController } from "./manual-input.ts";
import { ManuscriptEditor } from "./manuscript-editor.tsx";
import { CreateVolumeForm, ManuscriptTree } from "./manuscript-tree.tsx";
import { renameOwnedProject } from "./rename-project.ts";
import { setOwnedCurrentChapter } from "./set-current-chapter.ts";
import { TakeOverWriterButton } from "./take-over-writer-button.tsx";
import { WritingWorkspace } from "./writing-workspace.tsx";

interface Stage1ViewProps {
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}

interface ProjectReadyViewProps extends Omit<Stage1ViewProps, "state"> {
  state: ProjectReadyState;
  onArchived: () => void;
  onReopened: (state: ControlledProjectState) => void;
}

const SECURITY_POLICY_REVISION = "storyos.web-security-policy.release-1.v1";

function editorParagraphs(
  blocks: ProjectReadyViewProps["state"]["chapter"]["chapter"]["current_revision"]["blocks"],
): { manuscript_block_id: string; text: string; block_kind: "paragraph" | "heading" }[] {
  return blocks
    .filter((block) => block.block_kind === "paragraph" || block.block_kind === "heading")
    .map((block) => ({
      manuscript_block_id: block.manuscript_block_id,
      block_kind: block.block_kind,
      text: block.text,
    }));
}

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
  state, baseUrl, fetchImpl, cryptoImpl, onArchived, onReopened,
}: ProjectReadyViewProps) {
  const inputRef = useRef<ManualInputController | null>(null);
  const selectedChapterIdRef = useRef(state.chapter.chapter.chapter_id);
  const switchGenerationRef = useRef(0);
  const makeCurrentInFlightRef = useRef(false);
  const takeOverInFlightRef = useRef(false);
  const currentChapterId = state.project.project.open.kind === "current_chapter"
    ? state.project.project.open.current_chapter_id
    : state.chapter.chapter.chapter_id;
  const [pending, setPending] = useState<PendingEditProjection | null>(
    state.editor.kind === "editor-ready" ? state.editor.pending : null,
  );
  const [saveState, setSaveState] = useState<PendingEditProjection["save_state"]>(
    state.editor.kind === "editor-ready" ? state.editor.pending.save_state : "needs_attention",
  );
  const [editorFailure, setEditorFailure] = useState<string>();
  const [readOnly, setReadOnly] = useState(state.editor.kind !== "editor-ready");
  const [title, setTitle] = useState(state.project.project.title);
  const [revision, setRevision] = useState<string>();
  const [lifecycle, setLifecycle] = useState<"active" | "archived">();
  const [tree, setTree] = useState<GetManuscriptTreeResponse>();
  const [selectedChapter, setSelectedChapter] = useState<GetChapterResponse>(state.chapter);
  const [switchRecovery, setSwitchRecovery] = useState<string>();

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

  const selectChapter = (chapterId: string) => {
    if (makeCurrentInFlightRef.current) return;
    if (chapterId === selectedChapterIdRef.current) return;
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    void (async () => {
      const gate = await completeJournalOrRefuse({
        incompleteSemanticIntent: inputRef.current?.hasIncompleteSemanticIntent() ?? false,
        whenIdle: () => inputRef.current?.whenIdle() ?? Promise.resolve(),
      });
      if (generation !== switchGenerationRef.current) return;
      if (gate.kind === "refused") {
        setSwitchRecovery(chapterSwitchRecoveryMessage(gate.reason));
        return;
      }
      const opened = await openSelectedChapter({
        baseUrl,
        projectId: state.project.project.project_id,
        chapterId,
        expectedScope: state.project.project_scope,
        fetchImpl,
      });
      if (generation !== switchGenerationRef.current) return;
      if (opened.kind !== "opened") {
        setSwitchRecovery(chapterSwitchRecoveryMessage(opened.kind));
        return;
      }
      setSwitchRecovery(undefined);
      let currentPending = state.editor.kind === "editor-ready" ? state.editor.pending : null;
      if (opened.chapter.chapter.chapter_id === currentChapterId
        && state.editor.kind === "editor-ready") {
        currentPending = await rebuildPendingProjection(state.editor);
        state.editor.pending = currentPending;
      }
      if (generation !== switchGenerationRef.current) return;
      const surface = selectedChapterSurface({
        selectedChapterId: opened.chapter.chapter.chapter_id,
        currentChapterId,
        currentPending,
        opened: opened.chapter,
      });
      selectedChapterIdRef.current = opened.chapter.chapter.chapter_id;
      setSelectedChapter(opened.chapter);
      setPending(surface.pending);
      setSaveState(surface.save_state);
      setReadOnly(!surface.editable);
    })();
  };

  const makeCurrent = (chapterId: string) => {
    if (chapterId === currentChapterId || state.editor.kind !== "editor-ready") return;
    if (makeCurrentInFlightRef.current) return;
    makeCurrentInFlightRef.current = true;
    const editor = state.editor;
    const editorSessionId = editor.session.editor_session.editor_session_id;
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    void (async () => {
      try {
        await inputRef.current?.flush();
        const gate = await completeJournalOrRefuse({
          incompleteSemanticIntent: inputRef.current?.hasIncompleteSemanticIntent() ?? false,
          whenIdle: () => inputRef.current?.whenIdle() ?? Promise.resolve(),
        });
        if (generation !== switchGenerationRef.current) return;
        if (gate.kind === "refused") {
          setSwitchRecovery(chapterSwitchRecoveryMessage(gate.reason));
          return;
        }
        const drained = await rebuildPendingProjection(editor);
        editor.pending = drained;
        setPending(drained);
        setSaveState(drained.save_state);
        if (drained.unsettled_intent_count > 0) {
          setSwitchRecovery("无法设为当前章节。");
          return;
        }
        const opened = await openSelectedChapter({
          baseUrl,
          projectId: state.project.project.project_id,
          chapterId,
          expectedScope: state.project.project_scope,
          fetchImpl,
        });
        if (generation !== switchGenerationRef.current) return;
        if (opened.kind !== "opened") {
          setSwitchRecovery(chapterSwitchRecoveryMessage(opened.kind));
          return;
        }
        const switched = await setOwnedCurrentChapter({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId: state.project.project.project_id,
          chapterId,
          expectedCurrentChapterId: currentChapterId,
          expectedTargetRevisionId: opened.chapter.chapter.current_revision.revision_id,
          editorSessionId,
        });
        if (switched.effect.kind !== "authoritative_applied"
          && switched.effect.kind !== "no_effect") {
          setSwitchRecovery("无法设为当前章节。");
          return;
        }
        const next = await openControlledProject({
          baseUrl,
          projectId: state.project.project.project_id,
          fetchImpl,
          cryptoImpl,
        });
        onReopened(next);
      } catch {
        setSwitchRecovery("无法设为当前章节。");
      } finally {
        makeCurrentInFlightRef.current = false;
      }
    })();
  };

  const archived = lifecycle === "archived";
  const writer = state.editor.kind === "editor-ready"
    ? state.editor.session.writer
    : state.editor.writer;
  const editorBlocks = selectedChapter.chapter.chapter_id === currentChapterId && pending !== null
    ? pending.blocks.map((block) => ({
      manuscript_block_id: block.manuscript_block_id,
      block_kind: block.block_kind === "heading" ? "heading" as const : "paragraph" as const,
      text: block.text,
    }))
    : editorParagraphs(selectedChapter.chapter.current_revision.blocks);
  const refreshTree = () => {
    void getManuscriptTree({
      baseUrl,
      projectId: state.project.project.project_id,
      fetchImpl,
    }).then(setTree).catch(() => {});
  };
  return (
    <WritingWorkspace
      writer={writer}
      tree={(
        <>
          <h1>{title}</h1>
          {tree === undefined ? null : (
            <ManuscriptTree
              projectId={state.project.project.project_id}
              tree={tree}
              baseUrl={baseUrl}
              fetchImpl={fetchImpl}
              cryptoImpl={cryptoImpl}
              createEnabled={!archived}
              selectedChapterId={selectedChapter.chapter.chapter_id}
              onSelectChapter={selectChapter}
              currentChapterId={currentChapterId}
              makeCurrentEnabled={!archived && state.editor.kind === "editor-ready"}
              onMakeCurrent={makeCurrent}
              onChapterCreated={refreshTree}
              onVolumeUpdated={refreshTree}
            />
          )}
          {archived ? null : (
            <div className="tree-footer">
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
            </div>
          )}
        </>
      )}
      editor={(
        <>
          {switchRecovery === undefined ? null : <p role="alert">{switchRecovery}</p>}
          <h2>{selectedChapter.chapter.title}</h2>
          <ManuscriptEditor
            key={selectedChapter.chapter.chapter_id}
            blocks={editorBlocks}
            editable={!readOnly && !archived}
            persistWorkspace={
              state.editor.kind === "editor-ready"
                && selectedChapter.chapter.chapter_id === currentChapterId
                ? state.editor
                : undefined
            }
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            controllerRef={inputRef}
            onProjection={(projection) => {
              (state.editor as EditorReadyState).pending = projection;
              if (selectedChapterIdRef.current !== currentChapterId) return;
              setPending(projection);
              setSaveState(projection.save_state);
              if (projection.save_state !== "needs_attention") setEditorFailure(undefined);
            }}
            onFailure={(error) => {
              setReadOnly(true);
              setSaveState("needs_attention");
              setEditorFailure(
                error instanceof Error ? error.message : "Manuscript editor failed",
              );
            }}
          />
          <small
            data-save-state={saveState}
            data-editor-failure={editorFailure ?? ""}
            data-unsettled-intent-count={pending?.unsettled_intent_count ?? ""}
            data-authoritative-revision-id={
              pending?.authoritative_revision_id
                ?? selectedChapter.chapter.current_revision.revision_id
            }
            data-author-undo-frontier={pending?.author_undo_frontier_sequence ?? ""}
          >
            {saveState === "saved" ? "已保存"
              : saveState === "saving" ? "保存中"
              : saveState === "needs_attention" ? "需要处理"
              : "未保存"}
          </small>
          {saveState === "needs_attention" && state.editor.kind === "editor-ready" && pending !== null
            ? (
              <button
                type="button"
                data-reconfirm-legacy-blocks=""
                onClick={() => {
                  void reconfirmLegacyReplaceSelection(state.editor as EditorReadyState)
                    .then((projection) => {
                      (state.editor as EditorReadyState).pending = projection;
                      setPending(projection);
                      setSaveState(projection.save_state);
                    });
                }}
              >
                确认待写入正文
              </button>
            )
            : null}
          {writer?.kind === "read_only"
            ? (
              <TakeOverWriterButton
                state={state}
                writer={writer}
                baseUrl={baseUrl}
                fetchImpl={fetchImpl}
                cryptoImpl={cryptoImpl}
                inFlightRef={takeOverInFlightRef}
                onReopened={onReopened}
                onRefused={() => {
                  setSwitchRecovery("无法接管写作。");
                }}
              />
            )
            : null}
        </>
      )}
    />
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
    <WritingWorkspace
      tree={(
        <>
          <h1>{title}</h1>
          <ManuscriptTree
            projectId={project.project.project_id}
            tree={tree}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            createEnabled
            onChapterCreated={onChapterCreated}
            onVolumeUpdated={onVolumeCreated}
          />
          <CreateVolumeForm
            projectId={project.project.project_id}
            treeRevision={tree.tree_revision}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            onCreated={onVolumeCreated}
          />
          <div className="tree-footer">
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
          </div>
        </>
      )}
      editor={<p>空工作区</p>}
    />
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
        key={current.project.project.open.kind === "current_chapter"
          ? current.project.project.open.current_chapter_id
          : current.project.project.project_id}
        state={current}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        onArchived={() => {
          setCurrent({ kind: "protected-ready", profile: current.profile });
        }}
        onReopened={setCurrent}
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
