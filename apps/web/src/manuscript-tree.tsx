import { useState } from "react";

import type { GetManuscriptTreeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { ChapterTreeActions } from "./chapter-tree-actions.tsx";
import { createOwnedChapter } from "./create-chapter.ts";
import { createOwnedVolume } from "./create-volume.ts";
import { VolumeTreeActions } from "./volume-tree-actions.tsx";

function CreateChapterForm({
  projectId,
  volumeId,
  treeRevision,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  onCreated,
}: {
  projectId: string;
  volumeId: string;
  treeRevision: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  onCreated: () => void;
}) {
  return (
    <form
      data-create-chapter={volumeId}
      onSubmit={(event) => {
        event.preventDefault();
        const title = String(new FormData(event.currentTarget).get("chapter-title") ?? "").trim();
        if (!title) return;
        void createOwnedChapter({
          baseUrl,
          fetchImpl,
          cryptoImpl,
          projectId,
          volumeId,
          title,
          expectedTreeRevision: treeRevision,
        }).then((created) => {
          if (created.effect.kind !== "authoritative_applied") return;
          onCreated();
        }).catch(() => {});
      }}
    >
      <label>
        章标题
        <input name="chapter-title" required maxLength={1024} />
      </label>
      <button type="submit">创建章</button>
    </form>
  );
}

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
          if (created.effect.kind !== "authoritative_applied") return;
          onCreated();
        }).catch(() => {});
      }}
    >
      <label>
        卷标题
        <input name="volume-title" required maxLength={1024} />
      </label>
      <button type="submit">创建卷</button>
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
}) {
  const volumeCount = tree.volumes.length;
  const [collapsedVolumes, setCollapsedVolumes] = useState<ReadonlySet<string>>(() => new Set());
  return (
    <nav aria-label="稿件目录">
      <div className="tree-heading">目录</div>
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
                  expectedVolumeRevision={tree.tree_revision}
                  baseUrl={baseUrl}
                  fetchImpl={fetchImpl}
                  cryptoImpl={cryptoImpl}
                  onUpdated={onVolumeUpdated}
                />
              ) : null}
              <ul>
                {volume.chapters.map((chapter) => (
                  <ChapterTreeActions
                    key={chapter.chapter_id}
                    projectId={projectId}
                    chapterId={chapter.chapter_id}
                    title={chapter.title}
                    order={chapter.order}
                    chapterCount={volume.chapters.length}
                    expectedChapterRevision={tree.tree_revision}
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
                  />
                ))}
              </ul>
              {createEnabled ? (
                <CreateChapterForm
                  projectId={projectId}
                  volumeId={volume.volume_id}
                  treeRevision={tree.tree_revision}
                  baseUrl={baseUrl}
                  fetchImpl={fetchImpl}
                  cryptoImpl={cryptoImpl}
                  onCreated={onChapterCreated}
                />
              ) : null}
            </li>
          );
        })}
      </ul>
    </nav>
  );
}
