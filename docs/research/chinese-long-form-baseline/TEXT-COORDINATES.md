# UTF-16 text-coordinate baseline

Source snapshot: `479224809cdaae997cda51cb8853e3fafa242b65`. Date: 2026-10-05.

## Conclusion

**No text-coordinate mismatch was found in the successful 30,000-scalar pilot.** In six observations, independently expected text equaled the rendered editor's DOM text, the public Get Chapter body, and the decoded current PostgreSQL payload. This includes supplementary Han, decomposed Latin, emoji sequences, fullwidth forms, punctuation, and reload.

This conclusion covers the measured pilot below. The final four-scale run is separate evidence and was still pending when this report was written. No product, migration, generated contract, or existing test was changed.

## Measured evidence

Pilot artifact:

`prototypes/chinese-long-form-baseline/out/30000-attempt-1791135994785/coordinates.json`

Artifact SHA-256: `0f38d4574124872c201ab1bd17b7fc8d4ed7359aba701e1986be8cf945c97b4c`.

The pilot inserted this exact sequence, with `e` followed by U+0301:

```text
甲𠀀𠮷é👩‍👩‍👧‍👦😀，。！？ＡＢ１２　末
```

It contains **23 Unicode scalars and 30 UTF-16 code units**. The last space is U+3000. The family emoji contains four people and three U+200D joiners. `𠀀` is U+20000 and `𠮷` is U+20BB7.

| Observation | Action | Expected body scalars | Expected body UTF-16 units | Expected = DOM = API = decoded database |
| --- | --- | ---: | ---: | --- |
| Insert | Append the full probe to an existing Chapter | 2,181 | 2,188 | Yes |
| Supplementary Han | Replace `𠀀` with `界` | 2,181 | 2,187 | Yes |
| Combining sequence | Replace `e` + U+0301 with U+00E9 `é` | 2,180 | 2,186 | Yes |
| Emoji sequence | Replace `👩‍👩‍👧‍👦` with `👩🏽‍🚀` | 2,177 | 2,182 | Yes |
| Punctuation | Replace `，。！？` with `「好」` | 2,176 | 2,181 | Yes |
| Reload | Reload the production page and reopen the Project | 2,176 | 2,181 | Yes |

The final expected/DOM/API/database body has SHA-256 `f6f2662c89c7fa6241eb73865cf0790a34efc023f0433e732649c2a79dffb2a6` when encoded as UTF-8. The initial inserted body still contains decomposed `e` + U+0301. It becomes U+00E9 only in the explicit replacement. Thus this sample did not silently normalize to NFC during save or reload.

The current harness also includes `🇨🇳✈️` in its probe. **Those two sequences are absent from this pilot artifact.** Do not infer flag or U+FE0F variation-selector end-to-end coverage from the newer harness source alone. The final run can extend this table after its own artifacts exist.

## How the observation crosses the product

1. `prototypes/chinese-long-form-baseline/browser.mjs:8` launches installed Chrome in headless mode and loads the packaged production page. This uses the real editor and HTTP endpoints.
2. `browser.mjs:74` focuses the editor, sets a DOM selection, and calls browser text insertion. Each change waits for a new Authoritative Revision and a saved state. Replacement selection uses `start + target.length`, where JavaScript length is in UTF-16 units (`:156`, `:160`, `:164`). The script changes neither editor state objects nor database prose directly.
3. Expected text is built from the prior expected string using the selected target and replacement (`:143`, `:165`). The original body is first read from the editor; this checks preservation relative to that base, not an independent second verification of the entire original import.
4. Each capture separately reads editor child text joined with LF, the public Chapter endpoint, and the current authoritative database payload (`:42`, `:146`, `:148`, `:149`). The database read follows the current Head to its Revision and Payload and decodes UTF-8.
5. Versioned storage contains a JSON Block envelope, so comparison decodes that envelope and joins Block text with one LF (`:151`). Raw JSON envelope bytes are not expected to equal the flat display body. The artifact retains both the raw decoded storage string and the resulting body.
6. Equality uses exact strings, without normalization or trimming (`:153`). The six stored booleans were also checked against the four retained strings when this report was prepared. This is stronger than a screenshot-only or Server-only check. DOM text is the measured visible-text surface; glyph shape and cursor pixels were not measured.

The harness restores its existing authorized Editor Session handle in `sessionStorage` before page load (`browser.mjs:40`). This is session continuity for the disposable Project, not evidence about a fresh account login, native OS input method, or writer takeover.

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
| Web reload | Hydration builds editor nodes from returned Block text and marks the hydration transaction separately from author history. The final pilot capture checks the reloaded content against the existing expected string. | `apps/web/src/manuscript-tiptap-adapter.ts:68`; `prototypes/chinese-long-form-baseline/browser.mjs:170` |

PostgreSQL stores UTF-8 bytes here; it does not need to use UTF-16 offsets internally. Core performs the explicit UTF-16-to-byte mapping before persistence. The observed agreement is therefore agreement of selected ranges and text, not a claim that PostgreSQL string indices are UTF-16 indices.

## Scalar boundaries are not grapheme boundaries

For `A𠀀B`, the supplementary Han occupies UTF-16 range `[1,3)`. Offset 2 splits its surrogate pair and is rejected by the mapped Web/Core boundary checks. For `e` + U+0301, the boundary between the two scalars is valid even though the sequence commonly displays as one grapheme. The same distinction applies between complete scalars within a ZWJ emoji sequence.

The pilot replaces whole combining and emoji sequences. It shows that their multi-unit ranges survive the full path. It does **not** prove that every arrow key, backspace, selection gesture, or IME operation treats a grapheme as one editing unit. The current scalar-boundary contract does not require that. No half-surrogate request was submitted in this pilot; its rejection is source evidence, not a new runtime observation.

## Coverage and exclusions

Covered: Chrome production editor text insertion; exact DOM ranges; valid surrogate-pair endpoints; two supplementary Han; decomposed-to-precomposed explicit replacement; family and skin-tone/ZWJ astronaut emoji; a supplementary smiling emoji; fullwidth Latin/digits; fullwidth Chinese punctuation; ideographic space; unchanged surrounding Chapter text and LF joins; saved Revision changes; public readback; direct decoded PostgreSQL readback; same-browser reload.

Not covered by this pilot: OS IME composition; mobile/Safari/Firefox; font fallback and glyph rendering; partial grapheme keyboard behavior; invalid lone surrogates; all Unicode blocks or normalization forms; bidi controls; embedded NUL; CRLF clipboard semantics; cross-Block/mixed-owner structured replacement; Proposal candidate/Acceptance coordinate routes; concurrent writers; interrupted in-flight recovery; alternate storage encodings. The final four-scale run must be reported separately before claiming coverage at 300,000, 1,000,000, or 3,000,000 scalars.

No source-backed text mismatch was found in the inspected ordinary Author Edit path. The scalar/grapheme distinction and the pilot/new-harness probe difference are recorded above rather than classified as text corruption.
