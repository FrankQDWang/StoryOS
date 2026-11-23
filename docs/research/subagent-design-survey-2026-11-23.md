# Survey of subagent designs (2026-11-23)

Sources: `.reference/codex` at `794584993` (2026-11-23), `.reference/grok-build` at `2bdd1d6a` (newest upstream), `.reference/claude-code` 2.1.88 (npm package) and the public documents of Claude Code. The coordinator made the survey with three read-only survey agents for [ADR 0103](../adr/0103-deliver-a-smaller-stage-9-with-a-plan-list-of-a-run-and-a-subrun-of-one-level-that-the-parent-waits-for.md). `.reference/` is local reference material that Git ignores; each path below is a path of the upstream source at the named commit.

## Codex (paths under `codex-rs/`)

- Two generations. V2 is the default for the current models (`models-manager/models.json`, `core/src/config/mod.rs:1830-1857`); V1 (`multi_agent`) is deprecated.
- V2 tools (`core/src/tools/handlers/multi_agents_spec.rs`): `spawn_agent(task_name, message, fork_turns, agent_type, model, reasoning_effort)`, `send_message`, `followup_task`, `wait_agent(timeout_ms)`, `interrupt_agent`, `list_agents`.
- Child context: V2 `fork_turns` defaults to `"all"` (a copy of the full history; `multi_agents_v2/spawn.rs:214-238`), `"none"` is a fresh start. V1 default was a fresh start. A forked child gets a note that it is a child and reports to its parent (`core/src/agent/history.rs:536-542`). `preserve_fork_prefix` keeps the cached prompt prefix on a full fork (`features/src/feature_configs.rs:406-408`).
- Child config: a clone of the parent's config; base and developer instructions inherited; `subagent_developer_instructions` can replace them (`core/src/agent/child_config.rs:122-220`). Roles (`default`, `explorer`, `worker`) change instructions and the model, and only disable tools (`core/src/agent/role.rs`).
- Asynchronous: spawn returns at once. When the child ends, the runtime queues a mail `Message Type: FINAL_ANSWER` with the last message of the child (`core/src/agent/control/completion.rs:28-152`). A parent in a turn gets it before its next model request (`core/src/session/turn.rs:3502-3584`). After the final answer of the parent, late mail waits for the next turn, and an idle parent is not woken by default (`wake_on_agent_messages` off). `wait_agent` blocks until mail, default 30 s, and returns no content.
- Mailbox: `send_message` queues, `followup_task` wakes an idle target; each agent of the tree can send to each other agent; a message never interrupts a Tool call.
- Limits: 4 agents at once with the root, no depth limit (V1: depth 1, 6 threads). The interrupt of the parent does not go to the children.
- Default instruction text: do not spawn unless the user or the instructions ask for delegation (`prompts/src/model_messages/multi_agent.rs:46`).

## Grok Build (paths under `crates/`)

- Tools: `task(prompt, description, run_in_background=true, isolation, resume_from, cwd, model)`, `get_task_output(task_ids, timeout_ms)`, `kill_task`, `send_subagent_message` (off by default).
- Child context: fresh; the model's call hard-codes `fork_context: false` (`codegen/xai-grok-tools/src/implementations/grok_build/task/mod.rs:639`). The child has its own system prompt template (`xai-grok-agent/templates/subagent_prompt.md`) and the project instructions.
- Tools of the child: no ask-user tool; depth 1 by default (no grandchild); 32 children at once.
- Background by default; the result comes by `get_task_output` or by a reminder that the runtime attaches to the next Tool result of the parent, or by a wake-up of an idle parent (`xai-grok-tools/src/reminders/task_completion.rs`). At most 16,000 bytes inline. Foreground blocks, and after 600 s the child goes to the background.
- Result: the last report of the child. Statuses completed, failed, cancelled.

## Claude Code 2.1.88

- Tool `Agent(description, prompt, subagent_type, model, run_in_background, name, isolation)`.
- Child context: fresh, only the prompt. A fork mode exists in the code and is off in 2.1.88 (the newer documents say it is on by default). The child has the system prompt of its agent definition and the project instructions (Explore and Plan leave them out).
- Tools of the child: the definition's list; never Agent, AskUserQuestion, plan mode, TaskStop. No nesting in 2.1.88.
- Blocking by default; `run_in_background` returns at once and the runtime puts a `task-notification` into the input of the parent. Several Agent calls of one message run at the same time.
- `SendMessage` only with agent teams: queued for the next Tool round of a running agent; resumes a finished agent.
- Result: the text of the last assistant message; the parent does not see the Tool calls of the child.
- Guidance: brief the child as a colleague who did not see the conversation; never delegate understanding.

## StoryOS at `main` `5e043ab7`

- The Worker is one serial loop (`crates/storyos-worker/src/lib.rs:59-74`): one claim at a time, a Run of the author runs each RunStep in one claim, a background Run gives its claim back only for a queued Run of the author (`yields_to_author`). The Tool Gateway dispatches the ToolCalls of one Agent Decision in order. No code runs two Runs or two ToolCalls at the same time.
- The glossary already says for the Subrun Context Bundle: "Parent context never flows into it implicitly" (`docs/glossary/subrun.md:15-16`).
