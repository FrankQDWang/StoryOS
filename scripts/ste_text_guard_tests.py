"""Run the ASD-STE100 text guard on disposable Git repositories."""

import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().with_name("ste_text_guard.py")
WORD_LIST = SCRIPT.parents[1] / "docs/agents/ste-rejected-words.json"
LONG = " ".join(["word"] * 25) + " &."
WOULD = ('"would" is on the rejected-word list. Write the text again without it. '
         'Use the present tense or "will" for a fact.')
WHOLE = '"whole" is on the rejected-word list. Use "all".'
ABOUT = ('"about" is on the rejected-word list. Use "approximately". '
         'For a subject, use "for" or write the text again.')
SEMICOLON = "semicolon: prose contains a semicolon. Write two sentences."
SENTENCE = "sentence-length: the sentence has 26 words. The limit is 25 words."
PARAGRAPH = "paragraph-length: the paragraph has 7 sentences. The limit is 6 sentences."
SEVEN = "One. Two. Three. Four. Five. Six. Seven."
COMMIT_MESSAGE = (
    "Write about the guide\n\n"
    "The whole guide is here.\n\n"
    "Co-Authored-By: Would Could\n <fixture@example.invalid>\n"
)


class TextGuardTests(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix="ste-text-guard-"))
        self.addCleanup(shutil.rmtree, self.root)
        self.environment = {**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}
        self.git("init", "--quiet", "--initial-branch=main")
        (self.root / "docs/agents").mkdir(parents=True)
        shutil.copyfile(WORD_LIST, self.root / "docs/agents/ste-rejected-words.json")
        self.write("GLOSSARY.md", "# Glossary\n\n**Whole Draft**:\nA draft term.\n")
        self.base = self.commit("Add the base files.")

    def git(self, *args):
        result = subprocess.run(["git", "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
                                 *args], cwd=self.root, env=self.environment, capture_output=True, text=True,
                                check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def write(self, path, text):
        (self.root / path).parent.mkdir(parents=True, exist_ok=True)
        (self.root / path).write_text(text)
        self.git("add", path)

    def commit(self, message):
        self.git("commit", "--quiet", "--allow-empty", "-m", message)
        return self.git("rev-parse", "HEAD")

    def guard(self, *args, expected_code):
        result = subprocess.run([sys.executable, str(SCRIPT), *args], cwd=self.root, env=self.environment,
                                capture_output=True, text=True, check=False)
        self.assertEqual((result.returncode, result.stderr), (expected_code, ""))
        return result.stdout.splitlines()

    def test_each_rule_fires_once_and_markup_and_glossary_terms_are_skipped(self):
        self.write("rules.md", f"# Rules\n\n{LONG}\n\n{SEVEN}\n\nKeep it short; split it.\n\nThe guard would stop.\n")
        self.write("markup.md", (
            "---\ntitle: whole; would\n---\n\n"
            "# The whole heading; would\n\n"
            "```text\nThe whole code block; would.\n" + LONG + "\n```\n\n"
            "| Column | Value |\n| --- | --- |\n| whole; would | x |\n\n"
            "<div>\nThe whole HTML block; would.\n</div>\n\n"
            "See [the guide](https://example.com/would;whole) and `would; whole`.\n"
            "Read https://example.com/a;would.\n\n"
            "The Whole Draft stays.\n"
        ))
        self.write("src/lib.rs", (
            "//! Crate docs.\n\n"
            "/// The whole value.\n"
            "pub fn call() {\n"
            '    let text = "would; whole // not a comment";\n'
            "    run(/*enabled*/ true); // Keep it; it is short.\n"
            "}\n"
        ))
        self.write("web/app.ts", (
            f"/**\n * {SEVEN}\n * @param value the input\n */\n"
            'export const url = "https://example.com/would"; // eslint-disable-line whole\n'
        ))
        self.write("data.json", '{"text": "would; whole"}\n')
        self.assertEqual(self.guard("files", "--base", self.base, expected_code=1), [
            f"rules.md:3: {SENTENCE}",
            f"rules.md:5: {PARAGRAPH}",
            f"rules.md:7: {SEMICOLON}",
            f"rules.md:9: rejected-word: {WOULD}",
            f"src/lib.rs:3: rejected-word: {WHOLE}",
            f"src/lib.rs:6: {SEMICOLON}",
            f"web/app.ts:2: {PARAGRAPH}",
        ])

    def test_lexical_state_and_block_structure_span_lines(self):
        self.write("web/template.ts", (
            "const text = `\n// whole; would\n`;\n"
            "// The whole text.\n"
            "/** Good. */ const n = 1; // would\n"
        ))
        self.write("src/raw.rs", 'let s = r#"\n// would; whole\n"#;\nlet c = \'"\';\n// The whole text.\n')
        self.write("structure.md", (
            "The whole heading\n=================\n\n"
            "Name | Value\n--- | ---\nwhole; would | x\n\n"
            "Use `whole\nwould;` here.\n\n"
            "<script>\nconst a = 1;\n\n// would; whole\n</script>\n\n"
            "<https://example.com/would> is a link.\n"
        ))
        self.assertEqual(self.guard("files", "--base", self.base, expected_code=1), [
            f"src/raw.rs:5: rejected-word: {WHOLE}",
            f"web/template.ts:4: rejected-word: {WHOLE}",
            f"web/template.ts:5: rejected-word: {WOULD}",
        ])

    def test_only_added_lines_fire_in_the_worktree_and_in_a_commit_range(self):
        self.write("notes.md", f"Old text; it stays.\n\n{LONG}\n\nKeep it in\nplace.\n")
        base = self.commit("Add the notes.")
        self.write("notes.md", f"Old text; it stays.\n\n{LONG}\n\nKeep it in\nflight.\n\nNew text; it is new.\n")
        expected = [f'notes.md:5: rejected-word: "in flight" is on the rejected-word list. Use "in progress".',
                    f"notes.md:8: {SEMICOLON}"]
        self.assertEqual(self.guard("files", "--base", base, expected_code=1), expected)
        self.commit("Change the notes.")
        self.assertEqual(self.guard("files", "--base", base, "--head", "HEAD", expected_code=1), expected)
        self.assertEqual(self.guard("files", "--base", "HEAD", expected_code=0), [])

    def test_commit_mode_skips_merge_commits_and_trailers_and_message_mode_agrees(self):
        self.git("switch", "--quiet", "-c", "side")
        self.write("side.md", "Side text.\n")
        self.commit("Add the side text")
        self.git("switch", "--quiet", "main")
        self.write("guide.md", "Guide text.\n")
        commit = self.commit(COMMIT_MESSAGE)
        self.git("merge", "--quiet", "--no-ff", "-m", "Merge the whole side branch; it would fit", "side")
        expected = [f"1: rejected-word: {ABOUT}", f"3: rejected-word: {WHOLE}"]
        self.assertEqual(self.guard("commits", "--base", self.base, expected_code=1),
                         [f"{commit[:12]}:{finding}" for finding in expected])
        (self.root / "message.txt").write_text(
            COMMIT_MESSAGE + "\n# The whole comment would be removed by Git.\n"
            "# ------------------------ >8 ------------------------\nThe whole diff; would.\n")
        self.assertEqual(self.guard("message", "message.txt", expected_code=1),
                         [f"message.txt:{finding}" for finding in expected])


if __name__ == "__main__":
    unittest.main()
