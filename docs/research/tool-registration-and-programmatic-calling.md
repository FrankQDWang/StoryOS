# Tool registration, model orchestration, and programmatic calling

Status: research note for StoryOS issue #48, researched against first-party sources on 2026-07-14.

## Scope and source boundary

This note answers four related questions without treating similarly named mechanisms as interchangeable:

1. What ordinary function/tool calling actually does.
2. What OpenAI currently calls **Programmatic Tool Calling**.
3. How Anthropic's feature with the same name and the commit-pinned Codex `code mode` differ.
4. Where MCP and MCP Apps stop, and where a host such as StoryOS must begin.

OpenAI claims use current official developer documentation and the Responses API reference. Anthropic claims use Claude Platform documentation. MCP claims use the latest stable core specification (`2025-11-25`) and the stable MCP Apps extension (`2026-01-26`). Codex observations refer only to StoryOS's read-only submodule pin, commit [`1f0566d3f59298d1bb88820a0d35294f1eeb07ea`](https://github.com/openai/codex/tree/1f0566d3f59298d1bb88820a0d35294f1eeb07ea); they are source observations, not claims about every current Codex deployment.

## Executive answer

The user's mental model is substantially correct:

> A host gives a model named tools with documented inputs; the model chooses a tool and arguments; another runtime executes it and returns an output; the model can use several tools to complete a larger task.

