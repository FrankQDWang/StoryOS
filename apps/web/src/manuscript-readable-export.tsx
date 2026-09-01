import { useState } from "react";

import {
  getHumanReadableManuscriptExport,
  type GetHumanReadableManuscriptExportResponse,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { requestOwnedHumanReadableExport } from "./export-human-readable.ts";

type ExportOutcome =
  | { kind: "ready"; page: GetHumanReadableManuscriptExportResponse; downloadSha256: string }
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

  async function requestExport(): Promise<void> {
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
      const page = await getHumanReadableManuscriptExport({
        baseUrl,
        projectId,
        exportId,
        fetchImpl,
      });
      setOutcome({ kind: "ready", page, downloadSha256: "" });
    } catch {
      setOutcome({ kind: "unavailable" });
    } finally {
      setBusy(false);
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
      const blob = new Blob([page.manuscript_utf8], { type: "text/plain;charset=utf-8" });
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = "manuscript.txt";
      anchor.click();
      URL.revokeObjectURL(url);
      setOutcome({ kind: "ready", page, downloadSha256: page.content_sha256 });
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
