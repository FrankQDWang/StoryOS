import { afterEach, expect, it } from "vitest";

import {
  activityStream,
  createAgentRun,
  createProjectCommandChallenge,
  digestCreateAgentRun,
  digestUpdateProjectAssistance,
  getChapter,
  getManuscriptTree,
  updateProjectAssistance,
} from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import {
  applyImeComposition,
  applyTrustedInput,
  updateClientSessionCookie,
  updateClipboardPermission,
} from "../support/browser-command-client.ts";
import {
  focusManuscriptEnd,
  manuscriptBody,
  manuscriptEditor,
  MANUSCRIPT_EDITOR_SELECTOR,
} from "../support/manuscript-surface.ts";

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

let applicationFrame: HTMLIFrameElement | undefined;

function nextFrameLoad(frame: HTMLIFrameElement): Promise<void> {
  return new Promise((resolve, reject) => {
    const timeout = window.setTimeout(() => {
      reject(new Error("the exact-dist input page did not load"));
    }, 10_000);
    frame.addEventListener("load", () => {
      window.clearTimeout(timeout);
      resolve();
    }, { once: true });
  });
}

async function destroyApplicationFrame(frame: HTMLIFrameElement): Promise<void> {
  if (!frame.isConnected) return;
  const unloaded = nextFrameLoad(frame);
  frame.src = "about:blank";
  await unloaded;
  frame.remove();
}

afterEach(async () => {
  if (applicationFrame !== undefined) await destroyApplicationFrame(applicationFrame);
  applicationFrame = undefined;
  await updateClientSessionCookie({ action: "clear" });
  await updateClipboardPermission({ action: "clear" });
  document.body.replaceChildren();
});

function applicationWindow(frame: HTMLIFrameElement): Window & typeof globalThis {
  const result = frame.contentWindow;
  if (result === null) throw new Error("the production page realm is unavailable");
  return result as Window & typeof globalThis;
}

function appRoot(frame: HTMLIFrameElement): Element {
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the production page root is missing");
  }
  return root;
}

async function waitSaved(root: Element, previousRevisionId?: string): Promise<void> {
  await expect.poll(() => {
    const node = root.querySelector("[data-save-state]");
    const revision = node?.getAttribute("data-authoritative-revision-id") ?? "";
    const failure = node?.getAttribute("data-editor-failure") ?? "";
    const save = node?.getAttribute("data-save-state");
    const unsettled = node?.getAttribute("data-unsettled-intent-count");
    if (save === "saved"
      && unsettled === "0"
      && failure === ""
      && (previousRevisionId === undefined || revision !== previousRevisionId)) {
      return { ok: true as const, failure: "" };
    }
    return { ok: false as const, save, unsettled, failure, revision };
  }, { timeout: 10_000 }).toEqual({ ok: true, failure: "" });
}

function selectAll(editor: HTMLElement, realm: Window & typeof globalThis): void {
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.selectNodeContents(editor);
  selection.removeAllRanges();
  selection.addRange(range);
}

