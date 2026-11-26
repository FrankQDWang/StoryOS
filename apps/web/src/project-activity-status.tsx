import { useEffect, useRef, useState } from "react";

import type { EditorReadyState, ProjectActivityEvent } from "./editor-types.ts";
import {
  consumeOwnedProjectActivity,
  type ProjectActivitySyncResult,
} from "./project-activity-sync.ts";

export function ProjectActivityStatus({
  workspace,
  baseUrl,
  fetchImpl,
  revisionKey,
  onUnavailable,
  onEvents,
}: {
  workspace: EditorReadyState | undefined;
  baseUrl: string;
  fetchImpl: typeof fetch;
  revisionKey: string;
  onUnavailable: () => void;
  /** Receives the events of each consumed result. */
  onEvents?: ((events: readonly ProjectActivityEvent[]) => void) | undefined;
}) {
  const onUnavailableRef = useRef(onUnavailable);
  onUnavailableRef.current = onUnavailable;
  const onEventsRef = useRef(onEvents);
  onEventsRef.current = onEvents;
  const [result, setResult] = useState<ProjectActivitySyncResult>();
  useEffect(() => {
    if (workspace === undefined) return;
    const controller = new AbortController();
    void consumeOwnedProjectActivity(workspace, {
      baseUrl, fetchImpl, signal: controller.signal,
    }).then((next) => {
      if (controller.signal.aborted) return;
      setResult(next);
      if (next.kind === "unavailable") onUnavailableRef.current();
      else onEventsRef.current?.(next.ingest.events);
    }).catch(() => {
      if (!controller.signal.aborted) onUnavailableRef.current();
    });
    return () => controller.abort();
  }, [workspace, baseUrl, fetchImpl, revisionKey]);
  const ingest = result !== undefined && result.kind !== "unavailable" ? result.ingest : undefined;
  const lastEventId = result !== undefined && result.kind !== "unavailable"
    ? result.last_event_id ?? "" : "";
  const resync = result?.kind === "resynchronized" ? "applied"
    : result?.kind === "unavailable" ? "unavailable" : "";
  return (
    <small
      data-activity-replay-generation={ingest?.replay_generation ?? ""}
      data-activity-processed-through={ingest?.processed_through_stream_sequence ?? ""}
      data-activity-last-event-id={lastEventId}
      data-activity-resync={resync}
    >
      {result?.kind === "unavailable" ? "活动流无法同步"
        : ""}
    </small>
  );
}
