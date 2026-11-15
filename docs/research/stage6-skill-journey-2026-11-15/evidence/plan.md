## Live plan of S6-JRN-001 (written before the first request)

- Tip: `stage6-skills` at `2b50cdac95fbe35630c6bf33c1f743ba5d4ea86b`. Release package of that clean commit, manifest `sha256:1474d98340a7cc409b3a3350864cab25f1a0745da8ecb4ac4049b976e36341af`.
- Topology: packaged `storyos-server` (`STORYOS_WORKER=0`, `STORYOS_MODEL_DESTINATION=volcengine_agent_plan`, `STORYOS_SKILLS_DIR` on a copy of `fixtures/skill-packages`) and one `storyos-worker --once` for each Run, each with `env -i`. The key reaches the Worker only through its typed settings. Disposable database of `scripts/dev-postgres.sh up --interactive`.
- Request guard: the `live_guard` triggers of the record of 2026-11-14 with a cap of 25 model requests and 0 Exa exchanges, counted at the claim. The journey sends no web research request.
- Author surface: the packaged Web client in the built-in browser with a clear storage. The author drives each step in the panel.

| Step | Run | Requests | Expected reported input of each request |
| --- | --- | --- | --- |
| Startup refusals (done, no request) | none | 0 | none |
| 1. Select and run: `$dialogue-style-guide`, then the message typed with keyboard events; Proposal accepted | R-A | 3 (`propose_edit`, `finish`, closing) | 3,000; 4,000; 4,500 |
| 2 and 3. No chip; a request that fits `internal-comms`: `load_skill`, then `read_skill_resource` of a file of `examples/` | R-B | 4 (`load_skill`, `read_skill_resource`, `finish`, closing) | 3,500; 4,200; 5,500; 6,000 |
| 4. Two Skills: `$internal-comms` and a request that also fits `dialogue-style-guide` | R-C | 5 (`load_skill`, one read, `finish`, closing, one more Tool call) | 4,500; 5,200; 6,500; 7,200; 7,800 |
| 5. Usage record and reload | none | 0 | none |
| 6. Operator change: a load of `dialogue-style-guide`; the guard stops the Worker before the continuation (cap at that moment = used + 1); the author pauses; the operator changes the fiction `SKILL.md`, removes `internal-comms`, and starts the Server again; the author resumes with a guidance message | R-D | 4 (`load_skill`; after the resume: one read or `propose_edit`, `finish`, closing) | 4,000; 5,500; 6,500; 7,000 |
| Stale `$internal-comms` chip after the restart | none | 0 | none |
| 7. Continue without the Skill: Proposal, then manual writing | R-F | 3 (`propose_edit`, `finish`, closing) | 5,000; 6,000; 6,500 |

- Planned: **19 requests**, expected reported input about **107,000 tokens** in total (the later Runs of one Conversation carry the earlier turns, so each later request is larger). A send of a step one more time, an automatic Retrieve, or one more Tool call stays inside the guard cap of 25. Limits of the ticket: 30 requests and 400,000 reported input tokens.
- Log: each request (claim time, response ID, result, usage) as a comment on this ticket after each step. Stop only at an anomaly.
