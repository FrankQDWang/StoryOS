import { updateOwnedChapter } from "./update-chapter.ts";

export function ChapterTreeActions({
  projectId,
  chapterId,
  title,
  order,
  chapterCount,
  expectedChapterRevision,
  selectedChapterId,
  onSelectChapter,
  currentChapterId,
  makeCurrentEnabled,
  onMakeCurrent,
  createEnabled,
  onRemoveChapter,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onUpdated,
}: {
  projectId: string;
  chapterId: string;
  title: string;
  order: string;
  chapterCount: number;
  expectedChapterRevision: string;
  selectedChapterId?: string | undefined;
  onSelectChapter?: ((chapterId: string) => void) | undefined;
  currentChapterId?: string | undefined;
  makeCurrentEnabled?: boolean | undefined;
  onMakeCurrent?: ((chapterId: string) => void) | undefined;
  createEnabled: boolean;
  onRemoveChapter?: ((chapterId: string) => void) | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onUpdated: () => void;
}) {
  const currentOrder = Number(order);
  const canMove = Number.isInteger(currentOrder) && currentOrder >= 1;
  const submitUpdate = (nextTitle: string, nextOrder: string) => {
    void updateOwnedChapter({
      baseUrl,
      fetchImpl,
      cryptoImpl,
      projectId,
      chapterId,
      title: nextTitle,
      order: nextOrder,
      expectedChapterRevision,
    }).then((updated) => {
      if (
        updated.effect.kind !== "authoritative_applied"
        && updated.effect.kind !== "no_effect"
      ) {
        return;
      }
      onUpdated();
    }).catch(() => {});
  };
  return (
    <li data-chapter-id={chapterId} data-chapter-order={order}>
      {onSelectChapter === undefined ? (
        <span data-chapter-title>{title}</span>
      ) : (
        <button
          type="button"
          data-chapter-id={chapterId}
          data-chapter-title
          aria-current={chapterId === selectedChapterId}
          onClick={() => { onSelectChapter(chapterId); }}
        >
          {title}
        </button>
      )}
      {makeCurrentEnabled === true && onMakeCurrent !== undefined && chapterId !== currentChapterId ? (
        <button
          type="button"
          data-make-current-chapter={chapterId}
          onClick={() => {
            onMakeCurrent(chapterId);
          }}
        >
          设为当前章节
        </button>
      ) : null}
      {createEnabled ? (
        <>
          <form
            data-rename-chapter={chapterId}
            onSubmit={(event) => {
              event.preventDefault();
              const nextTitle = String(new FormData(event.currentTarget).get("chapter-title") ?? "").trim();
              if (!nextTitle) return;
              submitUpdate(nextTitle, order);
            }}
          >
            <label>
              章标题
              <input name="chapter-title" required maxLength={1024} defaultValue={title} />
            </label>
            <button type="submit">重命名</button>
          </form>
          <button
            type="button"
            data-chapter-move="up"
            disabled={!canMove || currentOrder <= 1}
            onClick={() => {
              if (!canMove || currentOrder <= 1) return;
              submitUpdate(title, String(currentOrder - 1));
            }}
          >
            上移
          </button>
          <button
            type="button"
            data-chapter-move="down"
            disabled={!canMove || currentOrder >= chapterCount}
            onClick={() => {
              if (!canMove || currentOrder >= chapterCount) return;
              submitUpdate(title, String(currentOrder + 1));
            }}
          >
            下移
          </button>
          {onRemoveChapter !== undefined ? (
            <button
              type="button"
              data-delete-chapter={chapterId}
              onClick={() => {
                if (!window.confirm("确认删除此章节？")) return;
                onRemoveChapter(chapterId);
              }}
            >
              删除章节
            </button>
          ) : null}
        </>
      ) : null}
    </li>
  );
}
