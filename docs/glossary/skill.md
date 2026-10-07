# Glossary: SkillPackage and Skill

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**SkillPackage**:
An open Agent Skills standard-compatible directory rooted at one `SKILL.md` with the required name, description, and Markdown instructions plus optional package resources. StoryOS consumes the standard authoring format without turning a Skill into a workflow runtime, Tool, Capability Grant, or source of author authority.
_Avoid_: StoryOS workflow manifest, Agent, Tool, Capability Grant, prompt snippet

**Skill Source**:
The host-verified provenance identity and locator from which a SkillPackage was resolved, such as the StoryOS distribution, a project-authored location, a user library, or a third-party repository. It disambiguates same-name packages and supports signature, installation, and project-allowlist policy but creates neither instruction precedence nor runtime authority.
_Avoid_: Skill Installation Scope, Skill priority, Capability Grant, package name, trusted execution

**Skill Installation Scope**:
The host-controlled ownership and visibility boundary of an installed SkillPackage: SystemOfficial is read-only and bundled with StoryOS, UserLocal is user-owned and available across projects, and ProjectLocal belongs to one novel project and travels with that project. Source and scope are independent: a ThirdParty source must still be installed into UserLocal or ProjectLocal, and same-name packages in any scopes coexist through distinct SkillPackage References rather than overriding or shadowing one another.
_Avoid_: Skill Source, search-path precedence, name override, AgentRun binding

**SkillPackage Reference**:
The immutable host-owned reference to one resolved SkillPackage, recording its Skill Source, standard name, optional declared version and provenance, and a host-verified digest over the exact package contents. Name and version aid selection, source disambiguates collisions, the digest proves the bytes used, and a running AgentRun never silently upgrades its reference.
_Avoid_: Skill name alone, mutable latest version, unverified package path, dependency snapshot

**SkillPackage Snapshot**:
The immutable, project-owned, content-addressed copy of the complete resolved SkillPackage that the Host persists before that Skill first enters a Skill Selection Set, regardless of Installation Scope. Step Snapshots reference it for audit, recovery, export, and exact replay; equal digests may deduplicate storage, but the snapshot is not an installation, is not discoverable or editable as a live Skill, and executes no package code. If policy, license, or storage constraints prevent persisting the exact package, that Skill is ineligible for the project AgentRun rather than being recorded only by name or provenance.
_Avoid_: Skill Installation Record, ProjectLocal Skill, mutable vendoring, external cache, provenance-only record

**Skill Installation Candidate**:
The isolated, non-executable snapshot produced by inspecting one exact external or local Skill Source before installation, including standard validation, license and compatibility, complete file inventory, digest, scripts, extensions, dependencies, and collision evidence. A Candidate is neither installed nor enabled and grants no capability.
_Avoid_: Installed Skill, executable checkout, Approval, SkillPackage Reference

**Skill Installation Record**:
The immutable result of an author-approved atomic installation, binding the exact Skill Source, Candidate digest, target scope, Approval, and resulting SkillPackage Reference. Update and uninstall append new records, never rewrite references held by existing AgentRuns, and installation itself runs no package code or dependency setup.
_Avoid_: SkillPackage, mutable installation state, Capability Grant, script execution

**Skill Revocation**:
The durable project-policy or security determination that an exact SkillPackage Reference may no longer be used for future work, distinct from ordinary update, disablement, or uninstall. Ordinary lifecycle changes affect discovery for new AgentRuns but never retarget an existing Run's snapshot; a revocation is revalidated before the next RunStep, forcing replanning for an AgentSelected Skill or a Run Wait for a Run Skill Requirement, while preserving all prior Step Snapshots and recording the revocation response.
_Avoid_: Package update, silent upgrade, historical deletion, retroactive Step rewrite

**Skill Draft**:
The isolated, author-reviewable candidate standard SkillPackage produced or revised through Skill creation, including its complete file tree, optional StoryOS Skill Extension, validation results, trigger examples, digest, and intended installation scope. A Draft is neither installed nor Agent-visible; author confirmation publishes it through the project write or Skill installation boundary.
_Avoid_: Installed Skill, mutable live package, Core Proposal, implicit publication

