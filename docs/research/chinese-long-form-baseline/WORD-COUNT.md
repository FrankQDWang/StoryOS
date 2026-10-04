# Chinese writing count baseline

Date: 2026-10-05. All web sources below were accessed on this date.

## Conclusions

1. At the 3,000,000-scalar scale, Word 16.89.1 reports **2,957,724 words** (1.409% fewer). The letters/numbers candidate reports **2,609,036** (13.032% fewer). These differences are large enough to label the profile explicitly in future product work.
2. The current `本章 N 字` label reports Unicode scalar values. It includes punctuation, spaces, line feeds, combining marks, and each scalar in an emoji sequence. This is the implemented contract, not a measured data-loss defect.
3. A platform name does not define a reproducible count. Official Word and WPS sources distinguish words from characters. Current platform help does not give a complete Unicode algorithm. The table below separates documented rules, actual Word measurements, and independent research profiles.
4. The candidate `letters-numbers.unicode-16.0.0.v1` counts only Unicode letters and numbers. It gives a precise Chinese writing measure without a word segmenter. It is a research option. The author still owns the choice; this work changes no product profile.
5. Exact WPS, Qidian, Fanqie, and current Jinjiang deltas remain unmeasured. No logged-in publishing flow was used. Invented platform algorithms would give false precision.

## Current StoryOS contract

- `apps/web/src/manuscript-statistics.tsx:93` displays `chapter.character_count`, not `word_count`.
- `crates/storyos-core/src/statistics_profile.rs:3` pins `storyos.statistics.unicode-16.0.0.v1`.
- `crates/storyos-core/src/statistics_profile.rs:22` adds one per scalar. Lines 24–29 count maximal runs separated by Unicode `White_Space` as `word_count`. A Chinese sentence without spaces can therefore be one `word_count` but many displayed characters.
- `crates/storyos-core/src/statistics_profile.rs:60` joins adjacent Block texts with one LF for statistics. It does not normalize stored text.
- `crates/storyos-application/src/manuscript_statistics.rs:103` sums Chapter counts. There is no added separator between Chapters in the total.

The comparison reads the exact generator JSON. It joins paragraphs within each Chapter with LF, counts each Chapter, then sums. Titles are excluded. The declared 30,000 / 300,000 / 1,000,000 / 3,000,000 scales refer to this current scalar count, not to billable platform words.

## First-party research

