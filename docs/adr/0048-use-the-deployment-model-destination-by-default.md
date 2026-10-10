---
status: accepted
---

# Use the Deployment Model Destination by Default

On 2026-10-10, at the refresh gate of [S4-01: Bind the Selected Real Model to a Project](https://github.com/FrankQDWang/StoryOS/issues/392), the author accepted this decision. Each deployment offers one model destination. A Project uses that destination by default, and the author does not select a destination. The Contract-Faithful Fake Destination is only for development and test deployments. The credential of the destination has one of two sources: an operator quota or a key that the author supplies (BYOK). Stage 4 implements only the operator quota.

## Context

At `main` `7b131cee`, migration 0045 permits only the Contract-Faithful Fake Destination. Each Project has at most one Processing Destination Identity, one Project Destination Grant, one `ProjectExternalUseBindingRevision`, and one External Contract Compatibility Decision. Only the `updateProjectAssistance` command makes these records, and the Protected Web Client does not send that command. Thus no production Project can get assistance, and no Project can move to another destination.

[ADR 0033](0033-use-volcengine-responses-for-the-first-real-model-path.md) selects the Volcengine Agent Plan Responses route for the first real path. [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md) puts the real route only on the author's Mac local deployment, with the macOS Keychain resolver in the `storyos-worker` composition root. The Agent Plan terms permit only personal, non-commercial use.

## Decision

### One destination for each deployment

- The deployment configuration gives the one model destination that the deployment offers. A production deployment offers a real destination. A development or test deployment can offer the Contract-Faithful Fake Destination.
- The author does not select a destination, and no author-facing view shows the fake destination.
- A Project uses the offered destination by default. The author does not do an enablement step. The author can still make assistance unavailable for a Project, and the manual editor stays usable (ADR 0040).
- The offered destination can be different from the destination of the current binding of a Project. Then the Host makes new records for the Project. These are a Processing Destination Identity with its evidence revision, a Project Destination Grant, a `ProjectExternalUseBindingRevision`, and a separate External Contract Compatibility Decision. Earlier records stay unchanged, and earlier AgentRuns keep the binding that they pinned.

### Two credential sources

- **Operator quota.** The deployment operator supplies the credential, and the author uses the quota of the operator account. This is the default source.
- **BYOK.** The author supplies a key for the author's own Provider account.

The `ProjectExternalUseBindingRevision` records the source of its Credential Reference. A change of source makes a new binding revision. A change of source alone does not prove the same account boundary.

### Stage 4 scope

- Stage 4 implements only the operator quota. On the author's Mac local deployment, the credential is a macOS Keychain generic password in the login keychain. Its service is `storyos-volcengine-agent-plan`, and its account is `frankqdwang`. The author is also the operator of this deployment.
- The Agent Plan route is a personal, non-commercial test route that keeps costs low. Before a public release, the operator quota moves to the pay-per-token Ark `/api/v3` route or to a commercial agreement. That move needs its own ADR.
- BYOK, its key entry view, and the quota of more than one user are deferred. A later ADR selects the BYOK credential store.

## Considered options

- An author-facing destination selector was rejected. It shows the fake destination to the author, and it asks the author for a technical choice (ADR 0040).
- A separate author enablement step for the real model was rejected. The author wants assistance to be usable by default.
- BYOK in Stage 4 was rejected. It adds a credential entry surface and a new credential store, and Stage 4 does not need it.

## Consequences

- S4-01 changes the per-Project uniqueness of the binding records so that a Project can move to a new destination.
- The in-process Server Worker claims only AgentRuns whose Model Registration binds the fake adapter. AgentRuns of the real route go only to the Worker binary that resolves the Keychain credential.
- The Linux VPS real route stays unavailable until a later ADR selects its resolver (ADR 0039).
- This decision authorizes no external account, spend, or deployment change.
