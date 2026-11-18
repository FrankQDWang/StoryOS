import { useEffect, useRef, useState } from "react";

import { listMemoryEntries } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { MemoryEntryInspect, MemoryEntrySourceInspect, MemoryEntryTypeInspect }
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
// The ticket #1629 gives the three control words and the two sentences. The author approved them on 2026-11-17.
const CHANGED_SENTENCE = "这条记忆已经改变，没有删除。";
const DELETE_FAILED_SENTENCE = "无法删除这条记忆，请稍后再试。";
// The ticket #1589 gives the words and the sentence of a conflict flag. The author approved them on 2026-11-16.
const FLAG_WORDS = "可能已过时";
const FLAG_SENTENCE = "可以在对话中告诉助手哪个是对的，或删除这条记忆。";

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
type DeleteStep = "confirm" | "sending" | "changed" | "failed";

/**
 * The Memory section of the settings page (ADR 0094 section 9). It shows each current Memory Entry at the time of the
 * read. The only control that changes an entry is its delete control, and a delete has no undo.
 */
export function MemorySection({ scope, baseUrl, fetchImpl, cryptoImpl }: SettingsSectionProps) {
  const [entries, setEntries] = useState<readonly MemoryEntryInspect[] | "failed">();
  const [reads, setReads] = useState(0);
  // Each entry has its own step and its own delete intent, so a delete of one entry does not change another entry.
  const [steps, setSteps] = useState<ReadonlyMap<string, DeleteStep>>(new Map());
  const flights = useRef(new Map<string, MemoryDeleteFlight>());
  // The entries that a delete removed. A read that started before the delete must not show them again.
  const removed = useRef(new Set<string>());
  const { owner_user_id: ownerUserId, project_id: projectId } = scope;
  useEffect(() => {
    let current = true;
    listMemoryEntries({ baseUrl, fetchImpl, projectId })
      .then((read) => sameScope(read.project_scope, { owner_user_id: ownerUserId, project_id: projectId })
        ? read.entries : "failed" as const, () => "failed" as const)
      .then((read) => {
        if (current) {
          setEntries(read === "failed" ? read : read.filter((item) => !removed.current.has(item.memory_entry_id)));
        }
      });
    return () => { current = false; };
  }, [baseUrl, fetchImpl, ownerUserId, projectId, reads]);
  const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
  const setStep = (memoryEntryId: string, step: DeleteStep | undefined) => setSteps((shown) => {
    const next = new Map(shown);
    if (step === undefined) next.delete(memoryEntryId);
    else next.set(memoryEntryId, step);
    return next;
  });
  const confirmDelete = async (entry: MemoryEntryInspect) => {
    const memoryEntryId = entry.memory_entry_id;
    setStep(memoryEntryId, "sending");
    const result = await deleteMemoryEntryOnce({ baseUrl, fetchImpl, cryptoImpl, scope, memoryEntryId,
      version: entry.version, flight: flights.current.get(memoryEntryId) });
    if (result.flight === undefined) flights.current.delete(memoryEntryId);
    else flights.current.set(memoryEntryId, result.flight);
    if (result.outcome === "deleted") {
      removed.current.add(memoryEntryId);
      setStep(memoryEntryId, undefined);
      setEntries((shown) => shown === "failed" || shown === undefined
        ? shown : shown.filter((item) => item.memory_entry_id !== memoryEntryId));
    } else {
      setStep(memoryEntryId, result.outcome);
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
                <Flag entry={entry} projectId={projectId} />
                <p className="memory-entry-text">{entry.text}</p>
                <p className="memory-entry-origin">
                  <Origin entry={entry} projectId={projectId} />
                  <time dateTime={entry.changed_at}>{time.format(new Date(entry.changed_at))}</time>
                </p>
                <div className="memory-entry-actions" data-memory-entry-actions="">
                  <DeleteControl
                    step={steps.get(entry.memory_entry_id)}
                    onDelete={() => setStep(entry.memory_entry_id, "confirm")}
                    onConfirm={() => void confirmDelete(entry)}
                    onCancel={() => setStep(entry.memory_entry_id, undefined)}
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
  step: DeleteStep | undefined;
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

/** The line of a Chapter source with its title, or `undefined` when the Chapter is not in the manuscript tree now. */
function chapterLine(source: MemoryEntrySourceInspect | null): string | undefined {
  return source?.kind === "chapter" && source.title !== null ? `来自章节《${source.title}》` : undefined;
}

/**
 * The origin line of an entry. An available Project Conversation or Chapter is a link to its workspace address with an
 * ordinary page load. An unavailable source has no link.
 */
function Origin({ entry, projectId }: { entry: MemoryEntryInspect; projectId: string }) {
  const line = entry.origin === "chapter" ? chapterLine(entry.source) : ORIGIN_LINES[entry.origin];
  return <SourceLine source={entry.source} line={line} projectId={projectId} mark="data-memory-entry-origin" />;
}

/**
 * The conflict flag of an entry below its title (ADR 0094 section 7): "可能已过时", the reason, the source of the conflict,
 * and the sentence that tells the author what to do. An entry with no flag has an empty place. No control removes a flag.
 */
function Flag({ entry, projectId }: { entry: MemoryEntryInspect; projectId: string }) {
  const { flag } = entry;
  if (flag === undefined) return <div className="memory-entry-flag" data-memory-entry-flag="" />;
  const line = flag.source.kind === "chapter" ? chapterLine(flag.source) : ORIGIN_LINES.conversation;
  return (
    <div className="memory-entry-flag" data-memory-entry-flag="flagged">
      <p><strong>{FLAG_WORDS}</strong> <span data-memory-flag-reason="">{flag.reason}</span></p>
      <p><SourceLine source={flag.source} line={line} projectId={projectId} mark="data-memory-flag-source" /></p>
      <p>{FLAG_SENTENCE}</p>
    </div>
  );
}

/**
 * A source as a link to its workspace address, or as its line and the sentence of an unavailable source with no link.
 * The attribute `mark` has the kind of the source, or `unavailable`.
 */
function SourceLine({ source, line, projectId, mark }: {
  source: MemoryEntrySourceInspect | null;
  line: string | undefined;
  projectId: string;
  mark: "data-memory-entry-origin" | "data-memory-flag-source";
}) {
  if (source === null || source.availability === "unavailable") {
    return <span {...{ [mark]: "unavailable" }}>
      {line === undefined ? null : <span>{line}</span>}
      <span>{SOURCE_UNAVAILABLE}</span>
    </span>;
  }
  const address = source.kind === "conversation"
    ? `/projects/${projectId}?conversation=${source.conversation_id}`
    : `/projects/${projectId}?chapter=${source.chapter_id}`;
  return <a href={address} {...{ [mark]: source.kind }}>{line}</a>;
}
