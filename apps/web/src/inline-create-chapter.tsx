import { useRef, useState } from "react";
import type { CreateChapterPlacement } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { createOwnedChapter } from "./create-chapter.ts";
import { HISTORICAL_ACKNOWLEDGEMENT_MESSAGE, historicalAcknowledgementUnavailable } from "./historical-acknowledgement.ts";

export function InlineCreateChapter(props: {
  projectId: string; volumeId: string; treeRevision: string; placement?: CreateChapterPlacement | undefined;
  baseUrl: string; fetchImpl: typeof fetch; cryptoImpl: Crypto; onCreated: () => void; onCancel: () => void;
}) {
  const pending = useRef(false);
  const cancelled = useRef(false);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const submit = (title: string) => {
    if (!title.trim() || pending.current || cancelled.current) return;
    pending.current = true; setSaving(true); setError("");
    void createOwnedChapter({ ...props, title: title.trim(), expectedTreeRevision: props.treeRevision })
      .then((created) => {
        if (created.effect.kind === "authoritative_applied") props.onCreated();
        else setError(created.effect.kind === "conflicted" ? "目录已改变，请取消后重新选择位置。" : "无法在此位置创建章，请检查目录。" );
      }).catch((cause: unknown) => {
        setError(historicalAcknowledgementUnavailable(cause) ? HISTORICAL_ACKNOWLEDGEMENT_MESSAGE : "创建未确认，标题已保留。请按 Enter 重试。");
      }).finally(() => { pending.current = false; setSaving(false); });
  };
  return <li className="inline-chapter-creation">
    <form data-create-chapter={props.volumeId} onSubmit={(event) => {
      event.preventDefault(); submit(String(new FormData(event.currentTarget).get("chapter-title") ?? ""));
    }}>
      <input autoFocus name="chapter-title" aria-label="章标题" placeholder="章标题" maxLength={1024}
        readOnly={saving} onBlur={(event) => submit(event.currentTarget.value)} onKeyDown={(event) => {
          if (event.key === "Escape" && !pending.current) { cancelled.current = true; props.onCancel(); }
        }} />
      {error ? <p data-create-chapter-error role="alert">{error}</p> : null}
    </form>
  </li>;
}