That is the core tool-use contract in both the [OpenAI function-calling guide](https://developers.openai.com/api/docs/guides/function-calling#how-it-works) and the [MCP tools specification](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#user-interaction-model).

Four qualifications matter:

- A model requests a client-owned tool call; it does not execute the implementation. The host executes that call, returns the result, and continues the model loop. Provider-hosted tools are instead executed inside the provider boundary.
- “Single-purpose” is good interface design, not a protocol rule that a tool may have only one observable effect. A single business operation can read local data, disclose some of it to a named service, and change that external service.
- The model may organize the semantic workflow, but the host controls which tools are visible. It fully validates and dispatches client-owned calls; for provider-hosted tools it can enforce only the exposure, configuration, approval, and audit controls the provider actually exposes.
- Programmatic Tool Calling changes *where predictable coordination happens*: generated code can call several eligible tools without a fresh model inference between every call. It does not transfer security authority from the host to the generated program.

Consequently, a tool's callable contract and its host policy are separate concerns. `publish_chapter_to_notion` can remain one clear tool while StoryOS separately records that it reads chapter text, discloses that text to Notion, and writes external state. An effect profile describes authorization and audit facts; it does not turn the tool into a workflow engine.

## 1. Ordinary function/tool calling

### 1.1 The loop

OpenAI documents ordinary function calling as a five-step exchange:

1. The application sends the model a request and a set of available tools.
2. The model may return one or more structured tool calls.
3. The application validates and executes those calls.
4. The application sends the matching tool outputs back.
5. The model either returns a final response or asks for more tools, so the application continues the loop as needed.

The model sees the tool's name, description, and input schema; it does not see or run the application's implementation. A function tool's schema describes inputs, while the host owns the actual code and result. The Responses API associates calls and results through `call_id`. [OpenAI: function-calling concepts and flow](https://developers.openai.com/api/docs/guides/function-calling#how-it-works)

The default `tool_choice: "auto"` lets the model call zero, one, or multiple tools. The application can instead require a tool call, force one named function, prohibit tool calls, or restrict the callable subset. The application can also disable parallel function calls. These controls make tool selection *model-directed within a host-defined envelope*, not unrestricted model authority. [OpenAI: tool choice and parallel calls](https://developers.openai.com/api/docs/guides/function-calling#tool-choice)

### 1.2 How narrow should a tool be?

OpenAI recommends clear, intuitive functions and schemas that make invalid states difficult to express. It also recommends combining functions that are always called in sequence, and avoiding parameters whose value the application already knows. This means “smallest possible function” is not the goal; the goal is a coherent operation with a clear contract. [OpenAI: function definition best practices](https://developers.openai.com/api/docs/guides/function-calling#best-practices-for-defining-functions)

A useful distinction is:

- **Purpose:** the business capability the model chooses, such as “publish this chapter to Notion.”
- **Implementation:** the internal API calls, validation, retries, or transactions hidden behind that capability.
- **Effects:** the security-relevant things the operation may do, such as read project prose, disclose it to Notion, and update a Notion page.

One tool should normally have one understandable purpose. That purpose can still have several effects. Splitting every effect into a separate model-facing tool can make the workflow less reliable and can expose implementation details that should remain inside deterministic host code.

### 1.3 What the model organizes, and what the host owns

The model is well suited to deciding questions such as “which source should I inspect next?” or “is the retrieved evidence sufficient?” The host remains responsible for non-semantic guarantees:

- actual tool inventory and visibility;
- authentication and capability grants;
- input validation and output validation;
- approval before sensitive work;
- timeouts, concurrency, retry ceilings, cancellation, and idempotency handling;
- provenance, durable records, and recovery.

The MCP specification makes the same division explicit: tools are designed to be model-controlled, but the protocol does not mandate a UI model, recommends a human ability to deny invocations, and assigns validation, access control, confirmation, timeouts, output validation, and audit logging to servers and clients. [MCP: user interaction model](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#user-interaction-model) [MCP: security considerations](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#security-considerations)

## 2. OpenAI Programmatic Tool Calling

### 2.1 It is now an exact official term

As of the research date, **Programmatic Tool Calling** is an official OpenAI Responses API feature, introduced for GPT-5.6. The current model guide says GPT-5.6 can write JavaScript that invokes eligible tools, passes results between calls, and processes intermediate data in a hosted runtime. [OpenAI: GPT-5.6 “What is new”](https://developers.openai.com/api/docs/guides/latest-model#what-is-new)

It is enabled by adding `{ "type": "programmatic_tool_calling" }` to the Responses API `tools` array. Eligible tools declare `allowed_callers`:

- omitted or `["direct"]`: directly callable by the model;
- `["programmatic"]`: callable only from generated program code;
- `["direct", "programmatic"]`: available through either route.

The official guide currently lists function and custom tools, MCP, `apply_patch`, local and hosted shell, and `code_interpreter` as eligible tool types. [OpenAI: configuring Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling#configure-programmatic-tool-calling)

### 2.2 Runtime and continuation

OpenAI runs each generated JavaScript program in a fresh isolated V8 runtime. It supports top-level `await`, but has no Node.js, package installation, direct network access, general filesystem, subprocesses, console, or persistent JavaScript state between program executions. External interaction is possible only through tools enabled by the request. [OpenAI: runtime environment](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling#understand-the-runtime-environment)

The hosted runtime runs the generated JavaScript, but the application still executes client-owned function calls. A response may contain:

- a `program` item with generated code;
- nested `function_call` items whose `caller` points to that program;
- a `program_output` item with the reduced result;
- eventually, a normal assistant `message`.

When a program reaches a client-owned tool, it pauses. The application executes the call and returns `function_call_output` with the original `call_id` and `caller`, allowing the service to resume the right program. The application continues until it receives the final message. [OpenAI: program response items and continuation loop](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling#understand-program-response-items)

### 2.3 When it helps—and when it does not

OpenAI recommends programmatic calling for a bounded stage with predictable data flow: filtering, joining, ranking, deduplication, aggregation, validation, or dependent calls whose later arguments can be derived mechanically. Intermediate results can remain inside the runtime and the program can return a much smaller structured result. [OpenAI: choosing Programmatic Tool Calling](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling#choose-when-to-use-programmatic-tool-calling)

OpenAI recommends direct calls when:

- one call is enough;
- each result needs fresh model judgment;
- the action requires approval or writes external state;
- final citations or native artifacts must be preserved and validated.

The official guidance explicitly says writes and approval-sensitive actions should use direct calling by default. It also asks applications to define the program's permitted tools, output shape, evidence, concurrency, retry, and stopping limits instead of merely telling the model to “use programmatic calling efficiently.” [OpenAI: routing direct and programmatic calls](https://developers.openai.com/api/docs/guides/tools-programmatic-tool-calling#guide-routing-when-both-modes-are-available)

This is not a replacement for the ordinary agent loop. It is an optional inner loop for predictable work. The outer application still selects the capability, exposes eligible tools, executes its own functions, handles pauses and failures, and receives the final model message.

## 3. Anthropic's feature with the same name

Anthropic also has an official **Programmatic tool calling** feature. It has the same high-level purpose—generated code coordinates multiple tools so intermediate results can be processed before returning to the model—but its current API mechanism differs. [Anthropic: Programmatic tool calling](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling)

Anthropic's implementation:

- uses Python in a code-execution container;
- requires the `code_execution_20260120` tool or a later supported version;
- marks eligible user tools with `allowed_callers: ["code_execution_20260120"]`;
- pauses code execution and returns a `tool_use` block when client code must execute a tool;
- resumes the container after the client returns a `tool_result`;
- can reuse a container by passing its ID.

[Anthropic: how programmatic calling works](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling#how-programmatic-tool-calling-works) [Anthropic: container lifecycle](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling#container-lifecycle)

Anthropic documents support across several current Claude model families and Claude-hosted API surfaces; the model list and deployment availability are dynamic and should be checked on its compatibility section before adoption. Anthropic currently says this feature is not eligible for Zero Data Retention. [Anthropic: model compatibility and retention](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling#model-compatibility)

Anthropic also warns that `allowed_callers` guides presentation and is validated against `tool_choice`, but is not a hard security boundary; the client must still be prepared to handle a direct call and must not use this field as authorization. [Anthropic: `allowed_callers`](https://platform.claude.com/docs/en/agents-and-tools/tool-use/programmatic-tool-calling#the-allowed_callers-field)

### Direct comparison

| Question | OpenAI PTC | Anthropic PTC |
| --- | --- | --- |
| API surface | Responses API | Messages API / supported Claude Platform surfaces |
| Current documented model entry point | GPT-5.6 | Several supported Claude families; consult compatibility table |
| Generated language | JavaScript | Python |
| Runtime | Fresh isolated hosted V8 program | Code-execution container that may be reused |
| Tool eligibility | `allowed_callers: ["programmatic"]` | `allowed_callers: ["code_execution_20260120"]` |
| Client-owned tool execution | Application executes calls and returns outputs | Application executes calls and returns results |
| Retention statement | Documented as ZDR-compatible when the complete request is eligible | Documented as not ZDR-eligible |
| Recommended boundary | Bounded predictable reduction; direct by default for writes/approvals | Multi-tool processing; `allowed_callers` is not authorization |

The similar name should therefore not be used as a portable wire-protocol assumption. StoryOS would need a provider adapter if it ever chose to expose either hosted mechanism.

## 4. What the pinned Codex source does

### 4.1 Ordinary direct tool routing

The pinned Codex source implements the conventional host-driven architecture:

- `ToolSpec` serializes model-visible function, namespace, search, web-search, and custom-tool definitions for the Responses API. [source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tools/src/tool_spec.rs)
- `ToolExecutor` keeps the model-facing specification beside the executable runtime and assigns one of four exposure modes: direct, deferred, direct-model-only, or hidden. [source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tools/src/tool_executor.rs)
- `spec_plan` assembles built-in, MCP, extension, dynamic, hosted, deferred, and code-mode tools into a model-visible list plus a dispatch registry. [source](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/spec_plan.rs)
- `ToolRouter` converts model `function_call`, `custom_tool_call`, and client `tool_search_call` response items into typed calls, while `ToolRegistry` resolves the runtime and applies hooks, telemetry, cancellation, and lifecycle behavior around execution. [router](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/router.rs) [registry](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/registry.rs)

This validates the user's general understanding while also showing the missing host half: the model chooses calls, but Codex owns discovery, exposure, dispatch, policy hooks, execution, result conversion, and concurrency.

### 4.2 Codex `code mode`

The same pin contains a separate, Codex-owned `code mode`. It exposes a custom freeform `exec` tool. Generated raw JavaScript runs in a fresh V8 isolate and invokes nested tools through a global `tools` object; the described runtime has no Node.js, direct filesystem, direct network, subprocess, or console access. [code-mode contract](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/code-mode-protocol/src/description.rs)

The host converts eligible `ToolSpec` values into typed nested-tool definitions, injects `exec`/`wait`, and can operate in `Direct`, `CodeMode`, or `CodeModeOnly`. In `CodeModeOnly`, most nested tools disappear from the top-level model tool list, but remain dispatchable from the code runtime. [tool conversion](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/tools/src/code_mode.rs) [mode planning](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/spec_plan.rs)

Nested calls do not bypass Codex's normal execution boundary. The code-mode broker sends them back through `ToolCallRuntime` and the same `ToolRouter`, so normal dispatch controls still apply. [code-mode dispatch broker](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/core/src/tools/code_mode/delegate.rs)

This resembles hosted Programmatic Tool Calling conceptually, but it is not the same implementation:

- the pinned `ToolSpec` has no `programmatic_tool_calling` variant;
- Codex itself supplies the `exec` custom tool, V8 runtime, nested-tool bridge, and `wait` lifecycle;
- the feature table marks `CodeMode` and `CodeModeOnly` under development at this pin, while the separate code-mode host process support is stable. [feature status](https://github.com/openai/codex/blob/1f0566d3f59298d1bb88820a0d35294f1eeb07ea/codex-rs/features/src/lib.rs)

Therefore the pinned source is useful architectural evidence for “generated code coordinates tools behind the same host policy boundary,” not evidence that StoryOS should couple itself to Codex's runtime or to a provider-specific PTC protocol.

## 5. MCP and MCP Apps boundaries

### 5.1 MCP tool registration is discovery, not automatic exposure

In core MCP, a server declares the `tools` capability. A client obtains tool definitions through `tools/list` and invokes a selected tool through `tools/call`. A definition contains a name, description, input schema, optional output schema, and optional annotations. The stable specification requires clients to treat annotations as untrusted unless the server itself is trusted. [MCP: listing and calling tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#protocol-messages) [MCP: tool data type](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#tool)

MCP does not require the host to put every discovered tool into the model's context. It standardizes discovery and invocation between an MCP client and server; the host still decides what it exposes to its model and UI. This is why Codex can map MCP tools into direct, deferred, or hidden host states, and why StoryOS can apply local capability and approval rules after discovery.

### 5.2 MCP Apps add a second possible caller

The stable MCP Apps extension associates a tool with a `ui://` resource and lets the host render that UI in a sandboxed iframe. Tool metadata can declare visibility to the `model`, the `app`, or both. An app-only tool must be omitted from the agent's tool list, while a View may request an allowed same-server tool through the host. [MCP Apps: tool visibility](https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx#resource-discovery)

The View does not become a privileged MCP client. It communicates with the host over JSON-RPC through `postMessage`; the host proxies permitted calls and controls capabilities. The official overview explicitly describes the sandbox, the host-controlled bridge, and shared `tools/call` messages. [MCP Apps overview](https://modelcontextprotocol.io/extensions/apps/overview#how-mcp-apps-work) [MCP Apps security model](https://modelcontextprotocol.io/extensions/apps/overview#security-model)

Caller route and execution carrier are two separate axes:

- **Caller route:** model-direct, model-programmatic, App-initiated, or host-only. This says who selected the operation and whether generated code coordinates it; it does not say where the operation runs.
- **Execution carrier:** StoryOS-dispatched or provider-hosted. Client-owned functions and custom tools—including a StoryOS-owned MCP adapter—return to StoryOS's local gateway for authorization and dispatch. Provider-hosted capabilities—including native provider web search, code execution, shell, or MCP—run inside the provider boundary.

A direct call can use either execution carrier, and so can a programmatic call. An App-initiated tool normally returns through the StoryOS host bridge, but still receives only the capabilities granted to that View. Provider-hosted execution lets StoryOS configure exposure, filtering, and supported approval behavior; it does not let StoryOS intercept every nested call. Enabling it therefore requires an explicit external-execution grant plus separate disclosure, retention, provenance, and audit review.

Registration tells each caller what operation exists. It does not grant any caller authority to perform it.

## 6. Implication for StoryOS ToolSpec design

The evidence supports keeping four concerns separate. They do not need four model-visible objects; they need four explicit host concepts so a stable tool contract does not absorb dynamic policy state.

### A. Registration metadata

This records what implementation StoryOS discovered or installed:

- stable identity, version, and source;
- provider or MCP server identity;
- registration validity and provenance.

Registration establishes that an operation exists. It does not make it visible or authorized.

### B. Callable contract

This is what the model or App needs to use the tool correctly:

- name and plain-language purpose;
- input schema;
- structured output contract and error shape.

The tool should normally represent one understandable business capability. The model can compose several such capabilities when semantic judgment is needed; predictable internal sequences can stay in ordinary code or, later, a bounded programmatic stage.

### C. Exposure projection

This is calculated for the current provider, Run/Step, caller, and policy state:

- direct, programmatic, App, or host-only caller route;
- initial, deferred, or hidden discovery state;
- the exact provider-facing tool projection for this model request.

Exposure is contextual and disposable. Changing caller route or discovery state should not require changing or versioning the stable callable contract.

### D. Host-owned execution policy

This is what StoryOS needs to decide whether and how a call may run:

- local project data read;
- external destination and disclosed data class;
- external read or external state change;
- Proposal creation;
- reversibility, idempotency, approval, and retry policy;
- capability, provenance, budget, and audit requirements.

This layer should not be inferred from a tool name, prompt, or untrusted MCP annotation. It also should not be presented to the model as a menu of extra actions. It is the host's policy description of the effects the single operation may produce.

For client-owned tools, including a StoryOS-owned MCP adapter exposed as a function or custom tool, StoryOS can enforce this policy at the local call gateway. Provider-hosted capabilities—including a provider's native MCP tool—have a weaker boundary: StoryOS can decide whether to expose and configure them and may require provider-supported approval, but the provider executes their nested calls. They must therefore be modeled as explicitly granted external execution, not treated as if they inherit every guarantee of a StoryOS-dispatched `ToolCall`.

### Neutral conclusion

The user's model of tools is right at the **callable-interface level**. It becomes incomplete only if “the model organizes the process” is taken to mean that the model also owns authorization, execution guarantees, or durable state.

For StoryOS, the cleanest direction is therefore:

- keep model-facing tools coherent and generally single-purpose;
- let the model compose tools for adaptive, meaning-dependent work;
- let deterministic code compose fixed sequences the model should not repeatedly decide;
- reserve programmatic calling for bounded, mostly read/transform/reduce stages;
- keep writes, disclosures, approvals, authoritative creative changes, and recovery under StoryOS-owned policy;
- describe those effects independently from the tool's functional purpose.

This conclusion supports a composable effect profile, but it does not yet determine its exact Rust types or serialized field names. Those should be decided separately as StoryOS's host contract, after choosing which facts must be enforced at registration time, run authorization time, and individual call time.
