// The writer and the reader of the restore drill of the personal instance
// (scripts/verify-personal-instance-restore.sh).
//
// write <baseUrl> <session> <text>: makes one Project with one Volume and one Chapter through the public commands,
// puts <text> into the Chapter, and prints the Project ID and the Chapter ID.
// read <baseUrl> <session> <projectId> <chapterId>: prints the text of the Chapter.
import {
  applyAuthorEdit, createChapter, createEditorSession, createProject, createProjectChallenge,
  createProjectCommandChallenge, createVolume, digestApplyAuthorEdit, digestCreateChapter, digestCreateEditorSession,
  digestCreateVolume, getChapter,
} from "../generated/typescript/storyos-public-release-1/client.mjs";
import { RELEASE_1_PROTOCOL_PROFILE } from "../generated/typescript/storyos-public-release-1/release-profile.mjs";

const BINDING = {
  client_contract_revision: RELEASE_1_PROTOCOL_PROFILE.release_identity.web_client_contract_revision,
  security_policy_revision: "storyos.web-security-policy.release-1.v1",
};
const key = (suffix) => `018f0000-0000-7001-8000-0000000f${suffix}`;

function sessionFetch(baseUrl, sessionHandle) {
  return (input, init) => {
    const headers = new Headers(init?.headers);
    headers.set("origin", baseUrl);
    headers.set("cookie", `storyos_session=${sessionHandle}`);
    return fetch(input, { ...init, headers });
  };
}

/** Solves the Command Challenge of one Project command and sends the command one time. */
async function challenged(command, method, route, request, digest, idempotencyKey, send) {
  const challenge = await createProjectCommandChallenge({
    baseUrl: command.baseUrl, projectId: command.projectId, fetchImpl: command.fetchImpl,
    request: {
      method, route_template: route, command_schema: request.command_schema,
      canonical_command_digest: await digest(request), idempotency_key: idempotencyKey,
    },
  });
  const response = await send({ ...command, idempotencyKey, antiForgery: challenge.nonce, request });
  if (response.effect && response.effect.kind !== "authoritative_applied") {
    throw new Error(`${request.command_schema} did not apply: ${response.effect.kind}`);
  }
  return response;
}

async function write(baseUrl, fetchImpl, text) {
  const createRequest = {
    command_schema: "storyos.command.create-project.request.v1",
    create_project_input: { title: "Restore drill", ...BINDING, correlation_id: key("0001") },
    idempotency_key: key("0002"),
  };
  const projectChallenge = await createProjectChallenge({ baseUrl, request: createRequest, fetchImpl });
  const created = await createProject({
    baseUrl, fetchImpl, idempotencyKey: createRequest.idempotency_key, antiForgery: projectChallenge.nonce,
    request: {
      command_schema: createRequest.command_schema,
      prospective_project_id: projectChallenge.prospective_project_id,
      create_project_input: createRequest.create_project_input,
    },
  });
  const command = { baseUrl, fetchImpl, projectId: created.project.project_id };
  const volume = await challenged(command, "POST", "/api/v1/projects/{project_id}/volumes", {
    command_schema: "storyos.command.create-volume.request.v1",
    create_volume_input: { title: "Volume", expected_tree_revision: "1", ...BINDING, correlation_id: key("0003") },
  }, digestCreateVolume, key("0004"), createVolume);
  const chapter = await challenged(command, "POST", "/api/v1/projects/{project_id}/volumes/{volume_id}/chapters", {
    command_schema: "storyos.command.create-chapter.request.v1",
    create_chapter_input: { title: "Chapter", expected_tree_revision: "2", ...BINDING, correlation_id: key("0005") },
  }, digestCreateChapter, key("0006"), (options) => createChapter({ ...options, volumeId: volume.effect.volume_id }));
  const session = await challenged(command, "POST", "/api/v1/projects/{project_id}/editor-sessions", {
    command_schema: "storyos.command.create-editor-session.request.v1", ...BINDING, correlation_id: key("0007"),
  }, digestCreateEditorSession, key("0008"), createEditorSession);
  const snapshot = session.base_snapshot;
  if (snapshot.chapter_id !== chapter.effect.chapter_id) {
    throw new Error("the Editor Session did not open the new Chapter");
  }
  const from = snapshot.materialized_revision.body.length;
  await challenged(command, "POST", "/api/v1/projects/{project_id}/manuscript/author-edits", {
    command_schema: "storyos.command.apply-author-edit.request.v1", ...BINDING, correlation_id: key("0009"),
    editor_session_id: session.editor_session.editor_session_id,
    writer_generation: session.writer.writer_generation,
    chapter_id: snapshot.chapter_id,
    expected_authoritative_revision_id: snapshot.authoritative_head_revision_id,
    expected_proposal_head_revision_ids: snapshot.proposal_head_revision_ids,
    target_refs: snapshot.target_refs,
    observed_ownership_partition: snapshot.observed_ownership_partition,
    editor_contract_revision: "storyos.editor-contract.release-1.v3",
    undo_group_id: key("000a"),
    completed_intent_record_id: key("000b"),
    local_intent_sequence: "1",
    author_edit_units: [{
      normalized_primitives: [{ kind: "replace_selection", from, to: from, text }],
      selection_snapshot: { coordinate_profile: "storyos.editor.utf16-code-unit.v1", from, to: from },
    }],
  }, digestApplyAuthorEdit, key("000c"), applyAuthorEdit);
  return `${command.projectId}\n${snapshot.chapter_id}\n`;
}

const [mode, baseUrl, sessionHandle, ...rest] = process.argv.slice(2);
const fetchImpl = sessionFetch(baseUrl, sessionHandle);
if (mode === "write" && rest.length === 1) {
  process.stdout.write(await write(baseUrl, fetchImpl, rest[0]));
} else if (mode === "read" && rest.length === 2) {
  const read = await getChapter({ baseUrl, fetchImpl, projectId: rest[0], chapterId: rest[1] });
  process.stdout.write(read.chapter.current_revision.body);
} else {
  throw new Error("usage: personal-instance-chapter.mjs write <baseUrl> <session> <text> | read <baseUrl> <session> "
    + "<projectId> <chapterId>");
}
