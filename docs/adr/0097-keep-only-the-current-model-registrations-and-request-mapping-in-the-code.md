---
status: accepted
---

# Keep Only the Current Model Registrations and Request Mapping in the Code

The coordinator decided this on 2026-11-20 in the cleanup of Stage 5 to Stage 7 ([#1715](https://github.com/FrankQDWang/StoryOS/issues/1715)), for the ticket [#1722](https://github.com/FrankQDWang/StoryOS/issues/1722).

## Context

Each change of a request mapping or a capability profile added a new Model Registration, and the code kept each earlier one. Before this decision, the code had:

- 23 Model Registrations, `…fa01` to `…fa23`. The deployment binds only two of them: `…fa18` for the fake destination and `…fa23` for the Agent Plan route.
- 4 Host Tool Registrations, `…0e0101` to `…0e0104`. Only `…0e0104` is current.
- 9 request mappings of the Agent Plan adapter, `responses.create.v1` to `responses.create.v9`, and the branches that build the earlier requests. Only `v9` is current.

The reason to keep an earlier Registration is a stored Run that names it: the Run must keep its request form, or it refuses (ADR 0053, ADR 0059, ADR 0093). [ADR 0079](0079-squash-the-migrations-into-one-baseline-and-derive-the-storage-identity.md) records that no database holds data. Thus no stored Run names an earlier Registration, and the earlier Registrations and mapping branches are code that no product path uses. Their tests prove only that the earlier requests stay the same.

## Decision

1. The code keeps only these Registrations:
   - the Model Registrations that `DeploymentDestination::registration()` binds: `…fa18` (`HOST_FAKE_TOOL_LOOP_REGISTRATION`) and `…fa23` (`AGENT_PLAN_REGISTRATION`);
   - the Embedding Registrations, the research Tool Registration of Exa, and the Host Tool Registration `…0e0104`.
2. The Agent Plan adapter has one request mapping, `v9`. It refuses each request on another mapping before dispatch, with `UnsupportedRequest`: a Create of each purpose, the qualification probe included, and a Retrieve. The fake destination keeps its own mapping identity.
3. A Run that pinned a removed revision refuses before dispatch. The current head is a different revision, so the check of the head refuses it with `model_registration_drift`. When the head check passes, the lookup gives no Registration, and the run refuses with `model_registration_unknown`.
4. The function calling probe of [ADR 0049](0049-admit-an-author-authorized-runtime-qualification.md) stays: `tool_probe`, its dispatch path, and its tests. The tests use one probe fixture, `PROBE_FIXTURE_REGISTRATION`: the revision `…fa16`, the current mapping, and a pending function calling entry. It is an ordinary constant in the one lookup list of `model_registration()`, so the test build and the product build have the same list, as for the Host fake Registrations. No deployment path binds it, and no Cargo feature and no second list select it. The Registration of a later production model with a pending entry replaces it.
5. Create admission keeps its check of an unqualified profile, the retrieval rule keeps its branch for an authorized qualification, and the compaction gate keeps its branch for a mapping that does not compact. No Registration of the one list needs these branches now, but a later provider version is a new Registration that can (ADR 0059). The tests examine each branch through its rule function, with no Registration that the test makes.
6. A test that names a removed Registration or mapping either goes, when it proves only the removed code, or moves to the current Registration with the same assertions, when it proves a behaviour that stays. A test of the drift refusal makes the earlier revision string in the test.

### The rule for later

A superseded Registration, and its mapping branch, stays in the code while a stored Run can name it. When a database holds data, a new Registration does not remove the earlier one. A later ticket removes it after a check shows that no stored Run that can continue names it, and the ticket records that check.

## Considered options

- **Keep each earlier Registration and mapping branch.** Refused: no stored Run needs them, and each later change of the adapter must keep about eight request forms the same.
- **Remove the probe path with the pending Registrations.** Refused: a later provider version needs the probe before its function calling qualifies, and ADR 0049 requires the probe.
- **Select the probe fixture with a Cargo feature or a test-only list.** Refused: the test build and the product build then resolve different lists, and a `#[cfg(test)]` item of `storyos-core` is not visible to the tests of the PostgreSQL adapter.

## Consequences

- [ADR 0053](0053-continue-the-agent-plan-conversation-with-the-prior-response.md), [ADR 0059](0059-pin-a-dated-provider-model-version-in-each-model-registration.md), and [ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) name Registrations and mappings that the code no longer has. Their decisions stay, and each links to this ADR.
- The research records under `docs/research/` keep the earlier revisions as evidence.
- A digest compare of the request bytes of the mapping `v9`, at the base and at the head of #1722, shows that the current requests do not change.
