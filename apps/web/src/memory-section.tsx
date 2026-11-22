import type { MemoryEntryInspect, MemoryEntrySourceInspect, MemoryEntryTypeInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { DeleteStep, MemoryEntries } from "./memory-entries.ts";
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
export const EMPTY_SENTENCE = "助手还没有记住任何内容。";
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

/** The sentence of a failed read and the control that reads again. */
export function ReadFailed({ memory }: { memory: MemoryEntries }) {
  return (
    <div role="alert" data-memory-read-failed="">
      <p>{READ_FAILED_SENTENCE}</p>
      <button type="button" data-memory-read-again="" onClick={memory.readAgain}>重新读取</button>
    </div>
  );
}

/**
 * The Memory section of the settings page (ADR 0094 section 9). It shows each current Memory Entry at the time of the
 * read, except the entries of the type character: the section "人物" shows them (ADR 0101 section 6). The only control
 * that changes an entry is its delete control, and a delete has no undo.
 */
export function MemorySection({ scope, memory }: SettingsSectionProps) {
  const { entries } = memory;
  return (
    <section className="settings-section memory-section" aria-labelledby="memory-heading" data-memory-section="">
      <h2 id="memory-heading">记忆</h2>
      {entries === undefined ? null : entries === "failed" ? <ReadFailed memory={memory} />
        : entries.length === 0 ? (
          <p data-memory-empty="">{EMPTY_SENTENCE}</p>
        ) : groups(entries.filter((entry) => entry.entry_type !== "character")).map((group) => (
          <section key={group.type} className="memory-group" data-memory-group={group.type}>
            <h3>{GROUP_NAMES[group.type]}</h3>
            <ul>
              {group.entries.map((entry) => (
                <li key={entry.memory_entry_id} className="memory-entry" data-memory-entry={entry.entry_number}>
                  <h4>{entry.title}</h4>
                  <EntryParts entry={entry} projectId={scope.project_id} memory={memory} />
                </li>
              ))}
            </ul>
          </section>
        ))}
    </section>
  );
}

const TIME = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });

/** The parts of one entry under its title: the conflict flag, the text, the origin line, the time, and the delete control. */
export function EntryParts({ entry, projectId, memory, onDeleted }: {
  entry: MemoryEntryInspect;
  projectId: string;
  memory: MemoryEntries;
  /** Runs after the delete removed the entry from the list. */
  onDeleted?: () => void;
}) {
  const id = entry.memory_entry_id;
  return <>
    <Flag entry={entry} projectId={projectId} />
    <p className="memory-entry-text">{entry.text}</p>
    <p className="memory-entry-origin">
      <Origin entry={entry} projectId={projectId} />
      <time dateTime={entry.changed_at}>{TIME.format(new Date(entry.changed_at))}</time>
    </p>
    <div className="memory-entry-actions" data-memory-entry-actions="">
      <DeleteControl
        step={memory.steps.get(id)}
        onDelete={() => memory.setStep(id, "confirm")}
        onConfirm={() => void memory.confirmDelete(entry).then((left) => { if (left) onDeleted?.(); })}
        onCancel={() => memory.setStep(id, undefined)}
      />
    </div>
  </>;
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
