const STORAGE_VERSION = 2;
const STORAGE_KEY = "storyos-prototype-tiptap-proposal-v2";

export function loadPrototypeState() {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    if (!saved) return null;

    const snapshot = JSON.parse(saved);
    const valid =
      snapshot?.version === STORAGE_VERSION &&
      snapshot.document?.type === "doc" &&
      snapshot.proposal?.id &&
      Array.isArray(snapshot.messages);
    return valid ? snapshot : null;
  } catch {
    return null;
  }
}

export function savePrototypeState({ document, proposal, messages }) {
  window.localStorage.setItem(
    STORAGE_KEY,
    JSON.stringify({
      version: STORAGE_VERSION,
      marker: "PROTOTYPE — safe to wipe",
      document,
      proposal,
      messages,
    }),
  );
}

export function clearPrototypeState() {
  window.localStorage.removeItem(STORAGE_KEY);
}
