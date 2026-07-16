# Transcript-embedded MCP App host prototype

> **THROWAWAY PROTOTYPE — do not merge this branch into `main`.**

## Question

Can a disposable StoryOS-owned transcript projection attach a reconstructable,
sandboxed App View Artifact to durable Run/Tool/Wait facts; survive reconnect and a
real host-process restart without rerunning the original tool or trusting browser
state; fail closed to an inspectable static fallback; and mediate every App action
without granting domain authority?

This harness answers a state/recovery question, not a visual-design question. It uses
provisional record shapes and a wipeable JSON store under `.scratch/`. The pure state
reducer is isolated in `src/model.mjs`; the terminal and browser shells are disposable.

## Run it

```bash
npm --prefix prototypes/transcript-mcp-app-host start
```

The terminal shows the complete relevant state after every action. Open
<http://localhost:4181> while it is running to inspect the real cross-origin sandbox,
bridge handshake, transcript projection, App-initiated edit request, and sibling-frame
rejection.

Press `v` in the terminal to execute the ten-case research matrix, or run it directly:

```bash
npm --prefix prototypes/transcript-mcp-app-host run matrix
```

The matrix performs real child-process restarts. It is executable prototype evidence,
not a production test suite.

## Deliberate boundaries

- `localhost:4181` is the StoryOS host origin.
- `127.0.0.1:4182` is a distinct Sandbox proxy origin.
- `127.0.0.1:4183` is a deterministic fake MCP server.
- The originating tool invocation is counted and never repeated for transcript replay.
- The exact resource bytes and digest, input/result references, negotiated protocol,
  effective capabilities, host context, and static fallback belong to the App View
  Artifact.
- Browser DOM, heap, storage, pending RPC callbacks, and live overlays are disposable.
- An App edit request creates a durable Run Wait and then a Proposal. Only a separate
  author Acceptance changes the prototype's Authoritative State.

## Reset

Press `z` in the TUI. It deletes only this prototype's `.scratch/` directory and starts
fresh child processes.
