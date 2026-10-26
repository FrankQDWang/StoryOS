/** Encodes a JSON value with its object keys in sorted order, so that two equal values give the same text. */
export function canonicalValue(value: unknown): string {
  const keys = new Set<string>();
  JSON.stringify(value, (key, item: unknown) => { keys.add(key); return item; });
  return JSON.stringify(value, [...keys].sort());
}
