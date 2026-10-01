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
  onCancel,
  onPendingChanged,
}: {
  projectId: string;
  treeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onCreated: () => void;
  onCancel: () => void;
  onPendingChanged: (pending: boolean) => void;
}) {
  const [historicalUnavailable, setHistoricalUnavailable] = useState(false);
  const pending = useRef(false);
  const cancelled = useRef(false);
  const [saving, setSaving] = useState(false);
  const submit = (value: string) => {
    const title = value.trim();
    if (!title || pending.current || cancelled.current) return;
    pending.current = true; onPendingChanged(true); setSaving(true);
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
    }).finally(() => { pending.current = false; onPendingChanged(false); setSaving(false); });
  };
  return (
    <form
      data-create-volume={projectId}
      onSubmit={(event) => { event.preventDefault(); submit(String(new FormData(event.currentTarget).get("volume-title") ?? "")); }}
    >
      <input autoFocus name="volume-title" aria-label="卷标题" placeholder="卷标题" required maxLength={1024}
        readOnly={saving} onBlur={(event) => submit(event.currentTarget.value)} onKeyDown={(event) => {
          if (event.key === "Escape" && !pending.current) { cancelled.current = true; onCancel(); }
        }} />
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
  const [menu, setMenu] = useState<{ x: number; y: number }>();
  const [creatingVolume, setCreatingVolume] = useState(false);
  const begin = (volumeId: string, placement?: CreateChapterPlacement) => {
    if (pendingCreation.current) return;
    setMenu(undefined); setCreatingVolume(false);
    setCreation({ volumeId, revision: tree.tree_revision, ...(placement === undefined ? {} : { placement }) });
    setCollapsedVolumes((current) => new Set([...current].filter((id) => id !== volumeId)));
  };
  const openMenu = (event: React.MouseEvent) => {
    event.preventDefault();
    const rect = event.currentTarget.getBoundingClientRect();
    setMenu({ x: rect.left, y: rect.bottom + 4 });
  };
  const volumeCount = tree.volumes.length;
  const [collapsedVolumes, setCollapsedVolumes] = useState<ReadonlySet<string>>(() => new Set());
  const renderCreation = () => creation === undefined ? null : <InlineCreateChapter
    projectId={projectId} volumeId={creation.volumeId} treeRevision={creation.revision}
    placement={creation.placement} baseUrl={baseUrl} fetchImpl={fetchImpl} cryptoImpl={cryptoImpl}
    onPendingChanged={(pending) => { pendingCreation.current = pending; }}
    onCancel={() => setCreation(undefined)} onCreated={() => { setCreation(undefined); onChapterCreated(); }} />;
  return (
    <nav aria-label="稿件目录">
      <div className="tree-heading">目录{createEnabled ? <button type="button" data-add-chapter
        aria-label="目录菜单" onClick={openMenu}>＋</button> : null}</div>
      {menu === undefined ? null : <ChapterCreationMenu point={menu} onClose={() => setMenu(undefined)}>
        <button type="button" data-create-volume-action onClick={() => { if (pendingCreation.current) return; setCreation(undefined); setCreatingVolume(true); }}>创建卷</button>
        <button type="button" data-chapter-placement="append" disabled={tree.volumes.length === 0}
          onClick={() => begin(tree.volumes[tree.volumes.length - 1]!.volume_id)}>创建章</button>
      </ChapterCreationMenu>}
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
              <div className="volume-row">
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
                  creationActions={<button type="button" data-chapter-placement="append"
                    onClick={() => begin(volume.volume_id)}>创建章</button>}
                  onRemoveVolume={createEnabled ? onRemoveVolume : undefined}
                />
              ) : <span data-volume-title>{volume.title}</span>}
              </div>
              <ul>
                {volume.chapters.map((chapter, index) => (
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
                    creationActions={<>
                      {index > 0 ? <button type="button" data-chapter-placement="before"
                        onClick={() => begin(volume.volume_id, { kind: "before", chapter_id: chapter.chapter_id })}>在上方创建章</button> : null}
                      {index < volume.chapters.length - 1 || volume.chapters.length === 1 ? <button type="button" data-chapter-placement="after"
                        onClick={() => begin(volume.volume_id, { kind: "after", chapter_id: chapter.chapter_id })}>在下方创建章</button> : null}
                    </>}
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
        {creatingVolume ? <li className="inline-volume-creation"><CreateVolumeForm projectId={projectId}
          treeRevision={tree.tree_revision} baseUrl={baseUrl} fetchImpl={fetchImpl} cryptoImpl={cryptoImpl}
          onPendingChanged={(pending) => { pendingCreation.current = pending; }}
          onCancel={() => setCreatingVolume(false)} onCreated={() => { setCreatingVolume(false); onVolumeUpdated(); }} /></li> : null}
      </ul>
    </nav>
  );
}
