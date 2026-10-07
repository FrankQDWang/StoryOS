#!/usr/bin/env python3
"""Compare explicit research counts; optionally measure installed Microsoft Word."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unicodedata as ucd

WHITE_SPACE = set('\t\n\v\f\r \u0085\u00a0\u1680\u2028\u2029\u202f\u205f\u3000') | set(map(chr, range(0x2000, 0x200B)))


def counts(text):
    result = dict(scalars=len(text), white_space_runs=0, non_white_space=0,
                  letters_numbers=0, wide_letters_other_tokens=0,
                  utf16_units=len(text.encode('utf-16-le')) // 2)
    in_word = in_token = False
    for char in text:
        space = char in WHITE_SPACE
        result['non_white_space'] += not space
        result['white_space_runs'] += not space and not in_word
        in_word = not space
        category = ucd.category(char)[0]
        letter_number = category in 'LN'
        result['letters_numbers'] += letter_number
        if letter_number and ucd.east_asian_width(char) in 'WF':
            result['wide_letters_other_tokens'] += 1
            in_token = False
        elif letter_number:
            result['wide_letters_other_tokens'] += not in_token
            in_token = True
        elif category != 'M':
            in_token = False
    return result


def word_counts(text):
    script = '''on run args
set sourceText to read (POSIX file (item 1 of args)) as «class utf8»
tell application "Microsoft Word"
    set baselineDoc to make new document
    set content of text object of baselineDoc to sourceText
    set values to {version, compute statistics baselineDoc statistic statistic words, compute statistics baselineDoc statistic statistic characters, compute statistics baselineDoc statistic statistic characters with spaces, compute statistics baselineDoc statistic statistic east asian characters}
    close baselineDoc saving no
    return values
end tell
end run'''
    if not text:
        script = script.replace('read (POSIX file (item 1 of args)) as «class utf8»', '""')
    with tempfile.NamedTemporaryFile(suffix='.txt', mode='w', encoding='utf8') as source:
        source.write(text)
        source.flush()
        output = subprocess.check_output(['osascript', '-e', script, source.name], text=True, timeout=300).strip().split(', ')
    return dict(version=output[0], **dict(zip(['words', 'characters', 'characters_with_spaces', 'east_asian_characters'], map(int, output[1:]))))


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('inputs', nargs='+', type=Path)
    parser.add_argument('--word', action='store_true')
    parser.add_argument('--golden', type=Path)
    args = parser.parse_args()
    assert ucd.unidata_version == '16.0.0', 'Use Python with Unicode 16.0.0 for this pinned comparison.'
    rows = []
    for path in args.inputs:
        raw = path.read_bytes()
        if path.suffix == '.json':
            corpus = json.loads(raw)
            bodies = ['\n'.join(chapter['paragraphs']) for chapter in corpus['chapters']]
        else:
            bodies = [raw.decode('utf8')]
        totals = counts('')
        for body in bodies:
            for key, value in counts(body).items():
                totals[key] += value
        baseline = totals['scalars']
        row = dict(input=str(path), sha256=hashlib.sha256(raw).hexdigest(), chapters=len(bodies), counts=totals,
                   delta_percent={key: round((value-baseline)*100/baseline, 6) if baseline else 0 for key, value in totals.items()})
        if args.word:
            row['microsoft_word_measured'] = word_counts('\n'.join(bodies))
            row['microsoft_word_delta_percent'] = {key: round((value-baseline)*100/baseline, 6) if baseline else 0 for key, value in row['microsoft_word_measured'].items() if key != 'version'}
        rows.append(row)
    result = dict(unicode_version=ucd.unidata_version, rows=rows)
    if args.golden:
        result['golden_observations'] = [dict(name=case['name'], text=case['text'], measured=counts(case['text']), microsoft_word_measured=word_counts(case['text']) if args.word else None) for case in json.loads(args.golden.read_text())['vectors']]
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
