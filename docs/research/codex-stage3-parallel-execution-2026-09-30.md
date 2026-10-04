# Codex execution options for the Stage 3 repair graph

> **Historical document.** This Codex-only execution proposal for Stage 3 is not
> current process. From Stage 4, [Issue tracker](../agents/issue-tracker.md) and
> [Repository verification](../agents/verification.md) apply to Claude Code and
> Codex. This file is retained as research evidence from 2026-09-30.

Status: research and proposed operating plan. No implementation authorization.
Date: 2026-09-30.
Repository baseline: `7ae035119749c5f5dc93794797aa71fd0b4c79c5`.

## Recommendation

Use the unmodified Matt `implement-spec` flow with native Codex subagents as the default. Keep one supervisor on GPT-6 Astra with low effort. Explicitly select GPT-6.1 Sol for implementers, the merger, and independent Standards and Spec reviewers. Use separate app chats only when a long ticket needs its own visible, independently resumable task, or after a verified capacity test shows that a mixed arrangement is useful. Do not equate more chats with more account quota or more host capacity.

This is an execution proposal, not a change to repository policy, skills, configuration, task assignments, or the existing implementation hold.

## Evidence and current local facts

- The installed Matt directory contains 27 `SKILL.md` files. Its `ask-matt` routes a multi-ticket build to `implement-spec`. That skill requires a dependency frontier, implementer worktrees, one integration branch, a merger subagent, and a final two-axis review. Local sources: `/Users/frankqdwang/.agents/skills/ask-matt/SKILL.md` and `/Users/frankqdwang/.agents/skills/implement-spec/SKILL.md`.
- The local CLI reports `codex-cli 0.145.0`. This is the CLI version, not proof of the desktop application's version or update status. `codex features list` reports multi-agent enabled. The global config selects `gpt-6.1-sol` with `low` effort; a global default does not establish the active chat model.
- This session exposes four concurrent agent slots, including the primary. The current `spawn_agent` tool accepts explicit `gpt-6-astra` and `gpt-6.1-sol` model IDs. A full-history fork cannot also override its model; a fresh context or bounded turn fork can. One research-only Sol agent was successfully dispatched during this investigation.
- The app's current `create_thread` tool accepts a model, reasoning effort, project, and local/worktree target. `wait_threads` waits on up to eight explicit task IDs and supports progress cursors. `read_thread` inspects a task; `send_message_to_thread` can continue it with human authorization. These are inspected callable tool contracts; this research did not create or benchmark an independent execution chat.
- The host reports 32 GiB physical memory and 10 logical CPUs. No throughput benchmark was run. The recommendation to limit simultaneous heavy builds is an initial resource policy, not a measured optimum.
- The 13 open tickets comprise nine dependency-ready implementation candidates, two implementation tickets with open prerequisites, one design-pending owner, and one final acceptance owner. All remain paused for implementation. The source-inspection design must be resolved before its implementation is scheduled.

## Native subagents and separate chats

| Property | Native subagent | Separate app chat |
| --- | --- | --- |
| Control | Parent controls an agent tree and receives completion messages | Peer task with its own ID; supervisor must explicitly inspect/wait and send authorized follow-up |
| Best fit | Matt implementers, focused exploration, merger, independent review | A long bounded ticket that benefits from a separately visible and resumable chat |
| Context | Can start with a bounded brief instead of the full parent history | Fresh task prompt, or a deliberate fork when old context is necessary |
| Model | Explicit spawn override or configured role/default | Explicit model and effort at creation; supported follow-up overrides |
| Files | Not automatically isolated by spawning; give each writer a separate worktree | A local chat is not automatically isolated; explicitly choose its worktree and starting state |
| Limits | Current session advertises four slots including the root | No verified global concurrency maximum in this research; shared account and host limits still apply |
| Result | Parent notification plus retained evidence | Explicit wait/readback plus retained evidence; creation returns before completion |

Public documentation confirms parallel subagents, per-agent model/effort settings, custom agent roles, and context isolation. It also states that delegation adds token work. The current configuration names are `agents.default_subagent_model`, `agents.default_subagent_reasoning_effort`, and `agents.max_concurrent_threads_per_session`. The last is documented as excluding the primary, while this session reports a four-slot runtime cap including the primary. Do not derive a larger live quota from the configuration description. A future configuration change needs a new-session capability check. [Subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents)

