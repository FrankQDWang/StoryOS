export const TRACE_SCHEMA = "storyos.issue69.trace.v1";
export const CORE_SCHEMA = "storyos.issue69.fake-core.v1";
export const JOURNAL_SCHEMA = "storyos.issue69.local-edit-journal.v1";
export const EDITOR_CONTRACT = "storyos.web-editor.session-semantics.v1";

export const PROJECT_ID = "project-issue-69";
export const DEFAULT_CHAPTER_ID = "chapter-alpha";
export const DEFAULT_TARGET_ID = "manuscript:chapter-alpha";

export const OUTCOMES = Object.freeze([
  "authoritative",
  "proposal",
  "refused",
  "conflicted",
  "no_effect",
]);

export const STRATEGIES = Object.freeze([
  "transaction",
  "semantic-intent",
  "bounded-idle",
  "fixed-window",
]);

export const HARD_BOUNDARY_KINDS = new Set([
  "composition",
  "paste",
  "cut",
  "drop",
  "structural",
  "explicit_command",
]);

export function stableJson(value) {
  if (value === null || typeof value !== "object") {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map(stableJson).join(",")}]`;
  }
  return `{${Object.keys(value)
    .sort()
    .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
    .join(",")}}`;
}

export async function sha256(value) {
  const bytes = new TextEncoder().encode(
    typeof value === "string" ? value : stableJson(value),
  );
  if (!globalThis.crypto?.subtle) {
    throw new Error("web_crypto_sha256_unavailable");
  }
  const digest = await globalThis.crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export function bindingFingerprint(binding) {
  return [
    binding.project_id,
    binding.chapter_id,
    binding.target_id,
    binding.ownership,
    binding.expected_head,
    binding.writer_generation,
    binding.admission_id,
    binding.admission_expires_at,
    binding.editor_contract,
    binding.undo_group,
  ].join("\u001f");
}

export function utf8Bytes(value) {
  return new TextEncoder().encode(value).byteLength;
}
