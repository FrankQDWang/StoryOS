#!/usr/bin/env python3
"""Project stop gate for StoryOS agent sessions.

Cursor cannot hard-block a stop. This hook converts a silent "I am done"
into an automatic follow-up, unless the agent first wrote a pending stop
gate for one of the two allowed conditions: authorization or judgment.

Allowed `allow_stop` values in `.cursor/hooks/state/pending.json`
(or `.cursor/hooks/state/<conversation_id>.json`):

- awaiting_authorization: wait for the user to authorize a specific act
- awaiting_judgment: wait for the user to choose among stated options
- user_authorized_end: the user already authorized ending this task
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any

STATE_DIR = Path(".cursor/hooks/state")
PENDING_NAME = "pending.json"
SAFE_ID = re.compile(r"^[A-Za-z0-9._-]+$")
ALLOWED_STOP = frozenset(
    {
        "awaiting_authorization",
        "awaiting_judgment",
        "user_authorized_end",
    }
)
NEAR_LIMIT = 12


def emit(payload: dict[str, Any]) -> int:
    sys.stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
    return 0


def followup_message(conversation_id: str, loop_count: int) -> str:
    gate_path = f".cursor/hooks/state/{conversation_id}.json"
    near_limit = loop_count >= NEAR_LIMIT
    lead = (
        "上一轮在未通过停止门的情况下结束了。不要把工作当成已经完成。"
        if not near_limit
        else (
            f"上一轮再次在未通过停止门的情况下结束了（自动续跑已 {loop_count} 次）。"
            "这一轮不要继续埋头干活。先判定现在卡住的是授权还是判断，然后停下来等用户。"
        )
    )
    work_step = (
        ""
        if near_limit
        else (
            "2. 阅读并遵循 Ask Matt skill，按当前工作状态路由，并立刻做下一步能做的事。\n"
        )
    )
    next_index = 2 if near_limit else 3
    return f"""{lead}

停止门只有两种合法原因，外加一种用户已经点头结束的情况：

- 授权：下一步必须得到用户明确同意，否则不能做。
- 判断：下一步必须由用户在选项中选择，你不能代选。
- 用户已授权结束：用户刚刚明确允许结束本任务。

流程：
1. 先判断这一轮为什么要停。
{work_step}{next_index}. 若工作仍可在没有用户决定的情况下继续：继续做，不要收工。
{next_index + 1}. 若现在需要授权或判断：先写入停止门文件，再向用户提问，然后结束这一轮。提问必须用中文，并包含：
   - 授权：要授权的具体事项；不授权时你会怎么做；为什么现在必须等。
   - 判断：背景；选项；推荐项；推荐理由。
{next_index + 2}. 只有用户刚刚明确允许结束本任务时，才写入 user_authorized_end。

先写下面两个路径之一，再结束这一轮（JSON 一行即可）：
- `{gate_path}`
- `.cursor/hooks/state/pending.json`

```json
{{
  "allow_stop": "awaiting_authorization",
  "conversation_id": "{conversation_id}",
  "what": "需要用户授权或判断的那一件事",
  "background": "判断时必填：背景",
  "options": ["判断时必填：选项 A", "选项 B"],
  "recommendation": "判断时必填：推荐项",
  "why": "判断时必填：推荐理由",
  "user_quote": "仅 user_authorized_end：用户原话"
}}
```

`allow_stop` 只能是 awaiting_authorization、awaiting_judgment 或 user_authorized_end。
"""


def load_gate(path: Path) -> dict[str, Any] | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    if not isinstance(data, dict):
        return None
    allow_stop = data.get("allow_stop")
    if allow_stop not in ALLOWED_STOP:
        return None
    return data


def gate_path(conversation_id: str) -> Path | None:
    specific = (
        STATE_DIR / f"{conversation_id}.json"
        if conversation_id != "unknown" and SAFE_ID.match(conversation_id)
        else None
    )
    if specific is not None and specific.is_file():
        return specific
    pending = STATE_DIR / PENDING_NAME
    if pending.is_file():
        return pending
    return None


def consume(path: Path) -> None:
    try:
        path.unlink()
    except OSError:
        pass


def main() -> int:
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError:
        return emit({})
    if not isinstance(payload, dict):
        return emit({})

    status = payload.get("status")
    if status == "aborted":
        return emit({})

    conversation_id = payload.get("conversation_id") or payload.get("session_id")
    if not isinstance(conversation_id, str) or not conversation_id.strip():
        conversation_id = "unknown"
    conversation_id = conversation_id.strip()

    loop_count = payload.get("loop_count", 0)
    if not isinstance(loop_count, int) or loop_count < 0:
        loop_count = 0

    path = gate_path(conversation_id)
    if path is not None and load_gate(path) is not None:
        consume(path)
        return emit({})

    return emit(
        {
            "followup_message": followup_message(conversation_id, loop_count),
        }
    )


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:
        raise SystemExit(emit({}))
