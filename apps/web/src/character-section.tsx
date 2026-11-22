import { useState, type ReactNode } from "react";

import type { MemoryEntryInspect } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { EMPTY_SENTENCE, EntryDelete, Flag, OriginLine, ReadFailed } from "./memory-section.tsx";
import type { SettingsSectionProps } from "./project-settings.tsx";

/** The author approved the placeholder and the heading with the sketch of the page on 2026-11-22. */
const SEARCH_PLACEHOLDER = "搜索人物";
const FILE_HEADING = "档案";

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
  const [chosen, setChosen] = useState<string>();
  const { entries } = memory;
  if (entries === undefined) return <CharacterFrame />;
  if (entries === "failed") return <CharacterFrame><ReadFailed memory={memory} /></CharacterFrame>;
  const files = characters(entries);
  if (files.size === 0) return <CharacterFrame><p data-character-empty="">{EMPTY_SENTENCE}</p></CharacterFrame>;
  const term = search.trim();
  const names = [...files.keys()].filter((name) => name.includes(term));
  const selected = chosen !== undefined && names.includes(chosen) ? chosen : names[0];
  const file = selected === undefined ? undefined : files.get(selected);
  // The delete of the last entry of a character removes its name, and the next name of the list is selected.
  const afterDelete = (name: string) => {
    if (files.get(name)?.length !== 1) return;
    const at = names.indexOf(name);
    setChosen(names[at + 1] ?? names[at - 1]);
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
                  data-character-name={name} onClick={() => setChosen(name)}>{name}</button>
              </li>
            ))}
          </ul>
        </div>
        {selected === undefined || file === undefined ? null : (
          <article className="character-file" aria-labelledby="character-file-name" data-character-file="">
            <h3 id="character-file-name">{selected}</h3>
            <h4>{FILE_HEADING}</h4>
            <ul>
              {file.map((entry) => (
                <li key={entry.memory_entry_id} className="character-part" data-character-entry={entry.entry_number}>
                  <div className="character-part-head">
                    <OriginLine entry={entry} projectId={scope.project_id} />
                    <EntryDelete entry={entry} memory={memory} onDeleted={() => afterDelete(selected)} />
                  </div>
                  <Flag entry={entry} projectId={scope.project_id} />
                  <p className="memory-entry-text">{entry.text}</p>
                </li>
              ))}
            </ul>
          </article>
        )}
      </div>
    </CharacterFrame>
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
