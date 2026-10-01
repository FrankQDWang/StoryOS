import { useState } from "react";

export function AssistantComposer({ enabled, onSend }: {
  enabled: boolean;
  onSend: (message: string, acknowledged: () => void) => void;
}) {
  const [message, setMessage] = useState("");
  return <div className="composer-dock">
    <form className="composer" data-writing-assistant-composer="" onSubmit={(event) => {
      event.preventDefault();
      if (enabled && message.trim().length > 0) onSend(message.trim(), () => setMessage(""));
    }}>
      <textarea name="assistant-message" aria-label="给写作助手的消息" rows={2} maxLength={4000}
        value={message} onChange={(event) => setMessage(event.currentTarget.value)}
        placeholder="描述想修改的当前章节文字" />
      <div className="composer-controls">
        <button className="send-button" type="submit" aria-label="发送"
          disabled={!enabled || message.trim().length === 0}>↑</button>
      </div>
    </form>
  </div>;
}
