import { useEffect, useRef, useState } from "react";

import {
  readProjectInstruction,
  saveProjectInstruction,
  type InstructionSaveFlight,
  type InstructionSaveOutcome,
  type SavedInstruction,
} from "./project-instruction.ts";
import type { SettingsSectionProps } from "./project-settings.tsx";

/** The sentence of each save outcome. The author approved these words on 2026-11-16. */
const OUTCOME_SENTENCES: Record<InstructionSaveOutcome["kind"], string> = {
  applied: "已保存。新的指令从下一次请求开始生效。",
  unchanged: "没有改动。",
  conflicted: "项目指令已在别处修改。请刷新后再保存。",
  over_limit: "项目指令太长，没有保存。请缩短后再保存。",
  unknown: "无法确认是否已保存，请再试一次。",
};

/** The Project Instruction section of the settings page (ADR 0094 sections 2 and 9). It has no automatic save. */
export function ProjectInstructionSection({
  scope, baseUrl, fetchImpl, cryptoImpl, onUnsavedChange,
}: SettingsSectionProps) {
  const [saved, setSaved] = useState<SavedInstruction | "unavailable">();
  const [text, setText] = useState("");
  const [saving, setSaving] = useState(false);
  const [outcome, setOutcome] = useState<InstructionSaveOutcome["kind"]>();
  const flight = useRef<InstructionSaveFlight>(undefined);
  const { owner_user_id: ownerUserId, project_id: projectId } = scope;
  useEffect(() => {
    let current = true;
    readProjectInstruction({ baseUrl, fetchImpl, scope: { owner_user_id: ownerUserId, project_id: projectId } })
      .catch(() => undefined)
      .then((read) => {
        if (!current) return;
        setSaved(read ?? "unavailable");
        if (read !== undefined) setText(read.text);
      });
    return () => { current = false; };
  }, [baseUrl, fetchImpl, ownerUserId, projectId]);
  const unsaved = typeof saved === "object" && text !== saved.text;
  useEffect(() => { onUnsavedChange(unsaved); }, [onUnsavedChange, unsaved]);
  const save = async (base: SavedInstruction) => {
    setSaving(true);
    setOutcome(undefined);
    const result = await saveProjectInstruction({ baseUrl, fetchImpl, cryptoImpl, scope, text,
      expected: base.revision, flight: flight.current });
    flight.current = result.flight;
    if (result.outcome.kind === "applied") setSaved(result.outcome.saved);
    setOutcome(result.outcome.kind);
    setSaving(false);
  };
  return (
    <section className="settings-section" aria-labelledby="project-instruction-heading"
      data-project-instruction-section="">
      <h2 id="project-instruction-heading">项目指令</h2>
      {saved === undefined ? null : saved === "unavailable" ? (
        <p role="alert">无法读取项目指令，请稍后再试。</p>
      ) : (
        <form onSubmit={(event) => {
          event.preventDefault();
          if (!saving) void save(saved);
        }}>
          <textarea aria-labelledby="project-instruction-heading" value={text} readOnly={saving}
            data-project-instruction="" onChange={(event) => setText(event.currentTarget.value)} />
          <p className="settings-hint">保存后从下一次请求开始生效，进行中的请求不受影响。</p>
          <button type="submit" disabled={saving} data-project-instruction-save="">保存</button>
          {outcome === undefined ? null : (
            <p role="status" data-project-instruction-outcome={outcome}>{OUTCOME_SENTENCES[outcome]}</p>
          )}
        </form>
      )}
    </section>
  );
}
