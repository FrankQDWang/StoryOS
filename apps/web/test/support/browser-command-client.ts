import { commands } from "vitest/browser";

import {
  type ClientSessionCookieRequest,
  type ClientSessionCookieResult,
  type ClipboardPermissionRequest,
  type ClipboardPermissionResult,
  type ImeCompositionRequest,
  type ImeCompositionResult,
  type ProductionHostRequest,
  type ProductionHostResult,
  type TrustedInputRequest,
  type TrustedInputResult,
  parseClientSessionCookieResult,
  parseClipboardPermissionResult,
  parseImeCompositionResult,
  parseProductionHostResult,
  parseTrustedInputResult,
  storyOSBrowserCommandNames,
} from "./browser-command-contract";

async function invokeStoryOSCommand(name: string, request: unknown): Promise<unknown> {
  const command: unknown = Reflect.get(commands, name);
  if (typeof command !== "function") {
    throw new Error(`StoryOS Browser Command ${name} is unavailable`);
  }
  const result: unknown = await Reflect.apply(command, commands, [request]);
  return result;
}

export async function applyImeComposition(
  request: ImeCompositionRequest,
): Promise<ImeCompositionResult> {
  return parseImeCompositionResult(
    await invokeStoryOSCommand(storyOSBrowserCommandNames.imeComposition, request),
  );
}

export async function applyTrustedInput(
  request: TrustedInputRequest,
): Promise<TrustedInputResult> {
  return parseTrustedInputResult(
    await invokeStoryOSCommand(storyOSBrowserCommandNames.trustedInput, request),
  );
}

export async function updateClipboardPermission(
  request: ClipboardPermissionRequest,
): Promise<ClipboardPermissionResult> {
  return parseClipboardPermissionResult(
    await invokeStoryOSCommand(storyOSBrowserCommandNames.clipboardPermission, request),
  );
}

export async function updateClientSessionCookie(
  request: ClientSessionCookieRequest,
): Promise<ClientSessionCookieResult> {
  return parseClientSessionCookieResult(
    await invokeStoryOSCommand(storyOSBrowserCommandNames.clientSessionCookie, request),
  );
}

export async function verifyProductionHost(
  request: ProductionHostRequest,
): Promise<ProductionHostResult> {
  return parseProductionHostResult(
    await invokeStoryOSCommand(storyOSBrowserCommandNames.productionHost, request),
  );
}
