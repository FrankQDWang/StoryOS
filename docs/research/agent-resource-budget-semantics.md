# Agent resource-budget semantics for StoryOS

- Status: research complete; decision input, not implementation authorization
- Scope: run-level turns, model/tool calls, tokens, time, cost, concurrency, soft/hard limits, and pooled burst capacity
- Sources accessed: 2026-07-15

## Conclusion

A representative sample of four mainstream agent stacks supports the narrower claim that explicit execution guardrails are normal. It does **not** support the statistical claim that “nearly all agents” implement resource budgets.

The surveyed frameworks mainly enforce hard counts or timeouts. Anthropic also exposes a USD stopping threshold. LangGraph provides the clearest graceful-degradation mechanism through `RemainingSteps`. None documents a YARN-style shared burst pool, dual expected/worst-case admission accounting, or an author-approved in-run budget-extension contract.

StoryOS should adopt a per-dimension `soft_target` plus `hard_ceiling`, but not treat both as the same kind of limit. The soft target protects planning and fairness without reducing model quality. The hard ceiling protects non-exceedable authority and spend boundaries. Parallel admission needs **both expected-use allocation and worst-case hard-headroom reservation**.

## What current agent stacks enforce

| Stack | Documented built-in enforcement | Limit behavior | What is not demonstrated |
|---|---|---|---|
| OpenAI Agents SDK | `max_turns` bounds model invocations; current serialized `RunState` defaults it to 10, and `None` disables it. Async function tools can have per-call timeouts and local tool concurrency can be capped. ([run loop](https://openai.github.io/openai-agents-python/running_agents/), [run state](https://openai.github.io/openai-agents-python/ref/run_state/), [tool timeouts](https://openai.github.io/openai-agents-python/tools/)) | Exceeding turns raises `MaxTurnsExceeded`. A tool timeout can become a model-visible error or fail the run. | The SDK tracks request and token usage, but its documented core runner does not expose a built-in run-level token or USD ceiling; applications can enforce custom policy from usage hooks. ([usage](https://openai.github.io/openai-agents-python/usage/)) |
| Anthropic Claude Agent SDK | `max_turns` limits tool-use round trips and `max_budget_usd` stops on a cost threshold; both default to unlimited. ([agent loop](https://code.claude.com/docs/en/agent-sdk/agent-loop)) | Limit exhaustion returns `error_max_turns` or `error_max_budget_usd`. A session can be resumed after a turn-limit error with a higher limit. | The docs call USD a stopping threshold and report total cost in the result; they do not promise pre-call cost reservation or mathematically zero overshoot. Resume is an application action, not an author-approval budget protocol. |
| Google ADK | `RunConfig.max_llm_calls` caps LLM calls per run; the documented default is 500, and non-positive values mean unlimited. ([runtime configuration](https://adk.dev/runtime/runconfig/)) | A discrete call-count guardrail prevents further runaway LLM calls. | The documented `RunConfig` does not supply run-level token, money, tool-call, pooled-burst, or human-extension semantics. |
| LangGraph / LangChain | LangGraph enforces a per-execution superstep `recursion_limit` and raises `GraphRecursionError`; `RemainingSteps` lets graph logic wrap up before exhaustion. LangChain middleware limits model and tool calls at run and thread scopes. ([graph limits](https://docs.langchain.com/oss/python/langgraph/graph-api), [call-limit middleware](https://docs.langchain.com/oss/python/langchain/middleware/built-in)) | Model-call exhaustion can end gracefully or raise. Tool-call exhaustion can block only excess calls, end, or raise. Async LangGraph nodes/entrypoints can have attempt timeouts. ([timeouts](https://docs.langchain.com/oss/python/langgraph/fault-tolerance)) | `RemainingSteps` is advance warning, not permission to exceed the recursion ceiling. The tool limiter explicitly has restrictions when several calls are already pending, showing that parallel admission is not solved by a scalar counter alone. |

These controls count different units. An OpenAI turn is a model invocation, Anthropic's documented turn limit counts tool-use round trips, ADK counts LLM calls, and LangGraph counts supersteps that may contain parallel nodes. They are not interchangeable budget dimensions.

## What the cluster analogy establishes

Apache Hadoop YARN's CapacityScheduler explicitly supports soft queue capacity, optional hard maximum capacity, and use of free capacity beyond the soft allocation. Its hierarchy prefers sharing within a parent, and its maximum capacity limits elasticity. ([Hadoop 3.4.3 CapacityScheduler](https://hadoop.apache.org/docs/r3.4.3/hadoop-yarn/hadoop-yarn-site/CapacityScheduler.html))

Kubernetes makes a related but different split: scheduling admission uses resource requests, while runtime limits are enforced by the kernel. CPU limits are throttled as hard limits; memory limits are reactive and can be crossed before an OOM kill. Kubernetes also notes that aggregate limits can be overcommitted even though requested capacity controls placement. ([pod resource management](https://kubernetes.io/docs/concepts/configuration/manage-resources-containers/))

Flink on YARN dynamically requests and releases TaskManagers, but YARN is the resource provider that allocates the containers. The soft/hard shared-queue analogy therefore comes from the cluster scheduler, not the Flink agent/job logic or ZooKeeper coordination. ([Flink on YARN](https://nightlies.apache.org/flink/flink-docs-stable/docs/deployment/resource-providers/yarn/))

This analogy has a hard boundary:

- CPU slots and concurrency leases are renewable capacity; they can be returned or reassigned.
- Money, tokens, disclosure, and completed external writes are cumulative consumption; used units cannot be preempted or reclaimed.
- Unused reservations can return to a pool, but already consumed cumulative budget cannot.
- A scheduler can reactively kill memory use; StoryOS cannot erase a provider charge, disclosed payload, or external side effect after it happens.

## Recommended StoryOS model

### 1. Use a soft target and a hard ceiling per dimension

Call the lower value `soft_target`, not “soft upper limit”: exceeding it is allowed only through an explicit path. The upper value is `hard_ceiling`; it is never silently exceeded.

At minimum, each dimension records:

- measurement unit and accounting scope;
- consumed amount;
- expected-use reservations;
- hard-headroom reservations;
- soft target and hard ceiling;
- any borrowed allocation and its parent/project source;
- estimator and price-contract version;
- finalization reserve and enforcement mode.

The enforcement mode must reflect physical reality:

- **Strict preflight:** money when worst-case cost is bounded, outbound disclosure, external-write count/risk, tool-call count, and concurrency slots.
- **Boundary checked:** total tokens and elapsed time, where an admitted attempt may cross the soft target before usage is known.
- **Provider enforced:** per-response output-token maximum or another provider-side cap.
- **Cooperative deadline:** wall time for external work; after the deadline StoryOS stops new work and cancels safely, but unresolved effects may still become `outcome_unknown`.

### 2. Protect model quality at the orchestration boundary

Reaching a soft target should not silently lower reasoning effort, truncate an already admitted decision, or discard partial work. Instead StoryOS should:

1. let the current operation finish within its reserved hard headroom;
2. persist and settle its actual usage;
3. spend a protected finalization reserve to summarize progress or produce a partial artifact;
4. stop admitting exploratory work;
5. borrow pre-authorized spare allocation, replan to a cheaper route, or create a Run Hold for the author.

This preserves task continuity. Defaults should be task-sensitive and observable, not tight global constants copied from another framework.

### 3. Allow bounded borrowing, not implicit expansion

A Run may exceed its soft target by borrowing unused allocation only when all of these hold:

- the Project and parent Run hard ceilings retain sufficient headroom;
- the Run Grant already authorizes burst borrowing for that dimension;
- the transfer is atomic, durable, attributable, and visible;
- sibling Runs' existing reservations are not taken;
- fairness/concurrency policy permits the loan.

This is a Policy Decision confirming pre-existing authority. If the Run Grant lacks burst authority, or the Run hard ceiling must increase, only an author Approval can expand it. If no authorized spare exists, the Run enters Hold and offers the author stop, replan, or extend choices.

For cumulative resources, “borrowed” units become consumed when used and do not return. Only unused reservation returns. For renewable concurrency capacity, a time-bounded lease can be released normally.

### 4. Reserve expected use and worst-case headroom separately

Expected-use reservation alone is unsafe under concurrency: two branches can each see the same remaining hard budget and jointly exceed it. Worst-case reservation alone is safe but can suppress useful parallelism and model performance.

Before every model call, ToolCall, or Subrun, one atomic admission transaction should:

1. estimate expected usage for soft allocation and fairness;
2. derive the maximum enforceable usage for hard headroom;
3. reserve expected usage from the Run allocation, borrowing authorized spare if needed;
4. reserve the maximum across the Run, parent, and Project hard ledgers;
5. reserve applicable concurrency slots and effect locks;
6. only then transition the operation to executable.

On settlement, actual usage is charged and both unused reservations are released. Subruns receive slices from the parent ledger; they never copy the parent's remaining balance.

If a strict dimension has no trustworthy maximum, the operation is not admissible near the hard ceiling. For money, a defensible maximum must include fixed input, provider-enforced output bounds, server-tool charges, and bounded retries. A post-response cost estimate such as an SDK stopping threshold is useful protection, but is not proof of a non-exceedable wallet ceiling.

### 5. Make hard exhaustion fail closed at a safe boundary

At a hard ceiling, no new operation starts. The Run does not pretend that killing a connection reverses an external effect. In-flight work follows the accepted cancellation and `ToolEffectOutcome` rules; measurement drift or provider overage is recorded as an invariant violation and causes Hold/reconciliation.

## Decision recommendation

Accept the proposed soft/hard model with these refinements:

- soft target controls planning, borrowing, and author interaction, not model intelligence;
- hard ceiling controls admission and authority;
- pre-authorized shared spare may be borrowed automatically within parent/project hard ceilings;
- otherwise soft exhaustion produces an inspectable Hold and author extension decision;
- every parallel operation reserves expected use **and** enforceable worst-case headroom;
- finalization capacity is reserved so the Agent can stop coherently;
- cumulative budgets are never described as reclaimable scheduler capacity;
- “never exceed” is promised only where StoryOS or the provider can enforce a true maximum.

This is stricter than the surveyed agent frameworks while preserving more continuity than a single blunt turn/token cutoff.
