import { defineBrowserCommand } from "@vitest/browser-playwright";
import type { BrowserCommandContext } from "vitest/node";

import {
  parseClientSessionCookieRequest,
  parseClipboardPermissionRequest,
  parseImeCompositionRequest,
  parseTrustedInputRequest,
  storyOSBrowserCommandNames,
} from "./browser-command-contract";

const CLIENT_SESSION_COOKIE = "storyos_session";

async function focusedApplicationFrame(context: BrowserCommandContext) {
  const testFrame = await context.frame();
  if (testFrame !== context.page.mainFrame()) {
    for (const frame of [testFrame, ...testFrame.childFrames()]) {
      if (await frame.locator(":focus").count() > 0) return frame;
    }
  }
  throw new Error("the privileged input target must be inside the application frame");
}

export const storyOSBrowserCommands = {
  [storyOSBrowserCommandNames.imeComposition]: defineBrowserCommand<[request: unknown]>(
    async (context, value) => {
      const request = parseImeCompositionRequest(value);
      await focusedApplicationFrame(context);
      const session = await context.context.newCDPSession(context.page);
      try {
        await session.send("Input.imeSetComposition", request);
      } finally {
        await session.detach();
      }
      return { kind: "ime_composition_applied" } as const;
    },
  ),
  [storyOSBrowserCommandNames.trustedInput]: defineBrowserCommand<[request: unknown]>(
    async (context, value) => {
      const request = parseTrustedInputRequest(value);
      await focusedApplicationFrame(context);
      if (request.operation === "insert_text") {
        await context.page.keyboard.insertText(request.text);
      } else {
        const key = {
          backspace: "Backspace",
          cut: process.platform === "darwin" ? "Meta+X" : "Control+X",
          delete: "Delete",
          paste: process.platform === "darwin" ? "Meta+V" : "Control+V",
        }[request.operation];
        await context.page.keyboard.press(key);
      }
      return { kind: "trusted_input_applied" } as const;
    },
  ),
  [storyOSBrowserCommandNames.clipboardPermission]: defineBrowserCommand<[request: unknown]>(
    async (context, value) => {
      const request = parseClipboardPermissionRequest(value);
      if (request.action === "grant") {
        const origin = new URL(context.page.url()).origin;
        await context.context.grantPermissions(
          ["clipboard-read", "clipboard-write"],
          { origin },
        );
        return { kind: "clipboard_permission_updated" } as const;
      }
      await context.context.clearPermissions();
      return { kind: "clipboard_permission_updated" } as const;
    },
  ),
  [storyOSBrowserCommandNames.clientSessionCookie]: defineBrowserCommand<[request: unknown]>(
    async (context, value) => {
      const request = parseClientSessionCookieRequest(value);
      if (request.action === "clear") {
        await context.context.clearCookies({ name: CLIENT_SESSION_COOKIE });
        return { kind: "client_session_cookie_updated" } as const;
      }
      const origin = new URL(context.page.url()).origin;
      await context.context.addCookies([{
        httpOnly: true,
        name: CLIENT_SESSION_COOKIE,
        sameSite: "Strict",
        secure: origin.startsWith("https:"),
        url: `${origin}/`,
        value: request.value,
      }]);
      return { kind: "client_session_cookie_updated" } as const;
    },
  ),
};
