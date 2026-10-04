# Reference Model Findings

The fixed product base is `479224809cdaae997cda51cb8853e3fafa242b65`. These findings do not describe later main. Product source is unchanged.

## D-001: Project changes are missing from Author Undo order (P2)

Classification: implementation defect.

Create Project, Update Project, and Archive Project successful Receipts have no Author Action. The public sequence `Create Project; Create Volume; Create Chapter; Create Editor Session; Author Edit("Keep this sentence."); Update Project(title); Author Undo` removes the earlier prose and leaves the later Project title. The Project change was skipped.

Contract: `docs/foundation/manuscript-revision-proposal-state-machine.md:142` (one sequence for every successful author-owned Core Transition) and section 10.1 (exact newest frontier; an unsupported handler is a Barrier, never a skipped action). The catalog classifies the three Project commands as admitted author commands.

Locations: `crates/storyos-server/src/create_project.rs:156`, `update_project.rs:195`, and `archive_project.rs:191` project `author_action_sequence: None`.

Minimal replay: `scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case project-undo --seed 403 --replay --output target/reference-model/D-001.json`. The setup creates only the objects and Session needed to edit. Evidence: third scenario in `replay-minimals.json`; the final canonical Chapter body is empty instead of `Keep this sentence.`.

## D-002: Create Chapter replay fails after an Editor Session opens (P2)

Classification: implementation defect.

Minimal sequence: `Create Project; Create Volume; Create Chapter; Create Editor Session; exact retry Create Chapter`. The initial and immediate retry responses are 200 and byte-equal. The delayed retry returns 503 `project_store_unavailable`. No prose edit or Project rename is needed.

Contract: protocol section 7.3 requires the same immutable acknowledgement for exact retry. ADR 0032 permits unavailable historical acknowledgements only for old pre-capture records, not this newly created record. ADR 0041 requires the shared replay read to preserve existing acknowledgement behavior.

Locations: `crates/storyos-adapter-postgres/src/command_replay.rs:136` uses `query_opt`; its SQL at lines 244-248 joins canonical Snapshots by Activity position, which is not a unique Snapshot identity. `crates/storyos-adapter-postgres/src/editor_session.rs:81` creates another canonical Snapshot at the existing Activity position. The join can return multiple rows.

Replay: `scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case chapter-after-session --seed 401 --replay --output target/reference-model/D-002.json`. Evidence: first scenario in `replay-minimals.json`.

## D-003: Create Editor Session retry returns a later editor base (P2)

Classification: implementation defect.

Minimal sequence: `Create Project; Create Volume; Create Chapter; Create Editor Session; Author Edit("Keep this sentence."); exact retry Create Editor Session`. Both command responses are 200, but replay changes the Snapshot, authoritative Head, payload digest, body, Activity position, and undo frontier.

Contract: protocol section 7.3 requires byte-stable acknowledgement replay. `getEditorSession` is the separate current-state query; replay of its create command is not that query.

Locations: `crates/storyos-adapter-postgres/src/editor_session.rs:108-127` resolves the old Session identity and reads its current record. `crates/storyos-server/src/editor_session.rs:199-261` projects that mutable record into the command response.

Replay: `scripts/dev-postgres.sh run python3 prototypes/reference-model/run.py --stage replay --case session-after-edit --seed 402 --replay --output target/reference-model/D-003.json`. Evidence: second scenario in `replay-minimals.json`.

## Contract questions

A-001 through A-006 are recorded in PROGRESS.md. Their final source and reachability crosswalk remains in progress. They must not be converted into implementation defects by guessing missing semantics.
