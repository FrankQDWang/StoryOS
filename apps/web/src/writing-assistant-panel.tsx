import { useState } from "react";

export function WritingAssistantPanel({ collapsed }: { collapsed: boolean }) {
  const [dispatch, setDispatch] = useState<"idle" | "refused">("idle");
  return (
    <aside
      id="writing-assistant-panel"
      className="agent-panel"
      data-writing-assistant=""
      data-assistant-availability="unavailable"
      data-assistant-dispatch={dispatch}
      aria-label={collapsed ? "写作助手已收起" : "写作助手对话"}
    >
      <div className="assistant-body" hidden={collapsed}>
        <header className="agent-header">
          <strong>写作助手</strong>
        </header>
        <p className="assistant-status" role="status">
          写作助手当前不可用。你仍可以直接写作。
        </p>
        <form
          className="composer"
          data-writing-assistant-composer=""
          onSubmit={(event) => {
            event.preventDefault();
            setDispatch("refused");
          }}
        >
          <input name="assistant-message" aria-label="给写作助手的消息" />
          <button type="submit">发送</button>
        </form>
      </div>
    </aside>
  );
}
