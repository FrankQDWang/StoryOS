---
status: accepted
---

# Reshape the ProjectCommand Trait to the Surface That the Sequence Reads

The repository cleanup specification of phase 7 ([#1364](https://github.com/FrankQDWang/StoryOS/issues/1364)) owns this ADR. The author decided it on 2026-11-01. Tickets [#1365](https://github.com/FrankQDWang/StoryOS/issues/1365) and [#1366](https://github.com/FrankQDWang/StoryOS/issues/1366) remove the repeated steps inside two command families. Ticket [#1367](https://github.com/FrankQDWang/StoryOS/issues/1367) declares the command kind once. Ticket [#1368](https://github.com/FrankQDWang/StoryOS/issues/1368) adds the new surface beside the old one. Tickets [#1369](https://github.com/FrankQDWang/StoryOS/issues/1369) to [#1372](https://github.com/FrankQDWang/StoryOS/issues/1372) migrate the commands in four batches. Ticket [#1373](https://github.com/FrankQDWang/StoryOS/issues/1373) deletes the old surface. This ADR replaces the mechanism paragraphs of [ADR 0043](0043-settle-every-project-command-through-declared-profiles-of-one-sequence.md) and [ADR 0076](0076-settle-every-project-command-through-one-sequence-skeleton.md) that it names below. The rule paragraphs of ADR 0076 stay in force.

## Context

The `ProjectCommand` trait of the PostgreSQL adapter had two associated constants, ten associated types, and thirteen methods. Twenty-four commands implemented it, through seven settlement profiles. Five of the ten types repeated the parameters of the settlement type that the application crate already declared for each command. Three types were reason types with the `Infallible` value in nine, two, and seven commands. Ten of the thirteen methods were optional hooks. Four hooks served zero-authority outcomes, and nineteen commands never overrode them. Seven of the ten commands that overrode the Receipt payload hook returned one constant string. One hook and one constant served one command each. The response record was an associated type with its own trait and three implementations, and twenty-one commands used the same one.

The command kind string of a command was written in six places, and no test compared the `CommandSpec` kind with the Route Catalog. The AgentRun status enumeration was defined three times with the same seven variants. Two command families repeated their bodies: the AgentRun control commands shared their lock and parse step, and the ActionOnly Proposal decisions shared their classification block and their transition decode.

## Decision

### 1. The declared surface of a project command

- `ProjectCommand` has five associated types: `Error`, `Profile`, `Plan`, `Applied`, and `Settlement`. `Settlement` names the settlement type that the application crate declares for the command. One shape trait, `SettlementShape`, implemented once on `ProjectCommandSettlement` in the adapter crate, gives the sequence the applied effect, the three reason types, the response record type, and the zero-authority effect of the command.
- The applied effect comes from the applied value of the profile. Each profile declares its applied value `Applied<E>` with the `AppliedRecord` trait, whose `Effect` is `E`.
- `ProjectCommand` has one associated constant, `SPEC`, of the type `CommandSpec<Self::Error>`. The `CommandSpec` carries the command kind, the applied variants, the Activity kind, the replay effect query, the response record, the contention refusal, and the constant Receipt payload. The contention refusal is `Option<fn() -> E>`, so that the constant has no destructor and the sequence can read it in a `const` context.
- `ProjectCommand` has seven methods. `classify`, `apply`, and `decode` are required. `applied_receipt`, `zero_authority`, `diagnose_missing_admission`, and `check_replay_binding` have defaults.
- `applied_receipt` gives one value with the applied variant, the Receipt payload, and the Receipt references. Its default takes the first applied variant and the constant payload of the `CommandSpec`, and empty references.
- The four zero-authority methods are the side trait `ZeroAuthority<C>`. A command implements it when it writes zero-authority effect or Activity rows, or when it decodes a zero-authority effect on replay. Five commands write such rows: `acceptProposal`, `rejectProposalOperations`, `steerAgentRun`, `takeOverProjectWriter`, and `undoLatestAuthorAction`. `applyAuthorEdit` is the decode-only case: its profile writes the zero-authority rows, and the command implements the side trait for `decode_zero_authority_effect` only. `zero_authority` is the one method of the base trait through which the sequence reaches it. It returns `Option<&(impl ZeroAuthority<Self> + Sync)>`, and its default returns `None`, which declares no zero-authority rows.
- The response record is a value of the closed enumeration `CommandResponse`: the Command-response Project, the Command-response Project with the Project assistance record, or no response record. The `CommandSpec` carries the value, because an exact retry reads the record before it has a classification. The settlement type converts the record with `TryFrom`. No command trait describes it. The `AdmitSpec` of an export admit step carries its `CommandResponse` value in the same way, and the `Response` type of the `AdmitCommand` converts the record with `TryFrom`.
- A write that runs after the profile records is a part of the value that `apply` returns. `apply` returns a `CommandWrite` with the profile write and an optional `after_authority` closure. The sequence calls the closure with the command and the applied value of the profile, at the same step as before.
- `AdmittedCommand` keeps its three methods and builds on this surface.
- The module `command_sequence/surface.rs` holds the declared surface, `command_sequence/profile.rs` holds the settlement profile traits, and `command_sequence/zero_authority.rs` holds the side trait and the zero-authority outcome types.
- The settlement order, the Admission forms, the profiles, the replay rule, the test ownership, and the Author Undo Disposition of ADR 0076 do not change.

### 2. One source of the command kind

- The contracts crate declares the closed enumeration `CommandKind`, with one variant for each command operation of the operation registry and a method that gives the string of the Route Catalog.
- The `CommandSpec`, the `AdmitSpec`, the `ForwardCommand` table, the Server routes, and the adapter test routes name a `CommandKind` variant. One contracts test compares the strings of `CommandKind` with the command operations of the operation registry. The adapter test of ADR 0079 compares the SQL domain with the Route Catalog, so the three sources are bound.
- The tracing field `command_kind` of each sequence entry keeps a literal value through the string method of the enumeration.

### 3. One AgentRun status in each leaf crate

- The core crate keeps its AgentRun status enumeration, named `AgentRunStatus`. The contracts crate keeps its wire enumeration. The application crate converts between them and declares no copy.
- The other same-shape pairs of a contracts wire enumeration and a core reason enumeration stay. They are the boundary between the wire format and the domain.

### 4. Shared steps inside a command family

- The three AgentRun control commands share one function that locks the AgentRun row and parses its status.
- The seven ActionOnly Proposal decisions share one classification constructor and one transition decode. The compensation modules of `replanProposal` and `reopenWithdrawnProposal` share one body.
- The structure command pairs stay as they are. Their shared lines are the SQL skeleton of different tables.

### 5. Delivery

- The work is an expand-contract sequence. The expand ticket adds the new surface beside the old one, and one sequence runner settles a command on either surface. Four batches migrate the commands in parallel worktrees. The contract ticket deletes the old surface.
- The contract ticket deletes the old surface and the `ResponseRecord` trait. The export admit step reads the response record from its `AdmitSpec`.
- Each ticket leaves `generated/`, `apps/web/`, and the migrations directory unchanged, and each existing adapter test passes with its assertions unchanged.

## Superseded parts

- ADR 0043, "What a command adapter declares": the paragraphs "Receipt shape", "Activity record", and "Response record" describe the hooks and the response type of the old surface. The declared surface is now section 1 of this ADR. The paragraphs "Admission profile", "Settlement profile of an applied outcome", "Effects of a zero-authority outcome", "Admit-only first use", and "Refusal before Admission" state rules and stay in force.
- ADR 0076, "Trait shape": the sentence "The contention refusal is a declared value of the command" and the example `apply_after_authority` describe the old surface. The rule of that section stays: the trait keeps only the parts that more than one command needs, and a hook stays when its step runs between two sequence steps that the command does not own.
- ADR 0076, "Consequences": "one `ProjectCommand` implementation that declares its profile" now reads: one `ProjectCommand` implementation that declares its five types and its `CommandSpec`.

## Considered options

- Three associated types and five methods. Rejected: no associated type has the same value in all commands, and the next fold puts `Error` or `Plan` into the settlement type of the application crate, which changes a boundary type for no reading gain.
- A data-driven command description with closures instead of the trait. Rejected by the author: one trait keeps one settlement order, as ADR 0076 decided.
- A Rust enumeration for each SQL domain of ADR 0079. Rejected: only the command kind and the AgentRun status have repeated definitions in Rust. An enumeration for the other five domains is a new mechanism.
- One AgentRun status enumeration shared by the core crate and the contracts crate. Rejected: the two crates are independent leaf crates, and the wire format stays apart from the domain.

## Consequences

- A new project command declares five types, one `CommandSpec`, and three methods. It implements the zero-authority side trait only when it writes zero-authority rows or decodes a zero-authority effect.
- A new command kind is one `CommandKind` variant, one registry entry, and one `ALTER DOMAIN`. The two cross-check tests fail when one is missing.
- The project command section of `CODING_STANDARDS.md` names the new surface.
- The framework under `command_sequence` grew by the declared surface, from 3,625 lines at the integration base to 3,831 lines, without its tests. The 23 files of the 24 command implementations went from 6,475 to 6,277 lines. Each command declares fewer types and hooks, and its `CommandSpec` has three more fields.
