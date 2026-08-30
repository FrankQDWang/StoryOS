import type { EditorWriterProjection } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openControlledProject } from "./boot.ts";
import type { ControlledProjectState, ProjectReadyState } from "./editor-types.ts";
import { takeOverOwnedProjectWriter } from "./take-over-writer.ts";

export function TakeOverWriterButton({
  state,
  writer,
  baseUrl,
  fetchImpl,
  cryptoImpl,
  inFlightRef,
  onReopened,
  onRefused,
}: {
  state: ProjectReadyState;
  writer: Extract<EditorWriterProjection, { kind: "read_only" }>;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  inFlightRef: { current: boolean };
  onReopened: (next: ControlledProjectState) => void;
  onRefused: () => void;
}) {
  const session = state.editor.kind === "editor-read-only-recovery"
    ? state.editor.editor_session
    : undefined;
  if (writer.reason !== "secondary_session" || session === undefined) return null;
  return (
    <button
      type="button"
      data-take-over-writer=""
      onClick={() => {
        if (inFlightRef.current) return;
        inFlightRef.current = true;
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
            onReopened(await openControlledProject({
              baseUrl,
              projectId: state.project.project.project_id,
              fetchImpl,
              cryptoImpl,
            }));
          } catch {
            onRefused();
          } finally {
            inFlightRef.current = false;
          }
        })();
      }}
    >
      接管写作
    </button>
  );
}
