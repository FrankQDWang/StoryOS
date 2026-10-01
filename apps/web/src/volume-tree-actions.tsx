import { useRef, useState, type ReactNode } from "react";

import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { ChapterCreationMenu } from "./chapter-creation-menu.tsx";
import { updateOwnedVolume } from "./update-volume.ts";

export function VolumeTreeActions({
  projectId,
  volumeId,
  title,
  order,
  volumeCount,
  expectedTreeRevision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onUpdated,
  onRemoveVolume,
  creationActions,
}: {
  projectId: string;
  volumeId: string;
  title: string;
  order: string;
  volumeCount: number;
  expectedTreeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onUpdated: () => void;
  onRemoveVolume?: ((volumeId: string) => void) | undefined;
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
    void updateOwnedVolume({
      baseUrl,
      fetchImpl,
      cryptoImpl,
      projectId,
      volumeId,
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
    <div className="volume-row-actions" onContextMenu={openMenu}>
      {editing ? <form
        data-rename-volume={volumeId}
        onSubmit={(event) => {
          event.preventDefault();
          const nextTitle = String(new FormData(event.currentTarget).get("volume-title") ?? "").trim();
          if (!nextTitle) return;
          submitUpdate(nextTitle, order);
        }}
      >
        <label>
          <input autoFocus name="volume-title" aria-label="卷标题" required maxLength={1024} defaultValue={title}
            readOnly={saving} onBlur={(event) => submitUpdate(event.currentTarget.value.trim(), order)}
            onKeyDown={(event) => { if (event.key === "Escape" && !pending.current) setEditing(false); }} />
        </label>

      </form> : <span data-volume-title>{title}</span>}
      <button type="button" data-create-chapter-menu={volumeId} aria-label="卷菜单" onClick={openMenu}>⋯</button>
      {menu === undefined ? null : <ChapterCreationMenu point={menu} onClose={() => setMenu(undefined)}>
        {creationActions}
        <button type="button" data-begin-rename-volume={volumeId}
          onClick={() => { setMenu(undefined); setEditing(true); }}>重命名</button>
      <button
        type="button"
        data-volume-move="up"
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
        data-volume-move="down"
        disabled={!canMove || currentOrder >= volumeCount}
        onClick={() => {
          if (!canMove || currentOrder >= volumeCount) return;
          submitUpdate(title, String(currentOrder + 1));
        }}
      >
        下移
      </button>
        {onRemoveVolume !== undefined ? <button type="button" data-delete-volume={volumeId}
          onClick={() => { setMenu(undefined); setPendingRemoval(true); }}>删除卷</button> : null}
      </ChapterCreationMenu>}
        {historicalUnavailable
          ? <p data-rename-volume-error>{HISTORICAL_ACKNOWLEDGEMENT_MESSAGE}</p>
          : null}
      {onRemoveVolume !== undefined ? (
        pendingRemoval ? (
          <>
            <button
              type="button"
              data-confirm-delete-volume={volumeId}
              onClick={() => {
                setPendingRemoval(false);
                onRemoveVolume(volumeId);
              }}
            >
              确认删除此卷？
            </button>
            <button
              type="button"
              data-cancel-delete-volume={volumeId}
              onClick={() => setPendingRemoval(false)}
            >
              取消
            </button>
          </>
        ) : null
      ) : null}
    </div>
  );
}
