import type {
  GetChapterResponse,
  ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { readOpenProposalList } from "./block-proposal-display.tsx";
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
  /** Writes the copy from a Chapter read response, or from the settled base of the Chapter that the tab leaves. */
  write(chapterId: string, text: ChapterText): void;
  /**
   * Starts a read of the open suggestions of a Chapter. The returned function records the result of that read. It
   * records nothing when the tab learned of a new or changed Proposal after the start.
   */
  suggestionRead(chapterId: string): (none: boolean) => void;
  /** The tab learned of a new or changed Proposal of the Project. */
  proposalsChanged(): void;
  /** The Project Activity events that the tab consumed. A Run can make Proposals, so a new Run event is a change. */
  activity(events: readonly { event_id: string; event_kind: string }[]): void;
  /** The pointer came onto (`inside`) or left a Chapter of the contents. */
  pointer(chapterId: string, inside: boolean): void;
  /** Stops the stay timers and the records of the reads in flight. */
  close(): void;
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
  const noSuggestion = new Set<string>();
  const seenRunEvents = new Set<string>();
  const stays = new Map<string, TimerHandle>();
  const reading = new Set<string>();
  // Each Proposal change starts a new epoch. A read of an earlier epoch records no fact.
  let epoch = 0;
  let closed = false;
  const copies: ChapterCopies = {
    get(chapterId) {
      const text = texts.get(chapterId);
      return text === undefined ? undefined : { ...text, noOpenSuggestion: noSuggestion.has(chapterId) };
    },
    write(chapterId, text) {
      texts.set(chapterId, text);
    },
    suggestionRead(chapterId) {
      const started = epoch;
      return (none) => {
        if (closed || started !== epoch) return;
        if (none) noSuggestion.add(chapterId);
        else noSuggestion.delete(chapterId);
      };
    },
    proposalsChanged() {
      epoch += 1;
      noSuggestion.clear();
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
      // A pointer that only moves across the list starts no read.
      if (!inside || closed || texts.has(chapterId)) return;
      stays.set(chapterId, setTimeoutImpl(() => {
        stays.delete(chapterId);
        // A Chapter has at most one such read in flight.
        if (closed || texts.has(chapterId) || reading.has(chapterId)) return;
        reading.add(chapterId);
        // Two reads and no command (ADR 0105).
        const record = copies.suggestionRead(chapterId);
        void Promise.all([
          openSelectedChapter({ baseUrl, projectId: scope.project_id, chapterId, expectedScope: scope, fetchImpl }),
          readOpenProposalList(scope, chapterId, baseUrl, fetchImpl),
        ]).then(([opened, list]) => {
          if (closed) return;
          // A read of the switch can be later than this read, so this read does not replace a copy.
          if (opened.kind === "opened" && !texts.has(chapterId)) texts.set(chapterId, chapterText(opened.chapter));
          if (list.kind === "listed") record(list.complete && list.locators.length === 0);
        }).finally(() => { reading.delete(chapterId); });
      }, POINTER_STAY_MS));
    },
    close() {
      closed = true;
      for (const stay of stays.values()) clearTimeoutImpl(stay);
      stays.clear();
    },
  };
  return copies;
}
