import { useRef, useState, type ReactNode } from "react";

import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { ChapterCreationMenu } from "./chapter-creation-menu.tsx";
import { updateOwnedChapter } from "./update-chapter.ts";

export function ChapterTreeActions({
  projectId,
  chapterId,
  title,
  order,
  chapterCount,
  expectedTreeRevision,
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
  creationActions,
}: {
  projectId: string;
  chapterId: string;
  title: string;
  order: string;
  chapterCount: number;
  expectedTreeRevision: string;
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
  creationActions?: ReactNode;
}) {
  const [editing, setEditing] = useState(false);
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const pending = useRef(false);
  const [saving, setSaving] = useState(false);
  const openMenu = (event: React.MouseEvent) => {
    event.preventDefault(); event.stopPropagation();
    const rect = event.currentTarget.getBoundingClientRect();
    setMenu({ x: event.type === "contextmenu" ? event.clientX : rect.left,
      y: event.type === "contextmenu" ? event.clientY : rect.bottom + 4 });
  };
  const [pendingRemoval, setPendingRemoval] = useState(false);
  const [historicalUnavailable, setHistoricalUnavailable] = useState(false);
  const currentOrder = Number(order);
  const canMove = Number.isInteger(currentOrder) && currentOrder >= 1;
  const submitUpdate = (nextTitle: string, nextOrder: string) => {
    if (pending.current || !nextTitle.trim()) return;
    pending.current = true; setSaving(true);
    void updateOwnedChapter({
      baseUrl,
      fetchImpl,
      cryptoImpl,
      projectId,
      chapterId,
      title: nextTitle,
      order: nextOrder,
      expectedTreeRevision,
    }).then((updated) => {
      setHistoricalUnavailable(false);
      if (
        updated.effect.kind !== "authoritative_applied"
        && updated.effect.kind !== "no_effect"
      ) {
        return;
      }
      setEditing(false); setMenu(undefined); onUpdated();
    }).catch((error: unknown) => {
      if (historicalAcknowledgementUnavailable(error)) {
        setHistoricalUnavailable(true);
      }
    }).finally(() => { pending.current = false; setSaving(false); });
  };
  return (
    <li data-chapter-id={chapterId} data-chapter-order={order} onContextMenu={createEnabled ? openMenu : undefined}>
      {editing ? (
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
              <input autoFocus name="chapter-title" aria-label="章标题" required maxLength={1024} defaultValue={title}
            readOnly={saving} onBlur={(event) => submitUpdate(event.currentTarget.value.trim(), order)}
            onKeyDown={(event) => { if (event.key === "Escape" && !pending.current) setEditing(false); }} />
            </label>

          </form>
      ) : onSelectChapter === undefined ? <span data-chapter-title>{title}</span> : (
        <button type="button" data-chapter-id={chapterId} data-chapter-title
          aria-current={chapterId === selectedChapterId} onClick={() => onSelectChapter(chapterId)}>{title}</button>
      )}
      {createEnabled ? <button type="button" data-chapter-menu={chapterId} aria-label="章菜单" onClick={openMenu}>⋯</button> : null}
      {menu === undefined ? null : <ChapterCreationMenu point={menu} onClose={() => setMenu(undefined)}>
        {creationActions}
        {makeCurrentEnabled === true && onMakeCurrent !== undefined && chapterId !== currentChapterId ? (
          <button type="button" data-make-current-chapter={chapterId}
            onClick={() => { setMenu(undefined); onMakeCurrent(chapterId); }}>设为当前章节</button>
        ) : null}
        <button type="button" data-begin-rename-chapter={chapterId}
          onClick={() => { setMenu(undefined); setEditing(true); }}>重命名</button>
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
        {onRemoveChapter !== undefined ? <button type="button" data-delete-chapter={chapterId}
          onClick={() => { setMenu(undefined); setPendingRemoval(true); }}>删除章节</button> : null}
      </ChapterCreationMenu>}
            {historicalUnavailable
              ? <p data-rename-chapter-error>{HISTORICAL_ACKNOWLEDGEMENT_MESSAGE}</p>
              : null}
          {onRemoveChapter !== undefined ? (
            pendingRemoval ? (
              <>
                <button
                  type="button"
                  data-confirm-delete-chapter={chapterId}
                  onClick={() => {
                    setPendingRemoval(false);
                    onRemoveChapter(chapterId);
                  }}
                >
                  确认删除此章节？
                </button>
                <button
                  type="button"
                  data-cancel-delete-chapter={chapterId}
                  onClick={() => setPendingRemoval(false)}
                >
                  取消
                </button>
              </>
            ) : null
          ) : null}

    </li>
  );
}
