# StoryOS repository instructions

This file is the operating-rule source for every agent client. Codex reads it directly. Claude Code reads it through `CLAUDE.md`, which imports only this file.

## Rules

- **New rule:** put an operating rule in this file and a coding rule in [CODING_STANDARDS.md](CODING_STANDARDS.md). A nested `AGENTS.md` holds only the boundaries and commands of its subtree.
- **Authority:** code and tracked generated contracts show what exists today. ADRs under `docs/adr/` and the current Wayfinder map state the product and architecture contract. A divergence from an ADR is a defect to fix or an exception that an ADR records.
- **Memory:** repository files and tracked design artifacts are the source of truth. Conversation history and agent client memory are supporting context only.
- **Decisions:** record a spoken deployment, hosting, or vendor decision as an ADR in the same session. A premise that no ADR records is not a contract. ADR 0022 sets the current production topology.
- **User changes:** preserve unrelated user changes exactly as you find them.
- **Language:** write all repository text in ASD-STE100 Simplified Technical English: code, comments, documentation, commit messages, GitHub Issues, and pull requests. Research reports under `docs/research/` are evidence records. Talk to the user in Simplified Chinese.
- **Glossary and domain:** use the terms of [GLOSSARY.md](GLOSSARY.md), and read only the area files that you need. Before domain exploration or design, read [Domain docs](docs/agents/domain.md).
- **Checks:** use the StoryOS-owned repository commands. After each product or test edit, run `make check-fast`. Before a pull request, run `make check` on a clean tree, and put its Check Evidence Line in the body. Run `make check-full` only when the ticket or an explicit request requires it ([ADR 0070](docs/adr/0070-verify-locally-with-one-check-command-and-keep-github-checks-cheap.md)).

## Delivery

- **Delivery:** read [Delivery](docs/agents/delivery.md) before you claim, implement, review, or merge a ticket. It gives the roles, the branches, the ticket life cycle, the Codex review, and the templates.
- **Issue tracker:** use GitHub. Read [Issue tracker](docs/agents/issue-tracker.md) before you publish a ticket or change the Wayfinder map. Map a triage role with [Triage labels](docs/agents/triage-labels.md).
- **Merge gate:** merge only when the GitHub `verify` check passes, a Check Evidence Line of `make check` or `make check-full` names the pull request head, and the Codex review of `make review PR=<number>` passes. Only the coordinator merges.

## Code and references

- **Coding standards:** read [CODING_STANDARDS.md](CODING_STANDARDS.md) before you change Rust code, a test, a generated artifact, or a contract, and before a Standards review.
- **Project commands:** a new project command settles through the command sequence and declares its profiles (ADR 0043). A new Forward command kind declares its Author Undo Disposition (ADR 0044).
- **Reference material:** `.reference/` is local-only, Git-ignored reference material. Keep it out of Git and out of the Cargo workspace, build, test, package, release, and runtime. Edit a reference only when a task refreshes it.
- **Own design:** learn from upstream patterns, and design StoryOS around its domain. Do not fork, embed, or wrap the Codex runtime. Before you copy upstream code, verify its fit, isolate it, review its license, and record its provenance.