async function openChapterEditor(): Promise<{
  frame: HTMLIFrameElement;
  projectId: string;
  chapterId: string;
}> {
  const frame = document.createElement("iframe");
  applicationFrame = frame;
  frame.title = "StoryOS exact-dist Chinese and English input";
  const loaded = nextFrameLoad(frame);
  frame.src = "/";
  document.body.append(frame);
  await loaded;
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app input[name="title"]')?.tagName
  ).toBe("INPUT");
  const title = frame.contentDocument?.querySelector<HTMLInputElement>('#app input[name="title"]');
  const form = title?.form;
  if (title === null || title === undefined || form === null || form === undefined) {
    throw new Error("the protected-ready form is missing");
  }
  title.value = "Input Novel";
  form.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("empty-project-ready");
  const volumeTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-volume] input[name="volume-title"]',
  );
  const volumeForm = volumeTitle?.form;
  if (volumeTitle === null || volumeTitle === undefined
    || volumeForm === null || volumeForm === undefined) {
    throw new Error("the Create Volume form is missing");
  }
  volumeTitle.value = "Volume A";
  volumeForm.requestSubmit();
  await expect.poll(() =>
    frame.contentDocument?.querySelector('#app form[data-create-chapter]') !== null
  ).toBe(true);
  const chapterTitle = frame.contentDocument?.querySelector<HTMLInputElement>(
    '#app form[data-create-chapter] input[name="chapter-title"]',
  );
  const chapterForm = chapterTitle?.form;
  if (chapterTitle === null || chapterTitle === undefined
    || chapterForm === null || chapterForm === undefined) {
    throw new Error("the Create Chapter form is missing");
  }
  chapterTitle.value = "Chapter A";
  chapterForm.requestSubmit();
  await expect.poll(() => {
    const root = frame.contentDocument?.querySelector("#app");
    const projectId = root?.querySelector("form[data-rename]")?.getAttribute("data-rename");
    const chapterId = root?.querySelector(
      'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
    )?.getAttribute("data-chapter-id");
    return root?.getAttribute("data-boot-state") === "project-ready"
      && root.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null
      && typeof projectId === "string"
      && UUID.test(projectId)
      && typeof chapterId === "string"
      && UUID.test(chapterId);
  }, { timeout: 10_000 }).toBe(true);
  const root = appRoot(frame);
  const projectId = root.querySelector("form[data-rename]")?.getAttribute("data-rename");
  const chapterId = root.querySelector(
    'nav[aria-label="稿件目录"] button[data-chapter-id][aria-current="true"]',
  )?.getAttribute("data-chapter-id");
  if (projectId === null || projectId === undefined
    || chapterId === null || chapterId === undefined) {
    throw new Error("the Project or Chapter identity is missing");
  }
  return { frame, projectId, chapterId };
}

async function readRevision(frame: HTMLIFrameElement, projectId: string, chapterId: string) {
  const childWindow = applicationWindow(frame);
  const chapter = await getChapter({
    baseUrl: childWindow.location.origin,
    projectId,
    chapterId,
    fetchImpl: childWindow.fetch.bind(childWindow),
  });
  return chapter.chapter.current_revision;
}

it("consumes real assistance and Run Activity before the author continues writing", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor();
  const childWindow = applicationWindow(frame);
  const baseUrl = childWindow.location.origin;
  const fetchImpl = childWindow.fetch.bind(childWindow);
  const key = (suffix: string) => `018f0000-0000-7001-8000-00000000f8${suffix}`;
  const binding = {
    client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
    security_policy_revision: "storyos.web-security-policy.release-1.v1",
  };
  const tree = await getManuscriptTree({ baseUrl, projectId, fetchImpl });
  const assistance = {
    command_schema: "storyos.command.update-project-assistance.request.v1" as const,
    update_project_assistance_input: {
      availability: "available" as const,
      expected_assistance_revision: "0",
      ...binding,
      correlation_id: key("01"),
    },
  };
  const assistanceChallenge = await createProjectCommandChallenge({
    baseUrl, projectId, fetchImpl,
    request: {
      method: "PUT",
      route_template: "/api/v1/projects/{project_id}/assistance",
      command_schema: assistance.command_schema,
      canonical_command_digest: await digestUpdateProjectAssistance(assistance),
      idempotency_key: key("02"),
    },
  });
  await updateProjectAssistance({
    baseUrl, projectId, fetchImpl, idempotencyKey: key("02"),
    antiForgery: assistanceChallenge.nonce, request: assistance,
  });
  const run = {
    command_schema: "storyos.command.create-agent-run.request.v2" as const,
    create_agent_run_input: {
      conversation: { kind: "new" as const },
      author_message: { text: "Help with this passage." },
      working_target: { kind: "current_chapter" as const, chapter_id: chapterId },
      instruction: { kind: "absent" as const },
      cause: { kind: "author_request" as const },
      ...binding,
      correlation_id: key("03"),
    },
  };
  const runChallenge = await createProjectCommandChallenge({
    baseUrl, projectId, fetchImpl,
    request: {
      method: "POST",
      route_template: "/api/v1/projects/{project_id}/agent-runs",
      command_schema: run.command_schema,
      canonical_command_digest: await digestCreateAgentRun(run),
      idempotency_key: key("04"),
    },
  });
  const admitted = await createAgentRun({
    baseUrl, projectId, fetchImpl, idempotencyKey: key("04"),
    antiForgery: runChallenge.nonce, request: run,
  });
  expect(admitted.effect.kind).toBe("admitted");
  const root = appRoot(frame);
  const editor = manuscriptEditor(root, childWindow);
  const before = await readRevision(frame, projectId, chapterId);
  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Start. " });
  await waitSaved(root, before.revision_id);
  const firstSavedRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  const stream = await activityStream({
    baseUrl, projectId, snapshotId: tree.snapshot.snapshot_id,
    protocolRelease: "storyos.public.release.1", fetchImpl,
  });
  const frames = stream.split("\n\n").filter(Boolean).map((block: string) => {
    const lines = block.split("\n");
    return {
      id: lines.find((line: string) => line.startsWith("id: "))?.slice(4) ?? "",
      kind: JSON.parse(lines.find((line: string) => line.startsWith("data: "))!.slice(6))
        .event_kind as string,
    };
  });
  expect(frames.map((event) => event.kind)).toEqual([
    "project_assistance_updated", "agent_run_created", "authoritative_author_edit_applied",
  ]);
  const lastEventId = frames.at(-1)?.id;
  expect(lastEventId).toMatch(/^v1\./);
  await expect.poll(() => root.querySelector("[data-activity-last-event-id]")
    ?.getAttribute("data-activity-last-event-id"), { timeout: 10_000 }).toBe(lastEventId);
  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Still writing." });
  await waitSaved(root, firstSavedRevisionId);
  expect((await readRevision(frame, projectId, chapterId)).body).toBe("Start. Still writing.");
});

