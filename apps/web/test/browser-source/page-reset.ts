import { beforeAll } from "vitest";

import { deleteJournal } from "./scenario.ts";

/**
 * Clears the shared page state before each browser-source test file.
 *
 * All browser-source files run in one browser page. Each file gets a new frame, but the
 * frames have one origin, so they share Web Storage and IndexedDB. The product keeps
 * state there, for example the active Editor Session key and the Local Edit Journal. A
 * file that leaves this state can change the path of a later file, and the default file
 * order changes between runs. This reset is a safety net: each test still sets and
 * removes the state that it uses.
 */
beforeAll(async () => {
  sessionStorage.clear();
  localStorage.clear();
  for (const { name } of await indexedDB.databases()) {
    if (name !== undefined) await deleteJournal(name);
  }
});
