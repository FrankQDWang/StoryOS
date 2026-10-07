# Glossary: Discovery writing, editor, and author commands

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**Discovery Writing**:
The StoryOS authorship model, inspired by Dean Koontz's page-by-page process, in which the author develops the novel from a live premise and characters, repeatedly refines the current passage, and discovers the story through writing, while Agent assistance stays grounded in the current passage and the author's present creative choices.

**Discovery Assistance Intent**:
The bounded author-facing meaning of one current request for help with discovery writing, expressed as present-passage discussion, explanation, research, brainstorming, limited alternatives, a Prose Change Request, or an explicit combination of those intents. It governs the assistance and handoff for that request but grants no authority, Capability, disclosure permission, lasting preference, or permission to expand the Working Target.
_Avoid_: Hidden router, inferred authorization, task-specific workflow, conversational momentum

**Prose Change Request**:
The exact part of a current author instruction that unambiguously asks StoryOS to prepare a change to identified prose or the current Working Target. It authorizes creation or revision of an inspectable Proposal for only that target and scope; it is never a Direct Author Action, Acceptance, authority change, permission to edit elsewhere, or authorization carried forward from an earlier request.
_Avoid_: Vague discomfort, discussion, automatic edit, implicit Acceptance, standing permission

**AI-Independent Editor Baseline**:
The first author-visible StoryOS release capability: from a new or controlled Project initialization, the author can organize volumes and chapters, write and revise manually, see save state, recover after reload or crash, navigate, search and replace, inspect basic writing statistics, use supported keyboard, clipboard, IME and undo behavior, sustain long sessions, and create a human-readable export without an available Agent or model.
_Avoid_: Chat-only demo, AI-dependent editor, pre-seeded current-passage demo

**Direct Author Action**:
A deterministic, immediately visible change caused through the author's own editor input path against one exact authoritative target under direct manipulation, including manual paste. Bulk, cross-location, not-fully-previsible, Agent-, Tool-, MCP-, or extension-produced changes remain Proposal-gated even when an author click initiates them.
_Avoid_: Author-triggered automation, silent bulk edit

**Author Edit**:
One completed semantic editor intent, or a bounded idle-coalesced sequence formed before Admission issuance while every pre-issuance input that the Admission contract will bind and every manuscript semantic field remain equal, submitted with one Author Command Admission for whole-command ownership classification by StoryOS Core. The final body and digest are never merged after Admission; Core produces an authoritative change, a Proposal Revision, a Refused Edit Draft, a conflict, or no effect without splitting one input across authority boundaries, while raw editor transactions and browser undo grouping remain diagnostic or presentation evidence rather than the domain command.
_Avoid_: Client-selected write path, ProseMirror transaction as authority, fixed-window batching, partial mixed edit

**Editor Verification Split**:
The two complementary deterministic gates for an Author Edit. Browser integration proves complete IME and editor intent capture plus local group coverage before Admission; Core proves ordered atomic settlement, with a typed zero-authority Receipt for an admitted refusal, conflict, or no effect and no Receipt for a pre-Admission refusal or infrastructure failure before commit.
_Avoid_: UI-only authority proof, server-only input-continuity proof, raw editor event as command truth

**Author Command Admission**:
The immutable Operational Record identified by `AuthorCommandAdmissionId` that binds one server-derived User, exact existing or Server-allocated prospective Project Scope, protected Client Session Binding, accepted client-contract and security-policy identities, applicable Editor Session and writer generation, action class, exact command digest, targets, expected Heads, nonce, idempotency record, bounded lifetime, and one terminal settlement. It admits one author-owned Core command without proving a physical-human gesture or granting reusable authority; post-admission uncertainty remains nonterminal `outcome_unknown`, recovery may invoke only the same unexpired fully matching direct edit, and an explicit, expired, changed, or unrecoverable command requires author reconfirmation. `outcome_unknown` is a reported condition, never a settlement. Missing response never proves non-commit.
_Avoid_: Physical-human attestation, client-supplied actor, session role as authority, Approval, reusable authorization token, missing response as failure

**Editor Recovery Creator**:
The closed StoryOS Host Artifact Creator used only for a Recovery Draft, binding one exact Editor Session, writer generation, complete recovered intent, and its Local Edit Journal, admission-settlement, takeover, or in-memory recovery evidence as applicable. It causes no Core invocation or Author Action, turns no browser cache into authority, and grants no reusable authorization.
_Avoid_: Author Command Admission, automatic edit replay, browser cache as truth, generic System

**Pre-Admission Refusal Record**:
An immutable, bounded, sanitized Operational Record created when StoryOS refuses an author command before Author Command Admission, carrying only the safe reached boundary, typed reason, applicable server-derived in-Scope identity, contract identities, correlation, and audit time. It proves there is no Admission, Command, nonce consumption, Receipt, or Core effect and is not an Admission lifecycle state.
_Avoid_: Author Command Admission, rejected request copy, foreign-object audit oracle

**Editor Session**:
One browser editing session for an exact User and Project Scope, identified by `EditorSessionId` and governed by the current Project writer generation. It owns local continuity and projection state while StoryOS Core and PostgreSQL retain authority.
_Avoid_: Browser tab as authority, Project identity, server transaction

**Local Edit Journal**:
The Project Scope-bound IndexedDB record of ordered, unsettled author editor intents for one Editor Session and writer generation. It preserves reload and crash continuity while remaining a non-authoritative input to Author Command Admission and Core settlement.
_Avoid_: Autosaved Authoritative State, server event log, ProseMirror history as truth

**Pending Edit Projection**:
The immediate author-facing editor view composed from one durable Server Snapshot plus the active Local Edit Journal. It exposes saving, saved, and needs-attention states without becoming an authoritative manuscript or Proposal Head.
_Avoid_: Optimistic authority, hidden pending state, network acknowledgement as domain truth
