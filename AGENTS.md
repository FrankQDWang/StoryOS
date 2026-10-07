# StoryOS repository instructions

This file is the operating-rule source for every agent client. Codex reads it directly. Claude Code reads it through `CLAUDE.md`, which imports only this file.

## Every task

- **New rule:** put an operating rule in this file and a coding rule in [CODING_STANDARDS.md](CODING_STANDARDS.md). A nested `AGENTS.md` holds only the boundaries and commands of its subtree.
- **Authority:** code and tracked generated contracts show what exists today. ADRs under `docs/adr/` and the current Wayfinder map state the product and architecture contract. A divergence from an ADR is a defect to fix or an exception that an ADR records.
- **Memory:** repository files and tracked design artifacts are the source of truth. Conversation history and agent client memory are supporting context only.
- **Decisions:** record a spoken deployment, hosting, or vendor decision as an ADR in the same session. A premise that no ADR records is not a contract. ADR 0022 sets the current production topology.
- **User changes:** preserve unrelated user changes exactly as you find them.
- **Language:** write all repository text in ASD-STE100 Simplified Technical English: code, comments, documentation, commit messages, GitHub Issues, and pull requests. Research reports under `docs/research/` are evidence records, and the text guard does not examine them. Talk to the user in Simplified Chinese.
- **Glossary:** use the ubiquitous language of the glossary. [GLOSSARY.md](GLOSSARY.md) lists the design area files under `docs/glossary/` and tells how to find a term with `grep`. Read only the area files that you need.
- **Domain:** before domain exploration or design, read [Domain docs](docs/agents/domain.md) for the glossary and ADR rules.
- **Daily loop:** at task start, run `make verify-status BASE=origin/main`. After each product or test edit, follow the [Daily loop](docs/agents/verification.md#daily-loop) to run the smallest check that can fail on that edit.
- **Commands:** use the StoryOS-owned repository commands for format, lint, test, schema generation, and verification.
- **Guards:** `make verify-policy` runs the ASD-STE100 text guard and the Rust literal guard. `make verify-status` shows the change size and module size advisory.

## Ticket delivery

- **Issue tracker:** use GitHub, and read [Issue tracker](docs/agents/issue-tracker.md) before you publish, claim, deliver, or close a ticket. It also gives the client commands that start workflow skills.
- **Triage:** when a skill gives a triage role, map it with [Triage labels](docs/agents/triage-labels.md).
- **Ownership:** one ticket has one implementation owner and an isolated worktree. One coordinator owns the specification and the shared tracker state. One merger owns integration.
- **Branches:** use one integration branch for each specification and a separate branch and worktree for each implementation ticket. Run independent dependency-ready tickets in parallel. Keep real product release gates.
- **Pull request:** push the integration branch for one aggregate pull request. Implementation commits go to `main` only through that pull request.
- **Merge:** the implementation session opens the PR. Only a coordinator session merges it, after it examines the evidence. Use an ordinary merge commit unless the specification requires another method.
- **Merge gate:** merge only after the current required `verify` check and independent read-only Standards and Spec reviews pass. Follow [Candidate review and admission](docs/agents/verification.md#candidate-review-and-admission) for reviewers, targeted checks, and fresh evidence after a change.
- **Complete run:** run `make verify-local` only when the ticket contract or an explicit request requires a complete local run. Follow [Candidate review and admission](docs/agents/verification.md#candidate-review-and-admission) for its order and its recovery.
- **After merge:** synchronize `main`, run `make verify-tracker`, and record final evidence for every child and parent requirement as the issue tracker specifies.

## Code change

- **Coding standards:** read [CODING_STANDARDS.md](CODING_STANDARDS.md) before you change Rust code, a test, a generated artifact, or a contract, and before a Standards review.

## Reference material

- **Local only:** `.reference/` is local-only, Git-ignored reference material. Keep all of its entries out of Git. Edit an individual reference only when a task explicitly refreshes that reference.
- **Build boundary:** keep `.reference/**` out of the StoryOS Cargo workspace, dependency graph, build, test, package, release, and product runtime.
- **Own design:** learn from upstream patterns, and design StoryOS independently around its domain. Do not fork, embed, or wrap the Codex runtime.
- **Copied code:** before you copy upstream implementation code, verify architectural fit, isolate the copied unit, review its license obligations, and record provenance. A copied design idea does not make upstream a production dependency.
