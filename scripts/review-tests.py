#!/usr/bin/env python3
"""Tests of the review request of a round after round 1, ADR 0070 amendment of 2026-11-21 (issue 1749).

The lanes-tests lane of make check runs them."""

import importlib.util
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("review", Path(__file__).resolve().parent / "review.py")
review = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(review)

BASE, ROUND_1, HEAD, TREE = "a" * 40, "b" * 40, "c" * 40, "d" * 40


def verdict_comment(axis, result, blocking, head=f"Head `{ROUND_1}`, tree `{TREE}`."):
    return {"url": f"https://example.test/{axis}",
            "body": f"## {axis} review, round 1: {result}\n\n{head}\n"
                    f"Reviewer: Codex plugin, new read-only thread `thread`.\nCheck Evidence Line: none.\n\n"
                    f"### Blocking\n\n{blocking}\n\n### Non-blocking\n\nNone.\n\n### Evidence\n\n- Read.\n"}


def pull(comments):
    return {"url": "https://example.test/pull/7", "body": "Body.", "closingIssuesReferences": [], "comments": comments,
            "headRefOid": HEAD, "baseRefOid": BASE, "baseRefName": "main"}


def request_text(number, value):
    with tempfile.TemporaryDirectory() as directory:
        return review.request(7, number, value, None, Path(directory)).read_text()


class RequestTests(unittest.TestCase):
    def test_round_2_gives_the_earlier_head_the_diff_command_and_each_blocking_list(self):
        blocking = "1. `a.py:3` has a defect:\n```text\n\n### Non-blocking\n```\n2. `b.md:9` breaks a rule."
        comments = [verdict_comment("Standards", "FAIL", blocking),
                    {"url": "https://example.test/note", "body": "A coordinator note."},
                    verdict_comment("Spec", "PASS", "None.")]
        text = request_text(2, pull(comments))
        start = text.index(f"- Scoped diff: `git diff {BASE}...{HEAD}`\n")
        self.assertEqual(text[start:text.index("# Pull request")],
                         f"- Scoped diff: `git diff {BASE}...{HEAD}`\n\n"
                         f"- Earlier round: round 1, head `{ROUND_1}`\n"
                         f"- Changes after the earlier round: `git diff {ROUND_1} {HEAD}`\n\n"
                         "# Blocking findings of the earlier rounds\n\n"
                         f"## Round 1, Standards axis\n\n{blocking}\n\n"
                         "## Round 1, Spec axis\n\nNone.\n\n")

    def test_round_1_gives_no_earlier_round(self):
        text = request_text(1, pull([verdict_comment("Spec", "FAIL", "1. `c.rs:1` has a defect.")]))
        for absent in ("Earlier round", f"git diff {ROUND_1}", "Blocking findings", "c.rs:1"):
            self.assertNotIn(absent, text)

    def test_round_2_refuses_an_earlier_round_with_no_head(self):
        comments = [verdict_comment(axis, "FAIL", "1. `a.py:3` has a defect.", head="") for axis in ("Standards", "Spec")]
        with self.assertRaisesRegex(review.Refused, "No verdict comment of round 1 gives its head"):
            request_text(2, pull(comments))


if __name__ == "__main__":
    unittest.main()
