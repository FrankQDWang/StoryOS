import { useEffect, useState } from "react";

import {
  getStatistics,
  StoryOSProtocolError,
  type GetStatisticsResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

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
}: {
  projectId: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  currentChapterId: string;
  saveState: string;
  treeRevision: string | undefined;
}) {
  const [outcome, setOutcome] = useState<StatisticsOutcome | undefined>(undefined);

  useEffect(() => {
    if (saveState === "saving") return;
    let cancelled = false;
    void (async () => {
      try {
        const page = await getStatistics({ baseUrl, projectId, fetchImpl });
        if (!cancelled) setOutcome({ kind: "ready", page });
      } catch (error) {
        if (cancelled) return;
        setOutcome(
          problemCode(error) === "projection_not_ready"
            ? { kind: "projection_not_ready" }
            : { kind: "unavailable" },
        );
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [baseUrl, currentChapterId, fetchImpl, projectId, saveState, treeRevision]);

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
      <p>
        本章 {chapter === null || chapter === undefined ? "—" : `${chapter.word_count} 词 / ${chapter.character_count} 字`}
        {" · "}
        全书 {page.manuscript.chapter_count} 章 / {page.manuscript.word_count} 词 / {page.manuscript.character_count} 字
      </p>
    </section>
  );
}
