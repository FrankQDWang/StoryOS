import { LocalRecoveryPanel } from "./local-recovery-panel.tsx";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { flushSync } from "react-dom";
import { createRoot, type Root } from "react-dom/client";

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
  StoryOSQueryOptions,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { openControlledProject, readCurrentChapter, reopenControlledProject } from "./boot.ts";
import {
  CHAPTER_SWITCH_WAITING_SENTENCE,
  chapterSwitchRecoveryMessage,
  openSelectedChapter,
  selectedChapterSurface,
} from "./chapter-navigation.ts";
import { ArrivalNoticeSlot, type ArrivalNotice, type UnsavedArrival } from "./arrival-notice.tsx";
import { createArrivalInput, type ArrivalInput, type EditorSelection } from "./arrival-input.ts";
import { chapterText, createChapterCopies, editorParagraphs, type ChapterCopies } from "./chapter-copies.ts";
import { deleteOwnedChapter } from "./delete-chapter.ts";
import { deleteOwnedVolume } from "./delete-volume.ts";
import type {
  ControlledProjectState,
  EditorReadyState,
  PendingEditProjection,
  ProjectReadyState,
} from "./editor-types.ts";
import { createChallengeAdmissionWait, type ChallengeAdmissionTimers, type ChallengeAdmissionWait }
  from "./challenge-admission-wait.ts";
import { AuthorUndoUnavailableError, InputBoundStopError, createEditorSessionWritingController }
  from "./editor-session-writing.ts";
import {
  BlockProposalDisplay,
} from "./block-proposal-display.tsx";
import type { ProposalLocator } from "./block-proposal-display.tsx";
import { ChapterCreationMenu } from "./chapter-creation-menu.tsx";
import { ManuscriptSearchPanel } from "./manuscript-search.tsx";
import { ManuscriptStatisticsPanel } from "./manuscript-statistics.tsx";
import { ManuscriptReadableExportPanel } from "./manuscript-readable-export.tsx";
import { ProjectActivityStatus } from "./project-activity-status.tsx";
import { ManuscriptTree } from "./manuscript-tree.tsx";
import { readManuscriptTree } from "./manuscript-tree-read.ts";
import { relabelDraftReferences } from "./selection-reference-draft.ts";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { renameOwnedProject } from "./rename-project.ts";
import { SkippedCurrentChapterError, UnknownCurrentChapterResultError, setOwnedCurrentChapter }
  from "./set-current-chapter.ts";
import { navigateProposal, type ProposalDestination, type ProposalFocus, type ProposalNavigation }
  from "./proposal-navigation.ts";
import { ProjectSettingsLink, ProjectSettingsView } from "./project-settings.tsx";
import { WritingWorkspace } from "./writing-workspace.tsx";
import { ADDRESS_CHAPTER_NOT_FOUND_SENTENCE, useAddressChapter } from "./address-chapter.ts";
import { SECURITY_POLICY_REVISION, uuidV7 } from "./client-identity.ts";

interface Stage1ViewProps {
  state: ControlledProjectState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  /** The timers of the Editor Session writing controller. The default is the global timers. */
  writingTimers?: ChallengeAdmissionTimers | undefined;
}


interface ProjectReadyViewProps extends Omit<Stage1ViewProps, "state"> {
  state: ProjectReadyState;
  proposalNavigation: ProposalNavigation;
  proposalFocus: ProposalFocus | undefined;
  proposalNotice: string | undefined;
  proposalLocatorStore: Map<string, ProposalLocator[]>;
  onProposalOpened: (state: ControlledProjectState, focus?: ProposalFocus, notice?: string) => void;
  onReopened: (state: ControlledProjectState) => void;
  onLocalRecoveryContinued: (state: ControlledProjectState) => void;
  /** Holds a settled rename. It is false when the workspace holds a later revision, and the view keeps its values. */
  onRenamed: (projectId: string, title: string, revision: string) => boolean;
  unsavedNotice: ArrivalNotice;
}

/** The commands of the editor part that the contents list and the assistant panel start. */
interface ProjectEditorHandle {
  /** `contents` is true for a selection in the contents: the selected Chapter is on the screen at once (ADR 0105). */
  selectChapter: (chapterId: string, contents?: boolean) => Promise<boolean>;
  removeChapter: (chapterId: string) => void;
  removeVolume: (volumeId: string) => void;
  navigateProposal: (destination: ProposalDestination) => void;
  openedProposal: (locator: ProposalLocator) => void;
  showRecovery: (sentence: string) => void;
}

/** The state of the editor part that the contents list and the assistant panel show. */
interface ProjectEditorStatus {
  selectedChapterId: string;
  canSubmit: boolean;
}

interface ProjectEditorViewProps extends ProjectReadyViewProps {
  tree: GetManuscriptTreeResponse | undefined;
  /** The state of the open whose read gave `tree`, and `first` when that read is the first tree read of the open. */
  treeOpen: { open: unknown; first: boolean } | undefined;
  /** The number of the last tree read of the workspace. */
  treeReads: { readonly current: number };
  handle: { current: ProjectEditorHandle | undefined };
  onStatus: (status: ProjectEditorStatus) => void;
  copies: ChapterCopies;
  /** The Arrival Input that a switch gives to the next editor part for its hand-over (ADR 0105, section 4). */
  handOverStore: { current: ArrivalInput | undefined };
  /** The caret of a focused Arriving Chapter with no Arrival Input, which the editor of the next editor part takes. */
  caretStore: { current: EditorSelection | undefined };
  /** The waiting selection. It stays through the change of the editor part (ADR 0105, section 6). */
  waitingStore: { current: WaitingSelection | undefined };
}

/** A selection that waits until the switch or the hand-over of Arrival Input ended. */
type WaitingSelection = { chapterId: string; contents: boolean };

const noChanges = () => () => {};

/** Resolves when no IME composition is open in the Arrival Input `typed`, after the capture of its end. */
function compositionEnded(typed: ArrivalInput): Promise<void> {
  return new Promise((resolve) => {
    if (!typed.composing()) {
      resolve();
      return;
    }
    const stop = typed.subscribe(() => {
      if (typed.composing()) return;
      stop();
      // The editor captures the confirmed text after it reports the end of the composition, in the same event.
      void Promise.resolve().then(resolve);
    });
  });
}

const INPUT_BOUND_STOP_MESSAGE =
  "等待保存时输入过多，编辑器已停止输入。已写入本机的输入会在刷新页面后继续保存，最后几次输入可能没有保存。请先复制需要保留的文字，再刷新页面。";

/** The status text for an Author Undo that the Server reports unavailable (ADR 0067). */
const AUTHOR_UNDO_UNAVAILABLE_MESSAGES: Record<AuthorUndoUnavailableError["reason"], string> = {
  barrier: "无法继续撤销：更早的操作不能撤销。",
  no_frontier: "没有可以撤销的操作。",
  source_unavailable: "无法撤销：这个操作的记录已不可用。",
};

/** The blocks of a Pending Edit Projection, as the editor shows them. */
function pendingParagraphs(pending: PendingEditProjection) {
  return pending.blocks.map((block) => ({
    manuscript_block_id: block.manuscript_block_id,
    block_kind: block.block_kind === "heading" ? "heading" as const : "paragraph" as const,
    text: block.text,
  }));
}

/**
 * The workspace of one Project. A command opens the Project again and gives a new `state`. The contents list and the
 * assistant panel stay on the screen, and only the editor part starts again with the new Chapter or base (#1876).
 */