| Product or platform | Supported rule | What the source does not establish |
| --- | --- | --- |
| Microsoft Word | The Word Count dialog gives words and characters. The official API has separate `Words`, `Characters`, `CharactersWithSpaces`, and `FarEastCharacters` statistics. [Support](https://support.microsoft.com/zh-cn/word/training/show-word-count), [API enum](https://learn.microsoft.com/zh-cn/office/vba/api/word.wdstatistic). | No complete Unicode word segmentation, punctuation, supplementary-plane, or combining-mark rule is given. We measure the installed application below. |
| Microsoft Word automation | `Words.Count` includes punctuation and paragraph marks; Microsoft says to use the Word Count dialog for actual words. [Words object](https://learn.microsoft.com/en-us/office/vba/api/word.words). | A `Words.Count` script cannot stand in for the writing count. This harness uses `compute statistics`, not that collection. |
| WPS Writer | The official tutorial exposes words, characters, Chinese characters, non-Chinese words, and inclusion options for text boxes and notes. [WPS Academy, 2020-09-14](https://www.wps.cn/learning/course/detail/id/14155.html?chan=pc_win_hover). | No exact per-scalar policy is published here. A [WPS blog](https://plus.wps.cn/blog/p120628.html) labels total words as including punctuation, while a [WPS-hosted office-software FAQ](https://365.wps.cn/content/cfd86f428d2948969a94362ca891bed8.html) says words exclude punctuation and spaces. These overview pages are not a versioned algorithm; product/context differences are not resolved. |
| Qidian / Yuewen writer help | The current official writer FAQ states Chapter submission thresholds and distinguishes description characters from prose length. [Help](https://help.yuewen.com/helpcenter/pc/menu?siteId=2), [public help API](https://help.yuewen.com/helpapi/api/getmenu?siteId=2). | Searching the official help for `字数` and `标点` did not yield a full counting rule. No claim that punctuation is included or excluded in billed words follows from these sources. |
| Fanqie | Chapter help gives a 1,000 minimum and 50,000 maximum, and lists 2,000–6,000 as common Chapter sizes. [Official article API](https://fanqienovel.com/api/author/hfc/article_info/v0/?category_id=10164). Another article says publication removes spaces within paragraphs and extra blank lines. [Formatting article API](https://fanqienovel.com/api/author/hfc/article_info/v0/?category_id=10165), [help UI](https://fanqienovel.com/writer/zone/help). | Formatting after publication is not a complete count rule. The material does not specify how Latin, digits, combining marks, emoji, or supplementary Han contribute. |
| Jinjiang | The legacy official author guide explicitly says spaces and line breaks increase its count. [Legacy guide](https://www.jjwxc.net/help_write.php). Current help has separate writing, publication, and attendance rules. [Current author guide](https://help.jjwxc.net/index.php?t_id=23). | The legacy guide has no algorithm version. It is not proof of the current draft, published, or paid-Chapter counter. No current exact algorithm was found in the consulted author-help pages. |
| Zongheng | Official author policy describes which daily updates and edits enter attendance totals. [Author policy](https://doc.zongheng.com/welfare/girls). | Attendance eligibility is distinct from scalar counting. This source does not specify punctuation or Unicode classification. |

Platform user posts, search-engine Q&A, and third-party counter tools were excluded. Public read-only help APIs were followed from each site's served JavaScript. No account was created and no manuscript was uploaded to a vendor. The local Word run creates an unsaved document, reads statistics, and closes that document without saving.

## Precise research profiles

All properties use Unicode **16.0.0**. `count-profiles.py` requires a Python runtime whose `unicodedata.unidata_version` is `16.0.0`; the measured runtime is Python 3.14.8. The script has no package dependencies.

| Field | Exact rule |
| --- | --- |
| `scalars` | Number of Unicode scalar values in the stored body; includes all whitespace and marks. This matches the current UI body count for valid scalar text. |
| `white_space_runs` | Number of maximal nonempty runs outside Unicode 16 `White_Space`. This matches the current Core `word_count`. |
| `non_white_space` | One per scalar not in Unicode 16 `White_Space`. Punctuation, marks, controls outside that property, and emoji scalars remain. This is a sensitivity profile, not a platform clone. |
| `letters_numbers` | One per scalar whose Unicode 16 `General_Category` is `Lu`, `Ll`, `Lt`, `Lm`, `Lo`, `Nd`, `Nl`, or `No`. Every other scalar contributes zero. This is candidate `letters-numbers.unicode-16.0.0.v1`. |
| `wide_letters_other_tokens` | Each letter or number with `East_Asian_Width=W` or `F` contributes one and ends a narrow token. Consecutive other letters/numbers contribute one token. Category `M` marks contribute zero and do not break a token. Every remaining scalar breaks a token. This is an independent sensitivity profile for per-character CJK plus Latin/number tokens. |
| `utf16_units` | One per BMP scalar and two per supplementary scalar. This is a coordinate measure, not a proposed writing count. |

The candidate reads original scalars with no NFC, NFKC, case folding, trimming, or stored-text rewrite. It counts supplementary Han as one, excludes punctuation and spaces, and excludes combining marks and ordinary emoji. A keycap sequence containing an `Nd` digit contributes one because the digit is a number. A string of six Latin letters contributes six. A decomposed Hangul syllable can contribute more than its precomposed form. These are explicit semantics, not claims of grapheme or linguistic word equivalence.

The optional token profile also has deliberate limits: wide/fullwidth Latin letters count separately; kana and Hangul letters count separately; apostrophes and hyphens split narrow tokens. It must not be called “Word compatible” or a Chinese-platform count.

Property definitions and pinned values: [Unicode 16 UAX #44](https://www.unicode.org/reports/tr44/tr44-34.html), [Unicode 16 PropList](https://www.unicode.org/Public/16.0.0/ucd/PropList.txt). `White_Space` has 25 scalars; U+001C is not one of them. Python's `str.isspace()` is therefore not used as a substitute.

`prototypes/chinese-long-form-baseline/count-golden.json` has 19 expected vectors. They cover fullwidth punctuation, BMP and supplementary Han, NFC/NFD Latin, a family ZWJ emoji, a keycap, variation selectors, mixed scripts, numbers, CRLF, an ideographic space, and Hangul normalization differences. This data is a research deliverable; no test suite or default-build hook is added.

## Public-domain fixture

`public-domain.txt` contains the opening 2,401 scalars of Chapter 1 of *Hong lou meng*, from the [Project Gutenberg digital edition 24264](https://www.gutenberg.org/ebooks/24264). The catalog credits Wei-yi Kao for this digital text, dates its update to 2021-12-22, and marks it public domain in the USA. This is primary text evidence for that edition, not a claim that its spelling matches all editions.

`public-domain-source.json` records the source URL, source and fixture SHA-256, access date, exact excerpt boundaries, and the sole line-ending transformation (CRLF to LF). Original mixed spelling, punctuation, hard wraps, and indentation remain. Its hard wraps increase the current scalar count; no conversion to modern simplified Chinese was made. The fixture excludes the site's modern summary and all site boilerplate.

## Measurement and repeat command

```sh
python3 prototypes/chinese-long-form-baseline/count-profiles.py \
  --word --golden prototypes/chinese-long-form-baseline/count-golden.json \
  prototypes/chinese-long-form-baseline/out/corpus-[0-9]*.json \
  prototypes/chinese-long-form-baseline/public-domain.txt \
  > docs/research/chinese-long-form-baseline/word-count-evidence.json
```

The full runner also produces `out/word-count-evidence.json`; set `WORD_COUNTS=1` to include native Word measurements. Use the generated corpus paths printed by the main runner if the output directory differs. Without `--word`, the command needs only Python with Unicode 16.0.0 and works without Word. The executable name `python3` alone does not guarantee that Unicode version; the script checks it before measurement. With it, this host uses Microsoft Word **16.89.1** through the installed first-party `Word.sdef` `compute statistics` command. Source text enters a temporary unsaved document as Unicode. Chapter bodies are separated by LF for Word; there are no titles or notes. Counts are observations of this version, not a reverse-engineered Word contract. No wall-clock performance claim comes from this run.

For native Word, the harness joins Chapter bodies with one additional LF between Chapters. Thus its submitted text contains `C - 1` more separators than the sum of independent Chapter bodies used for the StoryOS baseline. The main table reports Word words for that document. Auxiliary Word character statistics in the JSON include that document layout and must not be treated as counts over a byte-identical flat StoryOS body. Titles remain excluded.

Each measurement row retains the exact input SHA-256. Delta is `(profile count - current scalar count) / current scalar count * 100`; a negative value means the current UI shows a larger number.

| Current scalar scale / fixture | Core whitespace runs | Non-whitespace scalars | Candidate letters/numbers | Wide letters + tokens | Actual Word words |
| --- | ---: | ---: | ---: | ---: | ---: |
| 30,000 / 15 Chapters | 352 (-98.827%) | 29,583 (-1.390%) | 26,136 (-12.880%) | 26,131 (-12.897%) | 29,578 (-1.407%) |
| 300,000 / 150 Chapters | 3,442 (-98.853%) | 295,797 (-1.401%) | 260,884 (-13.039%) | 260,809 (-13.064%) | 295,722 (-1.426%) |
| 1,000,000 / 500 Chapters | 11,557 (-98.844%) | 986,023 (-1.398%) | 869,515 (-13.049%) | 869,265 (-13.073%) | 985,773 (-1.423%) |
| 3,000,000 / 1,500 Chapters | 34,626 (-98.846%) | 2,958,474 (-1.384%) | 2,609,036 (-13.032%) | 2,608,286 (-13.057%) | 2,957,724 (-1.409%) |
| Public-domain excerpt | 74 (-96.918%) | 2,312 (-3.707%) | 1,984 (-17.368%) | 1,984 (-17.368%) | 2,312 (-3.707%) |

The 19 native Word observations are retained in `word-count-evidence.json`. Examples: `雨落在窗沿。` gives Word words = 6, including the Chinese full stop; `𠀀𠮷` gives 2; `e` plus U+0301 gives words = 1 and characters = 2; the family ZWJ emoji gives words = 1 and characters = 7. U+E0100 variation selection and the keycap sequence also differ from simple scalar counting. These observations show why a generic “exclude punctuation” or “count graphemes” shortcut cannot be named as the Word algorithm.

The synthetic data gives a 12.88%–13.05% reduction for the candidate; the public-domain excerpt gives 17.37%. This is sample sensitivity, not a universal conversion factor. The current Core whitespace-run count is only 34,626 at 3,000,000 displayed scalars, so it is unsuitable as a direct Chinese-character label.

Exact vendor gaps apply to **every** synthetic scale and the public-domain fixture: WPS = unmeasured; Qidian = unknown algorithm and unmeasured; Fanqie = unknown algorithm and unmeasured; current Jinjiang = unknown algorithm and unmeasured. Their values are not zero and are not replaced by the independent profiles above.

The remaining vendor comparison is a research gap: the consulted first-party help does not define a complete current Unicode algorithm, and this run has no measured current WPS or platform counter. Reproduction of those deltas needs an identified application/version or an authorized platform counter. This report completes the documented-rule review and the Word/independent-profile measurements; it does not claim that every vendor delta was obtained.
