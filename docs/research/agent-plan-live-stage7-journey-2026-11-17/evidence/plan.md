**Live run plan of S7-E1b.** The coordinator accepted this plan on 2026-11-18. The first request waits for the confirmation of the coordinator on this issue.

- **Head:** `b0a39fdbe983d2dd1a32e101eaf090f2538af237`. This is `claude/1676-s7-e1b` with the merged `stage7-memory` tip `d87e1781`, which has the fix of #1685 (PR #1687) and S7-D7.
- **Package:** built from the source commit `b0a39fdbe983d2dd1a32e101eaf090f2538af237`, tree `71842c82`, manifest `sha256:8896f27471b2d65e585519a1e34a260bebd6737e237db5d0740fa8ea31aa48e4`. The package is clean.
- **Database:** a new dev-postgres database. The key reaches the request only through the typed settings. `STORYOS_EMBEDDING_DESTINATION=volcengine_agent_plan`.
- **Sample:** chapters 01 to 03 of `fixtures/memory-calibration`, typed through the editor before step 1. Later, `chapter-03-changed`.

**Statements of the test administrator connection.** Each statement is `UPDATE storyos.memory_work_schedules SET due_at = now()` for the Project and the kind, and also for the source when it changes 1 row:

| Statement | Rows | When |
| --- | --- | --- |
| C1, Conversation A | 1 | After step 1. The quiet time is 30 minutes. |
| A1, the 3 Chapter rows | 3 | No row is held: the commits occur minutes before the Run of step 1 completes. The rate is 2 minutes. |
| B1, the consolidation row | 1 | Only after P1 settles. While P1 is in progress, the claim gives `pass_in_progress` and an empty due time. |
| A2, Chapter 3 after the change | 1 | The interval is 6 hours. |
| B2, the consolidation row | 1 | After the capture of the changed Chapter 3 settles. |

That is 5 statements and 7 rows. Any other row count is a stop.

- **Passes:** P1 starts with no statement, after the first capture that has observations. B1 starts P2, and B2 starts P3.
- **Index work:** the quiet time is 10 minutes, and no statement changes it. Step 3 needs a passage of Chapter 1 or Chapter 2.

**Requests (planned / at most):**

| Part | Model requests |
| --- | --- |
| Step 1 | 3 / 5 |
| Captures | 9 / 15 |
| Passes | 3 / 5 |
| Step 3 | 6 / 10 |
| Resend | 4 / 10 |
| **Total** | **25 / 45** |

The input is about 270,000 tokens, at most 600,000. The embedding requests are about 40, at most 70. Exa requests: 0.

**Stops:**

- `live_guard` triggers at the claim: model 45, embedding 70, Exa 0.
- Read-only SQL checks each other stop before each next step and before each statement:
  - a capture request or a pass request above 60,000 input tokens;
  - an author Run request above 150,000 input tokens;
  - a total above 600,000 input tokens;
  - a failed capture or a failed pass;
  - a Run that is refused or cancelled.
- The record has a table of each check: the time, the values, and the result.

**Evidence:**

- The tag is `evidence/s7-journey-<UTC date of the run>`. It has the request payloads, the ToolCalls, the plan, and the logs. It has no database dump and no Project Export Archive.
- The record is `docs/research/agent-plan-live-stage7-journey-<date>.md`.
