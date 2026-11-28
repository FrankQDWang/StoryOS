import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it } from "vitest";
import statisticsFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-statistics.json";
import { ManuscriptStatisticsPanel } from "../../src/manuscript-statistics.tsx";
import { CHAPTER, PROJECT, jsonResponse } from "./scenario.ts";

// The statistics read of each open, each save, and each later tree of an open (#1937).
const STATISTICS = `GET /api/v1/projects/${PROJECT}/manuscript/statistics`;

const cleanups: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const cleanup of cleanups.splice(0)) await cleanup();
});

/** Mounts the panel. The fake serves only the statistics read and records each other request. */
async function mountPanel() {
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const requests: string[] = [];
  const unexpected: string[] = [];
  const fetchImpl: typeof fetch = async (input, init) => {
    const request = `${init?.method ?? "GET"} ${new URL(input instanceof Request ? input.url : input).pathname}`;
    if (request !== STATISTICS) {
      unexpected.push(request);
      throw new Error(`unexpected request: ${request}`);
    }
    requests.push(request);
    return jsonResponse(statisticsFixture);
  };
  cleanups.push(async () => {
    await act(async () => { root.unmount(); });
    host.remove();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  });
  const show = (props: { open: unknown; saveState: string; treeRevision: string | undefined;
    openTree?: boolean; currentChapterId?: string }) => act(async () => {
    root.render(createElement(ManuscriptStatisticsPanel, { projectId: PROJECT, baseUrl: location.origin, fetchImpl,
      currentChapterId: CHAPTER, openTree: true, ...props }));
  });
  return { show, requests, unexpected, host };
}

it("reads the statistics one time for an open and its first tree, and again for a later tree of the open", async () => {
  const panel = await mountPanel();
  const open = {};

  await panel.show({ open, saveState: "saved", treeRevision: undefined });
  await panel.show({ open, saveState: "saved", treeRevision: "3" });
  const first = panel.requests.length;
  await panel.show({ open, saveState: "saved", treeRevision: "4" });

  expect({ first, all: panel.requests.length, unexpected: panel.unexpected })
    .toEqual({ first: 1, all: 2, unexpected: [] });
});

it("reads the statistics after a save when the first tree of the open arrives during the save", async () => {
  const panel = await mountPanel();
  const open = {};

  await panel.show({ open, saveState: "saved", treeRevision: undefined });
  await panel.show({ open, saveState: "saving", treeRevision: undefined });
  await panel.show({ open, saveState: "saving", treeRevision: "3" });
  const during = panel.requests.length;
  await panel.show({ open, saveState: "saved", treeRevision: "3" });

  expect({ during, all: panel.requests.length, unexpected: panel.unexpected })
    .toEqual({ during: 1, all: 2, unexpected: [] });
});

it("reads the statistics again at a new open with the tree of the earlier open", async () => {
  const panel = await mountPanel();

  await panel.show({ open: {}, saveState: "saved", treeRevision: "3" });
  await panel.show({ open: {}, saveState: "saved", treeRevision: "3" });

  expect({ all: panel.requests.length, unexpected: panel.unexpected }).toEqual({ all: 2, unexpected: [] });
});

it("reads the statistics again when the first tree of an open is not the tree read of the open", async () => {
  const panel = await mountPanel();
  const open = {};

  await panel.show({ open, saveState: "saved", treeRevision: undefined });
  // A tree read after a Chapter creation replaced the tree read of the open.
  await panel.show({ open, saveState: "saved", treeRevision: "4", openTree: false });

  expect({ all: panel.requests.length, unexpected: panel.unexpected }).toEqual({ all: 2, unexpected: [] });
});

it("shows no Chapter count when the statistics give the count of another Chapter", async () => {
  const panel = await mountPanel();

  // The fixture gives the count of the Chapter CHAPTER.
  await panel.show({ open: {}, saveState: "saved", treeRevision: undefined,
    currentChapterId: "018f0000-0000-7001-8000-000000000a65" });

  const section = () => panel.host.querySelector("[data-manuscript-statistics]");
  await expect.poll(() => section()?.getAttribute("data-statistics-outcome")).toBe("ready");
  expect({ characters: section()?.getAttribute("data-statistics-chapter-characters"), text: section()?.textContent,
    unexpected: panel.unexpected }).toEqual({ characters: "", text: "本章 — 字", unexpected: [] });
});

it("reads nothing again for a later tree of an open with the revision of the tree read of the open", async () => {
  const panel = await mountPanel();
  const open = {};

  await panel.show({ open, saveState: "saved", treeRevision: undefined });
  await panel.show({ open, saveState: "saved", treeRevision: "3" });
  await panel.show({ open, saveState: "saved", treeRevision: "3", openTree: false });

  expect({ all: panel.requests.length, unexpected: panel.unexpected }).toEqual({ all: 1, unexpected: [] });
});
