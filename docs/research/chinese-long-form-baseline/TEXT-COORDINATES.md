# UTF-16 text-coordinate baseline

Product source: `479224809cdaae997cda51cb8853e3fafa242b65`. Harness source references: `9bcc6967`. Date: 2026-10-05.

## Conclusion

**No text-coordinate mismatch was found in 24 observations across the 30,000, 300,000, 1,000,000, and 3,000,000-scalar books.** At each scale, six exact comparisons agreed: independently expected text, rendered editor DOM text, the public Get Chapter body, and the decoded current PostgreSQL payload. No product, migration, generated contract, or existing test was changed.

The measured run is the completed controlled-plan run in `evidence/final/`. The earlier default run also has 24 exact comparisons; it is retained separately. The Unicode reload passed before the structural actions and export. A later reload after those actions entered a read-only state at every scale. That separate recovery finding does not show a text-coordinate mismatch and does not erase the successful earlier readback.

## Measured evidence

All four books contain the same seeded third Chapter. The harness edits that Chapter at each scale. The identical body lengths below are expected; this varies surrounding book size, not the selected Chapter size.

The exact inserted probe is:

```text
甲𠀀𠮷é👩‍👩‍👧‍👦😀🇨🇳✈️，。！？ＡＢ１２　末
```

It contains **27 Unicode scalars and 36 UTF-16 code units**. `𠀀` is U+20000 and `𠮷` is U+20BB7. The Latin sequence is U+0065 followed by U+0301. The family contains four people and three U+200D joiners. The flag contains two regional indicators. The airplane contains U+2708 followed by U+FE0F. The last space is U+3000.

| Observation | Body scalars | Body UTF-16 units | 30,000 | 300,000 | 1,000,000 | 3,000,000 |
| --- | ---: | ---: | --- | --- | --- | --- |
| Insert full probe | 1,930 | 1,939 | Equal | Equal | Equal | Equal |
| Replace `𠀀` with `界` | 1,930 | 1,938 | Equal | Equal | Equal | Equal |
| Replace `e` + U+0301 with `é` | 1,929 | 1,937 | Equal | Equal | Equal | Equal |
| Replace family with `👩🏽‍🚀` | 1,926 | 1,933 | Equal | Equal | Equal | Equal |
| Replace `，。！？` with `「好」` | 1,925 | 1,932 | Equal | Equal | Equal | Equal |
| Reload and reopen Project | 1,925 | 1,932 | Equal | Equal | Equal | Equal |

“Equal” means exact equality of all four strings. Counts and equality were recomputed from the retained strings, not copied only from `exact_equal`. Each evidence file also retains the raw UTF-8-decoded storage envelope.

| Scale | Retained evidence | File SHA-256 |
| --- | --- | --- |
| 30,000 | [30000-coordinates.json](evidence/final/30000-coordinates.json) | `31ae450679f98487ad47f71f099ff018cfb7b2bb82a822e8c0084c1d4d235e72` |
| 300,000 | [300000-coordinates.json](evidence/final/300000-coordinates.json) | `e5051febd23a75b38a061763bde9a847d2f32c38b24a33404b0a1f01c53233a7` |
| 1,000,000 | [1000000-coordinates.json](evidence/final/1000000-coordinates.json) | `467a8adf40feb4a5d9eb23923c92a3793def4271e9b9fa7f0e7d713a299dea78` |
| 3,000,000 | [3000000-coordinates.json](evidence/final/3000000-coordinates.json) | `5869b888038b58b9237734cb5096e8523766c771d17d8b48b1ca61b5ba2cb8be` |

At every scale the final expected/DOM/API/database body has UTF-8 SHA-256 `3ce33a6d05fe55445af417cb45870804dba64e03dfa296cfd2647df145c9490e`. Decomposed `e` + U+0301 survives the first save and becomes U+00E9 only after explicit replacement. The flag and U+FE0F sequence survive all edits and reload. This is evidence against silent normalization in this sample, not proof for every Unicode sequence.

