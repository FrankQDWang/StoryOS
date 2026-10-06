#!/usr/bin/env python3
"""Find ASD-STE100 defects in the prose that a change adds.

Modes:
  files --base REV [--head REV]    Markdown files and Rust and TypeScript comments in the diff.
  commits --base REV [--head REV]  Commit messages in the range, without merge commits.
          [--advisory]             Mark each finding "advisory: " and exit 0.
  message FILE                     One commit message from a file.
  install-hook                     Write the commit-msg hook that runs the message mode.

The range starts at the merge base of REV and the head. Without --head, the
files mode compares with the worktree and the commits mode stops at HEAD.
Each finding is one line `path:line: rule: message`. Exit 1 on a finding.
"""

import argparse
from collections import Counter
import json
from pathlib import Path
import re
import subprocess
import sys

WORD_LIST = "docs/agents/ste-rejected-words.json"
GLOSSARY = "GLOSSARY.md"
GLOSSARY_AREAS = "docs/glossary"
MAX_SENTENCE_WORDS = 25
MAX_PARAGRAPH_SENTENCES = 6
MARKDOWN = (".md",)
COMMENTED = {".rs": "rust", ".ts": "typescript", ".tsx": "typescript", ".mts": "typescript", ".cts": "typescript"}
# A placeholder for code, URLs, and entities: it counts as one word and matches no rule.
HOLE = ""

FENCE = re.compile(r"(`{3,}|~{3,})")
HEADING = re.compile(r"#{1,6}(\s|$)")
LIST_ITEM = re.compile(r"([-*+]|\d+[.)])\s+")
LINK_DEFINITION = re.compile(r"\[[^\]]+\]:\s")
RULE_LINE = re.compile(r"[-=*_ ]{3,}")
HTML_START = re.compile(r"<(!--|/[A-Za-z]|[A-Za-z][A-Za-z0-9-]*(?=[\s/>]|$))")
HTML_CLOSED_BY_TAG = ("script", "pre", "style", "textarea")
TABLE_DELIMITER = re.compile(r"\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)*\|?")
RAW_STRING = re.compile(r'r(#*)"')
CHAR_LITERAL = re.compile(r"'(\\.|[^\\'])'")
TOOL_COMMENT = re.compile(r"\s*(eslint-|@ts-|prettier-|biome-|#region|#endregion)")
SENTENCE_END = re.compile(r"[.?!]+[)\]\"'’”]*(?=\s|$)")
TRAILER = re.compile(r"[A-Za-z0-9][A-Za-z0-9-]*: \S")
HUNK = re.compile(r"@@ -\d+(?:,(\d+))? \+(\d+)(?:,(\d+))? @@")
INLINE = [
    (re.compile(r"(`+)(.+?)\1", re.DOTALL), HOLE),
    (re.compile(r"!?\[([^\]]*)\]\([^)]*\)"), r"\1"),
    (re.compile(r"\[([^\]]*)\]\[[^\]]*\]"), r"\1"),
    (re.compile(r"<https?://[^>]*>"), HOLE),
    (re.compile(r"https?://[^\s<>()]*[^\s<>().,;:!?'\"]"), HOLE),
    (re.compile(r"</?[A-Za-z][^>]*>"), ""),
    (re.compile(r"&#?\w+;"), HOLE),
]


class GuardError(Exception):
    """A usage, Git, or word-list error. It is not a text finding."""


def git(root, *args):
    result = subprocess.run(["git", "-c", "core.quotePath=false", *args], cwd=root,
                            capture_output=True, text=True, check=False)
    if result.returncode:
        raise GuardError(f"git {' '.join(args)}: {result.stderr.strip()}")
    return result.stdout


