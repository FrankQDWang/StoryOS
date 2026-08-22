import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "vitest";

import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import type { Release1ProtocolProfile } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { validateProtocolProfile } from "../../src/boot.ts";
import type { ProtocolBlockedState, StateDifference } from "../../src/editor-types.ts";

const fixture = <Value>(name: string): Value => JSON.parse(readFileSync(
  new URL(`../../../../generated/golden-wire/storyos-public-release-1/${name}`, import.meta.url), "utf8",
));
const compatible = (): Release1ProtocolProfile => fixture("get-protocol-profile.json");
const messages: Record<ProtocolBlockedState["code"], string> = {
  protocol_identity_missing: "服务器缺少 Release 1 兼容身份，受保护状态不会开放。",
  protocol_upgrade_required: "Web 客户端与服务器的 Release 1 契约不一致，请更新后重试。",
  protocol_capabilities_incompatible: "服务器能力与此 Web 客户端不兼容，受保护状态不会开放。",
  protocol_unavailable: "无法读取服务器协议身份，StoryOS 已停止进入受保护状态。",
};
const blocked = (
  code: ProtocolBlockedState["code"],
  details: StateDifference[],
): ProtocolBlockedState => ({ kind: "protocol-blocked", code,
  heading: "StoryOS 无法安全启动", message: messages[code], details });

test("a compatible Release 1 profile exposes protected application state", () => {
  const profile = compatible();
  assert.deepEqual(validateProtocolProfile(profile), { kind: "protected-ready", profile });
});

type BlockedCase = readonly [string, () => readonly [unknown, ProtocolBlockedState]];
const blockedCases: BlockedCase[] = [
  ["a missing compatibility identity fails closed with an inspectable message", () => [
    fixture<unknown>("get-protocol-profile.invalid.json"),
    blocked("protocol_identity_missing", [
      { path: "profile.release_identity", expected: "object", received: undefined },
    ]),
  ]],
  ["a stale generated-client identity requires an upgrade before protected state", () => [
    fixture<Release1ProtocolProfile>("get-protocol-profile.boundary.json"),
    blocked("protocol_upgrade_required", [{
      path: "profile.release_identity.generated_client_revision",
      expected: RELEASE_1_PROTOCOL_PROFILE.release_identity.generated_client_revision,
      received: "storyos.typescript-client.release-0.v1",
    }]),
  ]],
  ["a missing required capability keeps the client outside protected state", () => {
    const profile = compatible();
    profile.required_capabilities = profile.required_capabilities.filter(
      (capability) => capability !== "direct_author_edit",
    );
    return [profile, blocked("protocol_capabilities_incompatible", [{
      path: "profile.required_capabilities",
      expected: RELEASE_1_PROTOCOL_PROFILE.required_capabilities,
      received: profile.required_capabilities,
      missing: ["direct_author_edit"],
      unexpected: [],
    }])];
  }],
  ...([
    ["an unknown field cannot extend the closed compatibility identity", "release_identity", "unrecognized_revision"],
    ["an unknown top-level field cannot extend the closed protocol profile", null, "unrecognized_profile"],
  ] as const).map(([name, target, field]): BlockedCase => [name, () => {
    const profile = compatible();
    const extensionTarget: object = target === "release_identity"
      ? profile.release_identity
      : profile;
    Reflect.set(extensionTarget, field, "storyos.unknown.v1");
    const path = `profile.${target ? `${target}.` : ""}${field}`;
    return [profile, blocked("protocol_upgrade_required", [
      { path, expected: "absent", received: "storyos.unknown.v1" },
    ])];
  }]),
];

for (const [name, arrange] of blockedCases) {
  test(name, () => {
    const [profile, expected] = arrange();
    assert.deepEqual(validateProtocolProfile(profile), expected);
  });
}
