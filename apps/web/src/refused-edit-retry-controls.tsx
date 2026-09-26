import { useState } from "react";
import type { DraftPayloadPosition, RefusedEditDraftInspect }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { selectedDraftBlocks } from "./refused-edit-retry.ts";

export function RefusedEditRetryControls({ draft, disabled, submit }: { draft: RefusedEditDraftInspect; disabled: boolean;
  submit: (from: DraftPayloadPosition, to: DraftPayloadPosition, target: string, start: number, end: number) => Promise<void> }) {
  const [expanded, setExpanded] = useState(false);
  const [from, setFrom] = useState({ block_index: 0, offset: 0 });
  const [to, setTo] = useState({ block_index: 0, offset: 0 });
  const [target, setTarget] = useState("original");
  const [start, setStart] = useState(0), [end, setEnd] = useState(0);
  const [error, setError] = useState("");
  let preview;
  try { preview = selectedDraftBlocks(draft, from, to); } catch { preview = undefined; }
  if (!expanded) return <button type="button" data-draft-retry disabled={disabled} onClick={() => setExpanded(true)}>Retry a range</button>;
  return <form data-draft-retry-form onSubmit={(event) => {
    event.preventDefault(); setError("");
    void submit(from, to, target, start, end).catch((error: unknown) => setError(error instanceof Error ? error.message : "Retry unavailable"));
  }}>
    <p>Select replacement blocks and UTF-16 character boundaries. The end is excluded.</p>
    {(["from", "to"] as const).map((edge) => <fieldset key={edge}><legend>{edge === "from" ? "Start" : "End"}</legend>
      {(["block_index", "offset"] as const).map((field) => <label key={field}>{field === "block_index" ? "Block index" : "Character offset"}
        <input type="number" min="0" step="1" name={`draft-${edge}-${field}`} disabled={disabled}
          value={(edge === "from" ? from : to)[field]} onChange={(event) => {
            const number = event.currentTarget.valueAsNumber;
            const update = edge === "from" ? setFrom : setTo;
            update((value) => ({ ...value, [field]: number }));
          }} /></label>)}
    </fieldset>)}
    {preview?.map((block, index) => <pre key={index} data-retry-preview={block.block_kind}>{block.text}</pre>)}
    <label>Target<select name="draft-retry-target" disabled={disabled} value={target} onChange={(event) => setTarget(event.currentTarget.value)}>
      <option value="original">Original selected sources</option>
      {draft.payload.author_edit_units[0]?.selection_snapshot?.ordered_selection?.sources.map((source, index) =>
        <option key={index} value={String(index)}>{source.owner.kind === "proposal" ? "Proposal" : "Manuscript"}: {source.source_text}</option>)}
    </select></label>
    {target !== "original" ? <fieldset><legend>Current target range</legend>
      <label>Start<input type="number" name="draft-target-from" min="0" step="1" value={start} disabled={disabled}
        onChange={(event) => setStart(event.currentTarget.valueAsNumber)} /></label>
      <label>End<input type="number" name="draft-target-to" min="0" step="1" value={end} disabled={disabled}
        onChange={(event) => setEnd(event.currentTarget.valueAsNumber)} /></label>
    </fieldset> : null}
    <button type="submit" data-draft-retry-submit disabled={disabled || preview === undefined}>Retry selected range</button>
    <button type="button" disabled={disabled} onClick={() => setExpanded(false)}>Cancel</button>
    {error ? <p role="status">{error}</p> : null}
  </form>;
}
