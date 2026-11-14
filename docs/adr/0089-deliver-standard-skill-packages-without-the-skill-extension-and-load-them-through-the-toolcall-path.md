---
status: accepted
---

# Deliver Standard Skill Packages Without the Skill Extension and Load Them Through the ToolCall Path

The author decided this on 2026-11-12 in conversation, at the start of Stage 6 ([#364](https://github.com/FrankQDWang/StoryOS/issues/364)). This ADR records the scope of Stage 6, one reversal of the Skill glossary, and the disclosure rule for Skill text. It is the first ADR on Skills. Until now, [the Skill glossary](../glossary/skill.md) was the only Skill contract.

## Context

The Skill glossary dates from the planning of 2026-08-29. It defines a standard SkillPackage and, around it, an optional StoryOS Skill Extension with parameters, Tool Roles, Outcome Profiles, Hard Applicability, and an Invocation Policy. It also defines three Installation Scopes, four kinds of Skill Source, a Skill Revocation, and a Skill Script Execution. The Stage 6 parent asks for the smallest current implementation.

At `main` `8aa2bcb3`, the repository has these facts:

- No Skill code, table, route, or Web control exists.
- Each stored record belongs to one Project. No storage family is shared between Projects.
- No sandbox and no script runtime exists. The Tool Approval of Stage 5 was not built ([ADR 0083](0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md) section 4), and [ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md) withdrew the Provider sandbox.
- A Run continues to its next RunStep only through the Tool Result re-entry of [ADR 0085](0085-reenter-each-settled-tool-result-through-context-assembly.md). No Run Wait and no Run-level replan exists.
- A new Context source class needs a new scope revision of the standing grant ([ADR 0051](0051-record-the-standing-grant-scope-and-refuse-a-disclosure-outside-it.md)).
- No author-facing view shows Run internals ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)).
- The Draft Artifact kinds are gone ([ADR 0077](0077-remove-the-host-created-recovery-draft.md)).

The open Agent Skills format gives the package rules: one directory with a `SKILL.md`; a `name` of 1 to 64 characters that is the same as the directory name; a `description` of 1 to 1024 characters; optional `scripts/`, `references/`, and `assets/` directories. The format defines no archive.

## Decision

### 1. Stage 6 delivers standard Skills only

Stage 6 delivers the installation, the selection, the progressive loading, the joint use, the author-requested drafting, and the replacement and removal of a standard SkillPackage. A standard package needs no StoryOS-specific file.

These contracts stay deferred. Their glossary entries stay, and no released Stage implements them:

- The StoryOS Skill Extension and each concept that needs it: the Skill Parameter Set, the Skill Invocation Policy, Hard Applicability, the Skill Tool Role, the Skill Dependency Resolution, the Skill Outcome Profile, and the Skill Outcome Obligation.
- The Skill Script Execution. A package with scripts installs. Its scripts are files that StoryOS does not run, and the install review says so.
- The Skill Revocation. The author cancels a Run that must stop.
- The Installation Scopes SystemOfficial and UserLocal, and each Skill Source other than an upload by the author.

### 2. One source, one scope, one name

The author uploads one archive of the package directory in the Web client. The Host validates it against the Agent Skills format and makes a Skill Installation Candidate. The author reads the complete `SKILL.md` and the file list, and then installs it.

The only Installation Scope is ProjectLocal. A Skill name is unique in one Project. An upload with the name of an installed Skill is a replacement, and the review says so before the author installs it.

### 3. Loading is a ToolCall

The glossary defined the Skill Load Request and the Skill Resource Load Request as typed requests of an Agent Decision that are not ToolCalls. This ADR reverses that. The two requests are ToolCalls to two Tools that StoryOS itself executes: one loads the `SKILL.md` of an eligible Skill, and one reads one text resource of its Snapshot. They have no external destination and no effect outside the Run.

Their results enter the next RunStep through the re-entry of ADR 0085. The loaded text is not Data-only Context. It has its own Context source classes, and it keeps the instruction rank of the Context contract: below the Project Instruction and above Data-only Context. The Host loads the complete `SKILL.md` and never shortens it.

An explicit selection by the author needs no load call. The Host puts the `SKILL.md` of the selected Skill into the first RunStep.

### 4. Installation is the disclosure grant