**StoryOS Skill Extension**:
The optional `agents/storyos.yaml` product adapter accompanying a standard SkillPackage when it needs typed StoryOS invocation, Hard Applicability, parameters, Tool Roles, or Outcome Profiles. Its absence never invalidates or prevents use of a standard Skill, other Agent Skills clients may ignore it, and none of its fields grants capability or authority.
_Avoid_: Skill standard fork, required manifest, Tool Registration, Capability Grant

**Skill Parameter Set**:
The immutable, schema-validated values chosen from an optional StoryOS Skill Extension for one SkillPackage Reference and, when present, one Skill Outcome Profile. Parameters may narrow or specialize declared behavior but cannot alter the package digest, Hard Applicability, required evidence, Tool effects, Capability, or authority boundaries.
_Avoid_: Skill override, prompt patch, mutable settings, Capability Grant

**Skill Invocation Policy**:
The optional StoryOS Skill Extension ceiling that makes one enabled SkillPackage either ExplicitOrAgent, visible for author selection and Agent discovery, or ExplicitOnly, visible to the author but absent from the Agent catalog until selected; a standard Skill without the extension defaults to ExplicitOrAgent. Project settings may narrow or disable either policy but never widen ExplicitOnly, while persistent product rules belong to policy or higher-authority instructions rather than an AlwaysOn Skill.
_Avoid_: Implicit invocation, AlwaysOn Skill, Tool Exposure, project enablement

**Skill Invocation**:
Either an author action through the Codex-style `$` Skill picker or an Agent semantic choice from natural-language intent. The picker stores an exact SkillPackage Reference and creates a UserSelected Run Skill Requirement; an unambiguous natural-language instruction to use a named Skill does the same, while ordinary intent matching against standard descriptions produces an AgentSelected Skill Selection Decision. A raw ambiguous name creates a Run Wait before the first RunStep, with no scope, installation-time, or search-path precedence and no automatic installation; later updates never retarget the resolved reference.
_Avoid_: Slash command, keyword router, name-only binding, implicit precedence, approximate-name installation

**Skill Catalog Entry**:
The bounded model-visible discovery projection of one eligible SkillPackage, containing its exact standard name and description plus an opaque source-qualified reference that disambiguates same-name packages. StoryOS Skill Extension fields remain host-side during discovery and neither Installation Scope nor source trust creates ranking, authority, or instruction precedence.
_Avoid_: Full SKILL.md, StoryOS Skill Extension, Skill Eligibility, recommendation score

**Skill Instruction Context**:
The structured, attributable model context containing the exact full `SKILL.md` of a successfully loaded SkillPackage for a subsequent RunStep. The Host does not silently summarize, rewrite, or truncate it; extension-derived parameters and contracts enter separately as typed StoryOS context, while other package resources remain unloaded until explicitly requested.
_Avoid_: Skill Catalog Entry, prompt merge, StoryOS Skill Extension dump, package resource bundle

**Skill Context Composition**:
The immutable model-visible collection of separate Skill Instruction Context items used by one RunStep, canonically ordered by their exact SkillPackage References solely for reproducible serialization. Items remain semantic peers, are never merged into a synthetic Skill or ranked by position, and their compatible Outcome Obligations compose as a union; any incompatible combination follows Skill Conflict handling.
_Avoid_: Concatenated prompt, Primary Skill, list precedence, synthesized SkillPackage

**Skill Applicability Contract**:
The two-part boundary describing where a SkillPackage can be used: its standard description and instructions provide Semantic Applicability for the Agent, while an optional StoryOS Skill Extension may add host-evaluated Hard Applicability over typed StoryOS facts. Hard incompatibility cannot be bypassed by explicit selection, and an extension never replaces the standard description as the portable trigger contract.
_Avoid_: Keyword router, model classifier, arbitrary applicability code, Skill Eligibility

**Skill Tool Role**:
A provider-neutral named dependency slot in an optional StoryOS Skill Extension that constrains acceptable StoryOS ToolSpec identities and versions, required input and output semantics, and the maximum Tool Effect Envelope. A Role and the Agent Skills standard's experimental `allowed-tools` field are never StoryOS execution grants; a Role is required unless it declares a bounded optional fallback and its output consequences.
_Avoid_: Tool name, Tool Registration, provider adapter, Capability Grant

**Skill Dependency Resolution**:
The immutable host result that maps each Skill Tool Role used by one RunStep to an exact active Tool Registration and ToolSpec contract digest, or to its declared optional fallback. An unresolved required Role makes the Skill ineligible, while a successful resolution grants neither Tool Exposure nor Capability.
_Avoid_: Tool installation, Skill selection, Tool Exposure, authorization