def load_rules(root):
    try:
        data = json.loads((root / WORD_LIST).read_text())
    except (OSError, ValueError) as error:
        raise GuardError(f"{WORD_LIST}: {error}") from error
    entries = data.get("words") if isinstance(data, dict) else None
    if not isinstance(entries, list) or not entries:
        raise GuardError(f"{WORD_LIST}: the 'words' list is missing or empty")
    words = {}
    for entry in entries:
        if (not isinstance(entry, dict) or not set(entry) <= {"word", "alternative", "note"}
                or not isinstance(entry.get("word"), str) or not isinstance(entry.get("alternative"), str)
                or not isinstance(entry.get("note", ""), str)
                or entry["word"] != entry["word"].strip().lower() or not entry["word"]
                or entry["word"] in words):
            raise GuardError(f"{WORD_LIST}: invalid or duplicate entry {json.dumps(entry)}")
        words[entry["word"]] = entry
    pattern = re.compile(r"\b(" + "|".join(re.escape(word).replace(r"\ ", r"\s+")
                                           for word in sorted(words, key=len, reverse=True)) + r")\b",
                         re.IGNORECASE)
    terms = []
    for glossary in [root / GLOSSARY, *sorted((root / GLOSSARY_AREAS).glob("*.md"))]:
        if glossary.is_file():
            terms += re.findall(r"^\*\*([^*]+)\*\*:", glossary.read_text(), re.MULTILINE)
    glossary_pattern = (re.compile(r"\b(" + "|".join(re.escape(term) for term in sorted(terms, key=len, reverse=True))
                                   + r")\b") if terms else None)
    return words, pattern, glossary_pattern


def paragraphs(lines):
    """Group (line, text) pairs into prose paragraphs. A None text is a break."""
    result, current = [], []
    fence = html = None
    table = False

    def flush():
        nonlocal current
        if current:
            text = "\n".join(segment for _, segment in current)
            for pattern, replacement in INLINE:
                text = pattern.sub(lambda match: match.expand(replacement) + "\n" * (
                    match.group(0).count("\n") - match.expand(replacement).count("\n")), text)
            result.append(list(zip([number for number, _ in current], text.split("\n"))))
        current = []

    for number, text in lines:
        stripped = "" if text is None else text.strip()
        if text is None:
            flush()
            fence = html = None
            table = False
        elif fence:
            fence = None if stripped.startswith(fence) else fence
        elif html is not None:
            html = None if (html and html in stripped.lower()) or not (html or stripped) else html
        elif not stripped:
            flush()
            table = False
        elif table:
            continue
        elif match := FENCE.match(stripped):
            flush()
            fence = match.group(1)
        elif "|" in stripped and TABLE_DELIMITER.fullmatch(stripped) and current:
            current.pop()
            flush()
            table = True
        elif RULE_LINE.fullmatch(stripped):
            if set(stripped.replace(" ", "")) in ({"="}, {"-"}):
                current = []
            flush()
        elif stripped.startswith("|"):
            flush()
            table = True
        elif HEADING.match(stripped) or LINK_DEFINITION.match(stripped):
            flush()
        elif tag := HTML_START.match(stripped):
            flush()
            name = tag.group(1).lower()
            html = "-->" if name == "!--" else f"</{name}>" if name in HTML_CLOSED_BY_TAG else ""
            html = None if html and html in stripped[tag.end():].lower() else html
        else:
            stripped = stripped.lstrip("> ").strip() if stripped.startswith(">") else stripped
            item = LIST_ITEM.match(stripped)
            if item or stripped.startswith("@"):
                flush()
                stripped = stripped[item.end():] if item else stripped
            current.append((number, stripped))
    flush()
    return result


def markdown_lines(text):
    lines = list(enumerate(text.splitlines(), start=1))
    if lines and lines[0][1].strip() == "---":
        for index, (_, line) in enumerate(lines[1:], start=1):
            if line.strip() == "---":
                return [(number, None) for number, _ in lines[:index + 1]] + lines[index + 1:]
    return lines


