# Glossary: ToolSpec, Tool Gateway, and MCP

This file is one design area of the [StoryOS glossary](../../GLOSSARY.md).

## Language

**StoryOS ToolSpec**:
The provider-neutral, versioned semantic contract for one Tool, consisting only of its callable input, output, and error contract, Destination Context Intake Contract, Tool Effect Envelope, execution policy, and result and provenance rules. Implementation source belongs to Tool Registration and credential bindings belong to scoped project-use records, while project enablement, provider compatibility, Exposure, grants, Approval, pricing, and invocation state remain separate dynamic records.
_Avoid_: Provider function schema, Tool Registration, installed tool, ToolCall

**Destination Context Intake Contract**:
The exact provider-neutral fields, data categories, source classes, Purpose, and hard bounds one non-model Tool, MCP server, embedding service, telemetry sink, or other destination may receive, with no Ambient Context. Required project data is supplied only through explicit inputs or governed StoryOS-controlled references under one Project Scope, and the Contract grants neither source access, Capability, nor Disclosure Eligibility.
_Avoid_: Full prompt forwarding, implicit Transcript, inherited model context, arbitrary project read

**Ambient Context**:
Any Transcript, Project Instruction, Working Target, Agent Memory, project data, or other surrounding content supplied to a non-model destination without an exact Destination Context Intake Contract field and current minimum-necessary decision. Ambient Context is prohibited even when the destination shares a Provider, process, network connection, or AgentRun.
_Avoid_: Convenience metadata, default Tool context, provider session state

**Provider-hosted Tool Destination**:
A Tool processing boundary operated by or behind a model Provider and declared within a separately admitted Provider-hosted Operation. Its exact Registration, intake, permitted outward processing, and authority are distinct from the model destination's, while the operation binds the applicable manifests and actual submission evidence without inventing Host dispatch records for invisible internal steps; it inherits no model context or permission.
_Avoid_: Model Tool Request, StoryOS ToolCall, provider prompt capability, inherited disclosure

**Provider-hosted Operation**:
The bounded search, read, or temporary-computation work that one exact Model Attempt may cause through its complete admitted hosted Tool set. This Operational Record binds the explicit intake, permitted effects, destinations, authority, budget, and result evidence before submission without claiming StoryOS control of each Provider-internal step.
_Avoid_: StoryOS ToolCall, AgentRun, Agent Decision, Provider session, per-step Host approval

**Telemetry Disclosure**:
An Outbound Disclosure to a traces, metrics, logs, debug, crash-reporting, or support destination, defaulting to sanitized operational categories, identifiers, timings, and digests rather than project prose, prompts, research, Tool results, Project Instructions, or credentials. It is independently current-eligibility and Redaction checked, never enters Project Export or Restore, and telemetry's diagnostic purpose never grants ambient access to durable Run or Artifact payloads.
_Avoid_: Model Telemetry Projection, debug upload, support bundle as permission

**Tool Discovery Record**:
An immutable observation of a third-party Tool contract and source identity before StoryOS has assigned trusted Host semantics. It has no Tool Registration identity, project enablement, Exposure, or execution authority; explicit StoryOS-controlled mapping may use it to create a new Tool Registration.
_Avoid_: Tool Registration, installed tool, trusted ToolSpec

**Tool Registration**:
The host-owned, versioned record that binds a built-in implementation or exact Tool Discovery Record to its trusted StoryOS ToolSpec, implementation source, and adapter rules. Its lifecycle is active, quarantined, or retired; discovery and project enablement remain separate records.
_Avoid_: Tool Discovery Record, Project Tool Enablement, Tool Exposure

**Project Tool Enablement**:
A project's explicit enabled or disabled selection of one exact active Tool Registration. It permits the Registration to be considered for Exposure but grants no Run capability or execution authority.
_Avoid_: Tool Registration, Tool Exposure, Capability Grant