it("settles IME, clipboard, drop, and contiguous Block replacement without reusing identity", async () => {
  const { frame, projectId, chapterId } = await openChapterEditor();
  const root = appRoot(frame);
  const childWindow = applicationWindow(frame);
  const editor = manuscriptEditor(root, childWindow);
  const before = await readRevision(frame, projectId, chapterId);
  const leftId = before.blocks[0]?.manuscript_block_id;
  if (before.blocks.length !== 1 || leftId === undefined) {
    throw new Error("the starting Block is missing");
  }

  editor.focus();
  focusManuscriptEnd(editor, childWindow);
  await applyTrustedInput({ operation: "insert_text", text: "Hello" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Hello");
  await waitSaved(root, before.revision_id);
  const helloRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  selectAll(editor, childWindow);
  expect(root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
    .toBe("0");
  expect(manuscriptBody(editor)).toBe("Hello");

  await updateClipboardPermission({ action: "grant" });
  await childWindow.navigator.clipboard.writeText("Alpha\nBeta");
  selectAll(editor, childWindow);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Alpha\nBeta");
  await waitSaved(root, helloRevisionId);
  const pastedOnce = await readRevision(frame, projectId, chapterId);
  const firstMintedId = pastedOnce.blocks[1]?.manuscript_block_id;
  expect(pastedOnce.blocks.map((block) => block.text)).toEqual(["Alpha", "Beta"]);
  expect(pastedOnce.blocks[0]?.manuscript_block_id).toBe(leftId);
  expect(firstMintedId).toMatch(UUID);

  await childWindow.navigator.clipboard.writeText("Gamma\nDelta");
  selectAll(editor, childWindow);
  await applyTrustedInput({ operation: "paste" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Gamma\nDelta");
  await waitSaved(root, pastedOnce.revision_id);
  const pasted = await readRevision(frame, projectId, chapterId);
  const rightId = pasted.blocks[1]?.manuscript_block_id;
  expect(pasted.blocks.map((block) => block.text)).toEqual(["Gamma", "Delta"]);
  expect(pasted.blocks[0]?.manuscript_block_id).toBe(leftId);
  expect(rightId).toMatch(UUID);
  expect(rightId).not.toBe(firstMintedId);

  const paragraphs = [...editor.querySelectorAll("p")];
  const startText = paragraphs[0]?.firstChild;
  const endText = paragraphs[1]?.firstChild;
  if (!(startText instanceof childWindow.Text) || !(endText instanceof childWindow.Text)) {
    throw new Error("the manuscript text nodes are missing");
  }
  const cutRange = editor.ownerDocument.createRange();
  cutRange.setStart(startText, 1);
  cutRange.setEnd(endText, 1);
  const selection = childWindow.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  editor.focus();
  selection.removeAllRanges();
  selection.addRange(cutRange);
  await applyTrustedInput({ operation: "cut" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Gelta");
  await waitSaved(root, pasted.revision_id);
  const afterCutRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  focusManuscriptEnd(editor, childWindow);
  const imeOffset = () => manuscriptBody(editor).length;
  await applyImeComposition({
    text: "取消",
    replacementStart: imeOffset(),
    replacementEnd: imeOffset(),
    selectionStart: 2,
    selectionEnd: 2,
  });
  await applyImeComposition({
    text: "",
    replacementStart: imeOffset(),
    replacementEnd: imeOffset(),
    selectionStart: 0,
    selectionEnd: 0,
  });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Gelta");
  expect(root.querySelector("[data-save-state]")?.getAttribute("data-unsettled-intent-count"))
    .toBe("0");

  await applyImeComposition({
    text: "中文",
    replacementStart: imeOffset(),
    replacementEnd: imeOffset(),
    selectionStart: 2,
    selectionEnd: 2,
  });
  await applyTrustedInput({ operation: "insert_text", text: "中文" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Gelta中文");
  await waitSaved(root, afterCutRevisionId);
  const afterFirstIme = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";
  await applyImeComposition({
    text: "再",
    replacementStart: imeOffset(),
    replacementEnd: imeOffset(),
    selectionStart: 1,
    selectionEnd: 1,
  });
  await applyTrustedInput({ operation: "insert_text", text: "再" });
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toBe("Gelta中文再");
  await waitSaved(root, afterFirstIme);
  const afterImeRevisionId = root.querySelector("[data-save-state]")
    ?.getAttribute("data-authoritative-revision-id") ?? "";

  const transfer = new childWindow.DataTransfer();
  transfer.setData("text/plain", "Drop");
  const rect = editor.getBoundingClientRect();
  const dropEvent = new childWindow.DragEvent("drop", {
    bubbles: true,
    cancelable: true,
    clientX: rect.left + rect.width / 2,
    clientY: rect.top + rect.height / 2,
  });
  Object.defineProperty(dropEvent, "dataTransfer", { value: transfer });
  editor.dispatchEvent(dropEvent);
  await expect.poll(() => manuscriptBody(editor), { timeout: 10_000 }).toMatch(/Drop/);
  await waitSaved(root, afterImeRevisionId);

  await destroyApplicationFrame(frame);
  const reopened = document.createElement("iframe");
  applicationFrame = reopened;
  reopened.title = "StoryOS exact-dist input reload";
  const reopenedLoaded = nextFrameLoad(reopened);
  reopened.src = `/projects/${projectId}`;
  document.body.append(reopened);
  await reopenedLoaded;
  await expect.poll(() => {
    const nextRoot = reopened.contentDocument?.querySelector("#app");
    return nextRoot?.getAttribute("data-boot-state") === "project-ready"
      && nextRoot.querySelector(MANUSCRIPT_EDITOR_SELECTOR) !== null;
  }, { timeout: 10_000 }).toBe(true);
  const reopenedRevision = await readRevision(reopened, projectId, chapterId);
  expect(reopenedRevision.blocks[0]?.manuscript_block_id).toBe(leftId);
  expect(reopenedRevision.blocks.some((block) => block.manuscript_block_id === firstMintedId))
    .toBe(false);
  expect(reopenedRevision.blocks.some((block) => block.manuscript_block_id === rightId))
    .toBe(false);
  expect(reopenedRevision.body).toContain("Gelta");
  expect(reopenedRevision.body).toContain("中文");
  expect(reopenedRevision.body).toContain("再");
  expect(reopenedRevision.body).toContain("Drop");
});
