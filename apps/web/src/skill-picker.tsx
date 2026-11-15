import { useLayoutEffect, useRef, useState, type KeyboardEvent, type SyntheticEvent } from "react";
import type { SkillCatalogEntry, SkillPackageReference }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { filterSkills, SKILL_SELECTION_LIMIT, skillTrigger } from "./skill-selection.ts";

const PICKER_ID = "skill-picker";
const optionId = (name: string) => `skill-option-${name}`;

/**
 * The `$` picker of the composer (ADR 0089 section 5) and the Skills that the next message selects. The
 * selection is state of the composer only, so a reload of the page drops it.
 */
export function useSkillPicker({ skills, message, allowed, onMessage }: {
  /** The Skills of the deployment, or `undefined` when the read failed. */
  skills: readonly SkillCatalogEntry[] | undefined;
  message: string;
  /** False while the composer steers a Run in progress. */
  allowed: boolean;
  onMessage: (message: string) => void;
}) {
  const [selected, setSelected] = useState<SkillPackageReference[]>([]);
  const [caret, setCaret] = useState<number>();
  // The trigger whose list the author closed with Escape.
  const [dismissed, setDismissed] = useState<number>();
  const [activeIndex, setActiveIndex] = useState(0);
  const field = useRef<HTMLTextAreaElement>(null);
  const placedCaret = useRef<number>(undefined);
  const trigger = caret === undefined ? undefined : skillTrigger(message, caret);
  const entries = !allowed || skills === undefined || selected.length >= SKILL_SELECTION_LIMIT
    || trigger === undefined || trigger.start === dismissed ? []
    : filterSkills(skills, trigger.query, selected.map((skill) => skill.name));
  const active = Math.min(activeIndex, entries.length - 1);

  useLayoutEffect(() => {
    if (placedCaret.current === undefined) return;
    field.current?.setSelectionRange(placedCaret.current, placedCaret.current);
    placedCaret.current = undefined;
  }, [message]);

  const choose = (entry: SkillCatalogEntry) => {
    if (trigger === undefined || caret === undefined) return;
    onMessage(message.slice(0, trigger.start) + message.slice(caret));
    setSelected([...selected, entry.reference]);
    setCaret(trigger.start);
    placedCaret.current = trigger.start;
  };

  return {
    selected,
    clear: () => setSelected([]),
    remove: (name: string) => setSelected(selected.filter((skill) => skill.name !== name)),
    /** Call it from the change handler of the text field. */
    edited: (text: string, at: number) => {
      setCaret(at);
      setActiveIndex(0);
      if (skillTrigger(text, at) === undefined) setDismissed(undefined);
    },
    field: {
      ref: field,
      onSelect: (event: SyntheticEvent<HTMLTextAreaElement>) => setCaret(event.currentTarget.selectionStart),
      onKeyDown: (event: KeyboardEvent<HTMLTextAreaElement>) => {
        // Safari gives the key that ends a composition the key code 229.
        if (entries.length === 0 || event.nativeEvent.isComposing || event.keyCode === 229) return;
        if (event.key === "ArrowDown" || event.key === "ArrowUp") {
          event.preventDefault();
          setActiveIndex((active + (event.key === "ArrowDown" ? 1 : entries.length - 1)) % entries.length);
        } else if (event.key === "Enter") {
          event.preventDefault();
          choose(entries[active]!);
        } else if (event.key === "Escape") {
          event.preventDefault();
          setDismissed(trigger?.start);
        }
      },
      "aria-controls": entries.length === 0 ? undefined : PICKER_ID,
      "aria-activedescendant": entries.length === 0 ? undefined : optionId(entries[active]!.reference.name),
    },
    list: entries.length === 0 ? null : <ul id={PICKER_ID} className="skill-picker" role="listbox"
      aria-label="选择技能" data-skill-picker="">
      {entries.map((entry, index) => <li key={entry.reference.name} id={optionId(entry.reference.name)}
        role="option" aria-selected={index === active} data-skill-option={entry.reference.name}
        // The text field keeps the focus while the author clicks an entry.
        onMouseDown={(event) => event.preventDefault()} onClick={() => choose(entry)}>
        <span className="skill-name">{entry.reference.name}</span>
        <span className="skill-description">{entry.description}</span>
      </li>)}
    </ul>,
  };
}
