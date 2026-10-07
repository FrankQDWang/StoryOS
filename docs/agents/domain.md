# Domain docs

StoryOS uses a single domain context.

## Before exploring

- Read `GLOSSARY.md` at the repository root. It lists the glossary design areas and tells how to find a term in `docs/glossary/` with `grep`. Read only the area files that you need.
- Read the ADRs under `docs/adr/` that affect the area being explored.
- If either location does not exist, proceed silently. Domain files are created lazily when a term or architectural decision is actually resolved.

## Layout

```text
/
├── GLOSSARY.md
├── docs/
│   ├── adr/
│   └── glossary/
└── ...
```

`GLOSSARY.md` is the index of the glossary. `docs/glossary/` holds one file for each design area. The glossary is not a specification or implementation notebook. It defines canonical domain terms and explicitly rejected synonyms without implementation details.

`docs/adr/` contains only decisions that are hard to reverse, surprising without context, and the result of a real trade-off.

## Consumer rules

- Use the glossary's canonical vocabulary in issue titles, designs, APIs, test names, and implementation discussions.
- If a needed concept is absent, decide again whether a new term is necessary. If it is, resolve it through domain modeling. Then add it to the area file that owns its concept.
- If proposed work contradicts an existing ADR, identify the conflict explicitly instead of silently overriding the decision.
