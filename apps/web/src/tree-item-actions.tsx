import { useRef, useState, type ReactNode } from "react";

import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { ChapterCreationMenu } from "./chapter-creation-menu.tsx";
import { updateOwnedChapter } from "./update-chapter.ts";
import { updateOwnedVolume } from "./update-volume.ts";

/** The texts and the menu attribute of each kind of tree item. */
const TREE_ITEM = {
  chapter: {
    titleLabel: "章标题", menuLabel: "章菜单", menuAttribute: "data-chapter-menu",
    deleteLabel: "删除章节", confirmLabel: "确认删除此章节？",
  },
  volume: {
    titleLabel: "卷标题", menuLabel: "卷菜单", menuAttribute: "data-create-chapter-menu",
    deleteLabel: "删除卷", confirmLabel: "确认删除此卷？",
  },
} as const;

/** The rename, move, and delete-confirm actions of one Chapter or one Volume of the manuscript tree. */
export function TreeItemActions({
  kind,
  projectId,
  itemId,
  title,
  order,
  itemCount,
  expectedTreeRevision,
  selectedChapterId,
  onSelectChapter,
  onRemove,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onUpdated,
  creationActions,
}: {
  kind: "chapter" | "volume";
  projectId: string;
  itemId: string;
  title: string;
  order: string;
  /** The number of items in the list of this item. */
  itemCount: number;
  expectedTreeRevision: string;
  /** Only a Chapter can be selected. */
  selectedChapterId?: string | undefined;
  onSelectChapter?: ((chapterId: string) => void) | undefined;
  onRemove?: ((itemId: string) => void) | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onUpdated: () => void;
  creationActions?: ReactNode;
}) {
  const text = TREE_ITEM[kind];
  const data = (name: string, value: string | boolean = itemId) => ({ [`data-${name}`]: value });
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
    const update = { baseUrl, fetchImpl, cryptoImpl, projectId, title: nextTitle, order: nextOrder,
      expectedTreeRevision };
    void (kind === "chapter"
      ? updateOwnedChapter({ ...update, chapterId: itemId })
      : updateOwnedVolume({ ...update, volumeId: itemId })
    ).then((updated) => {
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
  const content = <>
    {editing ? (
      <form
        {...data(`rename-${kind}`)}
        onSubmit={(event) => {
          event.preventDefault();
          const nextTitle = String(new FormData(event.currentTarget).get(`${kind}-title`) ?? "").trim();
          if (!nextTitle) return;
          submitUpdate(nextTitle, order);
        }}
      >
        <label>
          <input autoFocus name={`${kind}-title`} aria-label={text.titleLabel} required maxLength={1024}
            defaultValue={title} readOnly={saving}
            onBlur={(event) => submitUpdate(event.currentTarget.value.trim(), order)}
            onKeyDown={(event) => { if (event.key === "Escape" && !pending.current) setEditing(false); }} />
        </label>
      </form>
    ) : kind === "volume" || onSelectChapter === undefined ? <span {...data(`${kind}-title`, true)}>{title}</span> : (
      <button type="button" data-chapter-id={itemId} data-chapter-title
        aria-current={itemId === selectedChapterId} onClick={() => onSelectChapter(itemId)}>{title}</button>
    )}
    <button type="button" {...{ [text.menuAttribute]: itemId }} aria-label={text.menuLabel} onClick={openMenu}>⋯</button>
    {menu === undefined ? null : <ChapterCreationMenu point={menu} onClose={() => setMenu(undefined)}>
      {creationActions}
      <button type="button" {...data(`begin-rename-${kind}`)}
        onClick={() => { setMenu(undefined); setEditing(true); }}>重命名</button>
      <button
        type="button"
        {...data(`${kind}-move`, "up")}
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
        {...data(`${kind}-move`, "down")}
        disabled={!canMove || currentOrder >= itemCount}
        onClick={() => {
          if (!canMove || currentOrder >= itemCount) return;
          submitUpdate(title, String(currentOrder + 1));
        }}
      >
        下移
      </button>
      {onRemove !== undefined ? <button type="button" {...data(`delete-${kind}`)}
        onClick={() => { setMenu(undefined); setPendingRemoval(true); }}>{text.deleteLabel}</button> : null}
    </ChapterCreationMenu>}
    {historicalUnavailable
      ? <p {...data(`rename-${kind}-error`, true)}>{HISTORICAL_ACKNOWLEDGEMENT_MESSAGE}</p>
      : null}
    {onRemove !== undefined ? (
      pendingRemoval ? (
        <>
          <button
            type="button"
            {...data(`confirm-delete-${kind}`)}
            onClick={() => {
              setPendingRemoval(false);
              onRemove(itemId);
            }}
          >
            {text.confirmLabel}
          </button>
          <button
            type="button"
            {...data(`cancel-delete-${kind}`)}
            onClick={() => setPendingRemoval(false)}
          >
            取消
          </button>
        </>
      ) : null
    ) : null}
  </>;
  return kind === "chapter"
    ? <li data-chapter-id={itemId} data-chapter-order={order} onContextMenu={openMenu}>{content}</li>
    : <div className="volume-row-actions" onContextMenu={openMenu}>{content}</div>;
}