def comment_lines(text, language):
    """Return the line comments and doc comments as (line, text) pairs, outside string literals."""
    bodies, index, line = {}, 0, 1
    rust = language == "rust"
    quotes = "\"" if rust else "\"'`"

    def skip_to(end):
        nonlocal index, line
        line += text.count("\n", index, end)
        index = end

    while index < len(text):
        char = text[index]
        if text.startswith("//", index):
            end = text.find("\n", index) % (len(text) + 1)
            body = re.sub(r"^[/!]", "", text[index + 2:end])
            if not TOOL_COMMENT.match(body):
                bodies.setdefault(line, []).append(body)
            index = end
        elif text.startswith("/*", index):
            end = text.find("*/", index + 2) % (len(text) + 1)
            if text.startswith(("/**", "/*!"), index) and not text.startswith("/**/", index):
                for offset, body in enumerate(text[index + 3:end].split("\n")):
                    body = body.strip()
                    bodies.setdefault(line + offset, []).append(body[1:] if body.startswith("*") else body)
            skip_to(end + 2)
        elif rust and (raw := RAW_STRING.match(text, index)) and not text[index - 1:index].isidentifier():
            end = text.find("\"" + raw.group(1), raw.end()) % (len(text) + 1)
            skip_to(end + 1 + len(raw.group(1)))
        elif rust and (literal := CHAR_LITERAL.match(text, index)):
            index = literal.end()
        elif char in quotes:
            end = index + 1
            while end < len(text) and text[end] != char:
                end += 2 if text[end] == "\\" else 1
            skip_to(end + 1)
        else:
            line += char == "\n"
            index += 1
    return [(number, " ".join(bodies[number]) if number in bodies else None)
            for number in range(1, text.count("\n") + 2)]


def message_lines(text):
    lines = []
    for line in text.splitlines():
        if line.startswith("#") and ">8" in line:
            break
        lines.append("" if HEADING.match(line) else line)
    while lines and not lines[-1].strip():
        lines.pop()
    start = len(lines)
    while start and lines[start - 1].strip():
        start -= 1
    block = lines[start:]
    if start and TRAILER.match(block[0]) and all(TRAILER.match(line) or line[:1].isspace() for line in block):
        lines = lines[:start]
    return list(enumerate(lines, start=1))


def check(path, lines, added, rules):
    """Return the findings of the paragraphs that contain an added line."""
    words, pattern, glossary = rules
    findings = set()
    for paragraph in paragraphs(lines):
        if not any(number in added for number, _ in paragraph):
            continue
        text, starts = "", []
        for number, segment in paragraph:
            starts.append((len(text), number))
            text += segment + " "

        def line_at(offset):
            return max((start, number) for start, number in starts if start <= offset)[1]

        sentences, begin = [], 0
        for end in [match.end() for match in SENTENCE_END.finditer(text)] + [len(text)]:
            count = len(text[begin:end].split())
            if count:
                sentences.append((begin, end, count))
            begin = end
        for begin, end, count in sentences:
            spanned = [line_at(offset) for offset in range(begin, end) if not text[offset].isspace()]
            if count > MAX_SENTENCE_WORDS and set(spanned) & added:
                findings.add((spanned[0], "sentence-length",
                              f"the sentence has {count} words. The limit is {MAX_SENTENCE_WORDS} words."))
        if len(sentences) > MAX_PARAGRAPH_SENTENCES:
            findings.add((paragraph[0][0], "paragraph-length",
                          f"the paragraph has {len(sentences)} sentences. "
                          f"The limit is {MAX_PARAGRAPH_SENTENCES} sentences."))
        for match in re.finditer(";", text):
            if line_at(match.start()) in added:
                findings.add((line_at(match.start()), "semicolon",
                              "prose contains a semicolon. Write two sentences."))
        masked = glossary.sub(lambda match: HOLE * len(match.group(0)), text) if glossary else text
        for match in pattern.finditer(masked):
            if not {line_at(offset) for offset in range(match.start(), match.end())} & added:
                continue
            phrase = " ".join(match.group(0).split())
            entry = words[phrase.lower()]
            advice = (f'Use "{entry["alternative"]}".' if entry["alternative"]
                      else "Write the text again without it.")
            note = f' {entry["note"]}' if entry.get("note") else ""
            findings.add((line_at(match.start()), "rejected-word",
                          f'"{phrase}" is on the rejected-word list. {advice}{note}'))
    return [f"{path}:{line}: {rule}: {message}" for line, rule, message in sorted(findings)]


