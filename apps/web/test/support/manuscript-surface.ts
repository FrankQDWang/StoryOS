export const MANUSCRIPT_EDITOR_SELECTOR = "[data-manuscript-editor]";

export function manuscriptEditor(
  root: ParentNode,
  realm: Window & typeof globalThis,
): HTMLElement {
  const node = root.querySelector(MANUSCRIPT_EDITOR_SELECTOR);
  if (!(node instanceof realm.HTMLElement)) {
    throw new Error("the manuscript editor is missing");
  }
  return node;
}

export function manuscriptBody(editor: Element): string {
  return editor.getAttribute("data-manuscript-body") ?? "";
}

export function manuscriptIsEditable(editor: Element): boolean {
  return editor.getAttribute("contenteditable") === "true";
}

export function focusManuscriptEnd(
  editor: HTMLElement,
  realm: Window & typeof globalThis,
): void {
  editor.focus();
  const selection = realm.getSelection();
  if (selection === null) throw new Error("the manuscript selection is unavailable");
  const range = editor.ownerDocument.createRange();
  range.selectNodeContents(editor);
  range.collapse(false);
  selection.removeAllRanges();
  selection.addRange(range);
}
