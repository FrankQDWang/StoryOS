# Embedding route and pgvector image probe, 2026-11-16

Evidence record of [S7-C1: Probe the Embedding Route and the pgvector Image](https://github.com/FrankQDWang/StoryOS/issues/1572), for the open points of [ADR 0095](../adr/0095-give-the-agent-the-whole-manuscript-through-host-tools-and-add-a-hybrid-search-with-pgvector-and-a-volcengine-embedding-model.md) section 5. Part 1 records the live requests to the embedding model `doubao-embedding-vision-251215` on the Agent Plan route. Part 2 records the PostgreSQL image with `pgvector` and the proof of Project Isolation. It is not a claim about model quality, Provider attention, retention, training, or billing.

Raw evidence: the tag [`evidence/embedding-probe-2026-11-16`](https://github.com/FrankQDWang/StoryOS/tree/evidence/embedding-probe-2026-11-16/docs/research/embedding-and-pgvector-probe-2026-11-16/evidence). It holds the plan, the stdout JSON of each request with the request body and the complete reply, the request log, the vector comparison, and the outputs of the database runs. No file has a key, an e-mail address, or a home path.

## Result

- The model works on the route of the plan: `POST https://ark.cn-beijing.volces.com/api/plan/v3/embeddings/multimodal` with the key of the plan. S7-C2a1 builds on the Volcengine model. No amendment of ADR 0095 selects a local model.
- One request gives **one** vector, also for two text items. `dimensions: 1024` gives 1,024 values. The default is 2,048, and its first 1,024 values are the 1,024 vector.
- The same text gave a vector that is not identical (cosine 0.99902).
- An input above the 128K window is refused before work: HTTP 400 `InvalidParameter`, with the same message start as the Responses route.
- Image: `pgvector/pgvector:0.8.7-pg16-bookworm`. The Storage Activation and both recovery hold drills pass on it.
- Read form: an exact scan in the Project Scope, with no vector index. An HNSW read below the policy returned **no row** with the default settings.

## Part 1: the embedding model

### Documented facts

Each page was read on 2026-11-16.

| Question | Documented fact | Source |
| --- | --- | --- |
| Endpoint of the plan | The base URL of the plan for the embedding model is `https://ark.cn-beijing.volces.com/api/plan/v3` ("Agent Plan 专属 Base URL 中包含/plan"). The model is `doubao-embedding-vision` (`doubao-embedding-vision-251215`). The model name must be in the configuration; "Auto" does not select it. | [Plan: embedding models](https://ark.volcengine.com/region:cn-beijing/docs/agent-plan-personal-vectorized-models) |
| Request path | The API reference gives `POST https://ark.cn-beijing.volces.com/api/v3/embeddings/multimodal`. The plan page gives no path. Its OpenClaw example uses the provider `openai` with the base URL of the plan, which is the OpenAI form `/embeddings`. Its OpenViking example uses the provider `volcengine`. The documents give two forms; the probe used the multimodal path. | [Multimodal embedding API](https://ark.volcengine.com/region:cn-beijing/docs/multimodal-vectorization-api), plan page above |
| Request body, text only | `model`, `input` (a list of `{"type":"text","text":...}`), optional `dimensions`, `encoding_format` (`float`, `base64`), `instructions`, `multi_embedding`, `sparse_embedding`. | API reference |
| Dimension parameter | `dimensions`: `1024` or `2048`, default `2048`. | API reference |
| Response | The reference says that `data` is a list that agrees one to one with `input`. Its example shows `data` as one object with one `embedding` for three input items. | API reference |
| Question and passage | Retrieval uses two `instructions` forms. Query: `Target_modality: {}.\nInstruction:{}\nQuery:`. Corpus: `Instruction:Compress the {} into one word.\nQuery:`. Only the `{}` parts change. The document tells the user not to use the default instruction for a query. | [Embedding guide](https://docs.volcengine.com/docs/ark/vectorization?lang=zh) |
| Input limit | One text item is at most the maximum input tokens of the model. The model list gives a 128K window and 2,048 dimensions, with 1,024 as an option. The plan overview gives "上下文窗口：128K". | API reference, [model list](https://ark.volcengine.com/region:cn-beijing/docs/model-list), [plan overview](https://ark.volcengine.com/region:cn-beijing/docs/agent-plan-personal-plan-overview) |
| Rate limits of the plan | The plan has AFP quotas for each 5 hours, week, and month (Small: 2,000, 7,000, 20,000 AFP). The plan gives "满足正常开发需求的 TPM" and no number. The model list gives RPM 15,000 and TPM 1,200,000 for the pay-as-you-go API, not for the plan. | Plan overview, model list |
| Price in the plan | AFP of one request = (input tokens × 0.5 + output tokens × 0.5) / 10,000 for `doubao-embedding-vision`. | [AFP rules](https://ark.volcengine.com/region:cn-beijing/docs/agent-plan-personal-afp-credits-billing-rules) |
| Usage fields | `usage.prompt_tokens`, `usage.prompt_tokens_details.image_tokens`, `usage.prompt_tokens_details.text_tokens`, `usage.total_tokens`. | API reference |
| API calls | The plan overview says: "文本生成模型及向量化模型不可用于 API 调用，在非 AI 工具中使用 Agent Plan 权益对应的 Base URL 和 API Key 有可能被识别为滥用/违规，会导致订阅停用或账号封禁。" The same sentence covers the text model that StoryOS uses for the personal use of the author (ADR 0095 section 3). | Plan overview |

### Authorization and plan

- The author approved the live path and the amendment of [ADR 0049](../adr/0049-admit-an-author-authorized-runtime-qualification.md#amendment-the-embedding-probe-path-2026-11-16) on 2026-11-16. The [Delivery](../agents/delivery.md) standing approvals permit the requests. Personal, non-commercial use (ADR 0095 section 3).
- The plan was posted before the first request: [7 requests, cap 10](https://github.com/FrankQDWang/StoryOS/issues/1572#issuecomment-6910918082).
- Sent: E1 to E7, from 08:15:01 to 08:15:29 UTC. 7 requests in all. No retry, no repeat, no contingency request, and no anomaly.

### Setup

| Fact | Value |
| --- | --- |
| URL | `https://ark.cn-beijing.volces.com/api/plan/v3/embeddings/multimodal` |
| Model | `doubao-embedding-vision-251215` |
| Path | `storyos-worker --probe-embedding <case>` at commit `892b0f9b`, one process for each request. Each process does one `prepare` and one `exchange` of `AgentPlanEmbeddings`: one POST, HTTP/1.1, no retry, no redirect, no proxy, 120 s timeout. It reads no database. No AgentRun, Model Attempt, or Outbound Disclosure Event records the requests. |
| Credential | The Worker settings resolved `settings:STORYOS_AGENT_PLAN_API_KEY` (ADR 0050), with `STORYOS_MODEL_DESTINATION=volcengine_agent_plan` in the process environment. No agent read the value. The stdout and the log of each process have no key. |
| Log | `STORYOS_LOG=info`. No process wrote a log line. |

### Request bodies

Each body is the canonical JSON of these members: sorted keys, no whitespace. The SHA-256 of each body is in the table of requests.

```json
{
  "model": "doubao-embedding-vision-251215",
  "instructions": "Instruction:Compress the text into one word.\nQuery:",
  "input": [{"type": "text", "text": "The lighthouse keeper rowed to the island at dawn and found that the lamp was broken."}],
  "encoding_format": "float",
  "dimensions": 1024
}
```

- E1 and E2 send this body (passage A). E3 sends it with no `dimensions` member.
- E4 (question) sends `"instructions": "Target_modality: text.\nInstruction:Find the passage that answers this question\nQuery:"` and the text `Who went to the island to repair the light?`.
- E5 (passage B) sends the corpus instruction and the text `The bakery sold all of its rye bread before noon on market day.`
- E6 sends the corpus instruction and two text items: passage A, then passage B.
- E7 sends the corpus instruction and one text item: `The probe repeats one constant sentence. ` 25,000 times (1,025,000 characters).

### Requests

| # | Time (UTC) | Request ID | Digest | Result | Usage (`prompt_tokens`) |
| --- | --- | --- | --- | --- | --- |
| E1 | 08:15:01 | `021794816901896c280e9bf492d190027beb8b799de38089b8c3d` | `sha256:a9c922e1…` | HTTP 200, one vector of 1,024 values, 316 ms | 40 |
| E2 | 08:15:19 | `021794816919793a4e0bfe9b623ca70f9090a72d521bae21b5999` | `sha256:a9c922e1…` | HTTP 200, one vector of 1,024 values, 498 ms | 40 |
| E3 | 08:15:19 | `0217948169202806cb63cadb26a6eb1a5dd8bb71cce3d7fb4612c` | `sha256:ca9d4c16…` | HTTP 200, one vector of 2,048 values, 423 ms | 40 |
| E4 | 08:15:20 | `021794816920829bbc1b38287ece59059a49659ad58bad9333957` | `sha256:00018207…` | HTTP 200, one vector of 1,024 values, 489 ms | 37 |
| E5 | 08:15:20 | `0217948169213794f644a6b4f65cd7c798419da6087253154af72` | `sha256:001e9668…` | HTTP 200, one vector of 1,024 values, 452 ms | 36 |
| E6 | 08:15:21 | `021794816921862af71e85a49ed90f7564e7e8026279fe73a9441` | `sha256:e794d517…` | HTTP 200, **one** vector of 1,024 values, 417 ms | 55 |
| E7 | 08:15:28 | `021794816928756593075d5bfc3e0cbf107ff469a4d1c91eef235` | `sha256:7b297872…` | HTTP 400, `{"code":"InvalidParameter","type":"BadRequest","message":"Total tokens of image and text exceed max message tokens. Request id: …"}`, 1,219 ms | none |

The request ID is the `x-request-id` header. For E1 to E6 it is also the `id` of the reply. Each reply had `object: list`, `model: doubao-embedding-vision-251215`, `data: {object: embedding, embedding: [...]}`, and `usage` with `image_tokens: 0`. Usage of E1 to E6: 248 tokens, approximately 0.0124 AFP. The reply of E7 has no usage.

### What the replies show

- **Usage.** `prompt_tokens` counts the instruction too: passage A has 17 words and 40 tokens. The documents of an earlier version give about 20 added tokens for each request.
- **Same text.** E1 and E2 differ: the largest difference of one value is 0.00586, and the cosine similarity is 0.99902. A vector is not a stable identity of a text. The vector side must compare a digest of the text, not the vector.
- **Default length.** E3 has 2,048 values. Its first 1,024 values have cosine similarity 1.0000 to E1.
- **Norm.** The norm of each vector is between 0.9978 and 1.0029, not exactly 1. The cosine operator is the correct distance, not the inner product.
- **Distance.** Cosine distance of the question (E4) to passage A (E1): 0.4750. To passage B (E5): 0.9688. This is not a claim about model quality.
- **Two items.** E6 has one vector, not two. Its cosine similarity is 0.6722 to E1 and 0.6388 to E5. The multimodal path combines the items into one vector. One passage needs one request.
- **Input limit.** E7 was refused with HTTP 400 after 1,219 ms. The native message starts with `Total tokens of image and text exceed max message tokens`, the same start as `INPUT_TOO_LONG_MESSAGE` of the Responses adapter.

### What this evidence qualifies

- The request path, the request form for text only, the 1,024 dimension option, the default length, one vector for each request, and the usage fields, on the exact route of the plan, the dated model version, and the key of the deployment.

### What it does not qualify

- The OpenAI form `/api/plan/v3/embeddings`, `encoding_format: base64`, sparse and multi vectors, and the rate limits of the plan.
- The quality of retrieval in Chinese text, and the best wording of the query instruction.
- If the route bills a refused request.
- The terms of use: the sentence of the plan overview about API calls stays as ADR 0095 section 3 records it.

## Part 2: the vector extension

### The image

| Fact | `pgvector/pgvector:0.8.7-pg16-bookworm` | `postgres:16-alpine` (baseline) |
| --- | --- | --- |
| Index digest | `sha256:484e838076e04939d27d3e176e91c86eea96236126023be8d3059761122e579f` | `sha256:81e77a4833487b8132fe4b00bedd39dc3f74b8641e387f19312a48f7de218c2e` |
| PostgreSQL | 16.16 (Debian 16.16-1.pgdg12+1) | 16.16 |
| Base | Debian 12 (bookworm), glibc | Alpine Linux, musl |
| pgvector | 0.8.7 (2026-10-01), in `/usr/share/postgresql/16/extension` | none |
| Size | 162 MB amd64 and 160 MB arm64 compressed; 481 MB local | 116 MB amd64 and 114 MB arm64 compressed; 288 MB local |
| `postgres` user | uid 999, gid 999 | uid 70, gid 70 |

The pgvector license is the PostgreSQL License ([LICENSE](https://github.com/pgvector/pgvector/blob/master/LICENSE)). Sources: [Docker Hub tags](https://hub.docker.com/r/pgvector/pgvector/tags), read on 2026-11-16.

The differences that a script depends on:

- Both images have the same entrypoint, the line `PostgreSQL init process complete`, `PGDATA=/var/lib/postgresql/data`, the user name `postgres`, `pg_basebackup`, `psql`, `pg_isready`, `sh`, `sha256sum`, `find`, `cp -a`, `dd`, and `chown`. The scripts use the user name, not the uid. The GNU tools of Debian accept each option that the scripts use.
- **Text order.** Both images initialize with `en_US.utf8`. musl sorts it by code point: `A B _z a b e é`. glibc sorts it by language: `a A b B e é _z`. Thus `ORDER BY` on text gives a different order. `POSTGRES_INITDB_ARGS=--locale=C.UTF-8` on the Debian image gives the order of Alpine. S7-C2b1 must decide this.

The lines that S7-C2b1 changes, at `main` `a401c82a`:

- `scripts/lib/controlled-postgres.sh`: line 13.
- `scripts/verify-recovery-hold.sh`: lines 80, 88, 253, 259, 283, 308, 316, 350, 362, and 376.

### The scripts on the image

With a local change of these 11 lines to `pgvector/pgvector:0.8.7-pg16-bookworm`, not committed, and the release package of commit `892b0f9b`:

| Run | Result |
| --- | --- |
| `scripts/dev-postgres.sh with ...`: the Storage Activation of `storyos-storage`, then the controlled fixture | PASS. The proof phase is `active`, and the fixture has 2 Projects. |
| `STORYOS_RECOVERY_DRILL=fixture-only scripts/verify-recovery-hold.sh` | PASS in 71 s. RPO 0 s of 900 s, RTO 9 s of 7,200 s. |
| `STORYOS_RECOVERY_DRILL=mixed scripts/verify-recovery-hold.sh` | PASS in 99 s. |

The change was reverted after the runs. The tag holds its diff.

### The proof of Project Isolation

[`embedding-and-pgvector-probe-2026-11-16-isolation.sql`](embedding-and-pgvector-probe-2026-11-16-isolation.sql) ran on a new container of the image as the superuser. It makes the table `probe.passage_vectors` with a `vector(1024)` column, forced row level security, and the exact Scope policy of the baseline (`manuscript_objects_exact_scope`). `probe_runtime` has the rights of `storyos_runtime`: no login, no superuser, no `BYPASSRLS`, and `SELECT, INSERT`. Project 1 has 20,000 rows on a curve in 1,024 dimensions. Project 2 has 1,000 rows near the question vector. The rows went in through the policy. The index is HNSW with `vector_cosine_ops` and the default build settings.

| Read as `probe_runtime` | Result |
| --- | --- |
| No Scope | 0 rows |
| Scope of Project 1 | 20,000 rows, 0 of another Project |
| Scope of Project 2 | 1,000 rows, 0 of another Project |
| Owner of Project 2 with the Project of Project 1 | 0 rows |
| Farthest row of Project 2 and nearest row of Project 1 to the question | 0.000629 and 0.000733: each row of Project 2 is nearer |
| Nearest 10 of Project 1, exact scan | `{4277,10001,10000,4276,4278,10002,9999,4275,4279,10003}` in 72.1 ms. The curve comes near itself at 4,277. |
| The same read with HNSW and the default settings | **0 rows**. The index gives its 40 nearest candidates (`hnsw.ef_search`), and the policy removes each of them. |
| HNSW with `SET LOCAL hnsw.iterative_scan = strict_order` | The same 10 rows in the same order, in 2.9 ms. The index scan removed the 1,000 rows of Project 2 first. |
| The same with `SET LOCAL hnsw.max_scan_tuples = 500` | 0 rows. The iterative scan stops at its bound, 20,000 by default. |
| No vector index and the default settings | The same 10 rows, in 61.4 ms (sequential scan) |

With the index, the planner also changes `min(distance)` into an HNSW read, and that aggregate gave no value with the default settings. The probe file therefore turns the index off for each exact read.

### The read form

S7-C2b1 and S7-C3a use an **exact scan in the Project Scope**: `ORDER BY embedding OPERATOR(extensions.<=>) $question LIMIT k` below the Scope policy, with the cosine operator and no vector index. The reasons:

- The result is correct by construction below the policy, and it is deterministic for the tests.
- An HNSW read below the policy is not correct with the default settings. With `strict_order` it was correct here, but its result depends on the number of nearer rows of other Projects (`hnsw.max_scan_tuples`), and it stays approximate.
- The exact scan took 61 to 72 ms at the upper size of one novel (20,000 rows).

Because the table has rows of all Projects, a sequential scan grows with all the rows of the deployment. When that becomes slow, the measured form of the index is HNSW with `vector_cosine_ops`, and each read sets `SET LOCAL hnsw.iterative_scan = strict_order` and an `hnsw.max_scan_tuples` above the row count of the deployment. That change needs its own record.

### Schema, role, and rights

- **Schema:** `extensions`, the schema of the hosted database. Each name is qualified: `extensions.vector(1024)` and `OPERATOR(extensions.<=>)`.
- **Role that creates it:** `vector` is not a trusted extension (its control file has no `trusted`), so the admin role of the Storage Activation creates it, not `storyos_owner`.
- **Rights of `storyos_runtime`:** `USAGE` on the schema `extensions`, and the table rights (`SELECT, INSERT, DELETE`). `storyos_owner` needs `USAGE` on `extensions` to make the table.

[`embedding-and-pgvector-probe-2026-11-16-migration.sql`](embedding-and-pgvector-probe-2026-11-16-migration.sql) is the migration text. It runs after the baseline in the transaction of the Storage Activation. When `pg_available_extensions` has `vector`, it creates the extension in `extensions`, refuses an extension in another schema, gives the rights, and makes the table as `storyos_owner`. Else it does nothing. Results after the real Storage Activation of the release package:

| Image | Result |
| --- | --- |
| `pgvector/pgvector:0.8.7-pg16-bookworm` | PASS: `storyos.passage_vectors`, owner `storyos_owner`, forced row level security, the primary key as its only index, pgvector 0.8.7. As `storyos_runtime` in a Scope: one insert, one count of 1, and one nearest read. |
| `postgres:16-alpine` | PASS: no vector table and no extension |

The tag holds the output of the first version of the text, which also made an HNSW index. The review of round 1 removed that index, because it contradicts the read form. The table above is the result of the final text.

### The hosted database

Documented facts only, read on 2026-11-16. ADR 0022 names hosted Supabase.

| Question | Documented fact | Source |
| --- | --- | --- |
| Offered | Yes. The extension name is `vector`. | [pgvector](https://supabase.com/docs/guides/database/extensions/pgvector) |
| How the operator enables it | Dashboard: Database, Extensions, "vector". Or SQL: `create extension vector with schema extensions;`. | pgvector page |
| Schema | Most extensions are in `extensions`, which `public` can use by default. | [Extensions](https://supabase.com/docs/guides/database/extensions) |
| Version | The image registry of Supabase Postgres lists `vector` 0.5.1 to 0.8.2 for PostgreSQL 15 and 17. Since 2026-08-05, a version clause is ignored, and the default version is installed. | [`nix/ext/versions.json`](https://github.com/supabase/postgres/blob/develop/nix/ext/versions.json), [changelog](https://supabase.com/changelog/extension-version-pinning-ignored) |
| Index types | HNSW and IVFFlat. Iterative scans since pgvector 0.8.0, off by default (`hnsw.iterative_scan`, `hnsw.max_scan_tuples` default 20,000). | [HNSW indexes](https://supabase.com/docs/guides/ai/vector-indexes/hnsw-indexes) |
| PostgreSQL | Supabase Postgres has PostgreSQL 15.19 and 17.11. A new hosted project has 17, not 16. | [supabase/postgres](https://github.com/supabase/postgres) |

The checks that stay for the deployment preparation of ADR 0022:

- The extension, its version, and its schema in the real hosted project.
- The isolation proof file and the migration text on that project, with its non-superuser `postgres` role.
- The Storage Activation on that project.

This ticket changed nothing in a hosted project.
