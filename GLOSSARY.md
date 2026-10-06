# StoryOS

StoryOS is a novel-project workspace in which the author retains authority over creative truth while Agents, Tools, Skills, and MCP Apps produce inspectable assistance around it.

## Language

The glossary terms are in one file for each design area under [`docs/glossary/`](docs/glossary/). Each term has one entry in one area file. An entry is the bold term line, one definition paragraph, and an `_Avoid_:` line with the rejected synonyms.

To find a term, search all the area files for its entry line. Do not read all the files:

```sh
grep -rn -A 2 '^\*\*Project Scope\*\*:' docs/glossary/
```

To find each entry that uses a term, use `grep -rn 'Project Scope' docs/glossary/`.

| Area | Scope |
| --- | --- |
| [Identity, session, and Project](docs/glossary/identity-and-project.md) | Users, Project ownership and scope, client sessions, and command challenges. |
| [Authoritative State, manuscript, and revision](docs/glossary/manuscript-and-revision.md) | Authoritative State, the manuscript tree, fiction truth, revisions, Core transitions, and block identity. |
| [Discovery writing, editor, and author commands](docs/glossary/editor-and-author-commands.md) | Discovery writing, the AI-independent editor, Author Edit, and Author Command Admission. |
| [Instructions, context, and memory](docs/glossary/context-and-memory.md) | Author instructions, Agent Memory, Context Assembly, retrieval, and context projection. |
| [Deployment, export, deletion, and recovery](docs/glossary/deployment-and-recovery.md) | The Foundation deployment, storage activation, Project export, restore, deletion, and recovery. |
| [Commands, protocol, and Activity Stream](docs/glossary/protocol-and-activity.md) | Command acknowledgement, protocol profiles, wire records, the Project Activity Stream, and queries. |
| [AgentRun, plan, retention, and budget](docs/glossary/agent-run.md) | AgentRun, triggers, Run lifecycle and events, holds and waits, plans, operational retention, and Run budgets. |
| [Subrun](docs/glossary/subrun.md) | Subrun requests, lifecycle, Mailbox, results, and joins. |
| [SkillPackage and Skill](docs/glossary/skill.md) | SkillPackage sources, installation, invocation, selection, and loading. |
| [Outbound disclosure, destination, and approval](docs/glossary/disclosure-and-approval.md) | Processing boundaries, Outbound Disclosure, destination grants and attempts, Capability Grants, and Approval. |
| [Deterministic verification](docs/glossary/deterministic-verification.md) | Deterministic verification boundaries, oracles, fault points, and evidence. |
| [Model Gateway](docs/glossary/model-gateway.md) | Model Gateway, model registration, routing, attempts, and usage. |
| [ToolSpec, Tool Gateway, and MCP](docs/glossary/tool-and-mcp.md) | StoryOS ToolSpec, provider-hosted tools, Tool registration and exposure, ToolCall, and Tool effects. |
| [Artifact, provenance, and Artifact kinds](docs/glossary/artifact-and-provenance.md) | Artifact identity and revisions, provenance, research evidence, Artifact lifecycle, and Core Artifact kinds. |
| [Proposal, Acceptance, undo, and Receipt](docs/glossary/proposal-and-acceptance.md) | Proposals, Acceptance, Author Action and undo, and Receipts. |
| [MCP App](docs/glossary/mcp-app.md) | App UI Resources, App Views, App View Instances, and App actions. |

## Changing a term

- Before you add a term, search all the area files. A term has one entry only.
- Put a new term in the area file that owns its concept. If no area owns it, add a new area file and a row in the table above.
- Keep the entry format. The text guard reads each `**Term**:` line in this file and in `docs/glossary/`.
