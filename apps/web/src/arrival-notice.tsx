import { useState } from "react";

import type { ManuscriptParagraph } from "./manuscript-doc.ts";

/** The text of an Arriving Chapter with Arrival Input that did not go to the writing controller (ADR 0105). */
export interface UnsavedArrival {
  readonly chapterId: string;
  readonly blocks: readonly ManuscriptParagraph[];
}

/** The notice of the text that is not saved. It stays until the author closes it (ADR 0105, section 4). */
export function ArrivalNotice({ unsaved, onClose }: { unsaved: UnsavedArrival; onClose: () => void }) {
  const [copied, setCopied] = useState(false);
  const text = unsaved.blocks.map((block) => block.text).join("\n");
  const copy = () => {
    void Promise.resolve()
      .then(() => navigator.clipboard.writeText(text))
      .then(() => { setCopied(true); }, () => {});
  };
  return (
    <section role="status" className="editor-recovery arrival-notice" data-arrival-notice={unsaved.chapterId}>
      <div className="editor-recovery-heading">
        <h3>这段文字没有保存</h3>
        <p>切换章节时输入的文字没有保存到这一章。请复制这段文字，再粘贴到需要的位置。</p>
      </div>
      <pre data-arrival-notice-text="">{text}</pre>
      <button type="button" data-arrival-notice-copy="" onClick={copy}>{copied ? "已复制" : "复制文字"}</button>
      <button type="button" data-arrival-notice-close="" onClick={onClose}>关闭</button>
    </section>
  );}
