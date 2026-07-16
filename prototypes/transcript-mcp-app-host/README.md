# Transcript-embedded MCP App host prototype

> **THROWAWAY PROTOTYPE — do not merge this branch into `main`.**

## Question

Can a disposable StoryOS-owned transcript projection attach a reconstructable,
sandboxed App View Artifact to durable Run/Tool/Wait facts; survive reconnect and a
real host-process restart without rerunning the original tool or trusting browser
state; fail closed to an inspectable static fallback; and mediate every App action
without granting domain authority?

This harness answers a state/recovery question, not a visual-design question. The
default browser route is now an **author-facing Transcript slice** around that logic;
internal state and security probes stay in the terminal. It uses provisional record
shapes and a wipeable JSON store under `.scratch/`.

## Run it

```bash
npm --prefix prototypes/transcript-mcp-app-host start
```

Open <http://localhost:4181>. That is the only human review surface. It shows a normal
StoryOS writing-assistant Transcript with a sandboxed App, Host-owned Approval, and the
Proposal boundary. The command starts from a clean, ready-to-review fixture.

The terminal is an internal technical control surface. It exposes process restart,
offline, interrupt, compaction, branching, resource drift, and denial probes; it is not
part of the product interaction being reviewed.

Press `v` in the terminal to execute the ten-case research matrix, or run it directly:

```bash
npm --prefix prototypes/transcript-mcp-app-host run matrix
```

The matrix performs real child-process restarts. It is executable prototype evidence,
not a production test suite.

## Deliberate boundaries

- `localhost:4181` is the author-facing Transcript host.
- The Sandbox proxy and fake MCP server use separate internal origins. They are not
  standalone pages and are never human review surfaces.
- The originating tool invocation is counted and never repeated for transcript replay.
- The exact resource bytes and digest, input/result references, negotiated protocol,
  effective capabilities, host context, and static fallback belong to the App View
  Artifact.
- Browser DOM, heap, storage, pending RPC callbacks, and live overlays are disposable.
- An App edit request creates a durable Run Wait and then a Proposal. Only a separate
  author Acceptance changes the prototype's Authoritative State.

## Reset

Press `z` in the TUI. It deletes only this prototype's `.scratch/` directory, starts
fresh child processes, and prepares the author-facing Transcript fixture again.
