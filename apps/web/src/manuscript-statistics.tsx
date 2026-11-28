import { useEffect, useRef, useState } from "react";

import {
  getStatistics,
  StoryOSProtocolError,
  type GetStatisticsResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProjectReads } from "./project-reads.ts";

type StatisticsOutcome =
  | { kind: "ready"; page: GetStatisticsResponse }
  | { kind: "projection_not_ready" }
  | { kind: "unavailable" };

function problemCode(error: unknown): string | undefined {
  if (!(error instanceof StoryOSProtocolError)) return undefined;
  try {
    const code = Reflect.get(JSON.parse(error.responseBody ?? ""), "code");
    return typeof code === "string" ? code : undefined;
  } catch {
    return undefined;
  }
}

export function ManuscriptStatisticsPanel({
  projectId,
  baseUrl,
  fetchImpl,
  currentChapterId,
  saveState,
  treeRevision,
  open,
  reads,
}: {
  projectId: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  currentChapterId: string;
  saveState: string;
  /** The revision of the tree of this open, or `undefined` before the tree read of this open gave it. */
  treeRevision: string | undefined;
  /** Each open of the Project reads the statistics again. */
  open: unknown;
  /** The reads of the open. The read of the open uses its statistics read (#1937). */
  reads?: ProjectReads | undefined;
}) {
  const [outcome, setOutcome] = useState<StatisticsOutcome | undefined>(undefined);
  // The Chapter and the tree revision of the last read. Only the latest read sets the outcome.
  const last = useRef<{ open: unknown; chapterId: string; treeRevision: string | undefined; sequence: number }>(
    undefined);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  useEffect(() => {
    const early = reads?.take("statistics");
    if (saveState === "saving") return;
    const earlier = last.current;
    // The read does not wait for the tree read, so the first tree of an open does not start a second read. A later
    // tree of the same open, for example after a new Chapter, reads again.
    if (earlier !== undefined && earlier.open === open && earlier.chapterId === currentChapterId
      && earlier.treeRevision === undefined && treeRevision !== undefined) {
      last.current = { ...earlier, treeRevision };
      return;
    }
    const sequence = (earlier?.sequence ?? 0) + 1;
    last.current = { open, chapterId: currentChapterId, treeRevision, sequence };
    const current = () => mounted.current && last.current?.sequence === sequence;
    void (async () => {
      try {
        const read = () => getStatistics({ baseUrl, projectId, fetchImpl });
        let page = await (early ?? read());
        // An early read can start before the Project read of a reopen, so it can have another Current Chapter.
        if (early !== undefined && page.current_chapter !== null
          && page.current_chapter.chapter_id !== currentChapterId && current()) page = await read();
        if (current()) setOutcome({ kind: "ready", page });
      } catch (error) {
        if (!current()) return;
        setOutcome(
          problemCode(error) === "projection_not_ready"
            ? { kind: "projection_not_ready" }
            : { kind: "unavailable" },
        );
      }
    })();
  }, [baseUrl, currentChapterId, fetchImpl, projectId, saveState, treeRevision, open]);

  if (outcome === undefined) return null;
  if (outcome.kind === "projection_not_ready") {
    return (
      <section data-manuscript-statistics="" data-statistics-outcome="projection_not_ready">
        统计投影尚未就绪。
      </section>
    );
  }
  if (outcome.kind === "unavailable") {
    return (
      <section data-manuscript-statistics="" data-statistics-outcome="unavailable">
        无法读取写作统计。
      </section>
    );
  }
  const page = outcome.page;
  const chapter = page.current_chapter;
  return (
    <section
      data-manuscript-statistics=""
      data-statistics-outcome="ready"
      data-statistics-watermark={page.projection_watermark}
      data-statistics-lag={page.lag}
      data-statistics-snapshot-id={page.source_snapshot.snapshot_id}
      data-statistics-counting-profile={page.counting_profile}
      data-statistics-chapter-count={page.manuscript.chapter_count}
      data-statistics-chapter-words={chapter?.word_count ?? ""}
      data-statistics-chapter-characters={chapter?.character_count ?? ""}
      data-statistics-manuscript-words={page.manuscript.word_count}
      data-statistics-manuscript-characters={page.manuscript.character_count}
    >
      <span>本章 {chapter === null || chapter === undefined ? "—" : chapter.character_count} 字</span>
    </section>
  );
}
