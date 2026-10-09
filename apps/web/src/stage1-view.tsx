import { LocalRecoveryPanel } from "./local-recovery-panel.tsx";
import { useEffect, useMemo, useRef, useState } from "react";
import { flushSync } from "react-dom";
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
  openSelectedChapter,
  selectedChapterSurface,
} from "./chapter-navigation.ts";
import { deleteOwnedChapter } from "./delete-chapter.ts";
import { deleteOwnedVolume } from "./delete-volume.ts";
import type {
  ControlledProjectState,
  EditorReadyState,
  PendingEditProjection,
  ProjectReadyState,
} from "./editor-types.ts";
import { createEditorSessionWritingController } from "./editor-session-writing.ts";
import {
  BlockProposalDisplay, readProposalLocators, rememberProposalLocator,
} from "./block-proposal-display.tsx";
import type { ProposalLocator } from "./block-proposal-display.tsx";
import { ChapterCreationMenu } from "./chapter-creation-menu.tsx";
import { ManuscriptSearchPanel } from "./manuscript-search.tsx";
import { ManuscriptStatisticsPanel } from "./manuscript-statistics.tsx";
import { ManuscriptReadableExportPanel } from "./manuscript-readable-export.tsx";
import { ProjectActivityStatus } from "./project-activity-status.tsx";
import { ManuscriptTree } from "./manuscript-tree.tsx";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { renameOwnedProject } from "./rename-project.ts";
import { setOwnedCurrentChapter } from "./set-current-chapter.ts";
import { TakeOverWriterButton } from "./take-over-writer-button.tsx";
import { navigateProposal, type ProposalFocus, type ProposalNavigation } from "./proposal-navigation.ts";
import { WritingWorkspace } from "./writing-workspace.tsx";

interface Stage1ViewProps {
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}

