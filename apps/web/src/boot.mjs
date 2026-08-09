import { getProtocolProfile } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../generated/typescript/storyos-public-release-1/release-profile.mjs";

const { release_identity: expectedIdentity, required_capabilities: expectedCapabilities, ...expectedProtocol } = RELEASE_1_PROTOCOL_PROFILE;

const blockedMessages = {
  protocol_identity_missing: "服务器缺少 Release 1 兼容身份，受保护状态不会开放。",
  protocol_upgrade_required: "Web 客户端与服务器的 Release 1 契约不一致，请更新后重试。",
  protocol_capabilities_incompatible: "服务器能力与此 Web 客户端不兼容，受保护状态不会开放。",
  protocol_unavailable: "无法读取服务器协议身份，StoryOS 已停止进入受保护状态。",
};
const blocked = (code, details = []) => ({
  kind: "protocol-blocked", code, heading: "StoryOS 无法安全启动", message: blockedMessages[code], details,
});
function closedMismatches(actual, expected, prefix) {
  return [...new Set([...Object.keys(expected), ...Object.keys(actual)])].flatMap((field) => {
    if (Object.hasOwn(expected, field) && actual[field] === expected[field]) return [];
    return [{
      path: `${prefix}.${field}`, expected: Object.hasOwn(expected, field) ? expected[field] : "absent",
      received: actual[field],
    }];
  });
}

export function validateProtocolProfile(profile) {
  const received = profile && typeof profile === "object" && !Array.isArray(profile) ? profile : {};
  if (!received.release_identity || typeof received.release_identity !== "object" || Array.isArray(received.release_identity)) {
    return blocked("protocol_identity_missing", [
      { path: "profile.release_identity", expected: "object", received: received.release_identity },
    ]);
  }

  const { release_identity: identity, required_capabilities: capabilities, ...protocol } = received;
  const differences = [
    ...closedMismatches(protocol, expectedProtocol, "profile"),
    ...closedMismatches(identity, expectedIdentity, "profile.release_identity"),
  ];
  if (differences.length) return blocked("protocol_upgrade_required", differences);

  const actualCapabilities = Array.isArray(capabilities) ? capabilities : [];
  const missingCapabilities = expectedCapabilities.filter((item) => !actualCapabilities.includes(item));
  const unexpectedCapabilities = actualCapabilities.filter((item) => !expectedCapabilities.includes(item));
  if (missingCapabilities.length || unexpectedCapabilities.length || actualCapabilities.length !== expectedCapabilities.length) {
    return blocked("protocol_capabilities_incompatible", [{
      path: "profile.required_capabilities",
      expected: expectedCapabilities, received: capabilities,
      missing: missingCapabilities, unexpected: unexpectedCapabilities,
    }]);
  }
  return { kind: "protected-ready", profile };
}

export async function bootProtectedWebClient({ baseUrl, fetchImpl = globalThis.fetch, signal }) {
  try {
    const profile = await getProtocolProfile({ baseUrl, fetchImpl, signal });
    return validateProtocolProfile(profile);
  } catch (error) {
    return blocked("protocol_unavailable", [{
      path: "request.getProtocolProfile", error: error instanceof Error ? error.message : String(error),
    }]);
  }
}
