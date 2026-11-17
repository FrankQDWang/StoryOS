## Plan of the live calibration run (S7-D7)

- **Head:** `ee880700` on `claude/s7-d7-memory-calibration` (base `stage7-memory` 6944d1fe, with S7-D4c, S7-D5, S7-D5b, S7-D6, S7-D6b). The release package is from `5a65d5c4` (manifest `sha256:cef86daf…c105c32`). The commit after it changes only `apps/web/test/support/memory-calibration.ts`, which is not in the package.
- **Route:** `doubao-seed-2-1-pro-260915`, Registration `fa22`, mapping `v8`, with the bounds and the Host instructions of the release. The packaged Server (`STORYOS_WORKER=0`) and one packaged Worker with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` (ADR 0049, ADR 0050). The key reaches a request only through the typed Worker and Server settings.
- **Sample:** `fixtures/memory-calibration/`: 10 Chapters (2,436 to 3,017 characters each), the two changed Chapters, the key, and the Conversation script. The text is original (`PROVENANCE.md`).
- **Script:** `apps/web/test/support/memory-calibration.ts`, started with `node`. It attaches to the packaged Server at its address, and it sends each command through the HTTP test support and the generated client, at most 8 commands in one minute. It writes no Chapter text, message, observation, or entry with SQL. Its only SQL writes are statements A, B, and C. Each one sets the due time of one schedule row to the present and must change exactly 1 row. No statement moves a rate time or moves a due time later. The script prints each statement with its time, and `due.sql` has each one.
- **Before the run:** a rehearsal of all steps on `host_fake`, done at `ee880700` with the same package: 33 commands applied, 14 captures (12 Chapter, 2 Conversation, each `no_observation`), 14 statements (each 1 row), 7 Runs of the author completed, the Worker stopped 1 minute after the last answer, `measures.json` written. The fake model records no observation, so the rehearsal had no pass and no statement B. It found two defects of the script, now fixed: a Run must name the Current Chapter, and the context items were listed two times. Then a new database; the `live_guard` triggers: model 150, Exa 0, embedding 0. The run uses no browser.

### Steps

1. The 10 Chapters enter the Project (one text edit and one split edit for each Chapter). One Run of the author completes in Conversation A. Statement C runs its capture before the first Chapter. Then statement A makes the row of each Chapter due in Chapter order, each one only after the pass of the Chapter before it settled.
2. After each capture, when the pass row has a due time later than now (the 30 minutes after the last pass), statement B makes it due. The Worker runs one pass. The script reads the entry set after each pass.
3. The author sends the three messages of the Conversation script in Conversation A. Statement C runs one capture, and one pass runs.
4. The author changes Chapter 3 and Chapter 9. Statement A runs two captures, and two passes run. The change of Chapter 9 conflicts with the entry of the author of message 1.
5. In Conversation B, the author asks the three questions. The Worker stops 1 minute after the last answer, so no capture of Conversation B starts.

Each Chapter row is due 1 hour after its commit. When a Chapter row becomes due by time before its statement A, the script sends no statement for it, counts the capture as started by time, and waits for it. When such a capture starts before the pass of the Chapter before it settled, the entry set "after each Chapter" is not exact for that Chapter: the record gives the captures in the input of each pass.

### Requests and expected reported input

| Kind | Count | Expected input of one request (tokens) |
| --- | --- | --- |
| Runs of the author | 7 Runs, about 3 requests each, about 21 | 4,000 to 12,000 |
| Chapter captures | 12, 2 to 4 requests each (at most 8), about 36 | 5,000 to 12,000 |
| Conversation captures | 2, 2 to 4 requests each, about 6 | 3,500 to 8,000 |
| Passes | at most 14, 1 request each | 1,500 to 12,000 |

- **Planned:** about 77 to 90 requests, at most 150.
- **Expected total:** about 600,000 reported input tokens, at most 2,500,000.
- **Stops:** a refusal of the route; a key or a home path in a payload or a log; more than 150 requests; one request of a capture or a pass above 60,000 reported input tokens; one request of a Run of the author above 150,000; another anomaly. Above a limit, the script stops the Worker, and the plan goes to the coordinator.
- **One run:** no second set and no repeat that this plan does not name.

### Counts of the record (in addition to the body)

- Each pass with only a conflict item, with the tokens of its request (finding O2 of S7-D6b).
- Each refusal `intake_malformed` of a capture, with the key that was missing (finding O1 of S7-D5b).
- Each `read_chapter` call of a Chapter capture whose input had the full text (finding O2 of S7-D5b).
- The captures that started by time, and the captures in the input of each pass.

### Evidence

- Record: `docs/research/stage7-memory-calibration-<date>.md`.
- Tag: `evidence/1590-memory-calibration-<date>`: the plan, the request payloads, the ToolCalls, `due.sql`, the guard log, `measures.json`, `entries.json`, the loader log, and the Server and Worker logs. No database dump and no Project Export Archive.
- Before the push and the tag push: a scan of each file for a key, an e-mail address, and a home path.
