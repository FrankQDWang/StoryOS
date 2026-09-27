import { useEffect, useState } from "react";
import type { DraftPayloadPosition, RefusedEditDraftInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { EditorWorkspace } from "./editor-types.ts";
import { readRetryTarget, selectedDraftBlocks, type RetryTargetRead } from "./refused-edit-retry.ts";

export function RefusedEditRetryControls({ draft, workspace, baseUrl, fetchImpl, disabled, submit }: {
  draft: RefusedEditDraftInspect; workspace: EditorWorkspace; baseUrl: string; fetchImpl: typeof fetch; disabled: boolean;
  submit: (from: DraftPayloadPosition, to: DraftPayloadPosition, target: string, start: number, end: number, read: RetryTargetRead) => Promise<void> }) {
  const [expanded, setExpanded] = useState(false);
  const [from, setFrom] = useState({ block_index: 0, offset: 0 }), [to, setTo] = useState({ block_index: 0, offset: 0 });
  const [target, setTarget] = useState("original"), [start, setStart] = useState(0), [end, setEnd] = useState(0);
  const [targetRead, setTargetRead] = useState<RetryTargetRead>();
  const [error, setError] = useState("");
  useEffect(() => {
    if (!expanded) return;
    let active = true; setTargetRead(undefined); setStart(0); setEnd(0); setError("");
    void readRetryTarget(workspace, draft, target, baseUrl, fetchImpl).then((read) => {
      if (active) setTargetRead(read);
    }).catch(() => { if (active) setError("Retry target unavailable"); });
    return () => { active = false; };
  }, [expanded, target, draft, workspace, baseUrl, fetchImpl]);
  const primitive = draft.payload.author_edit_units[0]?.normalized_primitives[0];
  const blocks = primitive?.kind === "replace_structured_selection" ? primitive.replacement : [];
  const text = blocks.map((block) => block.text).join("\n");
  const position = (offset: number): DraftPayloadPosition => {
    let beginning = 0;
    for (const [block_index, block] of blocks.entries()) {
      if (offset <= beginning + block.text.length) return { block_index, offset: offset - beginning };
      beginning += block.text.length + 1;
    }
    throw new Error("Select a valid Draft range");
  };
  let preview;
  try { preview = selectedDraftBlocks(draft, from, to); } catch { preview = undefined; }
  if (!expanded) return <button type="button" data-draft-retry disabled={disabled} onClick={() => setExpanded(true)}>Retry a range</button>;
  return <form data-draft-retry-form onSubmit={(event) => {
    event.preventDefault(); setError("");
    if (targetRead) void submit(from, to, target, start, end, targetRead)
      .catch((error: unknown) => setError(error instanceof Error ? error.message : "Retry unavailable"));
  }}>
    <label>Select text to retry<textarea name="draft-range-text" readOnly disabled={disabled} value={text}
      onSelect={(event) => { setFrom(position(event.currentTarget.selectionStart)); setTo(position(event.currentTarget.selectionEnd)); }} /></label>
    <p>Selected replacement</p>
    {preview?.map((block, index) => <pre key={index} data-retry-preview={block.block_kind}>{block.text}</pre>)}
    <label>Target<select name="draft-retry-target" disabled={disabled} value={target} onChange={(event) => setTarget(event.currentTarget.value)}>
      <option value="original">Original selected sources</option>
      {draft.payload.author_edit_units[0]?.selection_snapshot?.ordered_selection?.sources.map((source, index) =>
        <option key={index} value={String(index)}>{source.owner.kind === "proposal" ? "Proposal" : "Manuscript"}: {source.source_text}</option>)}
    </select></label>
    {target !== "original" && targetRead ? <label>Select current text to replace<textarea name="draft-target-text" readOnly
      disabled={disabled} value={targetRead.text} onSelect={(event) => {
        setStart(event.currentTarget.selectionStart); setEnd(event.currentTarget.selectionEnd);
      }} /></label> : null}
    <button type="submit" data-draft-retry-submit disabled={disabled || preview === undefined || targetRead === undefined}>Retry selected range</button>
    <button type="button" disabled={disabled} onClick={() => setExpanded(false)}>Cancel</button>
    {error ? <p role="status">{error}</p> : null}
  </form>;
}
