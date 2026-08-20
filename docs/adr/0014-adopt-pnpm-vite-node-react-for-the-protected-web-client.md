---
status: accepted
---

# Adopt pnpm, Vite, Node, and React for the Protected Web Client

The production Protected Web Client in `apps/web` is built with pnpm, Vite 6.4.3, `@vitejs/plugin-react` 5.2.0, Node, and React 19.2.7. The first delivery is a toolchain and host change only: it keeps the Stage 1 author surface (textarea journey) and does not change Editor Session, Local Edit Journal, or Author Command Admission meaning.

The first implementation Issue is inserted before Stage 1 evidence closeout (`#112`). React rewrites only the Stage 1 view. Existing `.mjs` boot, Editor Session, Journal, and submission modules stay in place and are imported by that view. `storyos-server` does not host Vite assets in this slice. The repository keeps one `pnpm-workspace.yaml` at the root and includes only `apps/web` in this slice. Browser tests stay on `node:test` plus desktop Chrome. The Web Client keeps importing the checked-in generated TypeScript client.

`make web` runs `vite build` first. At least one real Chrome test loads the Vite production page (`dist/index.html` and hashed assets). Existing module-harness Chrome tests keep importing source `.mjs` modules. Local `vite dev` may proxy `/api` to a running StoryOS Server; that proxy is not part of `make verify`.

Disposable prototypes remain outside production. The first delivery does not promote `prototypes/**`, does not add TipTap or a three-column writing workspace, and does not convert the existing `.mjs` session and journal modules to TypeScript.

## Considered options

- Keeping the current unbundled `.mjs` client was rejected because it is not a reviewable, lockfile-pinned production asset graph.
- Promoting `prototypes/tiptap-proposal-lab` or `prototypes/fixed-workspace-shell` into `apps/web` was rejected because those trees are Prototype Evidence Assets, not the Protected Web Client.
- Converting existing `.mjs` modules to TypeScript in the first delivery was rejected so the first Issue stays a toolchain slice inside the repository review limits.
- Rewriting every Chrome module harness into a Vite production entry was rejected because that is a test-platform rewrite, not a toolchain slice.
- Leaving `make verify` on raw `src/*.mjs` page loads was rejected because the new asset graph would then sit outside the evidence path.
- Adopting Vite 8 in this slice was rejected so the first delivery does not take a second major bundler line at the same time as the React host.

## Consequences

- Production hosting of Vite bytes, including same-Origin serving by `storyos-server`, remains a later slice.
- This decision does not change `web_client_contract_revision` from a named contract identity into a Vite content digest. A digest-bound asset identity remains a protocol-owner question.
- Full Stage 1 journey evidence remains `#112`. The toolchain slice proves the Vite-built page boots the existing Stage 1 surface; it does not replace the mandatory evidence bundle.
