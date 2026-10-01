import { Fragment, useRef, useState } from "react";

import type { GetManuscriptTreeResponse, CreateChapterPlacement } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ChapterTreeActions } from "./chapter-tree-actions.tsx";
import { InlineCreateChapter } from "./inline-create-chapter.tsx";
import { ChapterCreationMenu } from "./chapter-creation-menu.tsx";
import { createOwnedVolume } from "./create-volume.ts";
import {
  HISTORICAL_ACKNOWLEDGEMENT_MESSAGE,
  historicalAcknowledgementUnavailable,
} from "./historical-acknowledgement.ts";
import { VolumeTreeActions } from "./volume-tree-actions.tsx";

export function CreateVolumeForm({
  projectId,
  treeRevision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onCreated,
}: {
  projectId: string;
  treeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onCreated: () => void;
}) {
  const [historicalUnavailable, setHistoricalUnavailable] = useState(false);
  return (
    <form
      data-create-volume={projectId}
      onSubmit={(event) => {
        event.preventDefault();
        const title = String(new FormData(event.currentTarget).get("volume-title") ?? "").trim();
        if (!title) return;
        void createOwnedVolume({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId,
          title,
          expectedTreeRevision: treeRevision,
        }).then((created) => {
          setHistoricalUnavailable(false);
          if (created.effect.kind !== "authoritative_applied") return;
          onCreated();
        }).catch((error: unknown) => {
          if (historicalAcknowledgementUnavailable(error)) {
            setHistoricalUnavailable(true);
          }
        });
      }}
    >
      <label>
        卷标题
        <input name="volume-title" required maxLength={1024} />
      </label>
      <button type="submit">创建卷</button>
      {historicalUnavailable
        ? <p data-create-volume-error>{HISTORICAL_ACKNOWLEDGEMENT_MESSAGE}</p>
        : null}
    </form>
  );
}

