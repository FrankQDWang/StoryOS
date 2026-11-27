import { getEditorSession } from "../../../generated/typescript/storyos-public-release-1/client.mjs";

import type { ControlledProjectState } from "./editor-types.ts";
import { takeOverOwnedProjectWriter } from "./take-over-writer.ts";

/**
 * Takes over the writer for the Editor Session `editorSessionId` and opens the Project again (ADR 0106 section 1). With
 * no `observedWriterGeneration`, it first reads the Session for it: the refusal of an Author Edit gives no generation.
 * The result is undefined when the Server applied no takeover.
 */
export async function takeOverWriter(options: {
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  projectId: string;
  editorSessionId: string;
  observedWriterGeneration: string | undefined;
  reopen: () => Promise<ControlledProjectState>;
}): Promise<ControlledProjectState | undefined> {
  const { baseUrl, fetchImpl, projectId, editorSessionId } = options;
  let observed = options.observedWriterGeneration;
  if (observed === undefined) {
    const read = await getEditorSession({ baseUrl, projectId, editorSessionId, fetchImpl });
    if (read.project_scope.project_id !== projectId || read.editor_session.editor_session_id !== editorSessionId
      || read.writer.kind !== "read_only") return undefined;
    observed = read.writer.observed_writer_generation;
  }
  const takeover = await takeOverOwnedProjectWriter({ baseUrl, fetchImpl, cryptoImpl: options.cryptoImpl, projectId,
    editorSessionId, observedWriterGeneration: observed });
  return takeover.result.kind === "takeover_applied" ? options.reopen() : undefined;
}

export function TakeOverWriterButton({ running, onClick }: { running: boolean; onClick: () => void }) {
  return (
    <button type="button" data-take-over-writer="" disabled={running} onClick={onClick}>
      {running ? "正在接管写作…" : "接管写作"}
    </button>
  );
}
