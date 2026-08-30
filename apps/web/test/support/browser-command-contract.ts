export const storyOSBrowserCommandNames = {
  clipboardPermission: "storyosClipboardPermission",
  clientSessionCookie: "storyosClientSessionCookie",
  imeComposition: "storyosImeComposition",
  trustedInput: "storyosTrustedInput",
  productionHost: "storyosProductionHost",
} as const;

export interface ImeCompositionRequest {
  readonly replacementEnd: number;
  readonly replacementStart: number;
  readonly selectionEnd: number;
  readonly selectionStart: number;
  readonly text: string;
}

export type TrustedInputRequest =
  | Readonly<{ operation: "backspace" | "cut" | "delete" | "enter" | "paste" }>
  | Readonly<{ operation: "insert_text"; text: string }>;

export type ClipboardPermissionRequest = Readonly<{ action: "clear" | "grant" }>;

export type ClientSessionCookieRequest =
  | Readonly<{ action: "clear" }>
  | Readonly<{ action: "set"; value: string }>;

export type ImeCompositionResult = Readonly<{ kind: "ime_composition_applied" }>;
export type TrustedInputResult = Readonly<{ kind: "trusted_input_applied" }>;
export type ClipboardPermissionResult = Readonly<{ kind: "clipboard_permission_updated" }>;
export type ClientSessionCookieResult = Readonly<{ kind: "client_session_cookie_updated" }>;
export type ProductionHostRequest = Readonly<{ scenario: "open_edit_reload_takeover" }>;
export type ProductionHostResult = Readonly<{ kind: "production_host_verified" }>;

function exactObject(value: unknown, keys: readonly string[], label: string): object {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object`);
  }
  const actualKeys = Object.keys(value).sort();
  const expectedKeys = [...keys].sort();
  if (
    actualKeys.length !== expectedKeys.length
    || actualKeys.some((key, index) => key !== expectedKeys[index])
  ) {
    throw new TypeError(`${label} has unsupported fields`);
  }
  return value;
}

function property(value: object, key: string): unknown {
  return Reflect.get(value, key);
}

function boundedText(value: unknown, label: string): string {
  if (typeof value !== "string" || value.length > 1_048_576) {
    throw new TypeError(`${label} must be a bounded string`);
  }
  return value;
}

function boundedIndex(value: unknown, maximum: number, label: string): number {
  if (!Number.isSafeInteger(value) || typeof value !== "number" || value < 0 || value > maximum) {
    throw new TypeError(`${label} must be a valid text index`);
  }
  return value;
}

export function parseImeCompositionRequest(value: unknown): ImeCompositionRequest {
  const request = exactObject(value, [
    "replacementEnd",
    "replacementStart",
    "selectionEnd",
    "selectionStart",
    "text",
  ], "IME composition request");
  const text = boundedText(property(request, "text"), "IME composition text");
  const replacementStart = boundedIndex(
    property(request, "replacementStart"),
    1_048_576,
    "IME replacement start",
  );
  const replacementEnd = boundedIndex(
    property(request, "replacementEnd"),
    1_048_576,
    "IME replacement end",
  );
  const selectionStart = boundedIndex(
    property(request, "selectionStart"),
    text.length,
    "IME selection start",
  );
  const selectionEnd = boundedIndex(
    property(request, "selectionEnd"),
    text.length,
    "IME selection end",
  );
  if (replacementStart > replacementEnd || selectionStart > selectionEnd) {
    throw new TypeError("IME composition ranges must be ordered");
  }
  return { replacementEnd, replacementStart, selectionEnd, selectionStart, text };
}

export function parseTrustedInputRequest(value: unknown): TrustedInputRequest {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("trusted input request must be an object");
  }
  const operation = property(value, "operation");
  if (operation === "insert_text") {
    const request = exactObject(value, ["operation", "text"], "trusted input request");
    return {
      operation,
      text: boundedText(property(request, "text"), "trusted input text"),
    };
  }
  if (operation === "backspace" || operation === "cut" || operation === "delete"
    || operation === "enter" || operation === "paste") {
    exactObject(value, ["operation"], "trusted input request");
    return { operation };
  }
  throw new TypeError("trusted input operation is unsupported");
}

export function parseClipboardPermissionRequest(value: unknown): ClipboardPermissionRequest {
  const request = exactObject(value, ["action"], "clipboard permission request");
  const action = property(request, "action");
  if (action !== "clear" && action !== "grant") {
    throw new TypeError("clipboard permission action is unsupported");
  }
  return { action };
}

export function parseClientSessionCookieRequest(value: unknown): ClientSessionCookieRequest {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new TypeError("Client Session cookie request must be an object");
  }
  const action = property(value, "action");
  if (action === "clear") {
    exactObject(value, ["action"], "Client Session cookie request");
    return { action };
  }
  if (action === "set") {
    const request = exactObject(value, ["action", "value"], "Client Session cookie request");
    const sessionValue = boundedText(property(request, "value"), "Client Session cookie value");
    if (sessionValue.length === 0 || /[\u0000-\u0020;,\u007f]/u.test(sessionValue)) {
      throw new TypeError("Client Session cookie value is invalid");
    }
    return { action, value: sessionValue };
  }
  throw new TypeError("Client Session cookie action is unsupported");
}

function parseResult<Kind extends string>(
  value: unknown,
  kind: Kind,
  label: string,
): Readonly<{ kind: Kind }> {
  const result = exactObject(value, ["kind"], `${label} result`);
  if (property(result, "kind") !== kind) {
    throw new TypeError(`${label} result is invalid`);
  }
  return { kind };
}

export function parseImeCompositionResult(value: unknown): ImeCompositionResult {
  return parseResult(value, "ime_composition_applied", "IME composition");
}

export function parseTrustedInputResult(value: unknown): TrustedInputResult {
  return parseResult(value, "trusted_input_applied", "trusted input");
}

export function parseClipboardPermissionResult(value: unknown): ClipboardPermissionResult {
  return parseResult(value, "clipboard_permission_updated", "clipboard permission");
}

export function parseClientSessionCookieResult(value: unknown): ClientSessionCookieResult {
  return parseResult(value, "client_session_cookie_updated", "Client Session cookie");
}

export function parseProductionHostRequest(value: unknown): ProductionHostRequest {
  const request = exactObject(value, ["scenario"], "production host request");
  if (property(request, "scenario") !== "open_edit_reload_takeover") {
    throw new TypeError("production host scenario is unsupported");
  }
  return { scenario: "open_edit_reload_takeover" };
}

export function parseProductionHostResult(value: unknown): ProductionHostResult {
  return parseResult(value, "production_host_verified", "production host");
}
