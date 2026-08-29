import { useState, type ReactNode } from "react";

import type { EditorWriterProjection } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { WritingAssistantPanel } from "./writing-assistant-panel.tsx";

function writerGeneration(writer: EditorWriterProjection | undefined): string | undefined {
  if (writer === undefined) return undefined;
  return writer.kind === "current_writer"
    ? writer.writer_generation
    : writer.observed_writer_generation;
}

export function WritingWorkspace({
  tree,
  editor,
  writer,
}: {
  tree: ReactNode;
  editor: ReactNode;
  writer?: EditorWriterProjection | undefined;
}) {
  const [collapsed, setCollapsed] = useState(false);
  return (
    <section
      className={collapsed ? "workspace agent-is-collapsed" : "workspace"}
      data-writing-workspace=""
      data-writer-kind={writer?.kind}
      data-writer-generation={writerGeneration(writer)}
    >
      <aside className="tree-panel">{tree}</aside>
      <section className="editor-panel">{editor}</section>
      <WritingAssistantPanel collapsed={collapsed} />
      <button
        type="button"
        className="assistant-toggle"
        data-assistant-toggle=""
        aria-controls="writing-assistant-panel"
        aria-expanded={!collapsed}
        aria-label={collapsed ? "展开写作助手" : "收起写作助手"}
        onClick={() => {
          setCollapsed((value) => !value);
        }}
      >
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <rect
            x="3.5"
            y="4.5"
            width="13"
            height="11"
            rx="2"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          />
          <path d="M8.5 4.5v11" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
    </section>
  );
}
