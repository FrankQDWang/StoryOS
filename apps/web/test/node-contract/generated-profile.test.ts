import { readFile } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import type { Release1ProtocolProfile } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

describe("the generated Release 1 TypeScript pair", () => {
  it("exposes the exact frozen runtime profile through its generated declaration", async () => {
    const goldenBytes = await readFile(new URL(
      "../../../../generated/golden-wire/storyos-public-release-1/get-protocol-profile.json",
      import.meta.url,
    ));
    const goldenProfile: unknown = JSON.parse(goldenBytes.toString("utf8"));
    const typedProfile: Readonly<Release1ProtocolProfile> = RELEASE_1_PROTOCOL_PROFILE;

    expect(typedProfile).toEqual(goldenProfile);
    expect(Object.isFrozen(typedProfile)).toBe(true);
  });
});
