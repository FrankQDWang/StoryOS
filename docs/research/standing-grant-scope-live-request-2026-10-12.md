# Standing grant scope live request, 2026-10-12

Evidence record of [S4-03: Record the Standing Grant Scope and Refuse a Disclosure Outside It](https://github.com/FrankQDWang/StoryOS/issues/394) (ADR 0051). It records one live request to the qualified Volcengine Agent Plan route under the explicit standing grant scope. It is not a claim about model quality, Provider attention, retention, or training.

## Authorization and plan

- Author authorization of live requests: the standing authorization on #362 (2026-10-11).
- Planned live requests: 1, written before the run. Sent: 1. No retry. No anomaly.

## Setup

| Fact | Value |
| --- | --- |
| Build | Release package from commit `5c20af78b7c40fa2c0a93a43d49e3c8a3cd43881` |
| Deployment | Author's Mac, local HTTP profile, disposable PostgreSQL from `scripts/dev-postgres.sh`, Server with `STORYOS_WORKER=0`, one `storyos-worker --once` process |
| Route | Model Registration `018f0000-0000-7001-8000-00000000fa03`, compatibility Decision `qualified`, Credential Reference `settings:STORYOS_AGENT_PLAN_API_KEY`. No agent read the key. |
| Grant | `01a12ed2-f062-7893-ad6c-5e15987e18bc`, `grant_scope = project_writing_assistance_v1` (written by the binding of this build) |
| Content | Synthetic Project with two Chapters, 雨夜 and 渡口, one paragraph each, made in the author view for this run |
| Author request | `请改写《雨夜》的第一段和《渡口》的第一段`, sent from the panel while 渡口 was the current Chapter |

## Request

| # | Time (UTC) | Purpose | Outcome | Reported usage (input / output tokens) |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-12T06:35:51Z to 06:36:02Z | A cross-Chapter collection request under the standing grant scope | Dispatched with no author prompt. AgentRun `completed`. | 459 / 382 |

| Fact | Value |
| --- | --- |
| AgentRun | `01a12ed2-f046-7e71-9c87-b435b9ef2ce1` |
| Context | `sufficiency = complete`, `passage_targets` with 2 Chapters |
| Model Attempt | `01a12ed3-24fd-7ac1-a030-7f49b1f579e4`, `dispatch_state = settled`, one `create` request in the Worker log |
| Grant evidence | `disclosure_grant = {"grant_id": "01a12ed2-f062-7893-ad6c-5e15987e18bc", "grant_scope": "project_writing_assistance_v1"}` |
| Wire digest | `sha256:3a50940eb56528fe0d927aa1238775fb0df040aa57360b43a5eb1f1cdf34c4ef` |
| Sent content | The persisted serialized payload holds the paragraph of each of the two Chapters |
| Response reference | `resp_02179178695214914537b8110db3a4fe3c298b5fc7a850a79e435` |
| Decision | Advisory. The model text was a prose-change JSON object with a duplicated `"block_id":` key, so the Host did not validate it as a Prose Change and opened no Proposal. |

## Observations for later owners

- The model returned malformed JSON for a two-location change. The Host kept the text as an Advisory and executed nothing, as the Decision contract requires. Model output quality is outside StoryOS claims (S4-REQ-007). The real-author journey (S4-05) can meet the same case.
- In the author view, a Chapter selected in the outline stays read-only until the author uses 设为当前章节 in its Chapter menu.