function ProjectReadyView({ editorKey, ...props }: ProjectReadyViewProps & { editorKey: string }) {
  const { state, baseUrl, fetchImpl, cryptoImpl, onRenamed } = props;
  const projectId = state.project.project.project_id;
  const editor = useRef<ProjectEditorHandle | undefined>(undefined);
  const [title, setTitle] = useState(state.project.project.title);
  const [revision, setRevision] = useState(state.revision);
  const [tree, setTree] = useState<GetManuscriptTreeResponse | undefined>(undefined);
  const [treeOpen, setTreeOpen] = useState<{ open: unknown; first: boolean } | undefined>(undefined);
  const [status, setStatus] = useState<ProjectEditorStatus>({ selectedChapterId: state.chapter.chapter.chapter_id,
    canSubmit: false });
  // The copies of Chapter text of this Project (ADR 0105). They end when the tab leaves the Project.
  const [copies] = useState(() => createChapterCopies({ scope: state.project.project_scope, baseUrl, fetchImpl,
    timers: props.writingTimers }));
  useEffect(() => () => { copies.close(); }, [copies]);
  const handOverStore = useRef<ArrivalInput | undefined>(undefined);
  const caretStore = useRef<EditorSelection | undefined>(undefined);
  const waitingStore = useRef<WaitingSelection | undefined>(undefined);
  // The start of the read of an open is not known here, so the open writes only a copy that does not exist.
  useEffect(() => {
    if (copies.get(state.chapter.chapter.chapter_id) === undefined) {
      copies.write(state.chapter.chapter.chapter_id, chapterText(state.chapter));
    }
  }, [copies, state]);
  const [shown, setShown] = useState(state);
  if (shown !== state) {
    setShown(state);
    setTitle(state.project.project.title);
    setRevision(state.revision);
    setStatus((current) => ({ ...current, selectedChapterId: state.chapter.chapter.chapter_id }));
  }
  const currentChapterId = state.project.project.open.kind === "current_chapter"
    ? state.project.project.open.current_chapter_id
    : state.chapter.chapter.chapter_id;

  // Only the latest tree read sets the tree, so an earlier response cannot replace a later tree.
  const treeReads = useRef(0);
  const showTree = (read: Promise<GetManuscriptTreeResponse>, first = false) => {
    treeReads.current += 1;
    const sequence = treeReads.current;
    const open = state;
    void read.then((next) => {
      if (sequence !== treeReads.current) return;
      setTree(next);
      setTreeOpen({ open, first });
    }).catch(() => {});
  };
  const readTree = () => { showTree(readManuscriptTree({ baseUrl, projectId, fetchImpl })); };
  // Until the new tree arrives, the list shows the last tree. The first tree read of a state can be a read of its open.
  useEffect(() => {
    showTree(state.reads?.take("tree") ?? readManuscriptTree({ baseUrl, projectId, fetchImpl }), true);
  }, [baseUrl, fetchImpl, projectId, state]);
  // A Chapter that the author creates, moves, renames, or removes changes the Chapter part of the draft labels.
  useEffect(() => {
    if (tree !== undefined) relabelDraftReferences(state.project.project_scope, tree);
  }, [tree, state.project.project_scope]);

  const selectChapter = (chapterId: string) => editor.current?.selectChapter(chapterId) ?? Promise.resolve(false);
  const canChangeCurrentChapter = state.editor.kind === "editor-ready";
  const holdAddressConversation = useAddressChapter({ tree, currentChapterId, open: selectChapter,
    canChangeCurrentChapter,
    onNotFound: () => editor.current?.showRecovery(ADDRESS_CHAPTER_NOT_FOUND_SENTENCE) });
  return (
    <WritingWorkspace
      holdAddressConversation={holdAddressConversation}
      onNavigateProposal={(destination) => editor.current?.navigateProposal(destination)}
      onOpenedProposal={(locator) => editor.current?.openedProposal(locator)}
      assistant={{
        scope: state.project.project_scope,
        chapterId: currentChapterId, tree,
        canSubmit: status.canSubmit,
        baseUrl, fetchImpl, cryptoImpl, reads: state.reads,
      }}
      tree={(
        <>
          <RenameProjectForm
                title={title}
                workspace
                projectId={projectId}
                revision={revision}
                baseUrl={baseUrl}
                fetchImpl={fetchImpl}
                cryptoImpl={cryptoImpl}
                onRenamed={(nextTitle, nextRevision) => {
                  if (!onRenamed(projectId, nextTitle, nextRevision)) return;
                  setTitle(nextTitle);
                  setRevision(nextRevision);
                }}
              />
          <ManuscriptSearchPanel
            chapterTitles={new Map(tree?.volumes.flatMap((volume) => volume.chapters.map((chapter) => [chapter.chapter_id, chapter.title])))}
            onSelectChapter={selectChapter}
            projectId={projectId}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
          />
          {tree === undefined ? null : (
            <ManuscriptTree
              projectId={projectId}
              tree={tree}
              baseUrl={baseUrl}
              fetchImpl={fetchImpl}
              cryptoImpl={cryptoImpl}
              selectedChapterId={status.selectedChapterId}
              onSelectChapter={(chapterId) => { void editor.current?.selectChapter(chapterId, true); }}
              onChapterPointer={canChangeCurrentChapter ? copies.pointer : undefined}
              onChapterCreated={readTree}
              onVolumeUpdated={readTree}
              onRemoveChapter={(chapterId) => editor.current?.removeChapter(chapterId)}
              onRemoveVolume={(volumeId) => editor.current?.removeVolume(volumeId)}
            />
          )}
          <div className="tree-footer">
          <ProjectSettingsLink projectId={projectId} />
          <ManuscriptReadableExportPanel
            projectId={projectId}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
          />
          </div>
        </>
      )}
      editor={<ProjectEditorView key={editorKey} {...props} tree={tree} treeOpen={treeOpen} treeReads={treeReads}
        handle={editor}
        copies={copies}
        handOverStore={handOverStore}
        caretStore={caretStore}
        waitingStore={waitingStore}
        onStatus={setStatus} />}
    />
  );
}

