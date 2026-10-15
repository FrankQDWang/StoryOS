import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { expect, it } from "vitest";
import assistanceFixture from "../../../../generated/golden-wire/storyos-public-release-1/get-project-assistance.json";
import type { ChapterTextReference } from "../../src/selection-reference.ts";
import {
  addDraftReferences, copiedReferences, REFERENCE_CLIPBOARD_TYPE, type DraftReference,
} from "../../src/selection-reference-draft.ts";
import { WritingAssistantPanel } from "../../src/writing-assistant-panel.tsx";
import { BLOCK, CHAPTER, OWNER, PROJECT, REVISION, jsonResponse } from "./scenario.ts";

function entry(start: number, end: number): Omit<DraftReference, "key"> {
  const reference: ChapterTextReference = { kind: "chapter_text", chapter_id: CHAPTER,
    base_authoritative_revision_id: REVISION, coordinate_profile: "storyos.editor.utf16-code-unit.v1",
    start: { manuscript_block_id: BLOCK, offset: start }, end: { manuscript_block_id: BLOCK, offset: end } };
  return { reference, label: { chapter: "第 1 章 渡口", part: { kind: "paragraphs", first: 1, last: 1 } }, moved: false };
}

it("adds the references of a paste from the editor, and pastes text when the message cannot hold them", async () => {
  const scope = { owner_user_id: OWNER, project_id: PROJECT };
  const fetchImpl: typeof fetch = async (input) => {
    const path = new URL(input instanceof Request ? input.url : input).pathname;
    if (path.endsWith("/assistance")) return jsonResponse({ ...assistanceFixture, project_scope: scope });
    throw new Error(`Unexpected request ${path}`);
  };
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  /** Pastes the data of an editor copy and returns true when the composer took it as references. */
  const paste = async (values: Omit<DraftReference, "key">[]) => {
    const data = new DataTransfer();
    data.setData("text/plain", "渡口");
    data.setData(REFERENCE_CLIPBOARD_TYPE, copiedReferences(scope, new Set(), values, () => undefined));
    const event = new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true });
    await act(async () => { host.querySelector("textarea")!.dispatchEvent(event); });
    return event.defaultPrevented;
  };
  const labels = () => host.querySelectorAll("[data-selection-reference-label]").length;
  try {
    await act(async () => {
      root.render(createElement(WritingAssistantPanel, {
        collapsed: false,
        context: { scope, chapterId: CHAPTER, canSubmit: true, baseUrl: location.origin, fetchImpl, cryptoImpl: crypto },
        scheduleRefresh: () => () => {},
      }));
    });
    for (let turn = 0; turn < 5; turn += 1) await act(async () => { await Promise.resolve(); });

    expect({ taken: await paste([entry(0, 2), entry(3, 5)]), labels: labels() }).toEqual({ taken: true, labels: 2 });
    addDraftReferences(scope, Array.from({ length: 5 }, (_, index) => entry(10 + index, 11 + index)));
    await act(async () => { await Promise.resolve(); });
    // Seven references and two more are above the limit of eight, so the paste gives its text.
    expect({ taken: await paste([entry(20, 21), entry(22, 23)]), labels: labels() }).toEqual({ taken: false, labels: 7 });
  } finally {
    await act(async () => root.unmount());
    host.remove();
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
});
