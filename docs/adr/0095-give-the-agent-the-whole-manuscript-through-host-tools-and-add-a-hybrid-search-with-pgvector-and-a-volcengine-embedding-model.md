---
status: accepted
---

# Give the Agent the Whole Manuscript through Host Tools, and Add a Hybrid Search with pgvector and a Volcengine Embedding Model

The author decided this on 2026-11-16 in conversation, at the start of Stage 7 ([#365](https://github.com/FrankQDWang/StoryOS/issues/365)). This ADR records how the Agent reads and searches the whole manuscript, the vector store, and the embedding vendor. [ADR 0094](0094-deliver-stage-7-with-one-project-instruction-and-agent-memory-as-typed-entries.md) records the scope of Stage 7 and Agent Memory.

The evidence is the study [Memory and manuscript retrieval for Stage 7](../research/stage7-memory-and-manuscript-retrieval-2026-11-16.md).

## Context

At `main` `ae03e16b`, the repository has these facts:

- The context of a Run holds the current Chapter and the selection references of the author. No Host Tool reads or searches another Chapter.
- The standing grant already covers all content of the Project ([ADR 0051](0051-record-the-standing-grant-scope-and-refuse-a-disclosure-outside-it.md)): the author expects the assistant to read each Chapter.
- The author has a manuscript search in the writing workspace. It is an exact match that the application does, and it needs no database extension.
- The scripts of local development use the image `postgres:16-alpine`, which has no vector extension. Production PostgreSQL is hosted ([ADR 0022](0022-prefer-widely-validated-hosted-infrastructure.md)).
- The Memory contract permits a vector index only as a Retrieval Index that StoryOS can build again, and it calls an external embedding request an authorized disclosure.

The author gave three requirements:

- The Agent must be able to find text in the whole manuscript.
- A keyword search and a hybrid search are two tools. The keyword search is cheap and is the usual tool. The hybrid search is for a question that the keyword search cannot answer.
- The hybrid search has a vector side. A search by keywords only is not sufficient for fiction.

The study gives the facts for the vendor decision. Volcengine Ark offers one embedding model to new users, `doubao-embedding-vision-251215`, with 2,048 dimensions or 1,024 as an option. The plan of the deployment lists it. The overview of that plan says that the text models and the embedding models are not for API calls; the same sentence covers the text model that StoryOS uses today for the personal use of the author. The study did not verify the exact request path of the model in the plan, and it did not verify the vector extension in the hosted database.

## Decision

### 1. The Chapter list and three Host Tools

- **The Chapter list.** Each Run has one bounded Context item with the order, the title, and the length of each Chapter of the Project.
- **Keyword search.** One Host Tool finds an exact term in the whole manuscript. It uses the match of the author search. Its result is a bounded list of matches, each with its Chapter, its position, and the text around it.
- **Read.** One Host Tool reads one Chapter or a range of it, in the bounds of a Tool result.
- **Hybrid search.** One Host Tool gets a question in ordinary words and returns a bounded, ranked list of passages. It joins a list by exact terms and a list by vector distance with Reciprocal Rank Fusion.

The rules of a Host Tool apply ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md)): a ToolCall record, and a result that returns to the model in the next RunStep. The descriptions of the tools tell the model to use the keyword search first and the hybrid search when the keyword search finds nothing or the question has no exact term. The Host does not force that order.

### 2. The vector store

- The vectors are in PostgreSQL with the `pgvector` extension, in the Project Scope and below the same Project Isolation as each other record.
- The vectors are a Retrieval Index. StoryOS can build them again from the Chapters, and a loss of them loses no author content. The Project export does not hold them.
- Before the hybrid search returns a passage, the Host reads the current text of the Chapter. A passage whose text changed after its vector was made is not returned from the vector side.
- Agent Memory has no vector (ADR 0094).

### 3. The embedding source

- The embedding model is `doubao-embedding-vision-251215` of Volcengine Ark, with text input only and 1,024 dimensions. The request goes through the model destination that the operator of the deployment configures ([ADR 0048](0048-use-the-deployment-model-destination-by-default.md)).
- An embedding request sends Chapter text to the same Provider that gets the writing requests. The configuration by the operator is the consent, and the Host records the grant scope with no action of the author. The request needs its own scope revision (ADR 0051), its own bounds, and its own disclosure record.
- This route is for the personal use of the author, the same as the text model. A commercial use needs another route and a new ADR.

### 4. The index maintenance

- Background work makes the vectors of the passages of a Chapter after the authoritative text of that Chapter changed. Ordinary writing does not wait for it.
- Text that has no vector yet is found by the exact-term side and by the keyword search.
- When the deployment has no embedding configuration or the database has no vector extension, the hybrid search is not in the Tool Exposure of a Run. The Chapter list, the keyword search, and the read stay.