Managed worktrees provide separate checkouts. Starting state must be explicit: the current app tool contracts have different defaults for task creation and worktree creation. A worktree must start from the verified integration tip, not an assumed default main. Each checkout needs its own build outputs, dependency installation, test package, and database under the StoryOS resource contract. [Worktrees](https://learn.chatgpt.com/docs/environments/git-worktrees), [StoryOS parallel resources](../agents/parallel-resources.md)

## Proposed model allocation

| Role | Model and proposed effort | Responsibility |
| --- | --- | --- |
| Supervisor | GPT-6 Astra, low | Refresh graph, dispatch, resolve scope questions, own tracker state and acceptance decisions |
| Implementer | GPT-6.1 Sol, medium for non-trivial product work; low for bounded definition work | One issue, one owner, scoped implementation and targeted proof |
| Merger | GPT-6.1 Sol, medium; increase for a concrete hard conflict | Sole writer to the integration branch; preserve other owners' work |
| Standards reviewer | GPT-6.1 Sol, high when needed | Independent read-only review against repository standards |
| Spec reviewer | GPT-6.1 Sol, high when needed | Independent read-only review against the exact parent/child acceptance contract |

Worker effort choices are recommendations for later approval, not settings changed in this research. The supervisor cannot claim to have switched its own active model merely because it can specify models for children. Select and verify Astra/Light in the supervisor chat before execution.

Explicit model selection matters: otherwise an agent can inherit the supervisor model. Prefer fresh task context with links to the issue, parent, current contracts, integration SHA, owned files, test commands, and stop conditions. Return concise evidence pointers, commit/tree identities, commands, and unresolved findings instead of full logs. Keep reviewers separate from the implementation owner. [Subagent model selection](https://learn.chatgpt.com/docs/agent-configuration/subagents#choosing-models-and-reasoning)

## Capacity and execution plan

1. Start with the native four-slot runtime: one supervisor plus up to three workers. The merger uses a free slot when a result is ready; it need not occupy a permanent idle slot. Independent review uses separate reviewer contexts after integration.
2. Prioritize the Conversation and canonical-Block repairs because they unlock the composer and Inline work. Use the third slot for a bounded definition fix or independent display task. Keep the pending source-inspection design visible so it does not become a late release blocker.
3. Dispatch a newly ready ticket as soon as its real prerequisite has accepted main evidence or valid current integration evidence. Do not wait for an entire batch. A native open blocker is not satisfied merely by an agent saying it finished.
4. Separate reasoning/edit concurrency from command concurrency. As an initial host policy, allow several agents to read and edit disjoint worktrees but at most one or two heavy Rust/package/browser/database command groups at once. Adjust from actual memory pressure and measured command time.
5. Shared Proposal code and the shared definition verifier each have one edit owner at a time. This is resource coordination, not a fabricated domain blocking edge.
6. If three workers leave substantial independent work waiting while the host is healthy, evaluate a larger configured subagent capacity or a small number of independent task chats. Verify actual supported limits and setup before increasing dispatch. Do not launch all nine candidates just to maximize a count.
7. The aggregate PR, current synthetic-merge verification, independent Standards/Spec review, applicable complete verification, and exact-main resolutions remain required. The final journey and Stage 3 release are not replaced by individual worker PASS reports.

## Other useful current capabilities

- **Goal mode:** gives the supervisor a persisted objective and explicit completion conditions, with pause and resume. A goal does not make local work independent of connectivity or turn readiness into implementation permission. Set it only when the user approves execution. [Long-running work](https://learn.chatgpt.com/docs/long-running-work)
- **Event-based waiting and steering:** use native agent messages or `wait_threads` with cursors instead of repeatedly reading full transcripts. In-flight feedback is distinct from completion evidence. Current callable tool contracts are the evidence for the exact methods.
- **Local environment setup:** can prepare worktrees and expose repository commands as actions. It can reduce repeated installation mistakes; it does not replace StoryOS-owned verification or permit shared mutable `target/` and database state. [Local environments](https://learn.chatgpt.com/docs/environments/local-environment)
- **Hooks:** current lifecycle hooks include `SubagentStop` and `Stop`. A future bounded hook could check for a required result record; hook events do not prove a product pass. Keep existing deterministic repository checks as the enforcement owner. Non-managed hooks need explicit trust review. No hook is installed by this research. [Hooks](https://learn.chatgpt.com/docs/hooks)
- **Remote environments:** potentially useful to move heavy work off this Mac, but no remote host has been qualified here. A new environment must reproduce repository resources and verification, and cannot reuse local-only acceptance evidence. [Remote connections](https://learn.chatgpt.com/docs/remote-connections)
- **CLI, App Server, and SDK orchestration:** available routes for building a custom controller, but unnecessary for this repair graph while native coordination is sufficient. Avoid introducing a second scheduler and a new lifecycle/retry system merely to invoke the existing skills. [Codex App Server](https://learn.chatgpt.com/docs/app-server)

## Cost boundary

The official Standard API rates for prompts up to 272K tokens are $10 input and $50 output per million tokens for Astra, and $2 input and $10 output for Sol. Sol's listed ordinary input/output rate is one fifth in that API tier. This is not a promise of an 80 percent lower project bill, a Codex subscription-quota ratio, or equal token use. Total work includes retries, review, context, and every agent. ChatGPT/Codex plan credits and account limits are separate evidence. [Astra model](https://developers.openai.com/api/docs/models/gpt-6-astra), [Sol model](https://developers.openai.com/api/docs/models/gpt-6.1-sol)

The Responses API multi-agent feature is not the same control surface as the desktop tool. Its model-sharing rule must not be used to infer heterogeneous models in the app. Use the current app's verified explicit model overrides for the proposed Astra supervisor and Sol workers. [Responses multi-agent](https://developers.openai.com/api/docs/guides/responses-multi-agent)

## Decision still open

Choose whether to start with native subagents and the verified three-worker capacity, or to authorize a small mixed pilot with independently visible worker chats. Native subagents are the closest match to unmodified Matt. Independent chats are optional execution containers, not a substitute for its graph, one integration owner, TDD, two-axis review, or acceptance gates.

No worker chat, implementation worktree, branch, automation, goal, configuration edit, Claim, or product implementation was created by this research. The only delegated work was read-only model/cost research. This document is the sole tracked research artifact added in this turn.
