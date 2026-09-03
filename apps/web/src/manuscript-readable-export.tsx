import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";

import {
  getHumanReadableManuscriptExport,
  type GetHumanReadableManuscriptExportResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requestOwnedHumanReadableExport } from "./export-human-readable.ts";

type ReadyExportPage = Extract<GetHumanReadableManuscriptExportResponse, { status: "ready" }>;

type ExportOutcome =
  | { kind: "in_progress"; exportId: string; exportProfile: string }
  | { kind: "ready"; page: ReadyExportPage; downloadSha256: string }
  | { kind: "failed" }
  | { kind: "outcome_unknown" }
  | { kind: "unavailable" };

export function ManuscriptReadableExportPanel({
  projectId,
  baseUrl,
  fetchImpl,
  cryptoImpl,
}: {
  projectId: string;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
}) {
  const [busy, setBusy] = useState(false);
  const [outcome, setOutcome] = useState<ExportOutcome | undefined>(undefined);
  const pollGeneration = useRef(0);

  useEffect(() => {
    return () => {
      pollGeneration.current += 1;
    };
  }, []);

  async function requestExport(): Promise<void> {
    const generation = pollGeneration.current + 1;
    pollGeneration.current = generation;
    setBusy(true);
    try {
      const accepted = await requestOwnedHumanReadableExport({
        baseUrl,
        fetchImpl,
        cryptoImpl,
        projectId,
      });
      const exportId =
        accepted.operation_ref?.kind === "human_readable_manuscript_export"
          ? accepted.operation_ref.export_id
          : undefined;
      if (exportId === undefined) {
        setOutcome({ kind: "unavailable" });
        return;
      }
      setBusy(false);
      await pollExport(exportId, generation);
    } catch {
      setOutcome({ kind: "unavailable" });
    } finally {
      if (pollGeneration.current === generation) {
        setBusy(false);
      }
    }
  }

  async function pollExport(exportId: string, generation: number): Promise<void> {
    while (pollGeneration.current === generation) {
      const page = await getHumanReadableManuscriptExport({
        baseUrl,
        projectId,
        exportId,
        fetchImpl,
      });
      if (pollGeneration.current !== generation) {
        return;
      }
      if (page.status === "ready") {
        setOutcome({ kind: "ready", page, downloadSha256: "" });
        return;
      }
      if (page.status === "in_progress") {
        setOutcome({
          kind: "in_progress",
          exportId: page.export_id,
          exportProfile: page.export_profile,
        });
        await new Promise<void>((resolve) => {
          window.setTimeout(resolve, 250);
        });
        continue;
      }
      if (page.status === "failed") {
        setOutcome({ kind: "failed" });
        return;
      }
      if (page.status === "outcome_unknown") {
        setOutcome({ kind: "outcome_unknown" });
        return;
      }
      setOutcome({ kind: "unavailable" });
      return;
    }
  }

  async function downloadExport(): Promise<void> {
    if (outcome === undefined || outcome.kind !== "ready") return;
    setBusy(true);
    try {
      const page = await getHumanReadableManuscriptExport({
        baseUrl,
        projectId,
        exportId: outcome.page.export_id,
        fetchImpl,
      });
      if (page.status !== "ready") {
        setOutcome({ kind: "unavailable" });
        return;
      }
      flushSync(() => {
        setOutcome({ kind: "ready", page, downloadSha256: page.content_sha256 });
      });
      const blob = new Blob([page.manuscript_utf8], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "manuscript.txt";
      anchor.rel = "noopener";
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      URL.revokeObjectURL(url);
      window.focus();
    } catch {
      setOutcome({ kind: "unavailable" });
    } finally {
      setBusy(false);
    }
  }

  return (
    <section data-readable-export="" data-export-outcome={outcome?.kind ?? "idle"}>
      <button type="button" disabled={busy} onClick={() => void requestExport()}>
        导出可读稿件
      </button>
      {outcome?.kind === "unavailable" ? <p>无法导出可读稿件。</p> : null}
      {outcome?.kind === "failed" ? <p>可读稿件导出失败。</p> : null}
      {outcome?.kind === "outcome_unknown" ? <p>可读稿件导出结果未知。</p> : null}
      {outcome?.kind === "in_progress" ? (
        <p data-export-id={outcome.exportId} data-export-profile={outcome.exportProfile}>
          可读稿件正在导出。
        </p>
      ) : null}
      {outcome?.kind === "ready" ? (
        <>
          <pre data-readable-export-bytes="">{outcome.page.manuscript_utf8}</pre>
          <p
            data-export-sha256={outcome.page.content_sha256}
            data-export-profile={outcome.page.export_profile}
            data-export-id={outcome.page.export_id}
            data-export-download-sha256={outcome.downloadSha256}
          >
            导出已就绪。
          </p>
          <button type="button" disabled={busy} onClick={() => void downloadExport()}>
            下载
          </button>
        </>
      ) : null}
    </section>
  );
}
