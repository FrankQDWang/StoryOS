import { RECOVERY_PAGE_SIZE } from "./local-recovery-record.ts";
import { useEffect, useState } from "react";
import type { EditorWorkspace } from "./editor-types.ts";
import { readLocalRecovery, retainLocalRecovery, type LocalRecoveryMaterial } from "./local-edit-recovery.ts";

const reasons = {
  admission_expired: "原写入请求已过期",
  binding_changed: "原写入位置或会话已改变",
  direct_edit_intent_unrecoverable: "服务器无法恢复原写入请求",
};

export function LocalRecoveryPanel({ workspace, refreshKey, onContinue }: {
  workspace: EditorWorkspace;
  refreshKey: string;
  onContinue: () => Promise<void>;
}) {
  const [materials, setMaterials] = useState<LocalRecoveryMaterial[]>([]);
  const [failure, setFailure] = useState<string>();
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let active = true;
    void readLocalRecovery(workspace).then((next) => { if (active) setMaterials(next); })
      .catch(() => { if (active) setFailure("无法读取本地恢复内容"); });
    return () => { active = false; };
  }, [workspace, refreshKey]);
  if (materials.length === 0 && !failure) return null;
  return <section aria-label="本地恢复内容" data-local-recovery="">
    <h3>本地恢复内容</h3>
    <p>这些修改没有写入服务器正文。内容保留在此浏览器中，可查看和复制。</p>
    {materials.map((material) => {
      const settlement = material.group.settlement;
      if (settlement.kind !== "outcome_query_requires_reconfirmation") return null;
      return <article key={material.group.journal_submission_group_id}>
        <p>{material.group.frozen_request_body.proposal_target ? "候选修改" : "正文修改"} · {reasons[settlement.reconfirmation_reason]}</p>
        <pre data-local-recovery-text="">{material.text}</pre>
        <button type="button" data-local-recovery-copy="" onClick={() => {
          void navigator.clipboard.writeText(material.text).catch(() => setFailure("复制失败，请选择上方完整内容复制"));
        }}>复制完整内容</button>
        {material.disposition === "awaiting_author_choice" ? <>
          <p>继续后，编辑区显示当前服务器正文；上方未应用修改仍会保留，不会自动重试。请自行选择位置重新输入。</p>
          <button type="button" data-local-recovery-continue="" disabled={busy} onClick={() => {
            setBusy(true);
            void retainLocalRecovery(workspace, material.group.journal_submission_group_id)
              .then(onContinue).catch(() => setFailure("无法继续编辑，恢复内容仍保留"))
              .finally(() => setBusy(false));
          }}>保留恢复内容并继续当前正文</button>
        </> : <p>已保留供手动重新输入；原修改未标记为已保存。</p>}
      </article>;
    })}
    {materials.filter((item) => item.disposition === "retained_for_manual_reentry").length === RECOVERY_PAGE_SIZE
      ? <button type="button" onClick={() => { void readLocalRecovery(workspace, materials.at(-1)).then(setMaterials).catch(() => setFailure("无法读取更多恢复内容")); }}>更多恢复内容</button> : null}
    <button type="button" onClick={() => { void readLocalRecovery(workspace).then(setMaterials).catch(() => setFailure("无法读取恢复内容")); }}>返回恢复内容首页</button>
    {failure ? <p role="alert">{failure}</p> : null}
  </section>;
}
