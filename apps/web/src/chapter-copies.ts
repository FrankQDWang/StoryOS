import type {
  GetChapterResponse,
  ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readOpenProposalList } from "./open-proposal-list.ts";
import type { ChallengeAdmissionTimers, TimerHandle } from "./challenge-admission-wait.ts";
import { openSelectedChapter } from "./chapter-navigation.ts";
import type { ManuscriptParagraph } from "./manuscript-doc.ts";

/** The time that the pointer stays on a Chapter of the contents before the tab reads that Chapter (ADR 0105). */
export const POINTER_STAY_MS = 150;

/** The text of a Chapter as one read or one settled base gave it. */
export interface ChapterText {
  readonly title: string;
  readonly revisionId: string;
  readonly blocks: readonly ManuscriptParagraph[];
}

/** A copy of a Chapter in the tab. It is not authority: only the display of an Arriving Chapter uses it (ADR 0105). */
export interface ChapterCopy extends ChapterText {
  /** The tab knows that the Chapter has no open suggestion. */
  readonly noOpenSuggestion: boolean;
}

/** The copies of the Chapters of one open Project. They are in memory only and end with the view of the Project. */
export interface ChapterCopies {
  get(chapterId: string): ChapterCopy | undefined;
  /** Writes the copy now, for example from the settled base of the Chapter that the tab leaves. */
  write(chapterId: string, text: ChapterText): void;
  /**
   * Starts a Chapter read. The returned function writes the copy from the read response, only when no other write of
   * that copy started after the read started (ADR 0105).
   */
  chapterRead(chapterId: string): (text: ChapterText) => void;
  /**
   * Starts a read of the open suggestions of a Chapter. The returned function records the Proposal heads of a
   * complete list, or `undefined` for a list that is not complete. A list that is not the last list of the Chapter is
   * a change. The function records nothing when the tab learned of a new or changed Proposal after the start.
   */
  suggestionRead(chapterId: string): (heads: readonly string[] | undefined) => void;
  /** Reads the open suggestions of a Chapter, with no command, and records the list as `suggestionRead` does. */
  readSuggestions(chapterId: string): Promise<void>;
  /**
   * The tab learned of a new or changed Proposal of the Project. `decision` is the end of a decision of this tab: its
   * own command, and no news from the Server.
   */
  proposalsChanged(cause?: "decision"): void;
  /**
   * A check that is true while the tab learned of no new or changed Proposal after this call. The end of a decision of
   * this tab is no news.
   */
  mark(): () => boolean;
  /** The Project Activity events that the tab consumed. A Run can make Proposals, so a new Run event is a change. */
  activity(events: readonly { event_id: string; event_kind: string }[]): void;
  /** The pointer came onto (`inside`) or left a Chapter of the contents. */
  pointer(chapterId: string, inside: boolean): void;
  /** Stops the stay timers and the records of the reads in flight. */
  close(): void;
  /** Calls `listener` after each change of a copy or of a fact. */
  subscribe(listener: () => void): () => void;
  /** A number that each change of a copy or of a fact makes larger. */
  version(): number;
}

/** The paragraphs and headings of a Chapter Revision, as the editor shows them. */
export function editorParagraphs(
  blocks: GetChapterResponse["chapter"]["current_revision"]["blocks"],
): ManuscriptParagraph[] {
  return blocks
    .filter((block) => block.block_kind === "paragraph" || block.block_kind === "heading")
    .map((block) => ({
      manuscript_block_id: block.manuscript_block_id,
      block_kind: block.block_kind === "heading" ? "heading" : "paragraph",
      text: block.text,
    }));
}

/** The text of a Chapter read response. */
export function chapterText(read: GetChapterResponse): ChapterText {
  return { title: read.chapter.title, revisionId: read.chapter.current_revision.revision_id,
    blocks: editorParagraphs(read.chapter.current_revision.blocks) };
}

