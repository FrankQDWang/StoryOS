import { act } from "react";
import { createRoot, type Root } from "react-dom/client";

import { OWNER, PROJECT } from "./scenario.ts";

interface PanelHost {
  readonly host: HTMLDivElement;
  /** The current root on `host`. `reload` replaces it. */
  readonly root: Root;
  /** Unmounts the root and puts a new root on the host, as a page reload does. The session storage stays. */
  reload(): Promise<void>;
  /** Unmounts the root, removes the host, clears the session storage, and puts the earlier act flag back. */
  close(): Promise<void>;
}

/** Stores `request` as the prose request of the test Project, as the panel does after a send. */
export function seedProseRequest(request: object): void {
  sessionStorage.setItem(`prose_request:${OWNER}:${PROJECT}`, JSON.stringify(request));
}

/** Runs `body` with the React act flag, then clears the session storage and puts the earlier flag back. */
export async function inActEnvironment(body: () => Promise<void>): Promise<void> {
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  try {
    await body();
  } finally {
    sessionStorage.clear();
    Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
  }
}

/** Adds a new host to the page with a root on it, and sets the React act flag. */
export function openPanelHost(): PanelHost {
  const host = document.createElement("div");
  document.body.append(host);
  const previousAct = Reflect.get(globalThis, "IS_REACT_ACT_ENVIRONMENT");
  Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: true });
  let root = createRoot(host);
  return {
    host,
    get root() { return root; },
    async reload() {
      await act(async () => root.unmount());
      root = createRoot(host);
    },
    async close() {
      await act(async () => root.unmount());
      host.remove();
      sessionStorage.clear();
      Object.assign(globalThis, { IS_REACT_ACT_ENVIRONMENT: previousAct });
    },
  };
}
