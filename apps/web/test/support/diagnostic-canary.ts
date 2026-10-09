import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";

/** One JSON line that the Diagnostic Projection formatter writes (ADR 0047). */
export interface DiagnosticLine {
  readonly level: string;
  readonly target: string;
  readonly fields: Readonly<Record<string, unknown>>;
  readonly span?: Readonly<Record<string, unknown>>;
}

/** Returns a unique value that no StoryOS source contains. */
export function canary(label: string): string {
  return `storyos-canary-${label}-${randomUUID()}`;
}

/** Returns the test database URL with a canary application name, so that the URL is a canary. */
export function canaryDatabaseUrl(applicationName: string): string {
  const databaseUrl = process.env.STORYOS_TEST_DATABASE_URL;
  assert.ok(databaseUrl, "run through scripts/verify-project-scope.sh");
  const separator = databaseUrl.includes("?") ? "&" : "?";
  return `${databaseUrl}${separator}application_name=${applicationName}`;
}

/** Parses stderr and requires that each line is one JSON object. */
export function diagnosticLines(stderr: string): DiagnosticLine[] {
  return stderr.split("\n").filter((line) => line.length > 0).map((line) => {
    const parsed: unknown = JSON.parse(line);
    assert.ok(parsed !== null && typeof parsed === "object", line);
    return parsed as DiagnosticLine;
  });
}

/** Requires that stderr contains no canary value. */
export function assertNoCanary(stderr: string, canaries: readonly string[]): void {
  for (const value of canaries) {
    assert.equal(stderr.includes(value), false, `stderr contains the canary ${value}`);
  }
}

/** Returns the close events of the spans with this name. */
export function closedSpans(lines: readonly DiagnosticLine[], name: string): DiagnosticLine[] {
  return lines.filter((line) => line.fields.message === "close" && line.span?.name === name);
}