interface ProjectReadyViewProps extends Omit<Stage1ViewProps, "state"> {
  state: ProjectReadyState;
  proposalNavigation: ProposalNavigation;
  proposalFocus: ProposalFocus | undefined;
  proposalNotice: string | undefined;
  onProposalOpened: (state: ControlledProjectState, focus?: ProposalFocus, notice?: string) => void;
  onReopened: (state: ControlledProjectState) => void;
  onLocalRecoveryContinued: (state: ControlledProjectState) => void;
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
  state, baseUrl, fetchImpl, cryptoImpl, onReopened, onLocalRecoveryContinued, proposalNavigation, proposalFocus, proposalNotice, onProposalOpened,
}: ProjectReadyViewProps) {
  const [candidateTarget, setCandidateTarget] = useState<ProposalFocus>();
  const selectedChapterIdRef = useRef(state.chapter.chapter.chapter_id);
  const switchGenerationRef = useRef(0);
  const makeCurrentInFlightRef = useRef(false);
  const takeOverInFlightRef = useRef(false);
  const currentChapterId = state.project.project.open.kind === "current_chapter"
    ? state.project.project.open.current_chapter_id
    : state.chapter.chapter.chapter_id;
  const [pending, setPending] = useState<PendingEditProjection | null>(
    state.editor.kind === "editor-ready" ? state.editor.openedProjection : null,
  );
  const [saveState, setSaveState] = useState<
    PendingEditProjection["save_state"] | "pending"
  >(
    state.editor.kind === "editor-ready" ? state.editor.openedProjection.save_state : "needs_attention",
  );
  const [editorFailure, setEditorFailure] = useState<string>();
  const [readOnly, setReadOnly] = useState(state.editor.kind !== "editor-ready");
  const [title, setTitle] = useState(state.project.project.title);
  const [revision, setRevision] = useState<string>();
  const [lifecycle, setLifecycle] = useState<"active" | "archived">();
  const [tree, setTree] = useState<GetManuscriptTreeResponse>();
  const [selectedChapter, setSelectedChapter] = useState<GetChapterResponse>(state.chapter);
  const [switchRecovery, setSwitchRecovery] = useState<string>();
  const [proposalLocators, setProposalLocators] = useState<ProposalLocator[]>(
    () => readProposalLocators(state.project.project_scope),
  );
  const [proposalRefresh, setProposalRefresh] = useState(0);
  const onEditorFailureRef = useRef<(error: unknown) => void>(() => {});
  const editorState = state.editor;
  // One writing controller owns the Pending Edit Projection of this Editor Session (ADR 0046).
  const writing = useMemo(() => editorState.kind === "editor-ready"
    ? createEditorSessionWritingController({ workspace: editorState, baseUrl, fetchImpl, cryptoImpl,
      onFailure: (error) => { onEditorFailureRef.current(error); } })
    : undefined, [editorState, baseUrl, fetchImpl, cryptoImpl]);
  useEffect(() => () => { writing?.close(); }, [writing]);
  useEffect(() => writing?.subscribe(() => {
    if (selectedChapterIdRef.current !== currentChapterId) return;
    const { projection, local } = writing.snapshot();
    // Paint unjournaled input before the next poll so saved waiters do not treat it as settled.
    const show = () => {
      setPending(projection);
      setSaveState(projection.save_state);
    };
    if (local) flushSync(show);
    else show();
    if (projection.save_state !== "needs_attention") setEditorFailure(undefined);
  }), [writing, currentChapterId]);

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

  // A read-only editor has no writing controller and no input to wait for.
  const runWhenQuiet = <Result,>(condition: "journaled" | "settled",
    command: (projection: PendingEditProjection | null) => Promise<Result>) => writing === undefined
    ? command(null).then((result) => ({ kind: "ran" as const, result }))
    : writing.runAfterQuiesce(condition, command);

  const selectChapter = (chapterId: string) => {
    if (makeCurrentInFlightRef.current) return;
    if (chapterId === selectedChapterIdRef.current) return;
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    void (async () => {
      const quiet = await runWhenQuiet("journaled", async (projection) => projection);
      if (generation !== switchGenerationRef.current) return;
      if (quiet.kind === "refused") {
        setSwitchRecovery(chapterSwitchRecoveryMessage(quiet.reason));
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
      const surface = selectedChapterSurface({
        selectedChapterId: opened.chapter.chapter.chapter_id,
        currentChapterId,
        currentPending: quiet.result,
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
    if (chapterId === currentChapterId || state.editor.kind !== "editor-ready" || writing === undefined) return;
    if (makeCurrentInFlightRef.current) return;
    makeCurrentInFlightRef.current = true;
    const editorSessionId = state.editor.session.editor_session.editor_session_id;
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    void (async () => {
      try {
        const quiet = await writing.runAfterQuiesce("settled", async () => {
          if (generation !== switchGenerationRef.current) return;
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
          onReopened(await openControlledProject({
            baseUrl,
            projectId: state.project.project.project_id,
            fetchImpl,
            cryptoImpl,
          }));
        });
        if (quiet.kind === "refused" && generation === switchGenerationRef.current) {
          setSwitchRecovery(quiet.reason === "unsettled_input"
            ? "无法设为当前章节。" : chapterSwitchRecoveryMessage(quiet.reason));
        }
      } catch (error: unknown) {
        setSwitchRecovery(
          historicalAcknowledgementUnavailable(error)
            ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
            : "无法设为当前章节。",
        );
      } finally {
        makeCurrentInFlightRef.current = false;
      }
    })();
  };

  const removeFromTree = (noun: "章节" | "卷", remove: (expectedTreeRevision: string) => Promise<{
    effect: { kind: string };
  }>) => {
    if (tree === undefined) return;
    if (makeCurrentInFlightRef.current) return;
    makeCurrentInFlightRef.current = true;
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    void (async () => {
      try {
        const quiet = await runWhenQuiet("settled", async () => {
          if (generation !== switchGenerationRef.current) return;
          const latestTree = await getManuscriptTree({
            baseUrl,
            projectId: state.project.project.project_id,
            fetchImpl,
          });
          if (generation !== switchGenerationRef.current) return;
          const removed = await remove(latestTree.tree_revision);
          if (generation !== switchGenerationRef.current) return;
          if (removed.effect.kind !== "authoritative_applied" && removed.effect.kind !== "no_effect") {
            setSwitchRecovery(`无法删除${noun}。`);
            return;
          }
          const next = await openControlledProject({
            baseUrl,
            projectId: state.project.project.project_id,
            fetchImpl,
            cryptoImpl,
          });
          if (generation !== switchGenerationRef.current) return;
          onReopened(next);
        });
        if (quiet.kind === "refused" && generation === switchGenerationRef.current) {
          setSwitchRecovery(quiet.reason === "incomplete_semantic_intent" ? `无法删除${noun}：请先完成当前输入。`
            : quiet.reason === "journal_unavailable" ? `无法删除${noun}：本地编辑需要恢复。` : `无法删除${noun}。`);
        }
      } catch (error: unknown) {
        setSwitchRecovery(
          historicalAcknowledgementUnavailable(error)
            ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
            : `无法删除${noun}。`,
        );
      } finally {
        makeCurrentInFlightRef.current = false;
      }
    })();
  };

  const removeChapter = (chapterId: string) => removeFromTree("章节", (expectedTreeRevision) => deleteOwnedChapter({
    baseUrl, fetchImpl, cryptoImpl, projectId: state.project.project.project_id, chapterId, expectedTreeRevision,
  }));

  const removeVolume = (volumeId: string) => removeFromTree("卷", (expectedTreeRevision) => deleteOwnedVolume({
    baseUrl, fetchImpl, cryptoImpl, projectId: state.project.project.project_id, volumeId, expectedTreeRevision,
  }));

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
  const onEditorFailure = (error: unknown) => {
    if (historicalAcknowledgementUnavailable(error)) {
      setSwitchRecovery(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
      return;
    }
    setReadOnly(true);
    setSaveState("needs_attention");
    setEditorFailure(error instanceof Error ? error.message : "Manuscript editor failed");
  };
  onEditorFailureRef.current = onEditorFailure;
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
      onNavigateProposal={(destination) => navigateProposal({ state, destination,
        navigation: proposalNavigation, writing, baseUrl, fetchImpl, cryptoImpl,
        onOpened: (next, focus, notice) => {
          setCandidateTarget(focus); setSwitchRecovery(undefined); onProposalOpened(next, focus, notice);
        }, onFailure: setSwitchRecovery,
        onLocator: (locator) => setProposalLocators(rememberProposalLocator(state.project.project_scope, locator)),
      })}
      onOpenedProposal={(locator) => {
        setProposalLocators(rememberProposalLocator(state.project.project_scope, locator));
        setProposalRefresh((current) => current + 1);
      }}
      assistant={{
        scope: state.project.project_scope,
        chapterId: currentChapterId, candidateTarget, tree,
        canSubmit: selectedChapter.chapter.chapter_id === currentChapterId
          && saveState === "saved" && !readOnly && !archived,
        baseUrl, fetchImpl, cryptoImpl,
      }}
      tree={(
        <>
          <RenameProjectForm
                title={title}
                workspace
                disabled={archived}
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
          <ManuscriptSearchPanel
            chapterTitles={new Map(tree?.volumes.flatMap((volume) => volume.chapters.map((chapter) => [chapter.chapter_id, chapter.title])))}
            onSelectChapter={selectChapter}
            projectId={state.project.project.project_id}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
          />
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
              onRemoveChapter={removeChapter}
              onRemoveVolume={removeVolume}
            />
          )}
          <div className="tree-footer">
          <ManuscriptReadableExportPanel
            projectId={state.project.project.project_id}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
          />
          </div>
        </>
      )}
      editor={(
        <>
          {(switchRecovery ?? proposalNotice) === undefined ? null : (
            <p role="alert">{switchRecovery ?? proposalNotice}</p>
          )}
          <h2>{selectedChapter.chapter.title}</h2>
          <BlockProposalDisplay
            key={selectedChapter.chapter.chapter_id}
            scope={state.project.project_scope}
            chapterId={selectedChapter.chapter.chapter_id}
            authoritativeRevisionId={selectedChapter.chapter.chapter_id === currentChapterId
              && pending !== null ? pending.authoritative_revision_id
              : selectedChapter.chapter.current_revision.revision_id}
            focusProposal={proposalFocus}
            onCandidateFocus={setCandidateTarget}
            locators={proposalLocators}
            refreshKey={proposalRefresh}
            safeToProject={(state.editor.kind !== "editor-ready" && pending === null)
              || selectedChapter.chapter.chapter_id !== currentChapterId
              || saveState === "saved"
              || (pending !== null
                && pending.body === selectedChapter.chapter.current_revision.body
                && JSON.stringify(pending.blocks)
                  === JSON.stringify(selectedChapter.chapter.current_revision.blocks))}
            onAccepted={async () => {
              const next = await openControlledProject({
                baseUrl,
                projectId: state.project.project.project_id,
                fetchImpl,
                cryptoImpl,
              });
              onReopened(next);
            }}
            blocks={editorBlocks}
            editable={!readOnly && !archived}
            persistWorkspace={
              state.editor.kind === "editor-ready"
                && selectedChapter.chapter.chapter_id === currentChapterId
                ? state.editor
                : undefined
            }
            writing={selectedChapter.chapter.chapter_id === currentChapterId ? writing : undefined}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            onFailure={onEditorFailure}
          />
          <div className="editor-status">
          <ManuscriptStatisticsPanel
            projectId={state.project.project.project_id}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            currentChapterId={currentChapterId}
            saveState={saveState}
            treeRevision={tree?.tree_revision}
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
          <ProjectActivityStatus
            workspace={
              state.editor.kind === "editor-ready"
                && selectedChapter.chapter.chapter_id === currentChapterId
                ? state.editor
                : undefined
            }
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            revisionKey={pending?.authoritative_revision_id ?? ""}
            onUnavailable={() => {
              setReadOnly(true);
              setSaveState("needs_attention");
              setEditorFailure("活动流无法同步");
            }}
          />
          </div>
          {state.editor.kind === "editor-ready" ? <LocalRecoveryPanel
            workspace={state.editor}
            refreshKey={String(pending?.requires_local_reconfirmation === true)}
            onContinue={async () => {
              const next = await openControlledProject({ baseUrl,
                projectId: state.project.project.project_id, fetchImpl, cryptoImpl });
              onLocalRecoveryContinued(next);
            }}
          /> : null}
          {saveState === "needs_attention" && state.editor.kind === "editor-ready" && pending !== null
            && !pending.requires_local_reconfirmation
            ? (
              <button
                type="button"
                data-reconfirm-legacy-blocks=""
                onClick={() => {
                  void writing?.reconfirmLegacyBlocks();
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
  title, workspace = false, disabled = false,
  projectId,
  revision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onRenamed,
}: {
  title?: string;
  workspace?: boolean;
  disabled?: boolean;
  projectId: string;
  revision: string | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onRenamed: (title: string, revision: string) => void;
}) {
  const [historicalUnavailable, setHistoricalUnavailable] = useState(false);
  const [editing, setEditing] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const pending = useRef(false);
  const cancelled = useRef(false);
  const [saving, setSaving] = useState(false);
  const submit = (value: string) => {
    const title = value.trim();
    if (revision === undefined || !title || pending.current || cancelled.current) return;
    pending.current = true; setSaving(true);
    void renameOwnedProject({ baseUrl, fetchImpl, cryptoImpl, projectId, title, expectedProjectRevision: revision })
      .then((updated) => {
        setHistoricalUnavailable(false);
        if (updated.effect.kind !== "authoritative_applied") return;
        setEditing(false); onRenamed(updated.project.title, updated.effect.revision);
      }).catch((error: unknown) => {
        if (historicalAcknowledgementUnavailable(error)) setHistoricalUnavailable(true);
      }).finally(() => { pending.current = false; setSaving(false); });
  };
  return (
    <div className={workspace ? "project-heading" : undefined} data-project-id={projectId} onContextMenu={disabled ? undefined : (event) => {
      event.preventDefault(); setMenu({ x: event.clientX, y: event.clientY });
    }}>
      {editing ?
    <form
      data-rename={projectId}
      onSubmit={(event) => { event.preventDefault(); submit(String(new FormData(event.currentTarget).get("rename-title") ?? "")); }}
    >
      <input autoFocus name="rename-title" aria-label="项目标题" required maxLength={1024} defaultValue={title}
        readOnly={saving} onBlur={(event) => submit(event.currentTarget.value)} onKeyDown={(event) => {
          if (event.key === "Escape" && !pending.current) { cancelled.current = true; setEditing(false); }
        }} />
    </form> : workspace ? <h1>{title}</h1> : null}
      {disabled ? null : <button type="button" data-project-menu aria-label="项目菜单" onClick={(event) => {
        const rect = event.currentTarget.getBoundingClientRect(); setMenu({ x: rect.left, y: rect.bottom + 4 });
      }}>⋯</button>}
      {menu === undefined ? null : <ChapterCreationMenu point={menu} onClose={() => setMenu(undefined)}>
        <button type="button" data-begin-rename-project disabled={revision === undefined}
          onClick={() => { cancelled.current = false; setEditing(true); }}>重命名</button>
      </ChapterCreationMenu>}
      {historicalUnavailable
        ? <p data-rename-error>{HISTORICAL_ACKNOWLEDGEMENT_MESSAGE}</p>
        : null}
    </div>
  );
}

function EmptyProjectReadyView({
  project,
  tree,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onVolumeCreated,
  onChapterCreated,
}: {
  project: GetProjectResponse;
  tree: GetManuscriptTreeResponse;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onVolumeCreated: () => void;
  onChapterCreated: () => void;
}) {
  const [title, setTitle] = useState(project.project.title);
  const [revision, setRevision] = useState<string>();
  const [volumeRemoval, setVolumeRemoval] = useState<string>();
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
      assistant={{
        scope: project.project_scope,
        canSubmit: false,
        baseUrl, fetchImpl, cryptoImpl,
      }}
      tree={(
        <>
            <RenameProjectForm
              title={title}
              workspace
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
          <ManuscriptSearchPanel projectId={project.project.project_id} baseUrl={baseUrl}
            fetchImpl={fetchImpl} chapterTitles={new Map()} />
          {volumeRemoval === undefined ? null : (
            <p role="alert">{volumeRemoval}</p>
          )}
          <ManuscriptTree
            projectId={project.project.project_id}
            tree={tree}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
            createEnabled
            onChapterCreated={onChapterCreated}
            onVolumeUpdated={onVolumeCreated}
            onRemoveChapter={(chapterId) => {
              void deleteOwnedChapter({
                baseUrl,
                fetchImpl,
                cryptoImpl,
                projectId: project.project.project_id,
                chapterId,
                expectedTreeRevision: tree.tree_revision,
              }).then((removed) => {
                if (
                  removed.effect.kind !== "authoritative_applied"
                  && removed.effect.kind !== "no_effect"
                ) {
                  return;
                }
                onChapterCreated();
              }).catch((error: unknown) => {
                if (historicalAcknowledgementUnavailable(error)) {
                  setVolumeRemoval(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
                }
              });
            }}
            onRemoveVolume={(volumeId) => {
              void deleteOwnedVolume({
                baseUrl,
                fetchImpl,
                cryptoImpl,
                projectId: project.project.project_id,
                volumeId,
                expectedTreeRevision: tree.tree_revision,
              }).then((removed) => {
                if (
                  removed.effect.kind !== "authoritative_applied"
                  && removed.effect.kind !== "no_effect"
                ) {
                  setVolumeRemoval("无法删除卷。");
                  return;
                }
                onVolumeCreated();
              }).catch((error: unknown) => {
                setVolumeRemoval(
                  historicalAcknowledgementUnavailable(error)
                    ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
                    : "无法删除卷。",
                );
              });
            }}
          />
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
                      title={item.title}
                      projectId={item.project_scope.project_id}
                      revision={item.revision}
                      baseUrl={baseUrl}
                      fetchImpl={fetchImpl}
                      cryptoImpl={cryptoImpl}
                      onRenamed={refreshLibrary}
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
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [proposalFocus, setProposalFocus] = useState<ProposalFocus>();
  const [proposalNotice, setProposalNotice] = useState<string>();
  const proposalNavigation = useRef<ProposalNavigation>({ sequence: 0, queue: Promise.resolve() });
  useEffect(() => { setBootState(current.kind); }, [current, setBootState]);
  if (current.kind === "project-ready") {
    // Volume removal keeps the current Chapter, so chapter and writer keys
    // stay put. The new editor base snapshot remounts the tree after honor-deletion.
    return (
      <ProjectReadyView
        key={`${editorGeneration}:${
          current.project.project.open.kind === "current_chapter"
            ? current.project.project.open.current_chapter_id
            : current.project.project.project_id
        }:${
          current.editor.kind === "editor-ready"
            && current.editor.session.writer.kind === "current_writer"
            ? current.editor.session.writer.writer_generation
            : current.editor.kind
        }:${
          current.editor.kind === "editor-ready"
            ? current.editor.session.base_snapshot.snapshot_id
            : current.editor.kind
        }`}
        state={current}
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        proposalNavigation={proposalNavigation.current}
        proposalFocus={proposalFocus}
        proposalNotice={proposalNotice}
        onProposalOpened={(next, focus, notice) => { setProposalFocus(focus); setProposalNotice(notice); setCurrent(next); }}
        onReopened={setCurrent}
        onLocalRecoveryContinued={(next) => { setCurrent(next); setEditorGeneration((value) => value + 1); }}
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
