import routeCatalog from "../../../../generated/route-catalog/versioned-protocol-release-1-route-catalog.json";
import { jsonResponse } from "./scenario.ts";

// The rules of the Server that the fake Servers of `owned-command-retry` and `journal-earlier-version` apply (#1746).
// The fakes give each refusal that a request of a test can reach, in the order of the Server. They do not model the
// request site, the session cookie, or the body size, because the fake fetch of a test has none of them. A request
// to a route that a fake does not serve fails its test.

export type Operation = {
  operation_id: string; kind: string; method: string; path: string; schemas: { request: string };
};
export type Challenge = { method: string; route_template: string; command_schema: string; idempotency_key: string;
  canonical_command_digest: { algorithm: string; profile: string; value_hex_lowercase: string } };

const { operations } = routeCatalog as { operations: Operation[] };
/** The rows of the route catalog that are the Project command targets of the Server. */
export const PROJECT_COMMANDS = operations.filter(({ kind, path }) =>
  kind === "command" && path.startsWith("/api/v1/projects/{project_id}"));
/** The path of the Challenge route of a Project (`crates/storyos-server/src/lib.rs:526`), with the Project segment. */
export const CHALLENGE_ROUTE = /^\/api\/v1\/projects\/([^/]+)\/anti-forgery-challenges$/;
export const UUID_V7 = /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

export const problem = (code: string, message: string, status: number) =>
  jsonResponse({ schema_id: "storyos.problem.v1", code, message }, status);

const HYPHENATED = "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}";
/** The forms that `Uuid::parse_str` of uuid 1.18.1 reads: simple, hyphenated, braced, and URN with hyphens. */
const UUID_FORMS = new RegExp(`^(?:[0-9a-f]{32}|${HYPHENATED}|\\{${HYPHENATED}\\}|urn:uuid:${HYPHENATED})$`, "i");

/** The UUID of `text` as 32 lowercase hex digits, or undefined when `Uuid::parse_str` refuses it. */
export function canonicalUuid(text: string): string | undefined {
  return UUID_FORMS.test(text) ? text.toLowerCase().replace(/^urn:uuid:/, "").replace(/[{}-]/g, "") : undefined;
}

/** `lib.rs:757`: a path identity that is not a UUID. `authenticate_scope` (`lib.rs:657`) checks the Project first. */
export const identityRefusal = (...identities: string[]) =>
  identities.every((identity) => canonicalUuid(identity) !== undefined) ? undefined
    : problem("invalid_request", "The request identity is invalid.", 400);

/** The 405 of axum for a route with no handler for the method: no body, and the methods of the route in Allow. */
export const methodNotAllowed = (allow: string) => new Response(null, { status: 405, headers: { allow } });

/** `web_host.rs:57`: a path that is no API route goes to the Web surface, and no Web asset has the path. */
export const webFallback = () => new Response(null, { status: 404 });

/** Whether `path` is the path of a route of the route catalog, for one of its methods. */
export const isCatalogRoute = (path: string) => operations.some((operation) =>
  new RegExp(`^${operation.path.replace(/\{[a-z_]+\}/g, "[^/]+")}$`).test(path));

/** `project_command_challenge.rs:15`: the request must have one JSON content type. */
export function contentTypeRefusal(init: RequestInit | undefined): Response | undefined {
  const value = new Headers(init?.headers).get("content-type");
  const json = value !== null && !value.includes(",") && !value.trimEnd().endsWith(";")
    && /^application\/(?:json|[^\s/;+]+\+json)\s*(?:;.*)?$/i.test(value.trim());
  return json ? undefined
    : problem("unsupported_content_type", "The request must use one JSON content type.", 415);
}

/** `lib.rs:873`: a body that is not JSON or does not match its closed schema. */
export const shapeRefusal = () =>
  problem("invalid_request_shape", "The request does not match its closed schema.", 422);

const hasExactStringFields = (value: unknown, fields: string[]): value is Record<string, string> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
  && JSON.stringify(Object.keys(value).sort()) === JSON.stringify([...fields].sort())
  && fields.every((field) => typeof (value as Record<string, unknown>)[field] === "string");

/**
 * The closed schema of `CreateProjectCommandChallengeRequest` and `DigestValue`
 * (`crates/storyos-contracts/src/release1.rs:287`).
 */
function isChallenge(value: unknown): value is Challenge {
  if (typeof value !== "object" || value === null) return false;
  const { canonical_command_digest: digest, ...rest } = value as Record<string, unknown>;
  return hasExactStringFields({ ...rest, canonical_command_digest: "" },
    ["method", "route_template", "command_schema", "canonical_command_digest", "idempotency_key"])
    && hasExactStringFields(digest, ["algorithm", "profile", "value_hex_lowercase"])
    && digest.algorithm === "sha256";
}

/**
 * The answer of `create_project_command_challenge` (`project_command_challenge.rs:40`) to a request on the Challenge
 * route of `pathProject`, when `projectId` is the one Project of the author: a refusal, or the Challenge that the
 * Server binds to a nonce.
 */
export function readChallenge(pathProject: string, projectId: string, init: RequestInit | undefined):
  { refusal: Response } | { challenge: Challenge } {
  // `lib.rs:528`: the route has only its POST filter.
  if ((init?.method ?? "GET") !== "POST") return { refusal: methodNotAllowed("POST") };
  const identity = identityRefusal(pathProject);
  if (identity !== undefined) return { refusal: identity };
  const contentType = contentTypeRefusal(init);
  if (contentType !== undefined) return { refusal: contentType };
  let body: unknown;
  try { body = JSON.parse(String(init?.body)); } catch { return { refusal: shapeRefusal() }; }
  if (!isChallenge(body)) return { refusal: shapeRefusal() };
  // `lib.rs:881`: a target that is not a Project command of the route catalog.
  const target = PROJECT_COMMANDS.find((operation) => operation.method === body.method
    && operation.path === body.route_template && operation.schemas.request === body.command_schema);
  if (target === undefined) {
    return { refusal: problem("command_target_refused", "The command target is not in the closed Release 1 contract.",
      422) };
  }
  // `project_command_challenge.rs:70`: a digest of another profile or form, or a key that is not UUID v7.
  const { profile, value_hex_lowercase: value } = body.canonical_command_digest;
  if (profile !== `storyos.command.${target.operation_id}.jcs.v1` || !/^[0-9a-f]{64}$/.test(value)
    || !UUID_V7.test(body.idempotency_key)) {
    return { refusal: problem("invalid_request", "The request is invalid.", 400) };
  }
  // `project_command_challenge.rs:81`: the Server reads the Project by its UUID, and the author has no other Project.
  if (canonicalUuid(pathProject) !== canonicalUuid(projectId)) {
    return { refusal: problem("resource_unavailable", "The requested resource is unavailable.", 404) };
  }
  return { challenge: body };
}