export function ManuscriptTree({
  projectId,
  tree,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  createEnabled,
  selectedChapterId,
  onSelectChapter,
  currentChapterId,
  makeCurrentEnabled,
  onMakeCurrent,
  onChapterCreated,
  onVolumeUpdated,
  onRemoveChapter,
  onRemoveVolume,
}: {
  projectId: string;
  tree: GetManuscriptTreeResponse;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  createEnabled: boolean;
  selectedChapterId?: string;
  onSelectChapter?: (chapterId: string) => void;
  currentChapterId?: string;
  makeCurrentEnabled?: boolean;
  onMakeCurrent?: (chapterId: string) => void;
  onChapterCreated: () => void;
  onVolumeUpdated: () => void;
  onRemoveChapter?: (chapterId: string) => void;
  onRemoveVolume?: (volumeId: string) => void;
}) {
  const pendingCreation = useRef(false);
  const [creation, setCreation] = useState<{ volumeId: string; revision: string; placement?: CreateChapterPlacement }>();
  const [menu, setMenu] = useState<{ volumeId: string; chapterId?: string; x: number; y: number }>();
  const begin = (volumeId: string, placement?: CreateChapterPlacement) => {
    if (pendingCreation.current) return;
    setMenu(undefined);
    setCreation({ volumeId, revision: tree.tree_revision, ...(placement === undefined ? {} : { placement }) });
    setCollapsedVolumes((current) => new Set([...current].filter((id) => id !== volumeId)));
  };
  const openMenu = (volumeId: string, event: React.MouseEvent, chapterId?: string) => {
    event.preventDefault(); event.stopPropagation();
    const rect = event.currentTarget.getBoundingClientRect();
    setMenu({ volumeId, ...(chapterId === undefined ? {} : { chapterId }),
      x: event.type === "contextmenu" ? event.clientX : rect.left,
      y: event.type === "contextmenu" ? event.clientY : rect.bottom + 4 });
  };
  const menuVolume = tree.volumes.find((volume) => volume.volume_id === menu?.volumeId);
  const menuIndex = menuVolume?.chapters.findIndex((chapter) => chapter.chapter_id === menu?.chapterId) ?? -1;
  const volumeCount = tree.volumes.length;
  const [collapsedVolumes, setCollapsedVolumes] = useState<ReadonlySet<string>>(() => new Set());
  const renderCreation = () => creation === undefined ? null : <InlineCreateChapter
    projectId={projectId} volumeId={creation.volumeId} treeRevision={creation.revision}
    placement={creation.placement} baseUrl={baseUrl} fetchImpl={fetchImpl} cryptoImpl={cryptoImpl}
    onPendingChanged={(pending) => { pendingCreation.current = pending; }}
    onCancel={() => setCreation(undefined)} onCreated={() => { setCreation(undefined); onChapterCreated(); }} />;
  return (
    <nav aria-label="稿件目录" onContextMenu={(event) => {
      if (!createEnabled) return;
      const row = (event.target as Element).closest("li[data-chapter-id], li[data-volume-id]");
      const volumeId = row?.closest("li[data-volume-id]")?.getAttribute("data-volume-id");
      if (volumeId) openMenu(volumeId, event, row?.getAttribute("data-chapter-id") ?? undefined);
    }}>
      <div className="tree-heading">目录{createEnabled ? <button type="button" data-add-chapter
        aria-label="创建章" disabled={tree.volumes.length === 0}
        onClick={() => begin(tree.volumes[tree.volumes.length - 1]!.volume_id)}>＋</button> : null}</div>
      {menu !== undefined ? <ChapterCreationMenu point={menu} onClose={() => setMenu(undefined)}>
        {menu.chapterId === undefined ? <button type="button" data-chapter-placement="append"
          onClick={() => begin(menu.volumeId)}>创建章</button> : <>
          {menuIndex > 0 ? <button type="button" data-chapter-placement="before"
            onClick={() => begin(menu.volumeId, { kind: "before", chapter_id: menu.chapterId! })}>在上方创建章</button> : null}
          {menuIndex >= 0 && (menuIndex < (menuVolume?.chapters.length ?? 0) - 1 || menuVolume?.chapters.length === 1)
            ? <button type="button" data-chapter-placement="after"
              onClick={() => begin(menu.volumeId, { kind: "after", chapter_id: menu.chapterId! })}>在下方创建章</button> : null}
        </>}
      </ChapterCreationMenu> : null}
      <ul>
        {tree.volumes.map((volume) => {
          const expanded = !collapsedVolumes.has(volume.volume_id);
          return (
            <li
              key={volume.volume_id}
              data-volume-id={volume.volume_id}
              data-volume-order={volume.order}
              data-volume-expanded={expanded ? "true" : "false"}
            >
              <span data-volume-title>{volume.title}</span>
              {createEnabled ? <button type="button" data-create-chapter-menu={volume.volume_id}
                aria-label="卷菜单" onClick={(event) => openMenu(volume.volume_id, event)}>⋯</button> : null}
              <button
                type="button"
                data-volume-expand={volume.volume_id}
                aria-expanded={expanded}
                aria-label={expanded ? "折叠卷" : "展开卷"}
                onClick={() => {
                  setCollapsedVolumes((current) => {
                    const next = new Set(current);
                    if (next.has(volume.volume_id)) next.delete(volume.volume_id);
                    else next.add(volume.volume_id);
                    return next;
                  });
                }}
              >
                {expanded ? "▾" : "▸"}
              </button>
              {createEnabled ? (
                <VolumeTreeActions
                  projectId={projectId}
                  volumeId={volume.volume_id}
                  title={volume.title}
                  order={volume.order}
                  volumeCount={volumeCount}
                  expectedTreeRevision={tree.tree_revision}
                  baseUrl={baseUrl}
                  fetchImpl={fetchImpl}
                  cryptoImpl={cryptoImpl}
                  onUpdated={onVolumeUpdated}
                  onRemoveVolume={createEnabled ? onRemoveVolume : undefined}
                />
              ) : null}
              <ul>
                {volume.chapters.map((chapter) => (
                  <Fragment key={chapter.chapter_id}>
                  {creation?.volumeId === volume.volume_id && creation.placement?.kind === "before"
                    && creation.placement.chapter_id === chapter.chapter_id ? renderCreation() : null}
                  <ChapterTreeActions
                    key={chapter.chapter_id}
                    projectId={projectId}
                    chapterId={chapter.chapter_id}
                    title={chapter.title}
                    order={chapter.order}
                    chapterCount={volume.chapters.length}
                    expectedTreeRevision={tree.tree_revision}
                    selectedChapterId={selectedChapterId}
                    onSelectChapter={onSelectChapter}
                    currentChapterId={currentChapterId}
                    makeCurrentEnabled={makeCurrentEnabled}
                    onMakeCurrent={onMakeCurrent}
                    createEnabled={createEnabled}
                    onRemoveChapter={createEnabled ? onRemoveChapter : undefined}
                    baseUrl={baseUrl}
                    fetchImpl={fetchImpl}
                    cryptoImpl={cryptoImpl}
                    onUpdated={onVolumeUpdated}
                    onCreationMenu={(event) => openMenu(volume.volume_id, event, chapter.chapter_id)}
                  />
                  {creation?.volumeId === volume.volume_id && creation.placement?.kind === "after"
                    && creation.placement.chapter_id === chapter.chapter_id ? renderCreation() : null}
                  </Fragment>
                ))}
                {creation?.volumeId === volume.volume_id && creation.placement === undefined ? renderCreation() : null}
              </ul>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
