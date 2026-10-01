import { useState } from "react";

import {
  searchManuscript,
  StoryOSProtocolError,
  type ManuscriptSearchSelection,
  type SearchManuscriptResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

type SearchOutcome =
  | {
    kind: "ready";
    query: string;
    selection: ManuscriptSearchSelection;
    page: SearchManuscriptResponse;
  }
  | {
    kind: "projection_not_ready";
    query: string;
    selection: ManuscriptSearchSelection;
  }
  | {
    kind: "unavailable";
    query: string;
    selection: ManuscriptSearchSelection;
  };

function problemCode(error: unknown): string | undefined {
  if (!(error instanceof StoryOSProtocolError)) return undefined;
  try {
    const code = Reflect.get(JSON.parse(error.responseBody ?? ""), "code");
    return typeof code === "string" ? code : undefined;
  } catch {
    return undefined;
  }
}

export function ManuscriptSearchPanel({
  projectId,
  baseUrl,
  fetchImpl,
  chapterTitles,
  onSelectChapter,
}: {
  projectId: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  chapterTitles: ReadonlyMap<string, string>;
  onSelectChapter?: (chapterId: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [selection, setSelection] = useState<ManuscriptSearchSelection>("current_chapter");
  const [outcome, setOutcome] = useState<SearchOutcome | undefined>(undefined);
  const [selectedMatch, setSelectedMatch] = useState<{ chapterId: string; manuscriptBlockId: string; start: number; end: number }>();


  return (
    <section data-manuscript-search="" data-search-active={query.trim() ? "true" : "false"}>
      <form
        data-manuscript-search-form=""
        onSubmit={(event) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const query = String(data.get("manuscript-search-query") ?? "").trim();
          const nextSelection: ManuscriptSearchSelection =
            data.get("manuscript-search-selection") === "manuscript"
              ? "manuscript"
              : "current_chapter";
          if (!query) return;
          setQuery(query);
          setSelection(nextSelection);
          void (async () => {
            try {
              const page = await searchManuscript({
                baseUrl,
                projectId,
                fetchImpl,
                request: {
                  schema_id: "storyos.query.manuscript-search.request.v1",
                  selection: nextSelection,
                  query_text: query,
                  required_watermark: null,
                },
              });
              setOutcome({ kind: "ready", query, selection: nextSelection, page });
              const first = page.items[0];
              setSelectedMatch(first === undefined ? undefined : {
                chapterId: first.chapter_id,
                manuscriptBlockId: first.manuscript_block_id,
                start: Number(first.start),
                end: Number(first.end),
              });
            } catch (error) {
              setOutcome(
                problemCode(error) === "projection_not_ready"
                  ? { kind: "projection_not_ready", query, selection: nextSelection }
                  : { kind: "unavailable", query, selection: nextSelection },
              );
            }
          })();
        }}
      >
        <label className="manuscript-search-field">
          <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8" cy="8" r="5.5" fill="none" stroke="currentColor" /><path d="m12 12 5 5" stroke="currentColor" /></svg>
          <input
            aria-label="搜索稿件" placeholder="搜索稿件"
            name="manuscript-search-query"
            value={query} onChange={(event) => { setQuery(event.currentTarget.value); if (!event.currentTarget.value.trim()) setOutcome(undefined); }}
            maxLength={1024}
          />
        </label>
        <fieldset>
          <legend>范围</legend>
          <label>
            <input
              type="radio"
              name="manuscript-search-selection"
              value="current_chapter"
              checked={selection === "current_chapter"}
              onChange={() => setSelection("current_chapter")}
            />
            当前章节
          </label>
          <label>
            <input
              type="radio"
              name="manuscript-search-selection"
              value="manuscript"
              checked={selection === "manuscript"}
              onChange={() => setSelection("manuscript")}
            />
            全书
          </label>
        </fieldset>
        <button type="submit">查找</button>
      </form>
      {outcome === undefined ? null : outcome.kind === "projection_not_ready" ? (
        <p
          role="status"
          data-search-outcome="projection_not_ready"
          data-search-query={outcome.query}
          data-search-selection={outcome.selection}
        >
          检索投影尚未就绪。
        </p>
      ) : outcome.kind === "unavailable" ? (
        <p
          role="status"
          data-search-outcome="unavailable"
          data-search-query={outcome.query}
          data-search-selection={outcome.selection}
        >
          无法搜索稿件。
        </p>
      ) : (
        <div
          data-search-outcome="ready"
          data-search-query={outcome.query}
          data-search-selection={outcome.selection}
          data-search-completeness={outcome.page.completeness}
          data-search-lag={outcome.page.lag}
          data-search-watermark={outcome.page.projection_watermark}
          data-search-snapshot-id={outcome.page.source_snapshot.snapshot_id}
          data-search-count={String(outcome.page.items.length)}
        >
          {outcome.page.items.length === 0 ? (
            <p role="status">没有匹配。</p>
          ) : (
            <>
              <ol>
                {outcome.page.items.map((item) => {
                  const match = {
                    chapterId: item.chapter_id,
                    manuscriptBlockId: item.manuscript_block_id,
                    start: Number(item.start),
                    end: Number(item.end),
                  };
                  const selected = selectedMatch?.chapterId === match.chapterId
                    && selectedMatch.manuscriptBlockId === match.manuscriptBlockId
                    && selectedMatch.start === match.start
                    && selectedMatch.end === match.end;
                  return (
                    <li
                      key={`${item.chapter_id}:${item.manuscript_block_id}:${item.start}:${item.end}`}
                      data-search-match=""
                      data-chapter-id={item.chapter_id}
                      data-block-id={item.manuscript_block_id}
                      data-range-start={item.start}
                      data-range-end={item.end}
                      data-search-match-selected={selected ? "" : undefined}
                    >
                      <button
                        type="button"
                        onClick={() => { setSelectedMatch(match); onSelectChapter?.(item.chapter_id); }}
                      >
                        {chapterTitles.get(item.chapter_id) ?? "章节暂不可读"} · 第 {item.start}–{item.end} 字
                      </button>
                    </li>
                  );
                })}
              </ol>
            </>
          )}
        </div>
      )}
    </section>
  );
}
