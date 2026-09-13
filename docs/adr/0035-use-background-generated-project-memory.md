---
status: accepted
---

# Use Background-Generated Project Memory

StoryOS uses Codex CLI as the primary design reference for Agent Memory:
background extraction from eligible prior conversations, model-driven
consolidation of readable memory documents, a bounded navigation summary,
and Agent-directed search and read. This supports continuity without requiring
the author to express every creative correction as a formal control.

Generated Memory is fallible reference material. It does not change fiction
facts, author constraints, execution permissions, or Research evidence. Ordinary
corrections remain conversation Messages; an explicit memory-change request can
produce a plain-language Memory Note for later consolidation. General prompts
are part of the mechanism. Per-claim admission, semantic suppression rules,
derived-removal graphs, and automatic continuation resets for ordinary creative
corrections are not part of it. Precise semantic forgetting is not guaranteed.

The [Memory contract](../foundation/fiction-memory-and-research-provenance-semantics.md)
owns the details. StoryOS retains its PostgreSQL authority, exact User and
Project Isolation, real deletion, and external disclosure boundaries. Markdown
is the readable document form, not permission to create another local source
of truth. Conversation history, active context compaction, Provider continuation,
and cross-conversation Memory remain distinct.

## Reference evidence

The inspected local Codex snapshot is
`c9ef7eff005c3299a5a5f0004c34c6a3eedf2564` (2026-07-21). The design evidence is
its [background pipeline](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/memories/write/src/start.rs),
[extraction](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/memories/write/src/phase1.rs),
[consolidation](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/memories/write/src/phase2.rs),
and [summary/read injection](https://github.com/openai/codex/blob/c9ef7eff005c3299a5a5f0004c34c6a3eedf2564/codex-rs/ext/memories/src/prompts.rs).
The [official Memory guide](https://learn.chatgpt.com/docs/customization/memories)
also describes background generation and separate per-chat use and contribution
controls. This is design provenance, not copied runtime code, an upstream
dependency, or proof of a Volcengine model capability.
