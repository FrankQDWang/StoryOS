#!/usr/bin/env python3
"""Fail closed when the Author Edit batch contract loses its exact boundary."""

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[2]
ADR = ROOT / "docs/adr/0003-specify-manuscript-revision-and-proposal-state-machine.md"
CONTRACT = ROOT / "docs/foundation/manuscript-revision-proposal-state-machine.md"
CONTEXT = ROOT / "CONTEXT.md"
POLICY = "storyos.author-edit-batch.release-1.v1"

REQUIRED_CONTRACT_TEXT = (
    "| idle gap between adjacent completed intents | 250 ms |",
    "| ordered `AuthorEditUnit` values in one command | 240 |",
    "| normalized primitives across the complete command | 240 |",
    "| encoded public JSON command body | 1 MiB under `storyos.foundation.absolute.v1` |",
    "Core evaluates the list from first to last",
    "against one transient working body",
    "Core never commits a valid prefix",
    "The digest covers the exact ordered unit list",
    "One Receipt settles the complete group",
    "roll-forward uses only the\nfinal canonical result",
)

REQUIRED_BINDING_TEXT = (
    "authenticated requester User",
    "Project\nScope and scope kind",
    "Client Session Binding record and generation",
    "applicable Editor Session and\nwriter generation",
    "pre-domain idempotency record and key",
    "one-use anti-forgery nonce record resolved from its challenge",
    "One challenge, nonce, idempotency record, and\nAdmission bind the complete list",
)

REQUIRED_SETTLEMENT_TEXT = (
    "final body equal to the initial body is one whole-command\n`NoEffect`",
    "transaction failure\ncreates no partial unit settlement",
    "Exact retry returns the same whole-command\nresult and never reapplies a unit",
    "A failure before\ncommit exposes no partial domain effect",
    "Each covered record refers to this same\nimmutable settlement",
    "Projection applies every unsettled unit in frozen order from its durable\ncheckpoint",
    "only after both the final canonical\nresult and its Receipt-backed Project Activity position are observed",
    "cannot be created until the current serialized group settles and the complete\nauthorized `EditorBaseSnapshot` tuple is installed",
)

REQUIRED_CONSUMER_BOUNDARY_TEXT = (
    "Complete Bounded Manual Input and IME Semantics",
    "only for Direct Author Action\n`ReplaceSelection` units",
    "does not authorize Proposal or mixed-ownership settlement",
    "`RequiresReconfirmation`, acknowledgement-loss handling, reload, restart",
    "recovery, writer or replay-generation change",
    "writer or replay-generation change, Snapshot resync, late-result",
    "Local Edit Journal garbage collection",
    "Reconcile Acknowledgement Loss without Duplicate Authority",
    "Recover Settled and Unsettled Edits across Reload and Restart",
    "Fence Stale Writers and Resync across Replay Generations",
    "https://github.com/FrankQDWang/StoryOS/issues/108",
    "https://github.com/FrankQDWang/StoryOS/issues/109",
    "https://github.com/FrankQDWang/StoryOS/issues/110",
    "https://github.com/FrankQDWang/StoryOS/issues/111",
)


def contract_errors(adr: str, contract: str, context: str) -> list[str]:
    errors: list[str] = []
    if "**Author Edit**:" not in context or "bounded idle-coalesced sequence" not in context:
        errors.append("CONTEXT.md lost the Author Edit bounded-sequence definition")
    if adr.count(POLICY) != 1:
        errors.append("ADR 0003 must name the batch policy exactly once")
    if contract.count(POLICY) != 2:
        errors.append("the state machine must name the policy in its rule and invariant")
    for required in REQUIRED_CONTRACT_TEXT:
        if required not in contract:
            errors.append(f"the state machine lost required text: {required}")
    for required in REQUIRED_BINDING_TEXT:
        if required not in contract:
            errors.append(f"the state machine lost an exact batch binding: {required}")
    for required in REQUIRED_SETTLEMENT_TEXT:
        if required not in contract:
            errors.append(f"the state machine lost a batch settlement rule: {required}")
    for required in REQUIRED_CONSUMER_BOUNDARY_TEXT:
        if required not in contract:
            errors.append(f"the state machine lost a consumer boundary: {required}")
    for required in ("at most 240 ordered `AuthorEditUnit` values", "250 ms idle gap"):
        if required not in adr:
            errors.append(f"ADR 0003 lost required decision text: {required}")
    return errors


def current_sources() -> tuple[str, str, str]:
    return (
        ADR.read_text(encoding="utf-8"),
        CONTRACT.read_text(encoding="utf-8"),
        CONTEXT.read_text(encoding="utf-8"),
    )


def self_test() -> None:
    adr, contract, context = current_sources()
    assert contract_errors(adr, contract, context) == []
    assert contract_errors(adr, contract.replace("250 ms", "500 ms"), context)
    assert contract_errors(
        adr, contract.replace("one transient working body", "one body"), context
    )
    for required in (
        *REQUIRED_BINDING_TEXT,
        *REQUIRED_SETTLEMENT_TEXT,
        *REQUIRED_CONSUMER_BOUNDARY_TEXT,
    ):
        assert contract_errors(adr, contract.replace(required, "", 1), context)


def main() -> None:
    errors = contract_errors(*current_sources())
    if errors:
        raise SystemExit("Author Edit batch contract verification failed:\n- " + "\n- ".join(errors))
    print(
        f"verified {POLICY} ordering, bounds, exact bindings, atomic settlement, "
        "projection effects, and consumer boundary"
    )


if __name__ == "__main__":
    self_test() if sys.argv[1:] == ["--self-test"] else main()