export function createChapterCopies({ scope, baseUrl, fetchImpl, timers = {} }: {
  scope: ProjectScope;
  baseUrl: string;
  fetchImpl: typeof fetch;
  timers?: ChallengeAdmissionTimers | undefined;
}): ChapterCopies {
  const {
    setTimeoutImpl = (callback, timeout) => globalThis.setTimeout(callback, timeout),
    clearTimeoutImpl = (timer) => globalThis.clearTimeout(timer),
  } = timers;
  const texts = new Map<string, ChapterText>();
  // The last write that started for each copy.
  let writes = 0;
  const lastWrite = new Map<string, number>();
  const noSuggestion = new Set<string>();
  // The Proposals of the last complete list of each Chapter.
  const lists = new Map<string, string>();
  const listeners = new Set<() => void>();
  let version = 0;
  const changed = () => {
    version += 1;
    for (const listener of listeners) listener();
  };
  const seenRunEvents = new Set<string>();
  const stays = new Map<string, TimerHandle>();
  const reading = new Set<string>();
  // Each Proposal change starts a new epoch. A read of an earlier epoch records no fact.
  let epoch = 0;
  // Each Proposal change that the tab learned from the Server.
  let learned = 0;
  let closed = false;
  const copies: ChapterCopies = {
    get(chapterId) {
      const text = texts.get(chapterId);
      return text === undefined ? undefined : { ...text, noOpenSuggestion: noSuggestion.has(chapterId) };
    },
    write(chapterId, text) {
      copies.chapterRead(chapterId)(text);
    },
    chapterRead(chapterId) {
      writes += 1;
      const own = writes;
      lastWrite.set(chapterId, own);
      return (text) => {
        if (closed || lastWrite.get(chapterId) !== own) return;
        texts.set(chapterId, text);
        changed();
      };
    },
    suggestionRead(chapterId) {
      const started = epoch;
      return (heads) => {
        if (closed || started !== epoch) return;
        const list = heads === undefined ? undefined : [...heads].sort().join("|");
        // A Proposal can have a target in more than one Chapter, so a new list removes the fact from each copy.
        if (list === undefined || list !== (lists.get(chapterId) ?? "")) copies.proposalsChanged();
        if (list === undefined) lists.delete(chapterId);
        else lists.set(chapterId, list);
        if (list === "") noSuggestion.add(chapterId);
        changed();
      };
    },
    readSuggestions(chapterId) {
      const record = copies.suggestionRead(chapterId);
      return readOpenProposalList(scope, chapterId, baseUrl, fetchImpl).then((listed) => {
        if (listed.kind === "listed") record(listed.complete ? listed.heads : undefined);
      });
    },
    proposalsChanged(cause) {
      epoch += 1;
      if (cause !== "decision") learned += 1;
      noSuggestion.clear();
      changed();
    },
    mark() {
      const started = learned;
      return () => started === learned;
    },
    activity(events) {
      const runs = events.filter((event) => event.event_kind.startsWith("agent_run_")
        && !seenRunEvents.has(event.event_id));
      if (runs.length === 0) return;
      for (const event of runs) seenRunEvents.add(event.event_id);
      copies.proposalsChanged();
    },
    pointer(chapterId, inside) {
      const stay = stays.get(chapterId);
      if (stay !== undefined) {
        clearTimeoutImpl(stay);
        stays.delete(chapterId);
      }
      // A pointer that only moves across the list starts no read. A copy with text and the fact "no open suggestion"
      // needs no read.
      const known = () => texts.has(chapterId) && noSuggestion.has(chapterId);
      if (!inside || closed || known()) return;
      stays.set(chapterId, setTimeoutImpl(() => {
        stays.delete(chapterId);
        if (closed || known() || reading.has(chapterId)) return;
        reading.add(chapterId);
        // At most two reads and no command (ADR 0105). Each read writes its result when it answers. A copy with text
        // needs only the read of the open suggestions (#1934).
        const reads: Promise<void>[] = [];
        if (!texts.has(chapterId)) {
          const write = copies.chapterRead(chapterId);
          reads.push(openSelectedChapter({ baseUrl, projectId: scope.project_id, chapterId, expectedScope: scope,
            fetchImpl }).then((opened) => { if (opened.kind === "opened") write(chapterText(opened.chapter)); }));
        }
        reads.push(copies.readSuggestions(chapterId));
        void Promise.all(reads).finally(() => { reading.delete(chapterId); });
      }, POINTER_STAY_MS));
    },
    close() {
      closed = true;
      for (const stay of stays.values()) clearTimeoutImpl(stay);
      stays.clear();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => { listeners.delete(listener); };
    },
    version: () => version,
  };
  return copies;
}
