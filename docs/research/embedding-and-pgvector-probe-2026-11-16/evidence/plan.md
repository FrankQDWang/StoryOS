## Plan of the live requests

- Purpose: the facts of ADR 0095 section 5 for the embedding model `doubao-embedding-vision-251215` on the Agent Plan route of the deployment. The requests send constant text only and no Project content.
- Path: `storyos-worker --probe-embedding <case>`, one process for each request. Each process does one `prepare` and one `exchange` of the new embedding function: one POST, no retry, no redirect. The key comes only from the Worker settings (Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`). The process reads no database.
- URL: `https://ark.cn-beijing.volces.com/api/plan/v3/embeddings/multimodal`. This is the path of the API reference of the model (`/api/v3/embeddings/multimodal`) below the base URL of the plan (`/api/plan/v3`).
- Body: `{"model":"doubao-embedding-vision-251215","instructions":<text>,"input":[{"type":"text","text":<text>}, ...],"encoding_format":"float"}`, with `"dimensions":1024` when the case has it. A passage has the corpus instruction of the documents, and a question has the query instruction.
- Planned: 7 requests. Cap: 10.

| # | Case | Shows |
| --- | --- | --- |
| E1 | `passage-1024`: passage A, 1,024 dimensions | Status, number of vectors, length, usage |
| E2 | `passage-1024` again | Same text, same vector? |
| E3 | `passage-default`: passage A, no dimension option | Default length |
| E4 | `question-1024`: one question that fits passage A | The query form |
| E5 | `other-passage-1024`: passage B that does not fit the question | With E1 and E4: the distance of the question to each passage. Not a claim about model quality. |
| E6 | `two-items-1024`: passages A and B in one request | Number of vectors in the reply |
| E7 | `over-limit-1024`: one constant sentence repeated to about 175,000 tokens, above the 128K window of the model list | Status and native reason of the rejection |

- Expected usage: E1 to E6 below 100 input tokens each. E7 about 175,000 input tokens if the route counts it. At 0.5 AFP for each 10,000 tokens, the maximum is about 9 AFP.
- Contingency: when E1 gets HTTP 404, one extra request E1b sends the same text to `https://ark.cn-beijing.volces.com/api/plan/v3/embeddings` (the OpenAI form of the OpenClaw page of the plan), and E2 to E7 use the path that works. When the route refuses the model for the plan, I stop and report to the coordinator, as the ticket says.
