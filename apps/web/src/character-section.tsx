import { useState, type ReactNode } from "react";

import type { MemoryEntryInspect, MemoryEntryIntentInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { EMPTY_SENTENCE, EntryDelete, Flag, OriginLine, ReadFailed } from "./memory-section.tsx";
import type { SettingsSectionProps } from "./project-settings.tsx";

/** The author approved the placeholder and the heading with the sketch of the page on 2026-11-22. */
const SEARCH_PLACEHOLDER = "搜索人物";
const FILE_HEADING = "档案";
/** The author approved the headings of the two blocks of an intent result with the sketch (ADR 0101 section 6). */
const INTENT_HEADING = "接下来想做什么";
const QUESTIONS_HEADING = "助手还不清楚的事";

/**
 * The entries of the type character by name, in the order of the Memory Index. Two entries with one title are one
 * character. In each file, the entry of the author comes first (ADR 0101 sections 3 and 6).
 */
function characters(entries: readonly MemoryEntryInspect[]): Map<string, MemoryEntryInspect[]> {
  const files = new Map<string, MemoryEntryInspect[]>();
  for (const entry of entries) {
    if (entry.entry_type !== "character") continue;
    const file = files.get(entry.title);
    if (file === undefined) files.set(entry.title, [entry]);
    else if (entry.origin === "author_request") file.unshift(entry);
    else file.push(entry);
  }
  return files;
}

/**
 * The section "人物" of the settings page (ADR 0101 section 6): a search, the list of the names, and the file of the
 * selected character. The only control that changes an entry is its delete control.
 */
export function CharacterSection({ scope, memory }: SettingsSectionProps) {
  const [search, setSearch] = useState("");
  // The selected name. After "确认删除" in its file, also the other names of all names, the next names first, so that
  // the search at the end of the delete selects among them.
  const [chosen, setChosen] = useState<{ name: string; following?: readonly string[] }>();
  const { entries } = memory;
  if (entries === undefined) return <CharacterFrame />;
  if (entries === "failed") return <CharacterFrame><ReadFailed memory={memory} /></CharacterFrame>;
  const files = characters(entries);
  if (files.size === 0) return <CharacterFrame><p data-character-empty="">{EMPTY_SENTENCE}</p></CharacterFrame>;
  const all = [...files.keys()];
  const term = search.trim();
  const names = all.filter((name) => name.includes(term));
  // When a delete removed the last part of the selected character, the next name of the list is selected.
  const following = chosen === undefined || files.has(chosen.name) ? [] : chosen.following ?? [];
  const selected = [chosen?.name, ...following].find((name) => name !== undefined && names.includes(name))
    ?? names[0];
  const file = selected === undefined ? undefined : files.get(selected);
  // Only the part of the background work has a current intent result (ADR 0101 section 4).
  const intent = file?.find((entry) => entry.intent !== undefined)?.intent;
  const keep = () => {
    if (selected === undefined) return;
    const at = all.indexOf(selected);
    setChosen({ name: selected, following: [...all.slice(at + 1), ...all.slice(0, at).reverse()] });
  };
  return (
    <CharacterFrame>
      <div className="character-columns">
        <div className="character-list">
          <input type="text" value={search} placeholder={SEARCH_PLACEHOLDER} aria-label={SEARCH_PLACEHOLDER}
            data-character-search="" onChange={(event) => setSearch(event.currentTarget.value)} />
          <ul data-character-names="">
            {names.map((name) => (
              <li key={name}>
                <button type="button" aria-current={name === selected ? "true" : undefined}
                  data-character-name={name} onClick={() => setChosen({ name })}>{name}</button>
              </li>
            ))}
          </ul>
        </div>
        {selected === undefined || file === undefined ? null : (
          // The order of the sketch: the name with the delete control of the first part, its origin line, "档案",
          // and its text. Each later part has its origin line and its delete control on its first line.
          <article className="character-file" aria-labelledby="character-file-name" data-character-file="">
            {file.map((entry, index) => (
              <div key={entry.memory_entry_id} className="character-part" data-character-entry={entry.entry_number}>
                <div className="character-part-head">
                  {index === 0 ? <h3 id="character-file-name">{selected}</h3>
                    : <OriginLine entry={entry} projectId={scope.project_id} />}
                  <EntryDelete entry={entry} memory={memory} onConfirm={keep} />
                </div>
                {index === 0 ? <OriginLine entry={entry} projectId={scope.project_id} /> : null}
                <Flag entry={entry} projectId={scope.project_id} />
                {index === 0 ? <h4>{FILE_HEADING}</h4> : null}
                <p className="memory-entry-text">{entry.text}</p>
              </div>
            ))}
            {intent === undefined ? null : <IntentBlocks intent={intent} />}
          </article>
        )}
      </div>
    </CharacterFrame>
  );
}

/**
 * The block "接下来想做什么" with the intent and the Chapter at which the pass made it, and the block "助手还不清楚的事"
 * with the questions. A block with no content is not shown. The blocks have no control (ADR 0101 section 6).
 */
function IntentBlocks({ intent }: { intent: MemoryEntryIntentInspect }) {
  const chapter = intent.chapter;
  const title = chapter?.kind === "chapter" && chapter.availability === "available" ? chapter.title : null;
  return (
    <>
      <div className="character-intent" data-character-intent="">
        <h4>{INTENT_HEADING}</h4>
        <p className="character-intent-text">{intent.text}</p>
        {title === null ? null : <p className="character-intent-chapter">{`写到《${title}》时的想法`}</p>}
      </div>
      {intent.questions.length === 0 ? null : (
        <div className="character-intent" data-character-questions="">
          <h4>{QUESTIONS_HEADING}</h4>
          <ul>
            {intent.questions.map((question, index) => <li key={index}>{question}</li>)}
          </ul>
        </div>
      )}
    </>
  );
}

function CharacterFrame({ children }: { children?: ReactNode }) {
  return (
    <section className="settings-section character-section" aria-labelledby="character-heading"
      data-character-section="">
      <h2 id="character-heading">人物</h2>
      {children}
    </section>
  );
}
