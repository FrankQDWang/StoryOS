#!/usr/bin/env python3
"""Inject the project stop-gate reminder at session start.

Also keeps hooks.json from containing only a `stop` hook, which has
failed to load in some Cursor versions.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

Path(".cursor/hooks/state").mkdir(parents=True, exist_ok=True)
sys.stdin.read()
sys.stdout.write(
    json.dumps(
        {
            "additional_context": (
                "Stop gate for this project: end a turn only when you need the "
                "user's authorization, need the user's judgment, or the user "
                "already authorized ending the task. Write "
                "`.cursor/hooks/state/pending.json` with allow_stop="
                "awaiting_authorization | awaiting_judgment | user_authorized_end "
                "before ending. Otherwise keep working via the Ask Matt skill."
            )
        },
        ensure_ascii=False,
    )
    + "\n"
)
