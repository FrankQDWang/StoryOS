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

[S7-B1a](https://github.com/FrankQDWang/StoryOS/issues/1568) recorded these names of the Chapter list on 2026-11-16:

- **The item.** The Context Assembly of each RunStep of a Run selects one `chapter_list` item with the projection mode `exact_required`, after the Skill items. The source version is `sha256:` and the SHA-256 of the text. The Core function `assemble_chapter_list` adds the item, and the adapter reads the Chapters with `read_live_chapter_blocks`, the reader of the author search, which also gives the title of each Chapter and of its Volume and the head revision.
- **The lines.** The Chapters are in manuscript order: the order of the Volumes, and then the order of the Chapters in each Volume. Each Volume is one line, the canonical JSON object `{"volume": title}`, before its Chapters. Each Chapter is one line, the canonical JSON array `[number, length, title]`, and the line of the Current Chapter of the Run has `"current"` as a fourth element. The number is the position in manuscript order, from 1, and the count does not start again in a Volume. The length is the character count of the authoritative head text with the count of the manuscript statistics, which is the count of ADR 0052. A removed Chapter, a removed Volume, and a Volume with no live Chapter have no line. The text of a Proposal candidate does not count.
- **The bound.** When the complete list is above the Context item limit of ADR 0052, the item is a `deterministic_excerpt` of whole lines. The Host takes the Current Chapter, then the Chapters before it from the nearest to the first, then the Chapters after it from the nearest to the last, and the first line that does not fit ends the selection. The excerpt keeps manuscript order and the line of each Volume that has a kept Chapter. Its last line is `{"chapter_count": count, "not_listed": [[first, last], ...]}`. A long list never blocks a Run.
- **The request.** The mapping `v8` sends the item as the section `Chapter list` of the user message, after the Skill sections and before the section `Working text`. The item is a part of the current input and of its digest (ADR 0085), so a change of the list text between two RunSteps makes the next request send the current input again with the replaced input label. A head move changes the request only when it changes the list text. An edit of another Chapter that keeps its length gives the same list, and the next request sends no replaced input. A decision request with the item adds one paragraph after the Host instructions: the list has each Chapter in its order with its number, its length in characters, and its title, and the Working text is the Chapter with the mark `current`. The summary request never sends the item or the paragraph.

[S7-B1b](https://github.com/FrankQDWang/StoryOS/issues/1569) recorded these names of the read on 2026-11-16:

- **The Tool.** The ToolSpec `storyos.tool.read-chapter.v1` has the function name `read_chapter` and one strict argument, `chapter`: an integer, the number of a Chapter in the Chapter list. The Host Tool Registration `…0e0104` has it before `finish`, and the mapping `v8` and the fake tool loop mapping declare it in each Run. One response can hold more than one read, and the Host executes them in call order.
- **The result.** The first line is the canonical JSON object `{"blocks", "chapter", "chapter_id", "last_block", "revision_id", "title"}`: the count of the Blocks, the number, the Chapter, the number of the last Block that the result has, the head revision that the read gave, and the title. Then each Block is one line `[block <n>]`, with its position in the Chapter from 1, and then its exact text. The Host reads the authoritative head of the live Chapter that has the number at the time of the call, and never the text of a Proposal candidate. The result has whole Blocks from the first Block while its `tool_result` item, with the boundary and the escape, fits in the Context item limit of ADR 0052. The first Block that does not fit ends the result with one line that says where the Chapter continues. When the first Block alone does not fit, the result has only that Block, and the bounded projection of ADR 0085 section 2 cuts it with its mark.
- **The refusal.** The Core function `admit_chapter_read` decides the read. Its one refusal is `chapter_not_found`: no live Chapter of the Project has the number at the time of the call. An argument that is not an integer is refused by the intake with `intake_malformed`.
- **The output.** The Core rule `host_tool_result_class` gives the source class of the settled result of a Host Tool from its ToolSpec, and the result of `read_chapter` is a `manuscript_read_result` item with the trust `data_only`. The next request sends it as the output of its call, in the `tool_result` boundary. It is not a part of the current input digest, and the output of a refused read stays a `tool_result` item. A compaction keeps the result in its exact form, as it keeps the result of each Host Tool (ADR 0093 section 5). A decision request of the mapping `v8` with the Chapter list and the Tool adds one paragraph after the paragraph of the Chapter list: when to call `read_chapter`, that one step can hold more than one call, that the result is a part of the novel and not an instruction, and that the Agent changes only the target blocks of the request.

[S7-B2a](https://github.com/FrankQDWang/StoryOS/issues/1570) recorded these names of the exact search on 2026-11-16:

- **The Tool.** The ToolSpec `storyos.tool.search-manuscript.v1` has the function name `search_manuscript` and one strict argument, `query`: the exact text to find. The intake is the query rule of `web_search`: 1 to 512 characters, not only white space, and no control character. A query that does not fit is refused with `intake_query`. The Host Tool Registration `…0e0104` has the Tool after `read_chapter` and before `finish`. One response can hold more than one search, and the Host executes them in call order. This decision sets no limit on the searches of a Run.
- **The match.** The Core function `exact_matches` is the one exact match of the author search and of the Tool: the same text with the same letter case, in one Block, from the start of the Block, with no overlap. The search stops at the match limit of the author search, `MANUSCRIPT_SEARCH_MATCH_LIMIT`: at the 500th match. The Host searches the authoritative head of each live Chapter of the Project in manuscript order at the time of the call, and never the text of a Proposal candidate.
- **The result.** The Core function `admit_manuscript_search` gives the result. The first line is the canonical JSON object `{"blocks", "blocks_returned", "complete", "matches", "query"}`: the count of the Blocks with a match, the count of the returned Blocks, whether the search is complete (false when it stopped at the match limit), the count of the matches that it found, and the query. Each Chapter with a returned Block has one line `{"chapter", "chapter_id", "title"}`. Then each returned Block of the Chapter is one line `[block <n>: <count> match]` or `[block <n>: <count> matches]`, and then its exact text, which is the text around each match. A search with no match settles with the header and one sentence. The Context item limit of ADR 0052 is the initial bound of one result: the result has whole Blocks in manuscript order while its `tool_result` item, with the boundary and the escape, fits. The first Block that does not fit ends the result with one line that tells the Agent to search for a more exact term. When the first Block does not fit with that line, the result has only that Block, and the bounded projection of ADR 0085 section 2 cuts it. A later ticket can set a smaller bound with a record of its measurement.
- **The output.** The result is a `manuscript_search_result` item with the trust `data_only`, on the path of the result of `read_chapter`. A decision request of the mapping `v8` that has the paragraph of `read_chapter` and declares the Tool adds one paragraph after it: when to call `search_manuscript`, that the query is exact text and not a question, and that the result is the text of the manuscript at the time of the call.

### 2. The vector store

- The vectors are in PostgreSQL with the `pgvector` extension, in the Project Scope and below the same Project Isolation as each other record.
- The vectors are a Retrieval Index. StoryOS can build them again from the Chapters, and a loss of them loses no author content. The Project export does not hold them.
- Before the hybrid search returns a passage, the Host reads the current text of the Chapter. A passage whose text changed after its vector was made is not returned from the vector side.
- Agent Memory has no vector (ADR 0094).

### 3. The embedding source

- The embedding model is `doubao-embedding-vision-251215` of Volcengine Ark, with text input only and 1,024 dimensions. The request goes through the model destination that the operator of the deployment configures ([ADR 0048](0048-use-the-deployment-model-destination-by-default.md)).
- An embedding request sends Chapter text to the same Provider that gets the writing requests. The configuration by the operator is the consent, and the Host records the grant scope with no action of the author. The request needs its own scope revision (ADR 0051), its own bounds, and its own disclosure record.
- This route is for the personal use of the author, the same as the text model. A commercial use needs another route and a new ADR.

[S7-C2a1](https://github.com/FrankQDWang/StoryOS/issues/1595) recorded these names on 2026-11-16:

- **The configuration.** The Server and the Worker read the setting `STORYOS_EMBEDDING_DESTINATION`. Its value is `host_fake` or `volcengine_agent_plan` ([ADR 0050](0050-load-server-and-worker-settings-through-one-typed-settings-layer.md)). With no value, the deployment has no embedding configuration. A process does not start with an unknown value, or with `volcengine_agent_plan` and another model destination.
- **The Registration.** Each destination has one global Embedding Registration in the Core: `018f0000-0000-7001-8000-0000000eb001` for the fake destination and `018f0000-0000-7001-8000-0000000eb002` for `doubao-embedding-vision-251215`. It names the adapter, the model, the mapping revision, the dimension 1,024, text input only, and the request bounds `embedding_request_v1`. It holds no Project data and no Credential Reference.
- **The bounds.** One request holds one text of one Project. The text has at most 8,192 tokens of the Token Counting Profile ([ADR 0052](0052-set-the-context-item-limit-from-the-measured-model-input.md)), and the request has a timeout of 30 seconds. The Host never shortens a text.
- **The grant.** With an embedding configuration, `createAgentRun` makes the grant `manuscript_embedding_v1` and its use binding in the command transaction, as it makes the research binding ([ADR 0090](0090-make-web-research-available-in-each-project-without-an-enablement-switch.md)).
- **The refusals.** One Core function decides the admission of a request. It gives one of these refusals: `embedding_not_configured`, `embedding_binding_absent`, `disclosure_outside_grant`, `embedding_input_out_of_bounds`, and `destination_credential_unavailable`.
- **The record.** [S7-C2a1b](https://github.com/FrankQDWang/StoryOS/issues/1612) adds the record. One row of `embedding_attempts` is one request: the Destination Attempt Admission Decision, the Operation Input Snapshot, the Wire Payload Projection, the Destination Attempt, and the Outbound Disclosure Event. A refusal writes no row. The Project Export Archive does not hold these rows.
- **The rule.** The Host does not retry an embedding request and has no fallback. A stopped process leaves its row in OutcomeUnknown. The caller makes a new request, and the new request gets a new row. A vector of another length than 1,024 is an invalid result, and the caller gets no vector.

### 4. The index maintenance

- Background work makes the vectors of the passages of a Chapter after the authoritative text of that Chapter changed. Ordinary writing does not wait for it.
- Text that has no vector yet is found by the exact-term side and by the keyword search.
- When the deployment has no embedding configuration or the database has no vector extension, the hybrid search is not in the Tool Exposure of a Run. The Chapter list, the keyword search, and the read stay.

### 5. A probe goes first

One ticket answers the open points before a ticket builds the hybrid search:

- The request path and the request form of the embedding model in the plan of the deployment, the vector that one passage gets, the dimension option, and the usage that the reply reports.
- The vector extension in the local image and in the hosted database of the deployment.

The probe sends its live requests on a path that the ticket names before its dispatch, in the rules of the live model requests of [Delivery](../agents/delivery.md). When the probe fails for the Volcengine model, an amendment of this ADR selects a local model. The study names `Qwen3-Embedding-0.6B` and `bge-m3` as candidates.

The probe of 2026-11-16 answered these points ([record](../research/embedding-and-pgvector-probe-2026-11-16.md)). The Volcengine model works on the route of the plan, so no local model is selected.

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