**Tool Contract Drift**:
The condition in which a Tool Registration's pinned implementation identity, trusted model-visible callable contract, input or output contract, or trusted adapter mapping no longer matches the currently discovered implementation. Drift quarantines the Registration for new calls, clears derived Exposure, and requires a new StoryOS-controlled mapping; untrusted descriptive provenance alone does not cause Drift, and name equality never carries authority across versions.
_Avoid_: Compatible runtime update, automatic permission inheritance, retryable tool error

**Tool Exposure**:
The disposable Project Scope-bound projection of an enabled Tool for one caller and RunStep, computed from two orthogonal inputs: the Host-allowed caller routes and the current caller's initially-visible, deferred, or hidden discovery state. Exposure also depends on provider compatibility and current policy, but neither grants execution authority nor changes the Tool Registration.
_Avoid_: Tool Registration, project enablement, authorization

**ToolCall**:
An Operational Record for one requested invocation of an exact Tool Registration, including its caller route, validated arguments, resolved targets, Tool Effect Request, authorization state, execution lifecycle, and outcome. A ToolCall may produce Artifacts or other Operational Records but never inherits their lifecycle or authority.
_Avoid_: Tool result, Artifact, Approval, model message

**Tool Gateway**:
The sole StoryOS-owned authorization and execution boundary for every StoryOS-dispatched ToolCall, regardless of whether its caller is a model, generated program, MCP App, or host component. It resolves the Tool Registration, enforces the caller's exact Project Scope, derives effects, enforces grants and Approval, invokes the trusted implementation, validates output, and records the outcome; provider-hosted execution cannot claim this StoryOS-controlled guarantee.
_Avoid_: Tool Registry, provider runtime, direct adapter call

**Credential Reference**:
An opaque, host-owned, Project Scope-bound reference to credential material held by a deployment-specific secret backend and resolved only inside the execution boundary that needs it. PostgreSQL may retain its backend identity, non-secret locator and generation, availability or rebinding state, and Project Scope-bound use-binding metadata, but never the value or a value digest; the Reference, binding, and availability evidence grant no destination use, while scoped use bindings and Operational Records may identify them without containing the secret. The Foundation-local backend is macOS Keychain, later controlled-cloud deployments use the same resolver contract with a managed secret service, and environment variables are development/test inputs only. Ordinary database backups, logs, support material, and Project exports omit secret material; an import whose destination cannot resolve a Reference leaves it explicitly Unbound until an authorized rebind. Models, MCP Apps, generated programs, Tool arguments, outputs, transcripts, and external servers cannot inspect, select, or transport credential material.
_Avoid_: API key field, encrypted database secret, secret-value digest, portable secret export, production environment variable

**Tool Effect Envelope**:
The versioned, host-owned upper bound on the composable effects a registered Tool may request, covering project reads, Artifact writes, Outbound Disclosure, and external reads or writes. Artifact writes distinguish creating an Artifact from appending an Artifact Revision. The Envelope can never include mutation of Authoritative State, and untrusted Tool or MCP annotations cannot expand it.
_Avoid_: Tool category, approval, model-declared effects

**Tool Effect Request**:
The exact effects StoryOS derives for one ToolCall from its registered Tool Effect Envelope, validated arguments, resolved targets, and trusted adapter rules. It must fit both the Envelope and the effective Capability Grant; the model may choose business arguments but its own effect labels never grant authority.
_Avoid_: Model self-classification, Tool Effect Envelope, actual outcome

**Tool Effect Outcome**:
The structured observation of which requested effects were not attempted, confirmed, partially confirmed, or remain unknown after a ToolCall attempt. It binds actual project reads, Artifact Revisions, Outbound Disclosures, external reads, and external writes to evidence; an uncertain external effect remains unknown and cannot be treated as an ordinary failure or automatically retried.
_Avoid_: Tool success flag, Tool Effect Request, inferred side effect
