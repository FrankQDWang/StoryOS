# Domain docs

StoryOS uses a single domain context.

## Before exploring

- Read `CONTEXT.md` at the repository root when it exists.
- Read the ADRs under `docs/adr/` that affect the area being explored.
- If either location does not exist, proceed silently. Domain files are created lazily when a term or architectural decision is actually resolved.

## Layout

```text
/
├── CONTEXT.md
├── docs/
│   └── adr/
└── ...
```

`CONTEXT.md` is a glossary, not a specification or implementation notebook. It defines canonical domain terms and explicitly rejected synonyms without implementation details.

`docs/adr/` contains only decisions that are hard to reverse, surprising without context, and the result of a real trade-off.

## Consumer rules

- Use the glossary's canonical vocabulary in issue titles, designs, APIs, test names, and implementation discussions.
- If a needed concept is absent, reconsider whether a new term is necessary; if it is, resolve it through domain modeling and update the glossary at that time.
- If proposed work contradicts an existing ADR, identify the conflict explicitly instead of silently overriding the decision.
