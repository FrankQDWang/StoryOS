#!/usr/bin/env python3
"""Inject the project stop-gate reminder at session start.

Tells the agent its conversation id so it can write the per-conversation
gate file that `on-stop.py` prefers. Also removes stale gate files and
keeps hooks.json from containing only a `stop` hook, which has failed to
load in some Cursor versions.
"""

from __future__ import annotations

import json
import re
import sys
import time
from pathlib import Path

STATE_DIR = Path(__file__).resolve().parent / "state"
SAFE_ID = re.compile(r"^[A-Za-z0-9._-]+$")
STALE_SECONDS = 7 * 24 * 60 * 60


def main() -> int:
    try:
        raw = sys.stdin.read()
        payload = json.loads(raw) if raw.strip() else {}
    except json.JSONDecodeError:
        payload = {}
    if not isinstance(payload, dict):
        payload = {}

    raw_id = payload.get("conversation_id") or payload.get("session_id")
    conversation_id = (
        raw_id.strip()
        if isinstance(raw_id, str) and SAFE_ID.match(raw_id.strip())
        else None
    )

    STATE_DIR.mkdir(parents=True, exist_ok=True)
    now = time.time()
    for stale in STATE_DIR.glob("*.json"):
        try:
            if now - stale.stat().st_mtime > STALE_SECONDS:
                stale.unlink()
        except OSError:
            pass

    if conversation_id is not None:
        gate_line = (
            f"Write `.cursor/hooks/state/{conversation_id}.json` "
            f'(your conversation id is "{conversation_id}") '
        )
    else:
        gate_line = (
            "Write `.cursor/hooks/state/pending.json` "
            "(no conversation id is available for this session) "
        )
    sys.stdout.write(
        json.dumps(
            {
                "additional_context": (
                    "Stop gate for this project: end a turn only when you "
                    "need the user's authorization, need the user's "
                    "judgment, or the user already authorized ending the "
                    f"task. {gate_line}with allow_stop="
                    "awaiting_authorization | awaiting_judgment | "
                    "user_authorized_end before ending. Otherwise keep "
                    "working via the Ask Matt skill, but only inside a "
                    "claimed GitHub Issue execution contract; without one, "
                    "list candidate work and stop for the user's judgment."
                )
            },
            ensure_ascii=False,
        )
        + "\n"
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:
        sys.stdout.write("{}\n")
        raise SystemExit(0)