## How the observation crosses the product

1. `prototypes/chinese-long-form-baseline/browser.mjs:8` launches installed Chrome in headless mode and loads the packaged production page. It uses the real editor and HTTP endpoints.
2. `browser.mjs:82-87` focuses the editor, sets a DOM selection, inserts text, and waits for a new Authoritative Revision and a saved state. Replacement selection uses `start + target.length`, in JavaScript UTF-16 units (`:133-137`). The script does not write prose through editor internals or database mutation.
3. Expected text starts from the visible Chapter plus the probe (`:115-116`), then applies each selected replacement independently (`:138-139`). This checks preservation relative to that base; it is not a second independent validation of the entire original import.
4. Each capture reads editor children joined by LF (`:47`), the public Chapter endpoint (`:121`), and the current authoritative PostgreSQL payload (`:122`). The database query follows Head → Revision → Payload and decodes UTF-8.
5. Multi-Block storage uses a JSON envelope. The harness decodes the envelope and joins Block text with one LF (`:124-125`). Raw JSON bytes are not expected to equal the flat display body.
6. Equality uses exact strings without trimming or normalization (`:126`). Reload repeats the readback (`:143`). DOM text is the measured visible-text surface; glyph shape and cursor pixels are outside this measurement.

The browser uses a prepared **writer-generation-2** Editor Session. The runner calls public Create Editor Session, Take Over Project Writer, and Set Current Chapter (`measure.mjs:180-184`). Before page load, the harness places that server-issued Session handle in `sessionStorage` (`browser.mjs:42`); it does not seed journal records. The product builds the IndexedDB journal and processes browser edits. This setup exercises a valid takeover-session continuation. It does not measure initial account login or a first-writer session with generation 1.

## Source map for each boundary

| Boundary | Owning behavior | Source evidence |
| --- | --- | --- |
| Contract | Authoritative prose preserves the accepted Unicode sequence. No implicit NFC/NFKC rewrite is permitted. Ordered units use UTF-16 coordinates relative to preceding units' results. | `docs/foundation/manuscript-revision-proposal-state-machine.md:248`, `:254`, `:616` |
| Web document text | Block text is placed directly in editor text nodes; reading concatenates child text without normalization. Flat Chapter display uses LF between Blocks. | `apps/web/src/manuscript-doc.ts:104`, `:121`; `crates/storyos-core/src/manuscript_payload.rs:241` |
| Web UTF-16 boundary | `isUtf16Boundary` rejects the position between a high and low surrogate. `contiguousUtf16Replace` compares `charCodeAt`, then adjusts shared edges away from a split surrogate pair. Its result contains exact replacement text. | `apps/web/src/manuscript-doc.ts:113`, `:188`, `:195`, `:206`, `:213` |
| Web command construction | Journal primitives and selection snapshots carry the same numeric `from`/`to` and the explicit `storyos.editor.utf16-code-unit.v1` profile. | `apps/web/src/local-edit-journal.ts:832`, `:845`, `:858` |
| ProseMirror versus local text | Whole-document ProseMirror positions include structural tokens. Structured-source capture subtracts `position + 1` to obtain text-local offsets and names its source profile. Therefore “UTF-16 throughout” does not mean every layer uses identical global integers. | `apps/web/src/structured-edit-capture.ts:34`, `:46`, `:47`, `:81` |
| HTTP / Server | Server reads the bounded JSON body, deserializes the public request, validates it, and converts public Author Edit units to Core input. | `crates/storyos-server/src/author_edit.rs:34`, `:37`, `:39`, `:99` |
| Wire-to-Core conversion | Replace Selection and Replace Block Selection copy `from`, `to`, and `text` unchanged. Selection profile and coordinates are also copied unchanged. | `crates/storyos-application/src/author_edit_wire.rs:32`, `:39`, `:138` |
| Core Block edit | Core checks the coordinate profile and applies replacement to the identified Block. Selection-snapshot endpoints must match the primitive's endpoints. | `crates/storyos-core/src/manuscript_payload.rs:223`, `:314`, `:326` |
| UTF-16 to Rust bytes | `utf16_offset_to_byte` walks scalar boundaries, accumulates each scalar's UTF-16 width, returns the corresponding UTF-8 byte boundary, and refuses overshoot. `replace_checked_utf16_range` replaces that byte range without a normalization step. | `crates/storyos-core/src/lib.rs:540`, `:554`, `:563`, `:567` |
| PostgreSQL write | A one-paragraph payload can remain plain text; multiple Blocks use a versioned JSON envelope. Their text fields are serialized as supplied. SQL uses `convert_to($4, 'UTF8')` to store canonical bytes. | `crates/storyos-adapter-postgres/src/manuscript_block.rs:9`, `:19`, `:30`; `crates/storyos-adapter-postgres/src/author_edit_settlement.rs:499` |
| PostgreSQL read / public body | Chapter retrieval uses `convert_from(payload.canonical_bytes, 'UTF8')`, restores Blocks, and derives the display body. The parser copies each Block's `text`. | `crates/storyos-adapter-postgres/src/chapter_query.rs:46`, `:89`, `:103`; `crates/storyos-adapter-postgres/src/manuscript_block.rs:45`, `:66` |
| Web reload | Hydration builds editor nodes from returned Block text and marks the hydration transaction separately from author history. The final capture checks the reloaded content against the existing expected string. | `apps/web/src/manuscript-tiptap-adapter.ts:68`; `prototypes/chinese-long-form-baseline/browser.mjs:143` |

