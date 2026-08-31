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
}: {
  projectId: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
}) {
  const [selection, setSelection] = useState<ManuscriptSearchSelection>("current_chapter");
  const [outcome, setOutcome] = useState<SearchOutcome | undefined>(undefined);

  return (
    <section data-manuscript-search="">
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
        <label>
          搜索
          <input
            name="manuscript-search-query"
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
            <ol>
              {outcome.page.items.map((item) => (
                <li
                  key={`${item.chapter_id}:${item.manuscript_block_id}:${item.start}:${item.end}`}
                  data-search-match=""
                  data-chapter-id={item.chapter_id}
                  data-block-id={item.manuscript_block_id}
                  data-range-start={item.start}
                  data-range-end={item.end}
                >
                  {item.chapter_id} · {item.start}–{item.end}
                </li>
              ))}
            </ol>
          )}
        </div>
      )}
    </section>
  );
}
