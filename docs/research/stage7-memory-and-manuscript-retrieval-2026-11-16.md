# Memory and Manuscript Retrieval Study for Stage 7, 2026-11-16

This report is the evidence for the Stage 7 kickoff ([#365](https://github.com/FrankQDWang/StoryOS/issues/365)). The author asked how common assistants build memory, and decided that the assistant must find text in the whole manuscript through two tools: a keyword search first, and a hybrid keyword and vector search second.

The report records facts and one recommendation. It decides nothing. The author's decisions go into an ADR and the refreshed Stage 7 tickets.

## Method

Five read-only studies ran on 2026-11-16.

| Study | Source |
| --- | --- |
| Codex CLI memory | Local reference source `.reference/codex`, commit `cda82a2c6` (2026-10-01) |
| Claude Code memory | Local reference package `.reference/claude-code`, version 2.1.88, original sources from the source map |
| Grok Build memory and search | Local reference source `.reference/grok-build`, source revision `559751fd` (2026-09-29) |
| Memory in products and frameworks | Public documents and papers (links below) |
| Retrieval in PostgreSQL and embedding services | Public documents and papers (links below) |

Limits:

- Several Claude Code mechanisms sit behind remote flags. Their live values are not known.
- Two OpenAI pages refused a direct read. Their facts come from search excerpts.
- The web studies report vendor benchmarks. No neutral comparison of memory designs exists.
- Paths below are relative to each reference tree. `.reference/` is not in Git.

## StoryOS today

- The context of a request holds the current Chapter and the selection references of the message (`crates/storyos-core/src/assemble_candidate_context.rs`).
- The assistant has eight tools: `web_search`, `web_fetch`, `finish`, `propose_edit`, `submit_research`, `read_tool_result`, `load_skill`, `read_skill_resource` (`crates/storyos-core/src/tool_exposure.rs`). No tool searches or reads another Chapter.
- The author has a manuscript search. It is an exact substring match over the blocks, done in the application, with a limit of 500 items for each page (`crates/storyos-application/src/manuscript_search.rs`). It needs no database extension.
- Local scripts use the stock `postgres:16-alpine` image. Production PostgreSQL is hosted Supabase ([ADR 0022](../adr/0022-prefer-widely-validated-hosted-infrastructure.md)).
- The tables `project_conversations` and `conversation_memory_settings` exist. No Memory document, Memory tool, or Memory page exists.

## Memory in three coding assistants

| Point | Codex CLI | Claude Code | Grok Build (v2) |
| --- | --- | --- | --- |
| Form | Markdown files: `MEMORY.md`, `memory_summary.md`, optional skills | Markdown files: one file for each fact, and a `MEMORY.md` index | Markdown files: topic files, an inbox of observations, a generated index |
| Writer in the conversation | Only a note file, on a direct request of the user | The main agent writes files with its ordinary tools | The agent can edit topic files |
| Background writer | Phase 1: one structured model call for each idle conversation. Phase 2: a sandboxed agent edits the files | A restricted copy of the agent at the end of a turn, when the main agent wrote nothing | A model call with no tools returns typed observations; the host validates and stores them |
| Trigger | A conversation idle for 6 hours, at most 2 conversations for each run, 6 hour cooldown | End of each turn | End of each turn |
| Consolidation | The Phase 2 agent rewrites the files from the top 256 extraction rows | "Dream" pass after 24 hours and 5 sessions | "Dream" pass at 20 waiting observations or 24 hours; typed operations: create, update, delete, rename, merge, split |
| Always in context | `memory_summary.md`, cut to 2,500 tokens | The index, at most 200 lines or 25 KB | The index, 8 KB for each scope |
| Read on demand | Keyword search in the files with the shell | Read and grep; an optional small model call selects at most 5 files | Read and grep |
| Wrong or old memory | Prompt rules; fresher evidence wins | Prompt rules; an age warning on each memory older than 1 day | The consolidation pass; a delete writes a tombstone |
| Growth | Rows not used for 30 days are deleted; memory that only deleted inputs support is removed | The index limit; the dream pass prunes | Index budget; archived observations kept 30 days |
| Use counting | The model cites the memory that it used; each citation raises a counter | None found | Read counts rank the compact index |
| Author control | Two switches for each thread (use, generate); reset all; edit the files | Settings; edit or delete the files | A memory browser with delete; edit the files |
| Embeddings | None | None | None in v2 (optional in the old design, off by default) |

Evidence:

- Codex: trigger and limits `codex-rs/config/src/types.rs:55-60`, `codex-rs/memories/write/src/start.rs:33-91`; extraction `codex-rs/memories/write/src/phase1.rs`; consolidation `codex-rs/memories/write/src/phase2.rs:302-466`; forgetting rule `codex-rs/memories/write/templates/memories/consolidation.md:171-175`; read path `codex-rs/ext/memories/src/prompts.rs:35-63`; switches `codex-rs/tui/src/bottom_pane/memories_settings_view.rs:85-97`. The feature is off by default (`codex-rs/features/src/lib.rs:1186-1189`).
- Claude Code: types and exclusions `memdir/memoryTypes.ts:14-19,183-195`; index limits `memdir/memdir.ts:35-38,227`; background extraction `services/extractMemories/extractMemories.ts:121-222`; selector `memdir/findRelevantMemories.ts:18-122`; age warning `memdir/memoryAge.ts:33-42`; dream pass `services/autoDream/autoDream.ts:63-190`.
- Grok Build: capture `xai-grok-shell/src/session/acp_session_impl/memory_capture.rs:261-510`, `xai-grok-memory/src/v2_capture.rs:27-33`; consolidation `xai-grok-memory/src/v2_consolidation.rs:101-131`; injection budget `xai-grok-memory/src/v2.rs:20-21,103-139`; delete `xai-grok-shell/src/session/acp_session_impl/memory_forget.rs:1-3`.

One Claude Code rule is important for a writing product: it refuses to save a fact that the project itself already holds, "even when the user explicitly asks" (`memdir/memoryTypes.ts:183-195`). The same rule for StoryOS is: a fact that the manuscript holds is found by manuscript search, not stored in Memory.

## Memory in products and frameworks

- **ChatGPT.** Saved memories are written in the chat on a clear request and can be seen and deleted. A background process also builds context from earlier chats. OpenAI states that saved memories "go stale… and eventually become incorrect or irrelevant" ([OpenAI](https://openai.com/index/chatgpt-memory-dreaming/), [help](https://help.openai.com/en/articles/8590148-memory-in-chatgpt)). Users complain that the automatic rewrite has no pin and no history ([forum](https://community.openai.com/t/dreaming-v3-sacrifices-real-personalization-for-compute-savings-a-power-user-s-perspective/1383010)).
- **Claude.** Memory changed in July 2026 from a summary made each 24 hours to entries in categories that Claude reads and updates in the conversation. Each project has its own memory. The user can edit and delete entries ([release notes](https://support.claude.com/en/articles/12138966-release-notes), [guide](https://support.claude.com/en/articles/11817273-use-claude-s-chat-search-and-memory-to-build-on-previous-context)).
- **Gemini.** No list of memories exists. To delete an item, the user deletes each chat that holds it ([help](https://support.google.com/gemini/answer/16598469)).
- **GitHub Copilot.** Each memory has a citation. Copilot checks the citation before it uses the memory. A memory expires after 90 days unless it is used ([docs](https://docs.github.com/en/copilot/concepts/agents/copilot-memory)).
- **Cursor.** Version 1.2 added an approval by the user for memories made in the background ([changelog](https://cursor.com/changelog/1-2)).
- **Letta.** A small set of blocks stays in context; the rest is searched. The 2026 design is Markdown files in Git, with background agents that merge duplicates ([MemFS](https://docs.letta.com/concepts/memfs/index.md)).
- **mem0.** It extracts facts, finds similar memories, and the model selects ADD, UPDATE, DELETE, or NOOP ([paper](https://arxiv.org/html/2504.19413)).
- **Zep.** Facts have a validity interval. A new fact that contradicts an old one closes the interval of the old one, and the old one stays ([paper](https://arxiv.org/html/2501.13956)).
- **Files or vectors.** Letta reports 74.0% on LoCoMo for an agent that searches plain files, against 68.5% for a graph memory ([Letta](https://www.letta.com/blog/benchmarking-ai-agent-memory)). Zep shows that the full context baseline beat mem0 in mem0's own paper ([Zep](https://www.getzep.com/blog/lies-damn-lies-statistics-is-mem0-really-sota-in-agent-memory/)). These are vendor results. They show that files with search are competitive at this size, not that they are better.
- **Fiction products.** Sudowrite, Novelcrafter, and NovelAI keep the story facts as entries that the author curates. Entries enter the prompt always, or when a name appears in the text. None of them extracts memory from chats without a request ([Sudowrite](https://docs.sudowrite.com/using-sudowrite/1ow1qkGqof9rtcyGnrWUBS/saliency-engine/4KL8gFeLZNvk8CEeXpfwB2), [Novelcrafter](https://www.novelcrafter.com/help/docs/codex/codex-tracking), [NovelAI](https://docs.novelai.net/en/text/lorebook/)).

## Patterns that repeat

1. **Two levels.** A small index is always in context. The entries are read on demand.
2. **Two writers.** The assistant saves in the conversation on a clear request. A background pass finds what the conversation did not save and tidies the set.
3. **Readable text.** Entries are short Markdown text that a person can read, not rows of a vector store. No one of the three coding assistants uses embeddings for memory.
4. **Pruning is part of consolidation.** The same pass that merges entries removes old and contradicted ones. Limits on the index and expiry of unused entries bound the growth.
5. **Trust comes from control.** The user sees each entry and its source and can delete it. Silent rewrites with no history cause complaints.

## Manuscript retrieval

### Keyword search

- The three coding assistants search files with an exact or regular expression search on each call, with no index. Claude Code first used a local vector database and removed it; the reasons were stale indexes and permission rules ([interview](https://newsletter.pragmaticengineer.com/p/building-claude-code-with-boris-cherny)).
- StoryOS already has this search for the author. A novel has about 10,000 to 20,000 blocks, so an exact scan for each call is cheap. The first tool can reuse it.
- An index for Chinese keyword search is not necessary at this size. If it becomes necessary: `pg_trgm` does not use its index for a term of 1 or 2 characters, which is the usual length of a Chinese name ([pg_bigm](https://pgbigm.github.io/pg_bigm/pg_bigm_en.html)); PGroonga works for each length ([PGroonga](https://pgroonga.github.io/)); ParadeDB `pg_search` and VectorChord-bm25 give BM25 rank but have the AGPL-3.0 license ([ParadeDB](https://hub.docker.com/r/paradedb/paradedb), [VectorChord](https://vchord.pgsty.com/docs/about/license/)).

### Where keyword search fails

- A question such as "the scene where she first lies to her brother" has no word in common with the scene. Only the vector side finds it.
- Cursor reports 12.5% higher answer accuracy when semantic search is added to grep ([Cursor](https://cursor.com/blog/semsearch)). An Amazon study reports that agentic keyword search reaches 91.5% of the answer correctness of retrieval with embeddings ([paper](https://arxiv.org/html/2602.23368)).
- Both results support the author's order: keyword search first, hybrid search when it fails.

### Vector store

- pgvector 0.8.7. HNSW index; `vector` up to 2,000 dimensions, `halfvec` up to 4,000 ([pgvector](https://github.com/pgvector/pgvector)).
- A benchmark by a third party at 100,000 rows of 1,536 dimensions: recall 0.947 at 2.9 ms ([pgvector.app](https://pgvector.app/benchmarks/)). One novel is 10,000 to 20,000 chunks.
- The local image must change from `postgres:16-alpine` to an image with pgvector. NOT VERIFIED in this study: that the hosted Supabase project of ADR 0022 has pgvector enabled.
- Hybrid search is two ranked lists joined by Reciprocal Rank Fusion ([ParadeDB guide](https://www.paradedb.com/blog/hybrid-search-in-postgresql-the-missing-manual)).

### Chunks

- For narrative books, chunks cut at a scene change (334 tokens on average) scored DCG@20 62.09, against 49.00 for raw paragraphs ([LumberChunker](https://blog.ml.cmu.edu/2026/03/17/lumberchunker-long-form-narrative-document-segmentation/)).
- A short context line before each chunk cut the retrieval failures from 5.7% to 2.9% with embeddings and BM25 together ([Anthropic](https://www.anthropic.com/research/contextual-retrieval)).

### Embedding source

| Option | Facts | Open points |
| --- | --- | --- |
| Volcengine Ark, hosted | The only current model is `doubao-embedding-vision-251215`: 2,048 dimensions (1,024 optional), text at CNY 0.70 for each million tokens ([models](https://docs.volcengine.com/docs/ark/model-list), [prices](https://ark.volcengine.com/region:cn-beijing/docs/model-pricing)). The Agent Plan lists it at 0.5 AFP for each 10,000 tokens ([plan](https://ark.volcengine.com/region:cn-beijing/docs/agent-plan-personal-vectorized-models)). The text-only models are closed to new users ([notice](https://ark.volcengine.com/region:cn-beijing/docs/model-deprecation-notice)). | The plan overview says that text and embedding models are "不可用于 API 调用" ([overview](https://ark.volcengine.com/region:cn-beijing/docs/agent-plan-personal-plan-overview)). The same sentence covers the text model that StoryOS uses today for the author's personal use. NOT VERIFIED: the exact request path below `/api/plan/v3`, and that one request returns one vector for one chunk. |
| Local model | Qwen3-Embedding-0.6B: 1,024 dimensions, 32K tokens, Apache-2.0 ([model](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B)). bge-m3: 1,024 dimensions, 8,192 tokens, MIT ([model](https://huggingface.co/BAAI/bge-m3)). Both run in Rust through `fastembed-rs` ([repo](https://github.com/Anush008/fastembed-rs/blob/main/README.md)). | A model file of 0.6 to 1.2 GB on the server, processor load, and a new native dependency in the build. Throughput on a small VPS is not measured. |

## Recommendation for StoryOS

This section is the opinion of the coordinator. Each point that changes the product waits for the author.

**Memory.**

- A Memory entry is a short Markdown text in PostgreSQL with a title, a one-line description, a type, the source Conversation, and the times of creation, change, and last use.
- A bounded index of titles and descriptions is always in the context. The assistant reads an entry with a tool.
- Two writers. The assistant records, changes, or removes an entry in the conversation when the author asks for it. A background pass after the Conversation becomes quiet returns typed observations from a model call with no tools; StoryOS validates and stores them.
- A consolidation pass returns typed operations (create, update, delete, merge). StoryOS validates them and applies them in one transaction. Pruning is a named duty of this pass: contradicted entries, duplicates, entries not used for a long time, and an index above its budget.
- Each change keeps the earlier version. A delete by the author leaves a mark, so that the pass does not create the entry again.
- A fact that the manuscript holds is not stored. Memory holds what the conversations hold and the manuscript does not: the author's preferences and corrections, decisions about the story that are not yet written, and open threads.
- No embeddings for Memory. The index and a keyword search are sufficient for some hundreds of entries.

This follows the Grok Build design (typed output that the host validates) more than the Codex design (an agent with a shell that edits files in a sandbox). StoryOS has no shell agent and validates each model output, so the typed form fits. It replaces the "one complete published set" of the old Stage 7 tickets with operations on entries, and it changes the detail below [ADR 0035](../adr/0035-use-background-generated-project-memory.md). The direction of ADR 0035 stays: background generation, readable documents, a bounded summary, search and read by the assistant.

**Manuscript.**

- The Chapter list is always in the context. Three tools: keyword search over the whole manuscript, read a Chapter or a range of it, and hybrid search.
- The keyword tool reuses the exact match that the author's search has today.
- The hybrid tool uses pgvector and one embedding source. A background step embeds each changed block. A probe ticket must first answer the open points of the embedding source.
