import assert from 'node:assert/strict';
import { getExportOperation, getHumanReadableManuscriptExport, listProjects } from '../generated/typescript/storyos-public-release-1/client.mjs';

const [baseUrl, readablePair, archivePair] = process.argv.slice(2);
const fetchImpl = (input, init) => {
  const headers = new Headers(init?.headers);
  headers.set('origin', baseUrl);
  headers.set('cookie', 'storyos_session=session-a');
  return fetch(input, { ...init, headers });
};
const [readableProject, readableExport] = readablePair.split('/');
const [archiveProject, archiveExport] = archivePair.split('/');
const listed = await listProjects({ baseUrl, fetchImpl });
for (const projectId of [readableProject, archiveProject]) {
  assert.equal(listed.projects.find((item) => item.project_scope.project_id === projectId)?.lifecycle.kind, 'archived');
}
const readable = await getHumanReadableManuscriptExport({ baseUrl, projectId: readableProject, exportId: readableExport, fetchImpl });
assert.equal(readable.status, 'ready');
assert.ok(readable.manuscript_utf8.includes('The archived chapter keeps its own words.'));
const archive = await getExportOperation({ baseUrl, projectId: archiveProject, exportId: archiveExport, fetchImpl });
assert.equal(archive.status, 'ready');
const archiveResponse = await fetchImpl(`${baseUrl}/api/v1/projects/${archiveProject}/exports/${archiveExport}`, {
  headers: { Accept: 'application/vnd.storyos.project-archive+zip; profile="storyos.project-export.v1"' },
});
assert.equal(archiveResponse.status, 200);
assert.ok((await archiveResponse.arrayBuffer()).byteLength > 0);
