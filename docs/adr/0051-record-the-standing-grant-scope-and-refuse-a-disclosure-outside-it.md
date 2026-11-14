---
status: accepted
---

# Record the Standing Grant Scope and Refuse a Disclosure Outside It

On 2026-10-12, at the refresh gate of [S4-03: Record the Standing Grant Scope and Refuse a Disclosure Outside It](https://github.com/FrankQDWang/StoryOS/issues/394), the author accepted this decision. The standing Project Destination Grant covers all current content of its Project for the writing-assistance Purpose on the deployment destination. A length limit is not a disclosure bound. Thus Stage 4 has no request outside the grant, and Stage 4 asks the author for no disclosure approval.

## Context

At `main` `56df5eab`, each Project Destination Grant has only a `grant_kind`: `host_fake_use` or `volcengine_agent_plan_use`. The grant names its Processing Destination Identity, but it names no Purpose, no outbound data category, and no bound. The Context contract (section 9.4) says that a request outside the grant needs a one-shot Destination Disclosure Approval. Without a recorded scope, no request is outside the grant, and the Host cannot show that a dispatch is inside it.

The author made this decision from the view of an author. The author writes all of the Project with the assistant, and expects the assistant to read each Chapter of the Project without a question. The Context item limit of 10,000 characters is a technical context limit of one request. It does not tell what the author permits to send. A separate ticket measures the input limit of the model and changes that limit.

## Decision

- Each new Project Destination Grant records its `grant_scope`. The only scope is `project_writing_assistance_v1`. It names these items:
  - The Project Scope of the grant.
  - The Purpose `current_passage_assistance`.
  - The source classes `author_instruction`, `working_target`, and `instruction_binding`. The Working Target can be the current Chapter or a Proposal candidate. It can also be a collection of passages from more than one Chapter. Each is in the same Project.
  - The Processing Destination Identity of the grant.
- The scope sets no length bound. The Context item limit and the request bounds of the binding (ADR 0049) stay technical limits.
- A request inside the scope dispatches with no prompt. Create admission and the later dispatch claim examine the scope of the grant that the AgentRun pinned, also after a restart. A successor Attempt of ADR 0033 examines it again.
- A request that the scope does not cover refuses before the dispatch claim with `disclosure_outside_grant`. It writes no Model Attempt and no Outbound Disclosure Event. These requests are outside the scope:
  - A Context Assembly record of a different Project Scope.
  - A Purpose or a selected source class that the scope does not name.
  - A request destination or a use binding identity that is not the identity of the grant.
  - A grant without a known scope.
- The author sees one plain-language sentence for this refusal (ADR 0040).
- A grant row from before migration 0090 has no scope. It does not authorize dispatch. The next `createAgentRun` makes new binding records with a scope (ADR 0048). An AgentRun that pinned the old binding refuses before the dispatch claim.
- Each Model Attempt records the grant and its scope in its evidence.
- The grant scope adds no creative authority. It is different from a Tool Approval and from Acceptance.

## Considered options

- An approval for text from a Chapter that is not open was rejected. The author expects the assistant to read all of the Project.
- An approval for a request above a length limit was rejected. A length limit is a technical context limit, not an authorization.
- A private mark for content that the assistant must not read without approval was deferred. It is a new author feature.
- A one-shot approval at a change of the deployment destination was rejected for Stage 4. It asks again for each request, and the deployment offers one destination (ADR 0048).

## Consequences

- Migration 0090 adds the nullable `grant_scope` column to `project_destination_grants`. The Project archive export holds the new column.
- [Approve One Exact Destination Disclosure Outside the Standing Grant](https://github.com/FrankQDWang/StoryOS/issues/1102) owns the one-shot Destination Disclosure Approval, its Wait, its `decideApproval` target, and its author decision. The first stage that adds a disclosure outside the grant delivers it. ADR 0040 keeps that author decision type for it. Stage 4 has no such decision.
- A new Purpose, source class, or destination needs a new scope revision or that approval.

## Amendment 2026-11-15: the scope project_writing_assistance_v4

[ADR 0089](0089-deliver-standard-skills-from-the-deployment-and-load-them-with-host-tools.md) section 4 ([#1528](https://github.com/FrankQDWang/StoryOS/issues/1528)) adds the scope revision `project_writing_assistance_v4`. It names each Purpose and each source class of `project_writing_assistance_v3` and the Skill source classes `skill_catalog`, `skill_instruction`, and `skill_resource`. The provision of a Skill by the operator of the deployment is the consent to send its text, so the Host makes the scope current with no action of the author. Each new grant records it. The scopes `project_writing_assistance_v1` to `v3` and `research_tool_v1` name no Skill source class, so a request with a Skill item under one of them refuses with `disclosure_outside_grant`. A Project whose current grant has an earlier scope gets new binding records at its next `createAgentRun`, and each earlier record stays unchanged. The two `grant_scope` CHECK lists of the Release 1 baseline have the new value (ADR 0079).