/** The editor part of the workspace. It starts again with each new Chapter or base of the Editor Session. */
function ProjectEditorView({
  state, baseUrl, fetchImpl, cryptoImpl, onReopened, onLocalRecoveryContinued, proposalNavigation, proposalFocus, proposalNotice, onProposalOpened,
  proposalLocatorStore, writingTimers, tree, treeOpen, treeReads, handle, onStatus, copies, unsavedNotice,
  handOverStore, caretStore, waitingStore,
}: ProjectEditorViewProps) {
  const selectedChapterIdRef = useRef(state.chapter.chapter.chapter_id);
  const switchGenerationRef = useRef(0);
  const structureInFlightRef = useRef(false);
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
  const [readOnly, setReadOnlyState] = useState(state.editor.kind !== "editor-ready");
  // The input state of the editor now. The hand-over reads it when it decides (ADR 0105, section 4).
  const readOnlyRef = useRef(readOnly);
  const setReadOnly = (value: boolean) => {
    readOnlyRef.current = value;
    setReadOnlyState(value);
  };
  // True while the command of a Proposal navigation is in flight. Its answer can move the Editor Session base, so the
  // editor accepts no input until the answer.
  const [navigationCommand, setNavigationCommandState] = useState(false);
  const navigationCommandRef = useRef(false);
  const setNavigationCommand = (value: boolean) => {
    navigationCommandRef.current = value;
    setNavigationCommandState(value);
  };
  // The end of the command of a Proposal navigation in flight, and the count of these commands. A command of a contents
  // selection waits for that end, and it does not go out after a later navigation command.
  const navigationEndRef = useRef<Promise<void> | undefined>(undefined);
  const navigationEndedRef = useRef<() => void>(() => {});
  const navigationCommandsRef = useRef(0);
  // True after a Set Current Chapter command of this part ended with no answer of the Server, until the tab read the
  // Current Chapter of the Server. Until then, the editor accepts no input (ADR 0105, section 5).
  const [resultUnknown, setResultUnknownState] = useState(false);
  const resultUnknownRef = useRef(false);
  const setResultUnknown = (value: boolean) => {
    resultUnknownRef.current = value;
    setResultUnknownState(value);
  };
  const [selectedChapter, setSelectedChapter] = useState<GetChapterResponse>(state.chapter);
  const [switchRecovery, setSwitchRecovery] = useState<string>();
  // The Chapter whose switch waits for the Retry-After of a Command Challenge rate limit (#1679).
  const [switchWaiting, setSwitchWaiting] = useState<string>();
  // The Arriving Chapter of a selection in the contents, from the selection to the end of its switch. The editor of the
  // earlier Chapter ends. The editor part shows the copy, or the earlier text read-only until a copy exists. The copy
  // accepts Arrival Input when it has the fact "no open suggestion" and no wait is active (ADR 0105).
  const [arrival, setArrival] = useState<string>();
  useSyncExternalStore(copies.subscribe, copies.version);
  // The Arrival Input of the Arriving Chapter. It starts on the copy, and a newer copy replaces it while it is empty.
  const inputRef = useRef<ArrivalInput | undefined>(undefined);
  // The Arrival Input that the switch gave to this editor part. It goes to the writing controller when the read of the
  // open suggestions of this editor finds none, after an open IME composition ended (ADR 0105, section 4).
  const [handOver, setHandOver] = useState(() => {
    const given = handOverStore.current;
    handOverStore.current = undefined;
    return given;
  });
  const handOverRef = useRef(handOver);
  const [caret] = useState(() => {
    const given = caretStore.current;
    caretStore.current = undefined;
    return given;
  });
  useSyncExternalStore(handOver?.subscribe ?? noChanges, () => handOver?.version());
  // Each end of an arrival with no hand-over shows the Arrival Input as text that is not saved.
  const showUnsaved = (ended: ArrivalInput | undefined) => {
    if (ended?.hasInput() === true) unsavedNotice.show({ chapterId: ended.chapterId, blocks: ended.blocks() });
  };
  const endArrival = () => {
    showUnsaved(inputRef.current);
    inputRef.current = undefined;
    setArrival(undefined);
  };
  // A Proposal navigation or a reopen can close this part during a switch, and its Arrival Input then goes to a notice.
  useEffect(() => () => {
    showUnsaved(inputRef.current);
    showUnsaved(handOverRef.current);
    inputRef.current = undefined;
    handOverRef.current = undefined;
  }, []);
  // The wait of the current switch, from the click to its end. Closing the view ends it.
  const switchWaitRef = useRef<{ wait: ChallengeAdmissionWait; waiting: boolean } | undefined>(undefined);
  useEffect(() => () => { switchWaitRef.current?.wait.cancel(); }, []);
  // The newest selection in the contents during a switch with no Arrival Input. Only its Chapter gets an editor.
  const newestRef = useRef<string | undefined>(undefined);
  // The Current Chapter of the Server as this tab knows it. Each Set Current Chapter command expects it.
  const serverChapterRef = useRef(currentChapterId);
  // True after a command of this editor part had an effect. The Server then gave the Editor Session a new base, also
  // when the Current Chapter is again the Chapter of this editor.
  const movedRef = useRef(false);
  // A locator of the current AgentRun or of a navigation stays in memory and only causes an earlier list read.
  // A Chapter change starts this part again, so the store of the Project keeps the locators for the next part.
  const projectId = state.project.project.project_id;
  const [proposalLocators, setProposalLocators] = useState<ProposalLocator[]>(
    () => proposalLocatorStore.get(projectId) ?? [],
  );
  const rememberLocator = (locator: ProposalLocator) => {
    const next = [...(proposalLocatorStore.get(projectId) ?? [])
      .filter((item) => item.proposalId !== locator.proposalId), locator];
    proposalLocatorStore.set(projectId, next);
    setProposalLocators(next);
  };
  const [proposalRefresh, setProposalRefresh] = useState(0);
  const onEditorFailureRef = useRef<(error: unknown) => void>(() => {});
  const onEarlierBaseRef = useRef<() => void>(() => {});
  const editorState = state.editor;
  // One writing controller owns the Pending Edit Projection of this Editor Session (ADR 0046).
  const writing = useMemo(() => editorState.kind === "editor-ready"
    ? createEditorSessionWritingController({ workspace: editorState, baseUrl, fetchImpl, cryptoImpl, ...writingTimers,
      onFailure: (error) => { onEditorFailureRef.current(error); },
      onEarlierBase: () => { onEarlierBaseRef.current(); } })
    : undefined, [editorState, baseUrl, fetchImpl, cryptoImpl, writingTimers]);
  useEffect(() => () => { writing?.close(); }, [writing]);
  // Ends the hand-over: each held edit goes to the normal capture entry in its order, or the notice shows the input.
  const endHandOver = (typed: ArrivalInput, capture: boolean) => {
    handOverRef.current = undefined;
    typed.stop();
    if (capture && writing !== undefined && !readOnlyRef.current && !navigationCommandRef.current) {
      for (const item of typed.held()) writing.capture(item.edit, item.origin, item.blocks);
    } else if (typed.hasInput()) {
      unsavedNotice.show({ chapterId: typed.chapterId, blocks: typed.blocks() });
    }
  };
  // The answer of the read of the open suggestions of this editor decides the hand-over, after an open IME
  // composition ended. A complete list with no Proposal gives the input to the writing controller. A Proposal, an
  // incomplete list, a failed read, a Local Edit Journal that stops input, an editor that accepts no input, or a refusal
  // sends the input to the notice.
  // The report of a Proposal starts a new read of the list, and an answer of an earlier read does not decide.
  const reportedRef = useRef(0);
  const decideHandOver = (heads: readonly string[] | undefined, refreshKey = reportedRef.current) => {
    const typed = handOverRef.current;
    if (typed === undefined || refreshKey < reportedRef.current) return;
    void compositionEnded(typed).then(() => {
      if (handOverRef.current !== typed || refreshKey < reportedRef.current) return;
      endHandOver(typed, heads?.length === 0);
      flushSync(() => { setHandOver(undefined); });
      runWaiting();
    });
  };
  // A command with a quiet step refuses a hand-over that waits and has no open IME composition. The notice shows its
  // input in the next microtask, before an answer of the list can come. So no Author Edit of it comes after the quiet
  // step of the command, and the notice has one owner. The command is a later choice of the author than a waiting
  // selection, so the waiting selection ends.
  const refuseHandOver = () => {
    waitingStore.current = undefined;
    decideHandOver(undefined);
  };
  // The quiet step refuses a command during an IME composition of normal input. The writing controller does not have
  // the composition of a hand-over that waits, so the command refuses here.
  const handOverComposing = () => handOverRef.current?.composing() === true;
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

  // A reopen after a settled command of this page reads the Project, the list of Projects, the Chapter `chapterId` (by
  // default the Current Chapter of the page), and the Editor Session at the same time. It does not read the protocol
  // profile again.
  const opened = { profile: state.profile, project: state.project };
  const reopen = (chapterId?: string) => reopenControlledProject({ baseUrl, fetchImpl, cryptoImpl, opened, chapterId });

  // The tab found a later Revision: it opens again on it with its text, and shows no sentence (ADR 0107, section 2).
  // One catch-up runs for each editor part. It waits for an open IME composition, ends input, and makes the editor
  // read-only. It lets the captured input enter the Journal and the submission in flight settle, and closes the
  // controller before the reopen. The close abandons an Author Undo, so the Undo is not sent after the reopen. The
  // reopen uses `chapter`, or else the read of a front check in flight. When the reopen keeps the Chapter and opens
  // nothing, the editor stays read-only with its text, and the next return to the front reads the Chapter and opens
  // again (`keptRef`).
  const catchingUpRef = useRef(false);
  const [catchingUp, setCatchingUp] = useState(false);
  const frontReadRef = useRef<Promise<GetChapterResponse | undefined>>(undefined);
  const keptRef = useRef<readonly ManuscriptParagraph[]>(undefined);
  const reopenWith = async (shown: readonly ManuscriptParagraph[], read: GetChapterResponse | undefined) => {
    const next = await reopenControlledProject({ baseUrl, fetchImpl, cryptoImpl, opened, shown, chapter: read });
    if (next.kind === "chapter-kept") keptRef.current = shown;
    else onReopened(next);
  };
  const catchUp = (chapter?: GetChapterResponse) => {
    if (catchingUpRef.current || writing === undefined) return;
    catchingUpRef.current = true;
    const front = frontReadRef.current;
    void (async () => {
      await writing.whenComposed();
      writing.endInput();
      // The editor is read-only at once, so no input comes after the end of input.
      flushSync(() => { setCatchingUp(true); });
      await writing.whenIdle();
      const read = chapter ?? await front;
      const shown = writing.snapshot().projection.blocks;
      writing.close();
      await reopenWith(shown, read);
    })();
  };
  // A structural command in flight catches up when its quiet step refuses with `earlier_base`.
  onEarlierBaseRef.current = () => {
    if (!structureInFlightRef.current) catchUp();
  };
  // When the tab comes to the front, it waits for the end of an open IME composition, saves its input, and reads the
  // Chapter one time. At a later Revision, the reopen uses that read. A save that finds a later Revision during the
  // check also uses it.
  useEffect(() => {
    if (writing === undefined) return undefined;
    const front = () => {
      const kept = keptRef.current;
      if (document.visibilityState === "visible" && kept !== undefined) {
        keptRef.current = undefined;
        void readCurrentChapter({ baseUrl, fetchImpl } as StoryOSQueryOptions, state.project.project.project_id,
          currentChapterId, state.project.project_scope.owner_user_id).then((read) => reopenWith(kept, read),
          () => { keptRef.current = kept; });
        return;
      }
      if (document.visibilityState !== "visible" || structureInFlightRef.current || handOverRef.current !== undefined
        || selectedChapterIdRef.current !== currentChapterId || frontReadRef.current !== undefined
        || catchingUpRef.current) return;
      // Input during the quiet step refuses it, so the check waits for the end of that input and its save, and runs
      // again, with no limit: each wait is for the editor, not a timer. A tab that went to the background ends the
      // check, and its next return to the front starts a new one. A Revision that the tab installed during the read is
      // not earlier than the read, so that read starts no catch-up.
      const check = async () => {
        for (;;) {
          await writing.whenComposed();
          if (document.visibilityState !== "visible") return undefined;
          let installed: string | undefined;
          const quiet = await writing.runAfterQuiesce("settled", () => {
            installed = writing.snapshot().projection.authoritative_revision_id;
            return readCurrentChapter({ baseUrl, fetchImpl } as StoryOSQueryOptions, state.project.project.project_id,
              currentChapterId, state.project.project_scope.owner_user_id);
          });
          if (quiet.kind === "ran") {
            const editor = writing.snapshot().projection.authoritative_revision_id;
            return editor !== installed || quiet.result.chapter.current_revision.revision_id === editor
              ? undefined : quiet.result;
          }
          if (quiet.reason !== "incomplete_semantic_intent" || catchingUpRef.current) return undefined;
          await writing.whenIdle();
        }
      };
      const read = check().catch(() => undefined);
      frontReadRef.current = read;
      void read.then((chapter) => {
        frontReadRef.current = undefined;
        if (chapter !== undefined && !structureInFlightRef.current) catchUp(chapter);
      });
    };
    document.addEventListener("visibilitychange", front);
    return () => { document.removeEventListener("visibilitychange", front); };
  }, [writing]);

  // A read-only editor has no writing controller and no input to wait for.
  const runWhenQuiet = <Result,>(condition: "journaled" | "settled",
    command: (projection: PendingEditProjection | null) => Promise<Result>) => writing === undefined
    ? command(null).then((result) => ({ kind: "ran" as const, result }))
    : writing.runAfterQuiesce(condition, command);

  // Without a ready editor, a selected Chapter opens read-only.
  const browseChapter = (chapterId: string) => {
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    void (async () => {
      const quiet = await runWhenQuiet("journaled", async () => undefined);
      if (generation !== switchGenerationRef.current) return;
      if (quiet.kind === "refused") {
        setSwitchRecovery(chapterSwitchRecoveryMessage(quiet.reason));
        return;
      }
      const write = copies.chapterRead(chapterId);
      const opened = await openSelectedChapter({
        baseUrl,
        projectId: state.project.project.project_id,
        chapterId,
        expectedScope: state.project.project_scope,
        fetchImpl,
      });
      // The copy is not authority, so the read of a later selection does not discard it.
      if (opened.kind === "opened") write(chapterText(opened.chapter));
      if (generation !== switchGenerationRef.current) return;
      if (opened.kind !== "opened") {
        setSwitchRecovery(chapterSwitchRecoveryMessage(opened.kind));
        return;
      }
      setSwitchRecovery(undefined);
      const surface = selectedChapterSurface({
        selectedChapterId: opened.chapter.chapter.chapter_id,
        currentChapterId,
        // A save can complete during the chapter request, so the current snapshot is newer than the quiet read.
        currentPending: writing?.snapshot().projection ?? null,
        opened: opened.chapter,
      });
      selectedChapterIdRef.current = opened.chapter.chapter.chapter_id;
      setSelectedChapter(opened.chapter);
      setPending(surface.pending);
      setSaveState(surface.save_state);
      setReadOnly(!surface.editable);
    })();
  };

  // The author edits the selected Chapter, so the selection sets the Current Chapter (ADR 0026). The result is true
  // when the command changed the Current Chapter and the workspace opens again at the Chapter.
  const selectChapter = (chapterId: string, contents = false): Promise<boolean> => {
    // The latest click of the author wins: with no Arrival Input, a click ends the wait of an earlier switch (#1679).
    const earlier = switchWaitRef.current;
    if (structureInFlightRef.current && earlier?.waiting !== true) {
      // With no Arrival Input, a selection during a switch in the contents replaces the Arriving Chapter at once. The
      // switch then goes on to the newest selection (ADR 0105, section 6).
      // With Arrival Input, the selection waits until the switch ended.
      if (contents && newestRef.current !== undefined) {
        if (arrivalInputExists()) return waitFor({ chapterId, contents }, newestRef.current);
        // A later selection replaces a waiting selection, also when the input of that time is no longer there.
        waitingStore.current = undefined;
        if (newestRef.current !== chapterId) {
          newestRef.current = chapterId;
          proposalNavigation.sequence += 1;
          setArrival(chapterId);
        }
      }
      return Promise.resolve(false);
    }
    // Arrival Input from before the wait for a rate limit stays until the switch ends (ADR 0105, section 7).
    if (earlier?.waiting === true && arrivalInputExists()) return waitFor({ chapterId, contents }, newestRef.current);
    if (earlier?.waiting === true) {
      earlier.wait.cancel();
      switchWaitRef.current = undefined;
      setSwitchWaiting(undefined);
      endArrival();
      structureInFlightRef.current = false;
    }
    // A selection waits until the hand-over ended (ADR 0105, section 6).
    if (handOverRef.current !== undefined) return waitFor({ chapterId, contents }, selectedChapterIdRef.current);
    waitingStore.current = undefined;
    // After a command had an effect, a selection of the Chapter of the editor opens it again on the new base. After an
    // unknown result, it reads the Current Chapter of the Server.
    if (chapterId === selectedChapterIdRef.current && !movedRef.current && !resultUnknownRef.current) {
      return Promise.resolve(false);
    }
    // The selection is the latest choice of the author, so an earlier Proposal navigation opens no Chapter after it.
    proposalNavigation.sequence += 1;
    if (state.editor.kind !== "editor-ready" || writing === undefined) {
      browseChapter(chapterId);
      return Promise.resolve(false);
    }
    structureInFlightRef.current = true;
    const editorSessionId = state.editor.session.editor_session.editor_session_id;
    const generation = switchGenerationRef.current + 1;
    switchGenerationRef.current = generation;
    const active = { wait: createChallengeAdmissionWait(writingTimers), waiting: false };
    switchWaitRef.current = active;
    // The arrival ends the earlier editor. During an IME composition the quiet step refuses, and an Author Undo ends
    // with its editor, so then the earlier editor stays as today.
    const arrives = contents && !writing.hasIncompleteInput() && !writing.undoInProgress();
    newestRef.current = arrives ? chapterId : undefined;
    if (arrives) setArrival(chapterId);
    // The Chapter that the switch goes to: the newest selection.
    const goal = () => newestRef.current ?? chapterId;
    return (async () => {
      let reopened = false;
      let cancelled = false;
      // True when the switch gave a state to the workspace or a hand-over to this editor part. Then the next editor part
      // or the end of the hand-over runs the waiting selection. A Project with no Current Chapter has no contents of
      // Chapters to select, so the waiting selection ends with the workspace.
      let handedOff = false;
      // A repeat uses the Chapter of the first read, so the command and its idempotency key stay the same.
      const targets = new Map<string, GetChapterResponse>();
      // The Current Chapter of the last command that had an effect, which the reopen opens. When an earlier switch moved
      // the Editor Session of this editor, it is at first the Current Chapter of the Server.
      let reopenAt = movedRef.current ? serverChapterRef.current : undefined;
      let changed = reopenAt !== undefined;
      // The Chapter of the command of this switch whose result is not known, and the read that gave the Current Chapter
      // of the Server when it is the goal.
      let unknownChapter: string | undefined;
      let learned: ControlledProjectState | undefined;
      // A command of a Proposal navigation in flight can change the Current Chapter. So a step waits for its end before
      // a command or an end with no command, and it takes the result of that command. True when the step waited.
      const waitForNavigation = async () => {
        const end = navigationEndRef.current;
        if (end === undefined) return false;
        await end;
        if (movedRef.current) {
          changed = true;
          reopenAt = serverChapterRef.current;
        }
        return true;
      };
      try {
        const quiet = await active.wait.retry(() => {
          active.waiting = false;
          setSwitchWaiting(undefined);
          return writing.runAfterQuiesce("settled", async (settled) => {
            if (generation !== switchGenerationRef.current) return;
            // The switch writes the settled base of the Chapter that the tab leaves.
            copies.write(currentChapterId, { title: selectedChapter.chapter.title,
              revisionId: settled.authoritative_revision_id, blocks: pendingParagraphs(settled) });
            for (;;) {
              const chapter = goal();
              // After an unknown result, the tab reads the Project to learn the Current Chapter of the Server before it
              // sends a command. The read is the reopen when the goal is that Chapter (ADR 0105, section 5).
              if (resultUnknownRef.current) {
                const next = await reopen(chapter);
                if (generation !== switchGenerationRef.current) return;
                if (next.kind !== "project-ready" && next.kind !== "empty-project-ready") {
                  setSwitchRecovery(chapterSwitchRecoveryMessage("unavailable"));
                  return;
                }
                setResultUnknown(false);
                const open = next.project.project.open;
                const current = next.kind === "project-ready" && open.kind === "current_chapter"
                  ? open.current_chapter_id : undefined;
                const moved = current !== undefined && current !== serverChapterRef.current;
                if (moved) {
                  // The command had an effect, so the Editor Session has a new base.
                  reopened = true;
                  changed = true;
                  serverChapterRef.current = current;
                  movedRef.current = true;
                  reopenAt = current;
                }
                // With no move and no ready editor in the read, the tab shows the Project as the read gives it.
                const ready = next.kind === "project-ready" && next.editor.kind === "editor-ready";
                // The command of this switch had no effect, and its Chapter is still the newest selection. This editor
                // accepts input again only when no earlier command moved its base.
                const noEffect = !moved && chapter === unknownChapter && chapter === goal();
                if (current === undefined || (!moved && !ready) || (noEffect && movedRef.current)) {
                  handedOff = true;
                  onReopened(next);
                  return;
                }
                if (noEffect) {
                  setSwitchRecovery(chapterSwitchRecoveryMessage("unavailable"));
                  return;
                }
                if (current === goal()) learned = next;
                continue;
              }
              if (chapter !== serverChapterRef.current) {
                let target = targets.get(chapter);
                if (target === undefined) {
                  // With no fact "no open suggestion", the tab reads the open suggestions together with the Chapter
                  // read, so that the Arriving Chapter can accept input before the switch ends (ADR 0105, section 3).
                  // Only a selection in the contents has an Arriving Chapter.
                  if (arrives && copies.get(chapter)?.noOpenSuggestion !== true) void copies.readSuggestions(chapter);
                  const write = copies.chapterRead(chapter);
                  const opened = await openSelectedChapter({
                    baseUrl,
                    projectId: state.project.project.project_id,
                    chapterId: chapter,
                    expectedScope: state.project.project_scope,
                    fetchImpl,
                  });
                  // A newer Revision in the read replaces the copy and the shown text.
                  if (opened.kind === "opened") write(chapterText(opened.chapter));
                  // Arrival Input on another Revision cannot go to the writing controller, so the input stops at once.
                  const typed = inputRef.current;
                  if (opened.kind === "opened" && typed?.chapterId === chapter && typed.hasInput()
                    && typed.revisionId !== opened.chapter.chapter.current_revision.revision_id) typed.stop();
                  if (generation !== switchGenerationRef.current) return;
                  // A newer selection replaces this Chapter before its command goes out.
                  if (chapter !== goal()) continue;
                  if (opened.kind !== "opened") {
                    setSwitchRecovery(chapterSwitchRecoveryMessage(opened.kind));
                    return;
                  }
                  target = opened.chapter;
                  targets.set(chapter, target);
                }
                // After the wait for a navigation command, or a result that is not known, the step starts again.
                if (await waitForNavigation() || resultUnknownRef.current) {
                  if (generation !== switchGenerationRef.current) return;
                  continue;
                }
                const navigations = navigationCommandsRef.current;
                // A Command Challenge rate limit propagates to the wait. A newer selection after the Command Challenge
                // replaces the command before it goes out.
                const switched = await setOwnedCurrentChapter({
                  baseUrl,
                  fetchImpl,
                  cryptoImpl,
                  projectId: state.project.project.project_id,
                  chapterId: chapter,
                  expectedCurrentChapterId: serverChapterRef.current,
                  expectedTargetRevisionId: target.chapter.current_revision.revision_id,
                  editorSessionId,
                  // A command of a Proposal navigation during the Command Challenge also replaces the command.
                  proceed: () => chapter === goal() && navigationCommandsRef.current === navigations,
                }).catch((error: unknown) => {
                  if (error instanceof SkippedCurrentChapterError) return undefined;
                  if (!(error instanceof UnknownCurrentChapterResultError)) throw error;
                  unknownChapter = chapter;
                  setResultUnknown(true);
                  return undefined;
                });
                if (switched === undefined) continue;
                if (switched.effect.kind !== "authoritative_applied"
                  && switched.effect.kind !== "no_effect") {
                  // The command had no effect, so the switch goes on to the newest selection from the Current Chapter
                  // that the tab knows (ADR 0105, section 6).
                  if (generation === switchGenerationRef.current && chapter !== goal()) continue;
                  setSwitchRecovery(chapterSwitchRecoveryMessage("unavailable"));
                  return;
                }
                changed = true;
                if (switched.effect.kind === "authoritative_applied") {
                  reopened = true;
                  serverChapterRef.current = switched.effect.current_chapter_id;
                  movedRef.current = true;
                  reopenAt = switched.effect.current_chapter_id;
                }
                if (generation !== switchGenerationRef.current) return;
                // A selection during the command goes on from the result of the command.
                if (chapter !== goal()) continue;
              }
              if (await waitForNavigation()) {
                if (generation !== switchGenerationRef.current) return;
                continue;
              }
              // A selection back to the Current Chapter of the editor before a command had an effect needs no command.
              // Arrival Input on the settled base of this editor stays in this part as a hand-over, after an open IME
              // composition ended (ADR 0105, section 4). A new read of the open suggestions of this editor decides it.
              if (!changed) {
                const typed = inputRef.current;
                if (typed?.chapterId !== currentChapterId) return;
                await compositionEnded(typed);
                if (generation !== switchGenerationRef.current || !typed.hasInput()
                  || typed.revisionId !== settled.authoritative_revision_id) return;
                inputRef.current = undefined;
                handOverRef.current = typed;
                setHandOver(typed);
                reportedRef.current += 1;
                setProposalRefresh(reportedRef.current);
                handedOff = true;
                return;
              }
              const next = learned ?? await reopen(reopenAt);
              learned = undefined;
              // No editor mounts for a Chapter that a later selection replaced.
              if (chapter !== goal()) continue;
              // The failure screen has no way back to the workspace, so a waiting selection runs from this editor part.
              if (next.kind === "project-blocked" && waitingStore.current !== undefined) {
                setSwitchRecovery(chapterSwitchRecoveryMessage("unavailable"));
                return;
              }
              // Arrival Input on the base of the new editor goes to it. The editor changes only after an open IME
              // composition ended, so the composition is not cut (ADR 0105, section 4).
              const typed = inputRef.current;
              if (typed !== undefined && next.kind === "project-ready" && next.editor.kind === "editor-ready") {
                await compositionEnded(typed);
                // A Revision is of one Chapter, and a stop of the input comes from another
                // Revision, so the base check also refuses an editor of another Chapter and a stopped input. The base
                // has no unsettled input: the quiet step settled the Local Edit Journal of the Editor Session.
                if (typed.hasInput() && next.editor.openedProjection.authoritative_revision_id === typed.revisionId) {
                  inputRef.current = undefined;
                  handOverStore.current = typed;
                } else if (!typed.hasInput() && typed.focused() && next.chapter.chapter.chapter_id === typed.chapterId) {
                  // The editor of the Chapter keeps the focus and the caret of a click in the Arriving Chapter (#1934).
                  caretStore.current = typed.selection();
                }
              }
              handedOff = true;
              onReopened(next);
              return;
            }
          });
        }, () => {
          active.waiting = true;
          setSwitchWaiting(chapterId);
        });
        if (quiet === undefined) {
          cancelled = true;
        } else if (quiet.kind === "refused" && quiet.reason === "earlier_base"
          && generation === switchGenerationRef.current) {
          // A save found a later Revision. The tab catches up, and the next editor part runs the selection again.
          waitingStore.current = { chapterId: goal(), contents };
          handedOff = true;
          catchUp();
        } else if (quiet.kind === "refused" && generation === switchGenerationRef.current) {
          setSwitchRecovery(chapterSwitchRecoveryMessage(quiet.reason));
        }
      } catch (error: unknown) {
        setSwitchRecovery(
          historicalAcknowledgementUnavailable(error)
            ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE
            : chapterSwitchRecoveryMessage("unavailable"),
        );
      } finally {
        // A cancelled wait belongs to an earlier click or to a view that closed.
        if (!cancelled) structureInFlightRef.current = false;
        // A switch that reached the editor starts this part again. Each other end shows the earlier Chapter again.
        if (!cancelled && generation === switchGenerationRef.current) {
          endArrival();
          newestRef.current = undefined;
          // The Server gave the Editor Session a new base, so this editor accepts no input until a reopen.
          if (movedRef.current) setReadOnly(true);
        }
        if (switchWaitRef.current === active) switchWaitRef.current = undefined;
        if (!cancelled && generation === switchGenerationRef.current && !handedOff) runWaiting();
      }
      return reopened;
    })();
  };

  const arrivalInputExists = () => inputRef.current?.hasInput() === true || inputRef.current?.composing() === true;
  // A later selection replaces a waiting selection. A selection of the Chapter that the tab goes to ends it. The
  // selection is the latest choice of the author, so an earlier Proposal navigation opens no Chapter after it.
  const waitFor = (selection: WaitingSelection, goesTo: string | undefined) => {
    waitingStore.current = selection.chapterId === goesTo ? undefined : selection;
    proposalNavigation.sequence += 1;
    return Promise.resolve(false);
  };
  // The waiting selection runs as a new selection, so its quiet step waits for the save of the Arrival Input.
  const runWaiting = () => {
    const waiting = waitingStore.current;
    waitingStore.current = undefined;
    if (waiting !== undefined) void selectChapter(waiting.chapterId, waiting.contents);
  };
  // A switch that gave no Arrival Input to this editor part ended, so the waiting selection runs.
  useEffect(() => { if (handOverRef.current === undefined) runWaiting(); }, []);

  const removeFromTree = (noun: "章节" | "卷", remove: (expectedTreeRevision: string) => Promise<{
    effect: { kind: string };
  }>) => {
    if (tree === undefined) return;
    if (structureInFlightRef.current) return;
    if (handOverComposing()) {
      setSwitchRecovery(`无法删除${noun}：请先完成当前输入。`);
      return;
    }
    structureInFlightRef.current = true;
    refuseHandOver();
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
          const next = await reopen();
          if (generation !== switchGenerationRef.current) return;
          onReopened(next);
        });
        // A save found a later Revision: the tab catches up, and the author deletes again on the later Revision.
        if (quiet.kind === "refused" && quiet.reason === "earlier_base") {
          catchUp();
        } else if (quiet.kind === "refused" && generation === switchGenerationRef.current) {
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
        structureInFlightRef.current = false;
      }
    })();
  };

  const removeChapter = (chapterId: string) => removeFromTree("章节", (expectedTreeRevision) => deleteOwnedChapter({
    baseUrl, fetchImpl, cryptoImpl, projectId: state.project.project.project_id, chapterId, expectedTreeRevision,
  }));

  const removeVolume = (volumeId: string) => removeFromTree("卷", (expectedTreeRevision) => deleteOwnedVolume({
    baseUrl, fetchImpl, cryptoImpl, projectId: state.project.project.project_id, volumeId, expectedTreeRevision,
  }));

  const editorBlocks = handOver !== undefined ? handOver.blocks()
    : selectedChapter.chapter.chapter_id === currentChapterId && pending !== null
    ? pendingParagraphs(pending)
    : editorParagraphs(selectedChapter.chapter.current_revision.blocks);
  const onEditorFailure = (error: unknown) => {
    if (historicalAcknowledgementUnavailable(error)) {
      setSwitchRecovery(HISTORICAL_ACKNOWLEDGEMENT_MESSAGE);
      return;
    }
    if (error instanceof AuthorUndoUnavailableError) {
      setSwitchRecovery(AUTHOR_UNDO_UNAVAILABLE_MESSAGES[error.reason]);
      return;
    }
    if (error instanceof InputBoundStopError) setSwitchRecovery(INPUT_BOUND_STOP_MESSAGE);
    setReadOnly(true);
    setSaveState("needs_attention");
    setEditorFailure(error instanceof Error ? error.message : "Manuscript editor failed");
  };
  onEditorFailureRef.current = onEditorFailure;
  const openedProposal = (locator: ProposalLocator) => {
    copies.proposalsChanged();
    rememberLocator(locator);
    reportedRef.current += 1;
    setProposalRefresh(reportedRef.current);
  };
  const own: ProjectEditorHandle = {
    selectChapter, removeChapter, removeVolume, openedProposal, showRecovery: setSwitchRecovery,
    navigateProposal: (destination) => {
      if (handOverComposing()) {
        setSwitchRecovery("请先完成当前输入。");
        return;
      }
      refuseHandOver();
      navigateProposal({ state, destination,
        navigation: proposalNavigation, writing, baseUrl, fetchImpl, cryptoImpl,
        onCommandSent: () => {
          navigationCommandsRef.current += 1;
          navigationEndRef.current = new Promise((resolve) => { navigationEndedRef.current = resolve; });
          setNavigationCommand(true);
        },
        // After an effect, the Editor Session has a new base, so this editor accepts no input until a reopen, and the
        // next selection expects the new Current Chapter.
        onCommandEnded: (end) => {
          if (end.kind === "unknown") setResultUnknown(true);
          if (end.kind === "moved") {
            serverChapterRef.current = end.currentChapterId;
            movedRef.current = true;
            setReadOnly(true);
          }
          setNavigationCommand(false);
          navigationEndRef.current = undefined;
          navigationEndedRef.current();
        },
        onOpened: (next, focus, notice) => {
          setSwitchRecovery(undefined); onProposalOpened(next, focus, notice);
        }, onFailure: setSwitchRecovery, onLocator: openedProposal,
      });
    },
  };
  useLayoutEffect(() => {
    handle.current = own;
    return () => { if (handle.current === own) handle.current = undefined; };
  });
  const arriving = arrival === undefined ? undefined : copies.get(arrival);
  // The input of an earlier Arriving Chapter has no text, so a newer selection ends it.
  let input = arrival === undefined || inputRef.current?.chapterId !== arrival ? undefined : inputRef.current;
  if (arrival !== undefined && arriving !== undefined && (input?.chapterId !== arrival
    || (input.revisionId !== arriving.revisionId && !input.hasInput()))) {
    // A newer Revision replaces the text of the Arriving Chapter. A focused editor keeps the focus and the caret.
    input = createArrivalInput(arrival, arriving, input?.chapterId === arrival && input.focused() ? input.selection()
      : undefined);
  }
  inputRef.current = input;
  useSyncExternalStore(input?.subscribe ?? noChanges, () => input?.version());
  // The Arriving Chapter accepts input only when its copy has the fact "no open suggestion", no wait for a Challenge
  // rate limit is active, and no notice is open (ADR 0105, section 3).
  const accepting = input !== undefined && arriving?.noOpenSuggestion === true && switchWaiting === undefined
    && unsavedNotice.current === undefined && !input.stopped() && !resultUnknown;
  // While Arrival Input exists, the state line shows the saving state, and the tab asks before it unloads.
  const hasArrivalInput = input?.hasInput() === true || handOver !== undefined;
  const shownSaveState = hasArrivalInput ? "saving" : saveState;
  useEffect(() => {
    if (!hasArrivalInput) return;
    const prompt = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    globalThis.addEventListener("beforeunload", prompt);
    return () => { globalThis.removeEventListener("beforeunload", prompt); };
  }, [hasArrivalInput]);
  const selectedChapterId = selectedChapter.chapter.chapter_id;
  // A message to the assistant targets the Current Chapter, so the author cannot send one during the arrival.
  const canSubmit = selectedChapterId === currentChapterId && saveState === "saved" && !readOnly && !resultUnknown
    && arrival === undefined && handOver === undefined;
  // From the selection, the contents mark the Arriving Chapter as the selected Chapter.
  const markedChapterId = arrival ?? selectedChapterId;
  useEffect(() => { onStatus({ selectedChapterId: markedChapterId, canSubmit }); }, [markedChapterId, canSubmit,
    onStatus]);
  return (
    <>
      {switchWaiting !== undefined ? <p role="status">{CHAPTER_SWITCH_WAITING_SENTENCE}</p>
        : (switchRecovery ?? proposalNotice) === undefined ? null : (
          <p role="alert">{switchRecovery ?? proposalNotice}</p>
        )}
      <h2>{arriving?.title ?? selectedChapter.chapter.title}</h2>
      <ArrivalNoticeSlot notice={unsavedNotice} />
      <BlockProposalDisplay
        key={selectedChapter.chapter.chapter_id}
        arriving={arrival === undefined ? undefined
          : input === undefined ? { key: "earlier", blocks: editorBlocks }
          : { key: `${arrival}:${input.revisionId}`, blocks: input.blocks(), editable: accepting, input,
            initialSelection: input.focusAt() }}
        scope={state.project.project_scope}
        chapterId={selectedChapter.chapter.chapter_id}
        authoritativeRevisionId={selectedChapter.chapter.chapter_id === currentChapterId
          && pending !== null ? pending.authoritative_revision_id
          : selectedChapter.chapter.current_revision.revision_id}
        focusProposal={proposalFocus}
        locators={proposalLocators}
        refreshKey={proposalRefresh}
        safeToProject={(state.editor.kind !== "editor-ready" && pending === null)
          || selectedChapter.chapter.chapter_id !== currentChapterId
          || saveState === "saved"
          || (pending !== null
            && pending.body === selectedChapter.chapter.current_revision.body
            && JSON.stringify(pending.blocks)
              === JSON.stringify(selectedChapter.chapter.current_revision.blocks))}
        onDecisionEnd={copies.proposalsChanged}
        onAccepted={async (settled) => {
          // A decision with no settled response opens the Project with each check of the first open.
          onReopened(await (settled ? reopen() : openControlledProject({ baseUrl,
            projectId: state.project.project.project_id, fetchImpl, cryptoImpl })));
        }}
        onUndoCurrentChapter={(chapterId) => {
          // An Undo of a chapter switch moves the editor back to the earlier Current Chapter.
          if (chapterId === currentChapterId) return;
          void reopen(chapterId).then(onReopened);
        }}
        blocks={editorBlocks}
        arrival={handOver}
        initialSelection={handOver?.selection() ?? caret}
        onSuggestionList={handOver === undefined ? undefined : decideHandOver}
        // A Selection Reference needs saved text, so none is available while the hand-over waits.
        referenceSource={handOver !== undefined ? undefined : {
          scope: state.project.project_scope,
          chapterId: selectedChapter.chapter.chapter_id,
          revisionId: selectedChapter.chapter.chapter_id === currentChapterId && pending !== null
            ? pending.authoritative_revision_id : selectedChapter.chapter.current_revision.revision_id,
          saved: selectedChapter.chapter.chapter_id !== currentChapterId || pending === null
            || saveState === "saved",
          tree,
        }}
        editable={!readOnly && !navigationCommand && !resultUnknown && !catchingUp}
        // The display of each shown Chapter records the fact "no open suggestion", also of a read-only view (#1934).
        startSuggestionRead={() => copies.suggestionRead(selectedChapter.chapter.chapter_id)}
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
        treeRevision={treeOpen?.open === state ? tree?.tree_revision : undefined}
        openTree={treeOpen?.open === state && treeOpen.first}
        open={state}
        reads={state.reads}
      />
      <small
        data-save-state={shownSaveState}
        data-editor-failure={editorFailure ?? ""}
        data-unsettled-intent-count={pending?.unsettled_intent_count ?? ""}
        data-authoritative-revision-id={
          pending?.authoritative_revision_id
            ?? selectedChapter.chapter.current_revision.revision_id
        }
        data-author-undo-frontier={pending?.author_undo_frontier_sequence ?? ""}
        data-chapter-switch-waiting={switchWaiting ?? ""}
        data-arrival-no-open-suggestion={arriving !== undefined ? String(arriving.noOpenSuggestion)
          : handOver === undefined ? "" : String(copies.get(currentChapterId)?.noOpenSuggestion === true)}
      >
        {arrival !== undefined && !accepting && !hasArrivalInput ? "正在打开本章…"
          : shownSaveState === "saved" ? "已保存"
          : shownSaveState === "saving" ? "保存中"
          : shownSaveState === "needs_attention" ? "需要处理"
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
        onEvents={copies.activity}
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
    </>
  );
}

