import { useState } from "react";

import type { EditorWriterProjection } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ControlledProjectState, ProjectReadyState } from "./editor-types.ts";
import { takeOverOwnedProjectWriter } from "./take-over-writer.ts";

export function TakeOverWriterButton({
  state,
  writer,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  inFlightRef,
  reopen,
  onReopened,
  onRefused,
}: {
  state: ProjectReadyState;
  writer: Extract<EditorWriterProjection, { kind: "read_only" }>;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  inFlightRef: { current: boolean };
  /** Opens the Project again after the takeover: it reads the Project and the list, then the Chapter and the Session. */
  reopen: () => Promise<ControlledProjectState>;
  onReopened: (next: ControlledProjectState) => void;
  onRefused: () => void;
}) {
  // From the click to the new view, the button is disabled and says that the takeover runs.
  const [running, setRunning] = useState(false);
  const session = state.editor.kind === "editor-read-only-recovery"
    ? state.editor.editor_session
    : undefined;
  if (writer.reason !== "secondary_session" || session === undefined) return null;
  return (
    <button
      type="button"
      data-take-over-writer=""
      disabled={running}
      onClick={() => {
        if (inFlightRef.current) return;
        inFlightRef.current = true;
        setRunning(true);
        void (async () => {
          try {
            const takeover = await takeOverOwnedProjectWriter({
              baseUrl,
              fetchImpl,
              cryptoImpl,
              projectId: state.project.project.project_id,
              editorSessionId: session.editor_session_id,
              observedWriterGeneration: writer.observed_writer_generation,
            });
            if (takeover.result.kind !== "takeover_applied") {
              onRefused();
              return;
            }
            onReopened(await reopen());
          } catch {
            onRefused();
          } finally {
            inFlightRef.current = false;
            setRunning(false);
          }
        })();
      }}
    >
      {running ? "正在接管写作…" : "接管写作"}
    </button>
  );
}
