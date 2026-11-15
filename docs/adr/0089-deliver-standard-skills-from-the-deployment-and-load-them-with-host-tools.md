---
status: accepted
---

# Deliver Standard Skills from the Deployment and Load Them with Host Tools

The author decided this on 2026-11-12 in conversation, at the start of Stage 6 ([#364](https://github.com/FrankQDWang/StoryOS/issues/364)). This ADR records the scope of Stage 6, one reversal of the Skill glossary, and the disclosure rule for Skill text. It is the first ADR on Skills. Until now, [the Skill glossary](../glossary/skill.md) was the only Skill contract.

The first form of this ADR had an upload by the author, an installation review, and a Skill Draft. The author withdrew them on 2026-11-12, before this ADR merged: in the first release, only the operator of the deployment provides Skills. This text is the one form that `main` gets.

## Context

The Skill glossary dates from the planning of 2026-08-29. It defines a standard SkillPackage and, around it, an optional StoryOS Skill Extension with parameters, Tool Roles, Outcome Profiles, Hard Applicability, and an Invocation Policy. It also defines three Installation Scopes, four kinds of Skill Source, a Skill Revocation, and a Skill Script Execution. The Stage 6 parent asks for the smallest current implementation.

At `main` `963a6a3a`, the repository has these facts:

- No Skill code, table, route, or Web control exists.
- Each record of author content belongs to one Project. The only records that the Projects of a deployment share are the Model Registrations and the Tool Registrations.
- No sandbox and no script runtime exists. The Tool Approval of Stage 5 was not built ([ADR 0083](0083-use-exa-as-the-first-mcp-research-destination-and-admit-it-through-project-tool-enablement.md) section 4), and [ADR 0084](0084-execute-the-research-tool-in-storyos-and-perform-no-provider-hosted-operation.md) withdrew the Provider sandbox.
- A Run is a tool loop ([ADR 0091](0091-replace-the-typed-reply-form-with-host-tools-and-end-each-run-with-an-explicit-finish-call.md)). A Host Tool is a Tool that StoryOS itself executes: it has no external destination, and its result returns to the model in the next RunStep. Four Host Tools exist.
- The operator of the deployment configures the model destination and the research destination, and that configuration is the consent ([ADR 0048](0048-use-the-deployment-model-destination-by-default.md), [ADR 0090](0090-make-web-research-available-in-each-project-without-an-enablement-switch.md)). `createAgentRun` writes the records of both destinations with no action of the author.
- A new Context source class needs a new scope revision of the standing grant ([ADR 0051](0051-record-the-standing-grant-scope-and-refuse-a-disclosure-outside-it.md)).
- A change of a request form needs a new mapping revision and a new Registration ([ADR 0093](0093-compact-a-run-more-than-one-time-keep-tool-results-as-stubs-and-keep-the-recent-author-messages.md) section 3).
- No author-facing view shows Run internals ([ADR 0040](0040-show-assistance-results-not-run-internals-to-the-author.md)).

The open Agent Skills format gives the package rules: one directory with a `SKILL.md`; a `name` of 1 to 64 characters that is the same as the directory name; a `description` of 1 to 1024 characters; optional `scripts/`, `references/`, and `assets/` directories.

## Decision

### 1. Stage 6 delivers standard Skills only

Stage 6 delivers the provision of standard SkillPackages by the deployment, the selection, the progressive loading, and the joint use. A standard package needs no StoryOS-specific file.

These contracts stay deferred. Their glossary entries stay, and no released Stage implements them:

- Each Skill that an author provides: the upload, the Skill Installation Candidate, the Skill Installation Record, the replacement and the removal by an author, and the Skill Draft. A later release adds them with their own entry, and the author decides that design first.
- The StoryOS Skill Extension and each concept that needs it: the Skill Parameter Set, the Skill Invocation Policy, Hard Applicability, the Skill Tool Role, the Skill Dependency Resolution, the Skill Outcome Profile, and the Skill Outcome Obligation.
- The Skill Script Execution. A package can hold scripts. They are files that StoryOS does not run.
- The Skill Revocation. The author cancels a Run that must stop.
- The Installation Scopes UserLocal and ProjectLocal, and each Skill Source other than the deployment.

### 2. The deployment provides the Skills

The one Skill Source is the deployment, and the one Installation Scope is SystemOfficial: read-only, and the same in each Project of the deployment.

- **The directory.** One deployment setting names one directory. Each child directory is one package, and its name is the Skill name, so a name is unique in the deployment. A deployment with no setting has no Skill: a Run has no catalog, and the `$` picker has no entry.
- **The reader.** Only the Server reads the directory, one time, when it starts. It validates each package against the Agent Skills format and the limits of section 9. A package that fails stops the start, and the message names the package and the rule. The Worker never reads the directory.
- **The Snapshot.** Before it admits a Run, `createAgentRun` makes sure that the Project has the SkillPackage Snapshot of each Skill of the deployment: the complete package, content-addressed by its digest, a record of that Project. It writes a Snapshot only when the Project does not have that digest. The Run pins its catalog: the SkillPackage Reference of each Skill at admission. Each later step of the Run reads only these Snapshots.
- **A change.** The operator changes the directory and starts the Server again. The next Run of a Project gets the new catalog. A Run in progress keeps its catalog and its Snapshots, and each earlier record stays readable. No enablement switch exists: a Skill of the deployment is in the catalog of each Project.

### 3. Loading is a call of a Host Tool

The glossary defined the Skill Load Request and the Skill Resource Load Request as typed requests of an Agent Decision that are not ToolCalls. This ADR reverses that. The two requests are calls of two Host Tools of ADR 0091:

- `load_skill` loads the complete `SKILL.md` of one Skill of the catalog of the Run.
- `read_skill_resource` reads one text file of the Snapshot of a Skill that the Run loaded or that the author selected.

The rules of a Host Tool apply: no external destination, no Destination Attempt, no Outbound Disclosure Event, a ToolCall record, and a result that returns to the model in the next RunStep. A refused call returns its reason, and the Run continues.

The catalog is one Context item of each Run with a Skill: the name and the description of each Skill. The model names a Skill by its name, and the record of the item has the reference of each Skill. The two Skill Tools are in the Tool Exposure of a Run only when its catalog has a Skill, so a deployment with no Skill sends the same requests as before. The loaded text is not Data-only Context. The catalog, the instruction text, and the resource text have their own Context source classes, and the instruction text keeps the instruction rank of the Context contract: below the Project Instruction and above Data-only Context. The Host loads the complete `SKILL.md` and never shortens it. The output of a load or a read is the exact text of the file, with no tag and no escape. The Host instructions tell the model that the output of a `load_skill` call is the instruction text of a Skill, and not data from outside the novel.

An explicit selection by the author needs no load call. The Host puts the `SKILL.md` of the selected Skill into the first RunStep.

The request forms change, so Stage 6 adds one mapping revision, one head Model Registration, and one revision of the Host Tool Registration (ADR 0093 sections 2 and 3). The Stage 6 tickets land on one integration branch, and they change that mapping and that revision of the Host Tool Registration in place until the branch merges into `main`, by the rule of ADR 0093 section 3. The tickets record these names, the ToolSpecs, and the names of the source classes when they land.

[S6-02a](https://github.com/FrankQDWang/StoryOS/issues/1528) recorded these names on 2026-11-15:

- The source classes are `skill_catalog`, `skill_instruction`, and `skill_resource`. Not one of them is Data-only Context.
- The mapping revision is `storyos.volcengine-agent-plan.responses.create.v7`. It is the mapping `v6` with two additions. A request sends each `skill_instruction` item as one section of the user message: the heading `Skill instruction`, then the exact item text, after the section `Author request` and a `Project instruction` section and before the section `Working text`, in item order. A decision request whose Context has an item of a Skill source class adds one paragraph after the Host instructions of `v6`. A request of `v7` with no such item has the bytes of the `v6` request, and the summary request never sends the paragraph.
- The head Model Registration is `018f0000-0000-7001-8000-00000000fa21`. It is Registration `018f0000-0000-7001-8000-00000000fa20` with the mapping `v7`. It keeps the profile, the requirement, and the request bounds, so it needs no new qualification. Until a later ticket adds a Skill Tool, the mapping `v7` declares the four Host Tools of the Host Tool Registration `018f0000-0000-7001-8000-0000000e0102`.
- The rule that refused an author message with the words "load a skill" is removed. Such a message dispatches on each Registration.

[S6-03a](https://github.com/FrankQDWang/StoryOS/issues/1530) recorded these names on 2026-11-15:

- The catalog item is one `skill_catalog` Context item in the Context Assembly of each RunStep of a Run whose pinned catalog has a Skill. Its projection mode is `exact_required`. Its text has one line for each Skill, in the order of the Skill names: the canonical JSON object with `name` and `description`. The text has no digest. The source version of the item is `sha256:` and the SHA-256 of the text. A Run with no Skill in its pinned catalog has no catalog item.
- The Operation Requirement records the pinned SkillPackage References in the field `skill_catalog`: a list of objects with `name` and `package_digest`, in the order of the Skill names.
- The Core function `skill_catalog_text` makes the text, and `assemble_skill_catalog` adds the item after the `skill_instruction` items. The adapter reader `run_skill_catalog` reads the pinned catalog of a Run and the description of each Snapshot.
- The mapping `v7` sends the item as one section of the user message: the heading `Skill catalog`, then the exact item text, after the `Skill instruction` sections and before the section `Working text`. The item is a part of the current input and of its digest (ADR 0085), and the expected input of ADR 0056 counts it when the request sends the current input.
- `SkillSet::new` refuses a set of Skills whose catalog text is above 32,000 counted tokens (section 9). The start of the Server then stops with the rule `skill_catalog_limit_exceeded`.

[S6-03b](https://github.com/FrankQDWang/StoryOS/issues/408) recorded these names on 2026-11-15:

- The Host Tool `load_skill` has the ToolSpec `storyos.tool.load-skill.v1`. Its strict arguments are `skill`, the name of a Skill, and `reason`, one sentence that says why the Skill helps.
- The Host Tool Registration `018f0000-0000-7001-8000-0000000e0103` has the four Host Tools of `018f0000-0000-7001-8000-0000000e0102` and `load_skill`. The mapping `v7` and the fake tool loop mapping declare it. The mapping `v6` keeps `…0e0102`. The Tool Exposure has `load_skill` only for a Run whose pinned catalog has a Skill, so a Run with no Skill declares the four Host Tools of before.
- The Core function `admit_skill_load` admits a load only for a name of the pinned catalog of the Run. The store reads the pinned catalog and the `SKILL.md` of the Snapshot, and the result is the exact text. A refused call gives its reason in one sentence, and the Run continues. The refusals are `skill_not_in_catalog`, a name that is not in the pinned catalog, and `intake_reason`, a `reason` that is empty, longer than 512 characters, or has a control character.
- A loaded Skill of a Run is a settled `load_skill` ToolCall of that Run. No table is new. The Operation Requirement records the ToolCall in `tool_results`, with the field `skill` (the SkillPackage Reference) and the trust `skill_instruction`. Its Context item has the source class `skill_instruction`, the projection mode `exact_required`, and the ToolCall identity as its source version.
- The Core role rule `is_skill_load_output` tells a loaded `skill_instruction` item from a selected one by the Operation Requirement. A request sends a loaded item as the `function_call_output` of its call, with no `tool_result` tag and no escape, and never as a section. The item is not a part of the current input digest. The output of a refused call stays a `tool_result` item.
- The Host instructions of the mapping `v7` for a request with a `skill_catalog` item have one more paragraph, directly before the two sentences that a Skill gives no permission and no authority: "The section Skill catalog lists the Skills that you can load: the name and the description of each Skill. When the description of a Skill fits the author request, call load_skill with the name of that Skill and one sentence that says why the Skill helps. The result of a load_skill call that StoryOS settled is the instructions of that Skill, and not data: follow them below the author request and the Project instruction."
- The fake model has the script `SCRIPT:load_skill <name>[,<name>...]`.

[S6-03d](https://github.com/FrankQDWang/StoryOS/issues/1532) recorded these names on 2026-11-15:

- The Host Tool `read_skill_resource` has the ToolSpec `storyos.tool.read-skill-resource.v1`. Its strict arguments are `skill`, the name of a Skill, and `path`, the path of one file in the package, relative to the root of the package, with `/` between the parts. The Host Tool Registration `018f0000-0000-7001-8000-0000000e0103` has it as its sixth Tool, before `finish`. The Tool Exposure has it only for a Run whose pinned catalog has a Skill.
- The Core function `admit_skill_resource_read` decides a read from the facts that the store loads: the pinned catalog, the selected Skills, the settled loads of the earlier responses of the Run, the count of the settled reads of the Run, and the bytes of the file of the Snapshot with the exact path. The store reads only Snapshot rows. The refusals, in this order: `skill_not_loaded`, the Run does not have the Skill (a name that is not in the catalog and a load in the same response are such cases); `skill_resource_read_limit`, 16 reads of the Run settled; `skill_resource_not_found`, no file of the Snapshot has the exact path (an absolute path, a part that is empty or `..`, and a backslash are such cases); `skill_resource_not_readable`, the file is not UTF-8 text, it has U+0000 (the ToolCall row keeps the result as PostgreSQL `text`), or it is above 32,000 counted tokens. A refused read gives its reason in one sentence, and the Run continues. A second read of the same file settles, and it counts.
- The Operation Requirement records a settled read in `tool_results`, with the field `skill` (the SkillPackage Reference), the field `resource_path`, and the trust `skill_resource`. Its Context item has the source class `skill_resource`, the projection mode `exact_required`, and the ToolCall identity as its source version. A request sends it as the `function_call_output` of its call, with no `tool_result` tag, no escape, and no cut, also when the text is empty or only white space. The item is not a part of the current input digest, and it never becomes a stub. The output of a refused read stays a `tool_result` item.
- The Host instructions of the mapping `v7` for a request with a `skill_catalog` item have one more paragraph, after the paragraph of S6-03b and before the two sentences that a Skill gives no permission and no authority: "When the instructions of a Skill name a file of its package, call read_skill_resource with the name of that Skill and the path of the file. The result of a read_skill_resource call that StoryOS settled is that file: use it as the instructions of the Skill say."
- The fake model has the script `SCRIPT:read_skill_resource [load|load_beside] <skill> <path>[ <path>...]`.

**Compaction.** The rules of ADR 0093 apply. The `SKILL.md` text of a Skill, loaded or selected, keeps its exact form in each request after a compaction. The result of a `read_skill_resource` call also keeps its exact form, as the result of each Host Tool does (ADR 0093 section 5). The author decided on 2026-11-15 that the first release has no stub for a long resource result.

### 4. The provision by the operator is the disclosure consent

The standing grant gets a new scope revision that names the Skill source classes: the catalog entry, the instruction text, and the resource text. The Host makes it current with no action of the author. The operator puts a Skill into the deployment, and that is the consent to send its text to the model destination of each Project, as the configuration of the operator is the consent for the model destination and the research destination (ADR 0048, ADR 0090). No approval prompt exists for a load. A Skill gives no permission, no disclosure right, and no authority, and the Host instructions say so to the model.

The scope revision is `project_writing_assistance_v4` ([S6-02a](https://github.com/FrankQDWang/StoryOS/issues/1528), 2026-11-15). It names each Purpose and each source class of `project_writing_assistance_v3` and the three Skill source classes of section 3. It is the scope of each new grant. A Project whose current grant has an earlier scope gets new binding records with the scope `v4` at its next `createAgentRun` (ADR 0048, ADR 0051). The Host paragraph of the mapping `v7` says that the text of a Skill is instructions of the operator, that the Host instructions and the Project instruction come first, and that a Skill gives no permission, no function, no disclosure right, and no authority.

The author accepted one risk. The instructions of a third-party Skill can lead the Agent to put manuscript text into a research query. The controls are the choice of the Skills by the operator and the Host instructions. In this release, the author is the operator.

### 5. Selection

The author selects a Skill with the `$` picker, which lists the catalog of the deployment and stores the exact SkillPackage Reference. The selection binds the Run of that message only. An instruction in ordinary words to use a named Skill is a choice of the Agent from the catalog. The Host parses no Skill name from a message, so no name is ambiguous and no Run waits for one.

No record holds the Skills of a Project Conversation. In a later message, the model can still have the text from the Provider conversation, and the Agent loads the Skill again when it does not.

The author changed this rule on 2026-11-15, after the Stage 6 release, for a Skill that the author selected. [The follow-up ticket](https://github.com/FrankQDWang/StoryOS/issues/1547) delivers the change, and the two paragraphs above tell the behavior until it merges.

- A Skill that the author selected stays in effect for the later turns of the same Project Conversation. The author selects it one time.
- While the Provider conversation continues, the Host sends nothing more, because the model has the text. When a request has a full input and no prior response (after a compaction, or after an expired prior reference), the Host sends the `SKILL.md` of that Skill again.
- The text is the text of the Snapshot of the selection, also after the operator changed or removed the package.
- The list of these Skills comes from the Skill requirements of the earlier Runs of the Conversation. No table is new. At most 8 Skills are in effect: the 8 names with the newest selections.
- A Skill that the Agent loaded in an earlier turn does not get this rule. The catalog of each Run has its name and its description, and the Agent loads it again when the task needs it.
- Inside one Run, the compaction rule of section 3 stays: the `SKILL.md` text of each Skill of the Run, loaded or selected, keeps its exact form after a compaction. The author confirmed this on 2026-11-15, because the Agent is at work with that text.
- The usage record does not change. The turn of the selection names the Skill, and a later turn adds no line for it.
- The author surface gets no new control. A new Project Conversation ends the effect.

### 6. Joint use

Each loaded Skill is one separate, attributable Context item. The order of the items serves only reproducible serialization. The Host merges nothing and ranks nothing.

[S6-03c](https://github.com/FrankQDWang/StoryOS/issues/1531) recorded the order on 2026-11-15. A selected Skill is a section of the first RunStep in the order of its name. A loaded Skill is the output of its own call, in the item order of the call: the RunStep of the call, then the position of the call in its response. Two loads are two outputs and two Context items, also when one response has the two calls, and a full input sends them again in the same order. The Host finds no Skill Conflict. When two Skills ask for incompatible things, the Agent asks the author in its answer.

### 7. A Skill that is not available

A Skill problem does not stop the writing work. A `load_skill` call for a name that is not in the catalog of the Run, and a `read_skill_resource` call for a file that is not readable, are refused calls with a reason, and the Run continues. A message whose `$` selection names a reference that is not in the catalog of the deployment refuses before the Run starts, with one plain sentence, and the author sends the message again without the selection.

### 8. What the author sees

After a turn, the panel names each Skill that the turn used, in the place where it names each research call. For a Skill that the Agent loaded, the panel gives the reason that the Agent gave in the load call. For a Skill that the author selected, the panel says so. The `$` picker shows the name and the description of each Skill. No view shows a `SKILL.md`: the operator has the files. The panel has no entry to manage Skills. The composed instructions of a RunStep are Run internals and stay in the read-only audit query.

### 9. Limits

| Item | Limit |
| --- | --- |
| One package | 1 MiB and 64 files |
| `SKILL.md`, and each resource that the Agent can read | 32,000 tokens, UTF-8 text |
| Skills of one deployment | 32 |
| The catalog text of one deployment | 32,000 tokens |
| One Run | 8 Skill loads and 16 resource reads |

A package above its limit, a `SKILL.md` above its limit, and a set of Skills above its limit or with a catalog text above its limit fail the validation at the start of the Server. A file that is not UTF-8 text, or that is above the token limit, stays in the package, and a read of it is a refused call. An explicit selection uses one of the 8 loads.

The review of the aggregate pull request recorded one more rule on 2026-11-15. The record of a Run keeps the `SKILL.md` text and each decoded text of its YAML mapping as database text, and database text cannot hold the character U+0000. Thus `admit_skill_package` refuses a `SKILL.md` with U+0000 (`skill_md_not_text`), a decoded description with U+0000 (`description_invalid`), and a decoded optional text, metadata key, or metadata value with U+0000 (`optional_field_invalid`). A YAML escape can give U+0000 when the bytes of the file do not have it. The validation at the start of the Server fails for such a package.

[S6-03c](https://github.com/FrankQDWang/StoryOS/issues/1531) recorded the load refusals on 2026-11-15. The Core function `admit_skill_load` decides them from the pinned catalog, the selected Skills, and the settled loads of the Run, in this order: `skill_not_in_catalog`, `skill_already_loaded` (a settled load of the Run or a selected Skill has the name), and `skill_load_limit` (the selected Skills and the settled loads are 8). The Host executes the calls of one response in call order, so each settled load counts for the calls after it.

## Considered options

- **The complete glossary design in Stage 6.** Rejected. No real Skill of the author needs the Extension, and each of its parts needs a mechanism that does not exist: a Run Finalization Gate, a Run Wait, a second Tool Registration with an effect ceiling.
- **An upload by the author, with a Candidate review, a replacement, and a removal.** Rejected for the first release by the author on 2026-11-12. It was the first form of this ADR. Its tickets were more than half of the estimated size of Stage 6, and the author is the one person who adds a Skill today.
- **Packages in the repository, compiled into the binaries.** Rejected. A new Skill needs a release, each Skill changes the request fixtures of the tests, and a private or third-party Skill must enter the repository.
- **The Worker reads the directory.** Rejected. Two processes read the directory at two times, so a Run can pin a Skill that the Worker does not have. With the Snapshot in the Project, the Worker needs only the database.
- **A Snapshot only at the first load of a Skill.** Rejected. An Agent load occurs in the Worker, so the Worker must read the directory.
- **A stub for a long resource result after a compaction.** Deferred by the author on 2026-11-15. It was in the form of this ADR of 2026-11-14. A resource is small in each known package, and the stub needs a second stub rule beside ADR 0093 section 5.
- **A script runtime.** Rejected for Stage 6. It needs a server sandbox and a Tool Approval. An approval prompt for each script is the interaction that the author refused in Stage 5.
- **A new Agent Decision kind and a new continuation cause for a load.** Rejected. The tool loop of ADR 0091 already has the behavior: the model asks, the Host answers, the next RunStep reads the answer.
- **A one-shot approval for each Skill load.** Rejected. The operator chooses the Skills, and the Agent decides when a Skill helps.
- **A conversation-level list of Skills in effect.** Rejected for Stage 6. It needs a control to remove a Skill from the list and a rule to send the text again after each rebuild of the Provider conversation. The author accepted it on 2026-11-15 for a Skill that the author selected, with the rule of section 5 and with no control: a new Project Conversation ends the effect.

## Consequences

- The Skill glossary changes with this ADR. The Skill Load Request and the Skill Resource Load Request are calls of Host Tools. The Skill Invocation has no ambiguous-name wait. SystemOfficial is the scope of a Skill that the deployment provides. Each deferred entry says so.
- The user stories 1, 2, 9, 10, 11, and 12 of the Stage 6 parent are waived. The stories 7 and 13 are narrowed to the usage record and to a change by the operator that does not change active work.
- A Run that reads many long resources can end with `conversation_too_long`, because a compaction does not make a resource result smaller. A later ticket can add the stub.
- Each Project stores its own copy of each package of the deployment. The limits of section 9 bound it at 32 MiB for one catalog.
- In a later deployment with more than one author, the operator chooses the Skills for each author. That release decides again what an author sees and controls.
- A later Stage that adds a second scope or source decides how same-name packages coexist. A later Stage that adds a script runtime builds the Tool Approval first.
- The External MCP Surface of [ADR 0082](0082-keep-room-for-an-external-mcp-surface.md) is not affected.