function RenameProjectForm({
  title, workspace = false,
  projectId,
  revision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onRenamed,
}: {
  title?: string;
  workspace?: boolean;
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
    <div className={workspace ? "project-heading" : undefined} data-project-id={projectId} onContextMenu={(event) => {
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
      <button type="button" data-project-menu aria-label="项目菜单" onClick={(event) => {
        const rect = event.currentTarget.getBoundingClientRect(); setMenu({ x: rect.left, y: rect.bottom + 4 });
      }}>⋯</button>
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
  revision: openedRevision,
  tree,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onVolumeCreated,
  onChapterCreated,
  onRenamed,
  unsavedNotice,
}: {
  project: GetProjectResponse;
  revision: string;
  tree: GetManuscriptTreeResponse;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onVolumeCreated: () => void;
  onChapterCreated: () => void;
  onRenamed: (projectId: string, title: string, revision: string) => boolean;
  unsavedNotice: ArrivalNotice;
}) {
  const [title, setTitle] = useState(project.project.title);
  const [revision, setRevision] = useState(openedRevision);
  // An open that stays on an Empty Project keeps this view, so the view takes the title and the revision of each state.
  const [shown, setShown] = useState(project);
  if (shown !== project) {
    setShown(project);
    setTitle(project.project.title);
    setRevision(openedRevision);
  }
  const [volumeRemoval, setVolumeRemoval] = useState<string>();
  const [addressNotice, setAddressNotice] = useState<string>();
  // A Project with no Chapter has no Chapter to open from the address.
  const holdAddressConversation = useAddressChapter({ tree, currentChapterId: undefined, canChangeCurrentChapter: false,
    open: async () => false, onNotFound: () => setAddressNotice(ADDRESS_CHAPTER_NOT_FOUND_SENTENCE) });
  return (
    <WritingWorkspace
      holdAddressConversation={holdAddressConversation}
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
                if (!onRenamed(project.project.project_id, nextTitle, nextRevision)) return;
                setTitle(nextTitle);
                setRevision(nextRevision);
              }}
            />
          <ManuscriptSearchPanel projectId={project.project.project_id} baseUrl={baseUrl}
            fetchImpl={fetchImpl} chapterTitles={new Map()} />
          {(volumeRemoval ?? addressNotice) === undefined ? null : (
            <p role="alert">{volumeRemoval ?? addressNotice}</p>
          )}
          <ArrivalNoticeSlot notice={unsavedNotice} />
          <ManuscriptTree
            projectId={project.project.project_id}
            tree={tree}
            baseUrl={baseUrl}
            fetchImpl={fetchImpl}
            cryptoImpl={cryptoImpl}
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
          <div className="tree-footer">
            <ProjectSettingsLink projectId={project.project.project_id} />
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
          {library.map((item) => (
            <li key={item.project_scope.project_id}>
              <button
                type="button"
                data-project-id={item.project_scope.project_id}
                data-open={item.open.kind}
                data-revision={item.revision}
                onClick={() => {
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
              <RenameProjectForm
                title={item.title}
                projectId={item.project_scope.project_id}
                revision={item.revision}
                baseUrl={baseUrl}
                fetchImpl={fetchImpl}
                cryptoImpl={cryptoImpl}
                onRenamed={refreshLibrary}
              />
            </li>
          ))}
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

/** The Project, its title, and its revision of a state that opened a Project. */
function projectName(state: ControlledProjectState) {
  return state.kind === "project-ready" || state.kind === "empty-project-ready"
    ? { projectId: state.project.project.project_id, title: state.project.project.title, revision: state.revision }
    : undefined;
}

function Stage1View({
  state, baseUrl, fetchImpl, cryptoImpl, writingTimers, setBootState,
}: Stage1ViewProps & { setBootState: (kind: string) => void }) {
  const [current, setCurrent] = useState(state);
  // The Project title and revision that the workspace holds: from the last taken state or a settled rename. Only a
  // rename changes the revision, by 1, so a state with a lower revision is earlier and keeps the held values (#1888).
  const held = useRef(projectName(state));
  const take = (next: ControlledProjectState) => {
    const name = held.current;
    // Equal also keeps them: an open takes the title from the Project read and the revision from the list read.
    if ((next.kind === "project-ready" || next.kind === "empty-project-ready") && name !== undefined
      && name.projectId === next.project.project.project_id && BigInt(next.revision) <= BigInt(name.revision)) {
      setCurrent({ ...next, revision: name.revision,
        project: { ...next.project, project: { ...next.project.project, title: name.title } } });
      return;
    }
    held.current = projectName(next);
    setCurrent(next);
  };
  // A late rename response of an earlier revision, or of another Project, changes nothing.
  const renamed = (projectId: string, title: string, revision: string) => {
    const name = held.current;
    if (name === undefined || name.projectId !== projectId || BigInt(revision) < BigInt(name.revision)) return false;
    held.current = { projectId, title, revision };
    return true;
  };
  const [editorGeneration, setEditorGeneration] = useState(0);
  const [unsaved, setUnsaved] = useState<UnsavedArrival>();
  const [noticeActions] = useState(() => ({ show: setUnsaved, close: () => { setUnsaved(undefined); } }));
  const unsavedNotice: ArrivalNotice = { current: unsaved, ...noticeActions };
  const [proposalFocus, setProposalFocus] = useState<ProposalFocus>();
  const [proposalNotice, setProposalNotice] = useState<string>();
  const proposalNavigation = useRef<ProposalNavigation>({ sequence: 0, queue: Promise.resolve() });
  const proposalLocatorStore = useRef(new Map<string, ProposalLocator[]>());
  useEffect(() => { setBootState(current.kind); }, [current, setBootState]);
  if (current.kind === "project-ready") {
    // The workspace stays through each command of one Project. Volume removal keeps the current Chapter, so the chapter
    // key stays put. The new editor base snapshot starts the editor part again after honor-deletion.
    return (
      <ProjectReadyView
        key={current.project.project.project_id}
        editorKey={`${editorGeneration}:${
          current.project.project.open.kind === "current_chapter"
            ? current.project.project.open.current_chapter_id
            : current.project.project.project_id
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
        proposalLocatorStore={proposalLocatorStore.current}
        writingTimers={writingTimers}
        onProposalOpened={(next, focus, notice) => { setProposalFocus(focus); setProposalNotice(notice); take(next); }}
        onReopened={take}
        onLocalRecoveryContinued={(next) => { take(next); setEditorGeneration((value) => value + 1); }}
        onRenamed={renamed}
        unsavedNotice={unsavedNotice}
      />
    );
  }
  if (current.kind === "empty-project-ready") {
    return (
      <EmptyProjectReadyView
        project={current.project}
        revision={current.revision}
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
          }).then(take);
        }}
        onChapterCreated={() => {
          void openControlledProject({
            baseUrl,
            projectId: current.project.project.project_id,
            fetchImpl,
            cryptoImpl,
          }).then(take);
        }}
        onRenamed={renamed}
        unsavedNotice={unsavedNotice}
      />
    );
  }
  if (current.kind === "project-settings-ready") {
    return <ProjectSettingsView state={current} baseUrl={baseUrl} fetchImpl={fetchImpl} cryptoImpl={cryptoImpl} />;
  }
  if (current.kind === "protected-ready") {
    return (
      <ProtectedReadyView
        baseUrl={baseUrl}
        fetchImpl={fetchImpl}
        cryptoImpl={cryptoImpl}
        setCurrent={take}
      />
    );
  }
  return (
    <section role="alert">
      <h1>{current.heading}</h1>
      <p>{current.message}</p>
      <pre>{JSON.stringify({ code: current.code, details: current.details }, null, 2)}</pre>
      <ArrivalNoticeSlot notice={unsavedNotice} />
    </section>
  );
}

/** Renders the workspace into `root`. Unmount the returned React root to close its Editor Session controller. */
export function mountStage1View(root: HTMLElement, props: Stage1ViewProps): Root {
  root.dataset.bootState = props.state.kind;
  const reactRoot = createRoot(root);
  reactRoot.render(
    <Stage1View {...props} setBootState={(kind) => { root.dataset.bootState = kind; }} />,
  );
  return reactRoot;
}