The standing grant gets a new scope revision that names the Skill source classes: the catalog entry, the instruction text, and the resource text. The installation of a Skill into a Project is the consent of the author to send that text to the model destination of the Project. No approval prompt exists for a load. A Skill gives no permission, no disclosure right, and no authority, and the Host instructions say so to the model.

The author accepted one risk. The instructions of a third-party Skill can lead the Agent to put manuscript text into a research query. The controls are the review at installation and the Host instructions.

### 5. Selection

The author selects a Skill with the `$` picker, which stores the exact SkillPackage Reference. The selection binds the Run of that message only. An instruction in ordinary words to use a named Skill is a choice of the Agent from the catalog. The Host parses no Skill name from a message, so no name is ambiguous and no Run waits for one.

No record holds the Skills of a Project Conversation. In a later message, the model can still have the text from the Provider conversation, and the Agent loads the Skill again when it does not.

### 6. Joint use

Each loaded Skill is one separate, attributable Context item. The order of the items serves only reproducible serialization. The Host merges nothing and ranks nothing. The Host finds no Skill Conflict. When two Skills ask for incompatible things, the Agent asks the author with the Clarification Decision.

### 7. Replacement and removal

A replacement and a removal change the catalog for later Runs only. A Run in progress keeps its SkillPackage Snapshot, and each earlier record stays readable. No enablement switch exists: an installed Skill is in the catalog.

### 8. What the author sees

After a turn, the panel names each Skill that the turn used and the reason that the Agent gave, as it does for a research call. The Skill list shows the `SKILL.md` of each installed Skill. The composed instructions of a RunStep are Run internals and stay in the read-only audit query.

### 9. A Skill Draft is a Candidate

The Agent drafts a Skill only when the author asks for one. A Tool that StoryOS executes validates the drafted files and makes a Skill Installation Candidate. A drafted package holds a `SKILL.md` and text resources, and no script. The author reviews and installs it as an uploaded Candidate. No Draft Artifact family returns.

### 10. Limits

| Item | Limit |
| --- | --- |
| Archive | 1 MiB and 64 files |
| `SKILL.md`, and each loadable resource | 32,000 tokens, UTF-8 text |
| Installed Skills in one Project | 32 |
| Candidates of one Project that the author did not install or dismiss | 8 |
| One Run | 8 Skill loads and 16 resource reads |

A `SKILL.md` above its limit fails the validation of the Candidate. The counts of one Run are apart from the research budget of Stage 5.

## Considered options

- **The complete glossary design in Stage 6.** Rejected. No real Skill of the author needs the Extension, and each of its parts needs a mechanism that does not exist: a Run Finalization Gate, a Run Wait, a second Tool Registration with an effect ceiling.
- **A script runtime.** Rejected for Stage 6. It needs a server sandbox and a Tool Approval. An approval prompt for each script is the interaction that the author refused in Stage 5.
- **UserLocal as the one scope.** Rejected. It is the first record family that crosses Projects, with new isolation, Archive, and restore rules. The cost of ProjectLocal is one more upload for a second novel.
- **A new Agent Decision kind and a new continuation cause for a load.** Rejected. It builds a second re-entry path beside ADR 0085 for the same behavior: the model asks, the Host answers, the next RunStep reads the answer.
- **A one-shot approval for each Skill load.** Rejected. The author reads the Skill at installation, and the Agent decides when a Skill helps.
- **A conversation-level list of Skills in effect.** Rejected for Stage 6. It needs a control to remove a Skill from the list and a rule to send the text again after each rebuild of the Provider conversation.

## Consequences

- The Skill glossary changes with this ADR. The Skill Load Request and the Skill Resource Load Request are ToolCalls. The Skill Invocation has no ambiguous-name wait. The Skill Installation Record binds the install command of the author. The Skill Draft is a Candidate. Each deferred entry says so.
- The user stories 9, 10, and 11 of the Stage 6 parent are waived. The stories 2, 7, and 13 are narrowed to one scope, the usage record, and replacement and removal.
- Tool Registration gets a second kind for a Tool that StoryOS executes. The Stage 6 tickets record its ToolSpecs and the names of the new source classes when they land.
- A later Stage that adds a second scope or source decides again how same-name packages coexist. A later Stage that adds a script runtime builds the Tool Approval first.
- The External MCP Surface of [ADR 0082](0082-keep-room-for-an-external-mcp-surface.md) is not affected.
