import { useEffect, useState } from "react";

import { listProjectConversations } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type {
  ListProjectConversationsResponse, ProjectScope,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";

const CONVERSATION_LIST_UNAVAILABLE_SENTENCE = "无法读取历史对话，请稍后再试。";

/**
 * The Project Conversations of the Project, the newest first (ADR 0094 section 10). The list reads the Host each time
 * it opens. An entry has no rename control, no delete control, and no other control of a Conversation.
 */
export function ProjectConversationList({ scope, baseUrl, fetchImpl, currentConversationId, disabled, onSelect }: {
  scope: ProjectScope;
  baseUrl: string;
  fetchImpl: typeof fetch;
  currentConversationId: string | undefined;
  disabled: boolean;
  onSelect: (conversationId: string) => void;
}) {
  const [list, setList] = useState<ListProjectConversationsResponse>();
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    void listProjectConversations({ baseUrl, fetchImpl, projectId: scope.project_id }).then((response) => {
      if (!active) return;
      if (response.project_scope.owner_user_id === scope.owner_user_id
        && response.project_scope.project_id === scope.project_id) setList(response);
      else setFailed(true);
    }, () => {
      if (active) setFailed(true);
    });
    return () => { active = false; };
  }, [baseUrl, fetchImpl, scope.owner_user_id, scope.project_id]);
  const time = new Intl.DateTimeFormat("zh-CN", { dateStyle: "medium", timeStyle: "short" });
  return <section className="assistant-conversation-list" aria-label="历史对话" data-assistant-conversation-list="">
    {failed ? <p role="status">{CONVERSATION_LIST_UNAVAILABLE_SENTENCE}</p> : null}
    {list?.conversations.length === 0 ? <p className="assistant-status">还没有对话。</p> : null}
    {list === undefined || list.conversations.length === 0 ? null : <ul>
      {list.conversations.map((entry) => {
        const current = entry.conversation_id === currentConversationId;
        return <li key={entry.conversation_id}>
          <button type="button" data-assistant-conversation-entry={entry.conversation_id} disabled={disabled}
            aria-current={current ? "true" : undefined} onClick={() => onSelect(entry.conversation_id)}>
            <span className="assistant-conversation-label">{entry.label}</span>
            <time dateTime={entry.newest_run_created_at}>{time.format(new Date(entry.newest_run_created_at))}</time>
            {current ? <span className="assistant-conversation-current">当前</span> : null}
          </button>
        </li>;
      })}
    </ul>}
    {list?.completeness === "truncated" ? <p className="assistant-status">只显示最近的对话。</p> : null}
  </section>;
}
