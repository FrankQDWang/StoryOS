# Stage 3 interaction research, 2026-10-01

Status: research and proposed interaction direction. This note does not change ticket acceptance, grant a size exception, or claim Stage 3 release.

## Author feedback

The author supplied two Chapter action references: a context menu and a row overflow menu. Reuse the same action model for both entry points. The images establish the menu pattern, not every item shown in the source application. Search, export, statistics and save-state placement are not settled by these images.

The author asked to research related edits across separated paragraphs and Chapters. The concern is consistency when only part of a related change is accepted. Existing inline candidate editing remains the approved default; do not replace it with a default code diff.

The author confirmed active guidance and asked for mature Codex-like behavior. The pre-send source-preview proposal stays withdrawn.

## Local reference refresh

Both independent Git checkouts were clean before refresh. Fetch origin and advance to its current main, with no reset or removal of user work. No build or dependency installation ran.

| Reference | Previous commit | Current commit | Commits advanced | Result |
| --- | --- | --- | --- | --- |
| openai/codex | c9ef7eff005c3299a5a5f0004c34c6a3eedf2564 | cda82a2c6853b484e0ba56d38f13902adfb2a6a1 | 3300 | Clean detached checkout |
| xai-org/grok-build | a881e6703f46b01d8c7d4a5437683546df30449d | 2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8 | 45 | Clean main, fast-forward |

The local claude-code directory is an installed package snapshot, not an independent Git checkout. Its license reserves rights; do not call it open-source CLI implementation. It was not updated. No reference became a StoryOS dependency.

## Current StoryOS contract

[GLOSSARY.md](../../GLOSSARY.md) defines Steering Input as immutable ordered input to a nonterminal AgentRun at its next safe decision boundary. Pause, Cancel and exact Wait responses are distinct commands.

