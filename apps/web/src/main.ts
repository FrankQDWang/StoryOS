import { loadStoryOSWebState } from "./app.ts";
import { mountStage1View } from "./stage1-view.tsx";

// ICS-01 prototype: record each request of the page, with its time and body.
const icsRequests: { method: string; url: string; body: string | undefined; at: number; status?: number; end?: number }[] = [];
Reflect.set(globalThis, "__icsRequests", icsRequests);
const icsFetch = globalThis.fetch.bind(globalThis);
globalThis.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
  const record: (typeof icsRequests)[number] = { method: init?.method ?? "GET",
    url: input instanceof Request ? input.url : String(input),
    body: typeof init?.body === "string" ? init.body : undefined, at: performance.now() };
  icsRequests.push(record);
  const response = await icsFetch(input, init);
  record.status = response.status;
  if (/\/manuscript\/author-edits$/u.test(record.url)) {
    const value: unknown = await response.clone().json().catch(() => undefined);
    const effect = (value as { effect?: { kind?: string; reason?: string } } | undefined)?.effect;
    Reflect.set(record, "effect", effect?.kind);
    Reflect.set(record, "reason", effect?.reason);
  }
  record.end = performance.now();
  return response;
};
const loaded = await loadStoryOSWebState();
mountStage1View(loaded.root, loaded);