**Skill Outcome Profile**:
A named contract in an optional StoryOS Skill Extension that defines the typed Artifacts or Operational Results, provenance, validation evidence, completion criteria, and disclosed partial-result conditions for one way of applying the Skill. A standard Skill without Profiles remains usable under its instructions and AgentRun completion criteria; a Profile may be GuidanceOnly but never grants authority or makes output authoritative.
_Avoid_: Output example, Run Outcome, Core Proposal eligibility, permission

**Skill Outcome Obligation**:
The durable requirement created when a Skill Selection Decision chooses one exact Skill Outcome Profile for an AgentRun. The Run Finalization Gate must settle it as satisfied, explicitly partially satisfied, or failed from typed results and evidence; obligations from multiple Skills compose unless they form a Skill Conflict.
_Avoid_: Run Outcome, model completion claim, Artifact, Acceptance

**Skill Selection Set**:
The immutable, attributable set of exact SkillPackage References consulted for one RunStep, with each member recording UserSelected or AgentSelected provenance and its selection reason. Members are semantic peers; future RunSteps may use a different set, while prior selections never change.
_Avoid_: Active Skill, Primary Skill, Supporting Skill, mutable Run-wide Skill list

**Run Skill Requirement**:
An author-originated, reference-bound requirement that one AgentRun honor an explicitly selected SkillPackage across its remaining work. It persists through RunSteps, Waits, and Holds until prospectively superseded by Steering Input, but grants no capability, need not be consulted by every RunStep, and never carries into a new AgentRun.
_Avoid_: Skill Selection Set, permanent project Skill, Capability Grant, implicit inheritance

**Skill Eligibility**:
The deterministic host-derived determination that an exact SkillPackage Reference may be considered for one RunStep under current project enablement, compatibility, declared Skill conflicts, dependency availability, and policy. Eligibility grants no capability and does not choose the Skill on semantic grounds.
_Avoid_: Skill selection, recommendation score, Capability Grant, Tool Exposure

**Skill Selection Decision**:
The attributable part of an Agent Decision that chooses zero or more eligible SkillPackage References to load for subsequent work and, when declared, their exact StoryOS parameters and Skill Outcome Profiles. It records the semantic reason for each choice while the host retains eligibility, loading, and dependency enforcement.
_Avoid_: Host classifier, fixed workflow routing, Skill Eligibility, active Skill

**Skill Load Request**:
A typed request in one Agent Decision to resolve, validate, and load an eligible SkillPackage Reference and its exact Skill Instruction Context for later work. Successful loading may affect only a subsequent RunStep's Step Snapshot and Skill Selection Set, never the requesting RunStep, and grants no Tool, capability, or data access.
_Avoid_: Mid-step context mutation, ToolCall, Capability Grant, implicit activation

**Skill Resource Load Request**:
A typed request to load one exact package-relative resource from a SkillPackage Snapshot for a subsequent RunStep. The Host confines the path to the package, verifies its digest and supported content type, applies per-item and aggregate context limits, and records attribution; reading a resource grants no script execution, Tool use, capability, or outbound disclosure.
_Avoid_: Skill Load Request, arbitrary file read, automatic resource injection, script execution

**Skill Script Execution**:
An ordinary StoryOS ToolCall to an explicitly registered execution Tool, targeting one exact script path inside an immutable SkillPackage Snapshot and recording the runtime identity, inputs, outputs, effects, and execution evidence. Installing, snapshotting, discovering, loading, or reading a Skill never executes its scripts; execution remains subject to Tool Exposure, Capability Grant, Approval, and effect policy, while standard `allowed-tools` metadata grants nothing. If no compatible Tool or declared non-script fallback exists, the affected Skill requirement cannot be completed.
_Avoid_: Executable SkillPackage, installer hook, direct shell instruction, implicit interpreter, Skill-granted capability

**Skill Conflict**:
The condition in which two or more SkillPackage requirements or instructions cannot be jointly satisfied for the current work under higher-authority product, policy, domain, or author constraints. Declared structural conflicts make a combination ineligible, an AgentSelected semantic conflict requires recorded replanning, and a conflict between Run Skill Requirements creates an exact Run Wait for author resolution.
_Avoid_: List order, automatic override, silent merge, Primary Skill
