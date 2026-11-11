import { useEffect, useState } from "react";
import type { ReactNode } from "react";

import {
  getResearchArtifact, listResearchSources,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  GetAgentRunResponse, ListResearchSourcesResponse, ResearchArtifactInspect,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { researchArtifactLines, researchSourceLines, settledRun } from "./assistant-run-outcome.ts";
import type { AssistantContext } from "./writing-assistant-panel.tsx";

type ReadContext = Pick<AssistantContext, "scope" | "baseUrl" | "fetchImpl">;

// The research of a settled Run does not change (ADR 0087), so the page reads each Run once; a failed read can repeat.
const sourceReads = new Map<string, Promise<ListResearchSourcesResponse | undefined>>();
const artifactReads = new Map<string, Promise<ResearchArtifactInspect | undefined>>();

function readOnce<T>(cache: Map<string, Promise<T | undefined>>, key: string, read: () => Promise<T | undefined>) {
  let pending = cache.get(key);
  if (pending === undefined) {
    pending = read().catch(() => undefined).then((value) => {
      if (value === undefined) cache.delete(key);
      return value;
    });
    cache.set(key, pending);
  }
  return pending;
}

/** The sources of the Run, when the response has the Scope of the context and the Run. */
function readSources(context: ReadContext, runId: string) {
  const { scope } = context;
  return readOnce(sourceReads, `${scope.owner_user_id}:${scope.project_id}:${runId}`, async () => {
    const listed = await listResearchSources({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
      projectId: scope.project_id, runId });
    return listed.run_id === runId && listed.project_scope.owner_user_id === scope.owner_user_id
      && listed.project_scope.project_id === scope.project_id ? listed : undefined;
  });
}

/** The Research Artifact of the Run, when the response has the Scope of the context and the Run. */
function readArtifact(context: ReadContext, runId: string, researchArtifactId: string) {
  const { scope } = context;
  return readOnce(artifactReads, `${scope.owner_user_id}:${scope.project_id}:${runId}`, async () => {
    const read = await getResearchArtifact({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
      projectId: scope.project_id, researchArtifactId });
    return read.artifact.lineage.run_id === runId && read.project_scope.owner_user_id === scope.owner_user_id
      && read.project_scope.project_id === scope.project_id ? read.artifact : undefined;
  });
}

/** A Run whose decision is a function request or a research synthesis, so that it can have research sources. */
export function researchDecision(run: GetAgentRunResponse): boolean {
  return run.decision.kind === "tool_request" || run.decision.kind === "research_synthesis";
}

/**
 * The research of one settled Run: its source list, then the summary, the claims, and the citation list of its
 * Research Artifact (ADR 0040, ADR 0087). `children` go after the citation list. A failed read shows nothing, so
 * the turn shows only its result sentence.
 */
export function ResearchTurn({ context, run, history = false, children }: {
  context: ReadContext;
  run: GetAgentRunResponse;
  /** A history turn of a synthesis shows its sources only with its Artifact, so a failed Artifact read shows only
   * the result sentence (#1427). */
  history?: boolean;
  children?: ReactNode;
}) {
  const [sources, setSources] = useState<{ runId: string; listed: ListResearchSourcesResponse }>();
  const [artifact, setArtifact] = useState<{ runId: string; read: ResearchArtifactInspect }>();
  const runId = run.run_id;
  const settled = settledRun(run);
  const artifactId = run.decision.kind === "research_synthesis" ? run.decision.research_artifact_id : undefined;
  const { scope, baseUrl, fetchImpl } = context;

  useEffect(() => {
    if (!settled) return;
    let active = true;
    const read = { scope, baseUrl, fetchImpl };
    void readSources(read, runId).then((listed) => {
      if (active && listed !== undefined) setSources({ runId, listed });
    });
    if (artifactId !== undefined) void readArtifact(read, runId, artifactId).then((value) => {
      if (active && value !== undefined) setArtifact({ runId, read: value });
    });
    return () => { active = false; };
  }, [scope.owner_user_id, scope.project_id, baseUrl, fetchImpl, runId, settled, artifactId]);

  const artifactLines = artifact?.runId === runId ? researchArtifactLines(artifact.read) : undefined;
  const withoutArtifact = history && artifactId !== undefined && artifactLines === undefined;
  const sourceLines = sources?.runId === runId && !withoutArtifact ? researchSourceLines(sources.listed,
    run.decision.kind === "tool_request" ? run.decision.decision_id : undefined) : [];
  return <>
    {sourceLines.length > 0
      ? <ul className="assistant-research-sources" aria-label="本次检索的来源" data-assistant-research-sources="">
        {sourceLines.map((line, index) => <li key={index}>{line}</li>)}
      </ul> : null}
    {artifactLines === undefined ? null : <div className="assistant-research-artifact" data-assistant-research-artifact="">
      <p>{artifact!.read.summary}</p>
      <ul aria-label="研究结论">{artifactLines.claims.map((line, index) => <li key={index}>{line}</li>)}</ul>
      <ul aria-label="结论引用的来源" data-assistant-research-citations="">
        {artifactLines.citations.map((line, index) => <li key={index}>{line}</li>)}
      </ul>
      {children}
    </div>}
  </>;
}