[Manuscript state machine section 7.5](../foundation/manuscript-revision-proposal-state-machine.md#75-acceptproposal) requires a nonempty exact selection, dependency closure, current target versions, and atomic application of all selected effects. A convenient Accept All button alone does not prove this guarantee. These mechanics also do not prove narrative consistency or that a model found every necessary edit.

## Official Codex API evidence

The [official app-server reference](https://learn.chatgpt.com/docs/app-server#steer-an-active-turn) defines turn/steer for the active turn with an exact expectedTurnId. It fails without an active turn and does not start a new turn. Interruption is a separate method. This is a behavior reference, not permission to embed Codex runtime.

## Product comparison

This is documentation research, not a runtime product test.

| Product | Documented behavior | Unproven boundary |
| --- | --- | --- |
| Novelcrafter | [Multiple scenes and Chapters as context](https://www.novelcrafter.com/help/faq/chat/chat-multiple-scenes); [replace selected prose](https://www.novelcrafter.com/help/docs/write/text-replacement-prompts) | Context scope does not prove disjoint edit review or cross-Chapter atomic acceptance. |
| Sudowrite | [Chat proposes a plan for manuscript-wide changes](https://docs.sudowrite.com/using-sudowrite/1ow1qkGqof9rtcyGnrWUBS/chat/5vbuELXf6LZQnGfVzsEXCV). Authors adjust and approve it. | The source does not establish exact-text acceptance, enforced dependency closure, or one cross-document commit. |
| Strata, supplemental | [Grouped Agent edits can be reviewed and accepted](https://strata.space/documentation/guides/suggested-edits). | Maturity was not evaluated; story dependencies and cross-document atomicity are unproven. |
| Clarami, supplemental | [A selected sentence-edit batch applies in one transaction with one Undo](https://www.claramiai.com/help/streaming-diff-review). | A passage batch does not establish cross-Chapter support or narrative dependency enforcement. |

The useful combined pattern is review by shared purpose, inspection at each affected location, explicit required companions, and one exact atomic application. This is a StoryOS proposal, not a claim that one competitor supplies all of it.

Example: move discovery of a letter from Chapter 2 to Chapter 6, then revise the character reaction in Chapter 7. The author must be able to inspect all three replacements. If all three form a required group, accepting only the removal is invalid. Independent wording corrections can remain separate choices. Applying a chosen set atomically cannot prove that the set is narratively complete; the author must still inspect the proposal.

No checkbox, new pane, diff default, new cross-Chapter authorization, or specific membership gesture is approved by this research. Preserve existing in-place candidate editing and reconcile this proposal with the existing exact-set and dependency contract.

## Upstream steering code findings

- Codex defaults Enter to submit and Tab to queue. In-flight submit targets the current Turn; queue waits for completion. The CLI has explicit race/fallback handling, so not every late submit necessarily remains in a completed Turn. [Key bindings](https://github.com/openai/codex/blob/cda82a2c6853b484e0ba56d38f13902adfb2a6a1/codex-rs/tui/src/keymap.rs#L1678-L1679), [routing](https://github.com/openai/codex/blob/cda82a2c6853b484e0ba56d38f13902adfb2a6a1/codex-rs/tui/src/app/thread_routing.rs#L735-L832).
- Codex consumes ordered pending input at the next sampling iteration before a fresh step context. Feature-dependent preemption may interrupt the current response cooperatively; this is not rollback of completed effects. [Turn loop](https://github.com/openai/codex/blob/cda82a2c6853b484e0ba56d38f13902adfb2a6a1/codex-rs/core/src/session/turn.rs#L427-L503).
- Grok Build defaults to Queue and supports Steer. It promotes an eligible ordered prefix at safe points, such as after a tool batch. [Configuration](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shared/src/ui_config.rs#L334-L346), [promotion](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/session/acp_session_impl/prompt_queue.rs#L674-L737).
- A successful upstream steer acknowledgement does not prove durable storage or consumption. Codex explicitly replies before rollout persistence; Grok's interject handler returns queued before later safe-point handling. StoryOS must preserve its own durable command and recovery contract. [Codex boundary](https://github.com/openai/codex/blob/cda82a2c6853b484e0ba56d38f13902adfb2a6a1/codex-rs/core/src/session/turn_input.rs#L1-L10), [Grok handler](https://github.com/xai-org/grok-build/blob/2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8/crates/codegen/xai-grok-shell/src/extensions/interject.rs#L27-L48).
- Codex interrupt terminates its active Turn. It is not the same as StoryOS Pause, which creates a resumable Hold. Retain the StoryOS Pause/Cancel distinction.

Adopt the behavior pattern: send additional author input to the same active Run, retain it in order, and use it at the next safe decision boundary. Do not silently queue a new task or mutate a request already sent. Do not add a later-task queue UI unless separately approved. Use independent StoryOS implementation; no runtime wrapping or upstream code was copied.

## Clarification status

1. Chapter menu entry direction is clear. Use right-click and row overflow for the same Chapter actions. Exact item layout and search/export placement still require a concrete design reconciliation.
2. Related multi-location edits require further author review of a concrete group selection design. Cross-Chapter behavior must be accounted for explicitly, not silently treated as single-location support or added as an unbounded stage expansion.
3. Active guidance capability is confirmed. The upstream pattern is clear. Recalculate the existing-owner implementation breakdown within normal size limits; do not ask again whether the feature is wanted. A previously proposed 1500-line exception is not implied by this feedback.

Detailed local research records are retained under target/stage3-release/2026-09-30/execution/: multi-edit-product-research.md, active-steering-upstream-research.md, and reference-refresh-2026-10-01.json. The author images are retained in author-interaction-references/. These are research inputs, not final release evidence.

## Subsequent author decision

After this research, the author confirmed the design and resumed the full goal. The central-editor Proposal stays the shared component for continuation and revision, including manual edits, AI re-editing, acceptance and rejection. The right conversation shows a clickable entry per revision location with one short explanation. A click opens the corresponding Chapter and locates the paragraph Proposal. Do not replace this with an editor navigation toolbar, chat-hosted candidate prose, or a mandatory grouping workflow. The earlier grouping recommendation is not an approved requirement. Chapter context-menu and overflow entry points, and Codex-like active guidance, are confirmed. Utility placement remains subject to the concrete layout proposal.
