import { afterEach, expect, it } from "vitest";

import { createProjectChallenge } from "../../../../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../../../../generated/typescript/storyos-public-release-1/release-profile.mjs";
import { updateClientSessionCookie } from "../support/browser-command-client.ts";
import { closeApplicationFrame, openApplicationFrame } from "../support/exact-dist-frame.ts";
import { SECURITY_POLICY_REVISION } from "../../src/client-identity.ts";

const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
async function waitForProtectedReady(frame: HTMLIFrameElement): Promise<Element> {
  await expect.poll(() =>
    frame.contentDocument?.querySelector("#app")?.getAttribute("data-boot-state")
  ).toBe("protected-ready");
  const root = frame.contentDocument?.querySelector("#app");
  if (root === null || root === undefined) {
    throw new Error("the exact-dist page did not reach protected-ready");
  }
  return root;
}

afterEach(async () => {
  await closeApplicationFrame();
  await updateClientSessionCookie({ action: "clear" });
  document.body.replaceChildren();
});

it("a new local browser session reaches protected-ready without AI and acquires createProjectChallenge", async () => {
  const frame = await openApplicationFrame("StoryOS exact-dist local bootstrap");
  const root = await waitForProtectedReady(frame);
  expect(root.getAttribute("data-boot-state")).toBe("protected-ready");
  expect(root.querySelector('[role="alert"]')).toBeNull();
  expect(root.textContent).toContain("本地写作已就绪");
  expect(root.textContent).not.toContain("模型");
  expect(root.textContent).not.toContain("Agent");
  expect(root.textContent).not.toContain("Provider");
  expect(root.textContent).not.toContain("MCP");

  const challenge = await createProjectChallenge({
    baseUrl: location.origin,
    request: {
      command_schema: "storyos.command.create-project.request.v1",
      create_project_input: {
        title: "Prospective Novel",
        client_contract_revision:
          RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
        security_policy_revision: SECURITY_POLICY_REVISION,
        correlation_id: "018f0000-0000-7001-8000-000000000330",
      },
      idempotency_key: "018f0000-0000-7001-8000-000000000331",
    },
  });
  expect(challenge.prospective_project_id).toMatch(UUID_V7);
  expect(challenge.canonical_command_digest.profile).toBe("storyos.command.createProject.jcs.v1");
  expect(challenge.nonce).toMatch(/^[0-9a-f]{64}$/);
  expect(challenge.limit_profile_revision).toBe("storyos.foundation.absolute.v1");
});
