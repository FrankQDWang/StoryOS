---
status: accepted
---

# Own the Project Command Sequence in One Adapter Module

On 2026-10-03 the author accepted this decision. One module in `storyos-adapter-postgres` owns the fixed sequence of Author Command Admission, Core Transition, and Command Acknowledgement for each project command that it serves. StoryOS Core owns the Core Transition Outcome of each command: the outcome kind, the Domain Receipt result kind of each outcome, and the reason codes. The adapter sequence allocates Structural Authority Settlement records for an `Applied` outcome only, as the outcome contract states. Each command supplies only a small command adapter: its kind and isolation level, its fact load, its effect rows, and its replay decoder. The first scope is the six Manuscript Structure Transition commands: `createVolume`, `updateVolume`, `deleteVolume`, `createChapter`, `updateChapter`, and `deleteChapter`.

## Context

At `main` `ffcafb94`, each project command writes the same sequence by hand in the adapter: begin the transaction and set Project Scope, consume the Command Challenge, branch on exact retry or first use, insert the Author Command Admission, classify through Core, insert the Domain Receipt and the settlement link, write the effect rows, settle the Command Idempotency Fence, and commit. Each command also has its own replay query. The adapter has 31 copies of the transaction start, 27 copies of the Domain Receipt insert, and 25 replay queries. One refusal reason has four hand-written forms: the Core enum, the SQL text, the replay decode, and the contracts enum. The application crate has one shallow module for each command, with a single-method store trait and binding checks that compare values the Server built from the same input.

## Decision

- The settlement module runs these steps in this order. Begin at the isolation level that the command declares, and set Project Scope. Consume the Command Challenge. On an exact retry of a settled command, roll back and replay. On an exact retry of a command in progress, refuse with a binding conflict. On first use, lock the Project row; a missing Project is refused before Admission and writes no row. Load the command facts and classify through Core. Insert the Admission, the Domain Receipt, and the settlement link. When Core allocates authority, write the effect rows, the guarded Manuscript Tree Revision and Current Chapter change, the Activity payload, the Authoritative Commit, the Author Action, and the canonical Snapshot. Read the Command-response Project after the writes. Settle the Command Idempotency Fence. Commit.
- One replay query serves every command in scope. Each command decodes only its applied effect. A pre-capture record gives `historical_acknowledgement_unavailable` ([ADR 0032](0032-accept-historical-acknowledgement-unavailable.md)). Damaged new-format evidence is a store fault.
- Each Core reason enum owns one stable code for each reason. The adapter writes and reads this code in SQL. The Server maps the same code to the contracts enum. The code is equal to the existing SQL text and to the existing wire text, so the persisted format and the public schema do not change.
- The order of SQL statements inside one transaction can change. Rows, migrations, persisted formats, isolation levels, and transaction boundaries do not change. For example, a create command now inserts its manuscript object after the Domain Receipt.
- In this scope the sequence records a Forward Author Action for every applied outcome. A disposition field arrives with the first Compensation command, which adds it to the command adapter result.
- The command adapter interface is private to the crate. The public interface is one method for each command. The application crate keeps only the shared command envelope, the shared error, and the generic settlement types. Its per-command modules, store traits, and binding checks are removed.
- The Server has one generic admission sequence for these commands. It keeps each command's problem codes and messages.

## Relation to ADR 0007

[ADR 0007](0007-preserve-process-separable-server-worker-boundary.md) says that Core owns authoritative transitions. Before this decision, the adapter owned both the sequence and the outcome mapping of each command. After this decision, Core owns the outcome taxonomy, the Domain Receipt result kind of each outcome, and the reason codes. The adapter owns the storage sequence and allocates authority records only for the `Applied` outcome. Thus the difference from ADR 0007 becomes smaller. It does not go away: the step order stays in the adapter, because its only implementation is PostgreSQL.

## Considered options

- An application port with a generic sequence function, as [ADR 0039](0039-open-the-model-gateway-seam-between-committed-dispatch-boundaries.md) uses for the Model Gateway, was rejected. The Model Gateway seam has two adapters. This seam has one adapter, so a wide port with eight operations adds indirection and splits each command into two halves.
- A second structure layer above the generic sequence, with typed steps, was rejected for the first scope. It has one implementation, and it moves the Manuscript Tree Revision advance out of Core.
- A replay query for each command was rejected. The six queries have the same joins and differ only in the decoded fields.
- A cross-version replay comparison against old binaries was not done. No production data exists. The same-version first-use and replay equality is a permanent test.

## Consequences

- The remaining project commands keep their current code until a later specification moves them. Author Edit, `acceptProposal`, and Author Undo need later additions to the module: an `outcome_unknown` settlement, a Pre-Admission Refusal Record, and an Author Action disposition field for Compensation.
- A new structural command needs a Core classifier with reason codes, one command adapter, one Server route, and one contracts schema.
