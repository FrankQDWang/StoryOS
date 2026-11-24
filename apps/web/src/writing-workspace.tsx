import { useEffect, useState, type ReactNode } from "react";

import type { EditorWriterProjection } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { WritingAssistantPanel, type AssistantContext } from "./writing-assistant-panel.tsx";
import type { ProposalDestination } from "./proposal-navigation.ts";
import type { ProposalLocator } from "./block-proposal-display.tsx";

function writerGeneration(writer: EditorWriterProjection | undefined): string | undefined {
  if (writer === undefined) return undefined;
  return writer.kind === "current_writer"
    ? writer.writer_generation
    : writer.observed_writer_generation;
}

/** The value of one parameter of the workspace address, for example `?conversation={conversation_id}`. */
export function addressParameter(name: "conversation" | "chapter"): string | undefined {
  return new URLSearchParams(globalThis.location?.search ?? "").get(name) ?? undefined;
}

/** Removes one parameter from the address, so that a reload does not use it again. */
export function removeAddressParameter(name: "conversation" | "chapter"): void {
  const address = new URL(globalThis.location.href);
  address.searchParams.delete(name);
  globalThis.history.replaceState(globalThis.history.state, "", address);
}

export function WritingWorkspace({
  tree,
  editor,
  writer,
  assistant,
  onOpenedProposal, onNavigateProposal, holdAddressConversation = false,
}: {
  tree: ReactNode;
  editor: ReactNode;
  writer?: EditorWriterProjection | undefined;
  assistant?: AssistantContext;
  onNavigateProposal?: (destination: ProposalDestination) => void;
  onOpenedProposal?: (locator: ProposalLocator) => void;
  /** True while the workspace opens the Chapter of its address. The Conversation of the address waits for that open. */
  holdAddressConversation?: boolean;
}) {
  const [collapsed, setCollapsed] = useState(false);
  const [requestedConversation] = useState(() => addressParameter("conversation"));
  // The panel opens the Conversation of the address one time. A reload then shows the Conversation that the panel
  // records, and not the Conversation of the address again.
  useEffect(() => {
    if (requestedConversation === undefined || holdAddressConversation) return;
    removeAddressParameter("conversation");
  }, [holdAddressConversation]);
  return (
    <section
      className={collapsed ? "workspace agent-is-collapsed" : "workspace"}
      data-writing-workspace=""
      data-writer-kind={writer?.kind}
      data-writer-generation={writerGeneration(writer)}
    >
      <aside className="tree-panel">{tree}</aside>
      <section className="editor-panel">{editor}</section>
      <WritingAssistantPanel
        key={assistant === undefined ? "unbound" : `${assistant.scope.owner_user_id}:${assistant.scope.project_id}`}
        collapsed={collapsed}
        context={assistant}
        requestedConversationId={holdAddressConversation ? undefined : requestedConversation}
        onOpenedProposal={onOpenedProposal}
        onNavigateProposal={onNavigateProposal}
      />
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