def added_lines(root, base, head):
    """Map each changed text path to the line numbers that the range adds.

    A line that the range removes from a text path and adds with the same text is moved, not added.
    """
    diff = git(root, "diff", "--no-color", "--no-ext-diff", "-U0", "-M", base, *([head] if head else []))
    added, removed = [], Counter()
    old_path = path = None
    old_left = new_left = number = 0
    for line in diff.splitlines():
        if old_left and line.startswith("-"):
            old_left -= 1
            if old_path:
                removed[line[1:]] += 1
        elif new_left and line.startswith("+"):
            new_left -= 1
            if path:
                added.append((path, number, line[1:]))
            number += 1
        elif old_left or new_left:
            continue
        elif line.startswith(("--- a/", "+++ b/", "--- /dev/null", "+++ /dev/null")):
            name = line[6:] if line[4:6] in ("a/", "b/") else None
            text_path = name if name and (Path(name).suffix in MARKDOWN or Path(name).suffix in COMMENTED) else None
            if line.startswith("-"):
                old_path = text_path
            else:
                path = text_path
        elif match := HUNK.match(line):
            old_left, number, new_left = int(match.group(1) or 1), int(match.group(2)), int(match.group(3) or 1)
    result = {}
    for path, number, text in added:
        if text.strip() and removed[text]:
            removed[text] -= 1
        else:
            result.setdefault(path, set()).add(number)
    return result


def merge_base(root, base, head):
    return git(root, "merge-base", base, head or "HEAD").strip()


def run_files(root, rules, base, head):
    findings = []
    for path, added in sorted(added_lines(root, merge_base(root, base, head), head).items()):
        text = git(root, "show", f"{head}:{path}") if head else (root / path).read_text(errors="replace")
        suffix = Path(path).suffix
        lines = markdown_lines(text) if suffix in MARKDOWN else comment_lines(text, COMMENTED[suffix])
        findings += check(path, lines, added, rules)
    return findings


def run_commits(root, rules, base, head):
    findings = []
    start = merge_base(root, base, head)
    for commit in git(root, "rev-list", "--no-merges", "--reverse", f"{start}..{head or 'HEAD'}").split():
        lines = message_lines(git(root, "show", "-s", "--format=%B", commit))
        findings += check(commit[:12], lines, {number for number, _ in lines}, rules)
    return findings


def run_message(rules, path):
    try:
        text = Path(path).read_text()
    except OSError as error:
        raise GuardError(str(error)) from error
    lines = message_lines(text)
    return check(path, lines, {number for number, _ in lines}, rules)


HOOK = """#!/bin/sh
# StoryOS commit-msg hook from `make install-hooks`. It runs the ASD-STE100 text guard.
guard="$(git rev-parse --show-toplevel)/scripts/ste_text_guard.py"
[ -f "$guard" ] || exit 0
exec python3 "$guard" message "$1"
"""


def install_hook(root):
    hook = root / git(root, "rev-parse", "--git-path", "hooks").strip() / "commit-msg"
    if hook.exists() or hook.is_symlink():
        if hook.is_file() and hook.read_text(errors="replace") == HOOK:
            return f"{hook}: the hook is current."
        raise GuardError(f"{hook}: a different hook exists. Remove it, then run the command again.")
    hook.parent.mkdir(parents=True, exist_ok=True)
    hook.write_text(HOOK)
    hook.chmod(0o755)
    return f"{hook}: the hook is installed."


def main(argv):
    parser = argparse.ArgumentParser(description="Find ASD-STE100 defects in the prose that a change adds.")
    modes = parser.add_subparsers(dest="mode", required=True)
    for mode in ("files", "commits"):
        command = modes.add_parser(mode)
        command.add_argument("--base", required=True)
        command.add_argument("--head")
        if mode == "commits":
            command.add_argument("--advisory", action="store_true")
    modes.add_parser("message").add_argument("path")
    modes.add_parser("install-hook")
    arguments = parser.parse_args(argv)
    try:
        root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").strip())
        if arguments.mode == "install-hook":
            print(install_hook(root))
            return 0
        rules = load_rules(root)
        if arguments.mode == "files":
            findings = run_files(root, rules, arguments.base, arguments.head)
        elif arguments.mode == "commits":
            findings = run_commits(root, rules, arguments.base, arguments.head)
        else:
            findings = run_message(rules, arguments.path)
    except GuardError as error:
        print(f"ste-text-guard: {error}", file=sys.stderr)
        return 2
    advisory = getattr(arguments, "advisory", False)
    for finding in findings:
        print(f"advisory: {finding}" if advisory else finding)
    return 1 if findings and not advisory else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
