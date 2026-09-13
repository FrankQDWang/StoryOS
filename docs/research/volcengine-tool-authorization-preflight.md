# Volcengine Tool Authorization Preflight

Date: 2026-09-13. Scope: public first-party documentation. This note makes no
credential, inference, account, or exact-model acceptance claim. It supports
[ADR 0034](../adr/0034-bound-provider-hosted-tool-operations.md).

## Current integration scope

The current
[MCP research registration ticket](https://github.com/FrankQDWang/StoryOS/issues/397)
requires one approved real operation. It leaves the exact service, account,
allowed operations, destination, and spend to that integration ticket. The
Stage 5 Responses specification refresh is still pending. This note does not
pretend that a concrete MCP server or full Tool inventory is already selected.

The author-approved hosted scope is search, reading, and temporary computation.
That scope permits eligible capabilities; it does not assert that every named
Provider exposes a general code interpreter or require one for research.

## General Ark Responses evidence

The current [Create Response reference](https://docs.volcengine.com/docs/82379/1569618?lang=zh)
documents these controls for the general Ark API:

| Surface | Observed contract | StoryOS consequence |
| --- | --- | --- |
| `web_search` | The Tool configuration has no approval-continuation field. | Configure the permitted search mode before submitting an admitted request. |
| Remote MCP | `allowed_tools` selects explicit Tool names. `require_approval` defaults to `always` and supports `never` or named filters. | An exact permitted Tool set can use pre-submission approval configuration. Set `never` only after current StoryOS authority covers that complete set and intake. |
| `max_tool_calls` | The value controls rounds, not calls within a round. The documented range is 1 through 10 and enforcement is best effort. | Do not use this field alone as an enforceable Tool-count or cost ceiling. |

These documented fields do not prove support for the user's exact Agent Plan
endpoint, model, or account. They also do not prove internal Tool behavior.
Unknown required limits remain a qualification gap; no prompt or completed
response converts a best-effort field into a hard guarantee.

## Approval conclusion

The examined controls do not establish a present requirement for a generic
pause, author-approval, and resume workflow. Use a selected Tool's supported
pre-submission authorization mode when it meets the StoryOS operation contract.
Do not add a speculative product ban on approval continuation.

If a concrete selected Tool requires a Provider approval exchange, inspect
that exact exchange and current authority. A covered protocol decision is not
a new author Approval. Any missing continuation contract belongs to its
existing semantic owner before that specific mode is enabled.

## Agent Plan boundary

Agent Plan Responses and separately configured Harness MCP services are
different integration paths. The existing
[Agent Plan capability preflight](agent-plan-responses-capability-preflight.md)
records the first-party route and CLI evidence. A Harness entitlement is not
proof of in-Response hosted Tool support. Ordinary StoryOS-dispatched MCP calls
retain the Tool Gateway boundary even when their service is sold with Agent Plan.

Exact capability, effect bounds, account availability, and spending evidence
remain part of the selected integration's qualification. This planning note
neither makes a paid request nor releases Stage 5.
