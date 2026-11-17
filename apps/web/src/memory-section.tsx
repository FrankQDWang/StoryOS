import { useEffect, useRef, useState } from "react";

import { listMemoryEntries } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { MemoryEntryInspect, MemoryEntryTypeInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { deleteMemoryEntryOnce } from "./delete-memory-entry.ts";
import type { MemoryDeleteFlight } from "./delete-memory-entry.ts";
import { sameScope } from "./project-instruction.ts";
import type { SettingsSectionProps } from "./project-settings.tsx";

// The author approved the group names, the origin lines, and the two sentences on 2026-11-16.
const GROUP_NAMES: Record<MemoryEntryTypeInspect, string> = {
  writing_preference: "写作偏好",
  foreshadowing: "伏笔",
  character: "人物",
  relationship: "关系",
  major_event: "重大事件",
  other: "其他",
};
const ORIGIN_LINES = { author_request: "你让助手记住的", conversation: "来自对话" } as const;
const EMPTY_SENTENCE = "助手还没有记住任何内容。";
const SOURCE_UNAVAILABLE = "来源已不可用";
const READ_FAILED_SENTENCE = "无法读取记忆，请稍后再试。";
// The ticket #1579 gives the three control words. The two sentences wait for the words of the author.
const CHANGED_SENTENCE = "这条记忆已经改变，没有删除。";
const DELETE_FAILED_SENTENCE = "无法删除这条记忆，请稍后再试。";

type Group = { type: MemoryEntryTypeInspect; entries: MemoryEntryInspect[] };

/** The entries in groups by type. The query gives them in Memory Index order, so a group is a run of one type. */
function groups(entries: readonly MemoryEntryInspect[]): Group[] {
  const result: Group[] = [];
  for (const entry of entries) {
    const last = result.at(-1);
    if (last?.type === entry.entry_type) last.entries.push(entry);
    else result.push({ type: entry.entry_type, entries: [entry] });
  }
  return result;
}

/** The step of the delete control of one entry, and the sentence under the entry after a delete. */
type DeleteStep = { memoryEntryId: string; step: "confirm" | "sending" | "changed" | "failed" };

/**
 * The Memory section of the settings page (ADR 0094 section 9). It shows each current Memory Entry at the time of the
 * read. The only control that changes an entry is its delete control, and a delete has no undo.
 */
export function MemorySection({ scope, baseUrl, fetchImpl, cryptoImpl }: SettingsSectionProps) {
  const [entries, setEntries] = useState<readonly MemoryEntryInspect[] | "failed">();
  const [reads, setReads] = useState(0);
  const [deleting, setDeleting] = useState<DeleteStep>();
  const flight = useRef<MemoryDeleteFlight>(undefined);
  const { owner_user_id: ownerUserId, project_id: projectId } = scope;
  useEffect(() => {
    let current = true;
    listMemoryEntries({ baseUrl, fetchImpl, projectId })
      .then((read) => sameScope(read.project_scope, { owner_user_id: ownerUserId, project_id: projectId })
        ? read.entries : "failed" as const, () => "failed" as const)
      .then((read) => { if (current) setEntries(read); });
    return () => { current = false; };
  }, [baseUrl, fetchImpl, ownerUserId, projectId, reads]);
  const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
  const confirmDelete = async (entry: MemoryEntryInspect) => {
    const memoryEntryId = entry.memory_entry_id;
    setDeleting({ memoryEntryId, step: "sending" });
    const result = await deleteMemoryEntryOnce({ baseUrl, fetchImpl, cryptoImpl, scope, memoryEntryId,
      version: entry.version, flight: flight.current });
    flight.current = result.flight;
    if (result.outcome === "deleted") {
      setDeleting(undefined);
      setEntries((shown) => shown === "failed" || shown === undefined
        ? shown : shown.filter((item) => item.memory_entry_id !== memoryEntryId));
    } else {
      setDeleting({ memoryEntryId, step: result.outcome });
      if (result.outcome === "changed") setReads((count) => count + 1);
    }
  };
  return (
    <section className="settings-section memory-section" aria-labelledby="memory-heading" data-memory-section="">
      <h2 id="memory-heading">记忆</h2>
      {entries === undefined ? null : entries === "failed" ? (
        <div role="alert" data-memory-read-failed="">
          <p>{READ_FAILED_SENTENCE}</p>
          <button type="button" data-memory-read-again="" onClick={() => {
            setEntries(undefined);
            setReads((count) => count + 1);
          }}>重新读取</button>
        </div>
      ) : entries.length === 0 ? (
        <p data-memory-empty="">{EMPTY_SENTENCE}</p>
      ) : groups(entries).map((group) => (
        <section key={group.type} className="memory-group" data-memory-group={group.type}>
          <h3>{GROUP_NAMES[group.type]}</h3>
          <ul>
            {group.entries.map((entry) => (
              <li key={entry.memory_entry_id} className="memory-entry" data-memory-entry={entry.entry_number}>
                <h4>{entry.title}</h4>
                {/* The place of the conflict flag of S7-D6. */}
                <div className="memory-entry-flag" data-memory-entry-flag="" />
                <p className="memory-entry-text">{entry.text}</p>
                <p className="memory-entry-origin">
                  <Origin entry={entry} projectId={projectId} />
                  <time dateTime={entry.changed_at}>{time.format(new Date(entry.changed_at))}</time>
                </p>
                <div className="memory-entry-actions" data-memory-entry-actions="">
                  <DeleteControl
                    step={deleting?.memoryEntryId === entry.memory_entry_id ? deleting.step : undefined}
                    onDelete={() => setDeleting({ memoryEntryId: entry.memory_entry_id, step: "confirm" })}
                    onConfirm={() => void confirmDelete(entry)}
                    onCancel={() => setDeleting(undefined)}
                  />
                </div>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </section>
  );
}

/**
 * The delete control of one entry: "删除", then "确认删除" and "取消". Only "确认删除" sends the command. After a delete
 * that did not apply, the entry shows one sentence under the control.
 */
function DeleteControl({ step, onDelete, onConfirm, onCancel }: {
  step: DeleteStep["step"] | undefined;
  onDelete: () => void;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const sentence = step === "changed" ? CHANGED_SENTENCE : step === "failed" ? DELETE_FAILED_SENTENCE : undefined;
  return <>
    {step === "confirm" || step === "sending" ? <>
      <button type="button" data-memory-delete-confirm="" disabled={step === "sending"} onClick={onConfirm}>
        确认删除
      </button>
      <button type="button" data-memory-delete-cancel="" disabled={step === "sending"} onClick={onCancel}>取消</button>
    </> : <button type="button" data-memory-delete="" onClick={onDelete}>删除</button>}
    {sentence === undefined ? null : <p role="alert" data-memory-delete-notice={step}>{sentence}</p>}
  </>;
}

/**
 * The origin line of an entry. An available Project Conversation or Chapter is a link to its workspace address with an
 * ordinary page load. An unavailable source has no link.
 */
function Origin({ entry, projectId }: { entry: MemoryEntryInspect; projectId: string }) {
  const { source } = entry;
  const line = entry.origin === "chapter"
    ? source?.kind === "chapter" && source.title !== null ? `来自章节《${source.title}》` : undefined
    : ORIGIN_LINES[entry.origin];
  if (source === null || source.availability === "unavailable") {
    return <span data-memory-entry-origin="unavailable">
      {line === undefined ? null : <span>{line}</span>}
      <span>{SOURCE_UNAVAILABLE}</span>
    </span>;
  }
  const address = source.kind === "conversation"
    ? `/projects/${projectId}?conversation=${source.conversation_id}`
    : `/projects/${projectId}?chapter=${source.chapter_id}`;
  return <a href={address} data-memory-entry-origin={source.kind}>{line}</a>;
}