PostgreSQL stores UTF-8 bytes here; it does not need to use UTF-16 offsets internally. Core performs the explicit UTF-16-to-byte mapping before persistence. The observed agreement is therefore agreement of selected ranges and text, not a claim that PostgreSQL string indices are UTF-16 indices.

## Scalar boundaries are not grapheme boundaries

For `A𠀀B`, the supplementary Han occupies UTF-16 range `[1,3)`. Offset 2 splits its surrogate pair and is rejected by the mapped Web/Core boundary checks. For `e` + U+0301, the boundary between scalars is valid even though the sequence commonly displays as one grapheme. The same distinction applies inside ZWJ, regional-indicator, and variation-selector sequences.

This run replaces whole combining and emoji sequences. It proves that the measured multi-unit ranges survive the full path. It does not prove that every arrow key, backspace, selection gesture, or IME treats a grapheme as one unit. The scalar-boundary contract does not require that. No half-surrogate request was submitted; rejection of that request is source evidence, not a new runtime observation.

## Coverage and exclusions

Covered at all four book sizes: production Chrome editor insertion; exact DOM selection; valid surrogate endpoints; two supplementary Han; decomposed-to-precomposed explicit replacement; family and skin-tone/ZWJ astronaut emoji; supplementary smiling emoji; regional-indicator flag; U+FE0F variation selector; fullwidth Latin/digits and punctuation; ideographic space; unchanged surrounding Chapter text and LF joins; saved Revision changes; public and direct database readback; same-browser reload before structure/export.

Not covered: OS IME composition; mobile/Safari/Firefox; glyph rendering; partial grapheme keyboard behavior; invalid lone surrogates; all Unicode blocks or normalization forms; bidi controls; embedded NUL; CRLF clipboard semantics; cross-Block/mixed-owner replacement; Proposal candidate/Acceptance coordinate routes; concurrent writers; interrupted in-flight recovery; alternate encodings. The fixed Chapter does not measure giant-Chapter behavior.

No source-backed text mismatch was found in the inspected ordinary Author Edit path. The later recovery failure is a distinct observed product state and is reported in the scale envelope.