### 5. A probe goes first

One ticket answers the open points before a ticket builds the hybrid search:

- The request path and the request form of the embedding model in the plan of the deployment, the vector that one passage gets, the dimension option, and the usage that the reply reports.
- The vector extension in the local image and in the hosted database of the deployment.

The probe sends its live requests on a path that the ticket names before its dispatch, in the rules of the live model requests of [Delivery](../agents/delivery.md). When the probe fails for the Volcengine model, an amendment of this ADR selects a local model. The study names `Qwen3-Embedding-0.6B` and `bge-m3` as candidates.

### 6. The request revisions of Stage 7

[S7-B0](https://github.com/FrankQDWang/StoryOS/issues/1567) recorded these names on 2026-11-16. They are the one set of request revisions of Stage 7 for the requests of the Agent Plan mapping. The later Stage 7 tickets add their items, their Tools, their sections, their request forms, and their instruction paragraphs to these revisions in place, by the rule of ADR 0093 section 3: `main` has no `v8`, no `fa22`, and no `…0e0104` before the aggregate pull request of [#365](https://github.com/FrankQDWang/StoryOS/issues/365) merges.

- **The source classes.** The Core and the public contract get ten Context source classes. Each of them is Data-only Context. The manuscript classes are `chapter_list` (the Chapter list of section 1), `manuscript_read_result` (the result of the read), and `manuscript_search_result` (the result of the keyword search and of the hybrid search). The Memory classes of ADR 0094 are `memory_index`, `memory_read_result`, `memory_search_result`, `memory_capture_input`, `memory_delete_marks`, `memory_entries`, and `memory_observations`. `getAgentRun` shows the classes of a Run of the author.
- **The Purposes.** The Core gets the Purposes `memory_capture` and `memory_consolidation` of the background Memory work (ADR 0094 sections 5 and 6).
- **The grant scope.** The scope revision `project_writing_assistance_v5` names each Purpose and each source class of `project_writing_assistance_v4`, the ten source classes, and the two Purposes. It is the scope of each new grant (ADR 0051, amendment of 2026-11-16).
- **The mapping and the Registrations.** The mapping revision is `storyos.volcengine-agent-plan.responses.create.v8`. The head Model Registration is `018f0000-0000-7001-8000-00000000fa22`: Registration `fa21` with the mapping `v8`. The Host Tool Registration revision `018f0000-0000-7001-8000-0000000e0104` has the six Host Tools of `…0e0103`, and the mapping `v8` and the fake tool loop mapping declare it (ADR 0053, amendment of 2026-11-16). Until a later ticket selects an item of a new class or adds a Tool, a request of `v8` has the bytes of the `v7` request of the same input.
- **The Exposure rule.** One Core function decides if the Tool Exposure of a RunStep has a Host Tool, from the facts of the RunStep. Each Host Tool has one rule. Each later ticket that adds a Host Tool adds its fact and its rule.
- **The embedding request.** The embedding request of section 3 is not a request of this mapping. Its ticket adds its own grant scope value, `manuscript_embedding_v1`, with the Purpose `manuscript_embedding` and the classes `chapter_passage` and `search_question`. These are not values of the Context Purpose or of the Context source class, because no Context item has them.

## Considered options

- One search that always uses the vector side was rejected by the author. The keyword search is cheaper and answers most questions.
- A search with no vector side was rejected by the author.
- The whole manuscript in each context was rejected. The cost of each request would grow with the length of the novel.
- A local embedding model is the fallback. It adds a model file of 0.6 to 1.2 GB to the server, processor load, and a native dependency of the build, and its speed on a small server is not measured.
- A keyword index (PGroonga, or a BM25 extension) was rejected for now. One novel has about 10,000 to 20,000 blocks, and an exact scan is fast at that size. The BM25 extensions of the study have the AGPL-3.0 license.
- A separate vector database was rejected. PostgreSQL stays the one store, with one Project Isolation.

## Consequences

- The Chapter list and the results of the three tools are new Context source classes, so the standing grant needs a new scope revision (ADR 0051). The new tools change a request form, so the route needs a new mapping revision and a new Registration ([ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) section 3).
- Local development, the checks, and the release package use a PostgreSQL image with `pgvector`. The operator enables the extension in the hosted database.
- The embedding requests are a new cost of the deployment. The study gives CNY 0.70 for each million tokens of text at the list price.
- The old ticket for an embedding service for Memory has no owner content after ADR 0094. The refresh of the Stage 7 parent names the tickets of the manuscript tools.
