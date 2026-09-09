import { createProject, createProjectChallenge } from "../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../generated/typescript/storyos-public-release-1/release-profile.mjs";

const COMMAND_SCHEMA = "storyos.command.create-project.request.v1";
const SECURITY_POLICY = "storyos.web-security-policy.release-1.v1";

const PROJECTS = [
  {
    idempotencyKey: "018f0000-0000-7001-8000-00000000e701",
    correlationId: "018f0000-0000-7001-8000-00000000e711",
  },
  {
    idempotencyKey: "018f0000-0000-7001-8000-00000000e702",
    correlationId: "018f0000-0000-7001-8000-00000000e712",
  },
];

function sessionFetch(baseUrl, sessionHandle) {
  return (input, init) => {
    const headers = new Headers(init?.headers);
    headers.set("origin", baseUrl);
    headers.set("cookie", `storyos_session=${sessionHandle}`);
    return fetch(input, { ...init, headers });
  };
}

async function createEmptyProject(baseUrl, fetchImpl, title, keys) {
  const request = {
    command_schema: COMMAND_SCHEMA,
    create_project_input: {
      title,
      client_contract_revision:
        RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
      security_policy_revision: SECURITY_POLICY,
      correlation_id: keys.correlationId,
    },
    idempotency_key: keys.idempotencyKey,
  };
  const challenge = await createProjectChallenge({ baseUrl, request, fetchImpl });
  const created = await createProject({
    baseUrl,
    fetchImpl,
    idempotencyKey: keys.idempotencyKey,
    antiForgery: challenge.nonce,
    request: {
      command_schema: request.command_schema,
      prospective_project_id: challenge.prospective_project_id,
      create_project_input: request.create_project_input,
    },
  });
  if (created.project.open.kind !== "empty") {
    throw new Error(`createProject did not return an empty Project: ${created.project.open.kind}`);
  }
  if (created.project.project_id !== challenge.prospective_project_id) {
    throw new Error("createProject returned a different Project ID than the challenge");
  }
  return created.project.project_id;
}

const [baseUrl, sessionHandle, titleOne, titleTwo] = process.argv.slice(2);
if (!baseUrl || !sessionHandle || !titleOne || !titleTwo) {
  throw new Error("usage: create-public-empty-projects.mjs <baseUrl> <session> <titleOne> <titleTwo>");
}

const fetchImpl = sessionFetch(baseUrl, sessionHandle);
const first = await createEmptyProject(baseUrl, fetchImpl, titleOne, PROJECTS[0]);
const second = await createEmptyProject(baseUrl, fetchImpl, titleTwo, PROJECTS[1]);
if (first === second) {
  throw new Error("createProject reused one Project ID for two empty Projects");
}
process.stdout.write(`${first}\n${second}\n`);
