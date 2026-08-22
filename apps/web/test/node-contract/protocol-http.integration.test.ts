import assert from "node:assert/strict";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { bootProtectedWebClient } from "../../src/boot.ts";
import {
  startStoryOSServer,
  stopStoryOSServer,
} from "../support/node-integration.ts";

const repositoryRoot = fileURLToPath(new URL("../../../..", import.meta.url));
const serverBinary = join(repositoryRoot, "target", "debug", process.platform === "win32" ? "storyos-server.exe" : "storyos-server");

test("the generated client boots protected Web state over the real Server HTTP boundary", async () => {
  const { baseUrl, server } = await startStoryOSServer({ repositoryRoot, serverBinary });
  try {
    assert.deepEqual(await bootProtectedWebClient({ baseUrl }), {
      kind: "protected-ready", profile: RELEASE_1_PROTOCOL_PROFILE,
    });
  } finally {
    await stopStoryOSServer(server);
  }
});
