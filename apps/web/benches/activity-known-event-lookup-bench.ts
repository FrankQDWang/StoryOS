import { performance } from "node:perf_hooks";

const ADMITTED_REPLAY = 1_000;
const SEEDED_KNOWN = 1_000;
const SEEDED_INCOMING = 250;

interface KnownEvent {
  event_id: string;
  stream_sequence: string;
}

function eventId(index: number): string {
  return `018f0000-0000-7001-8000-${index.toString(16).padStart(12, "0")}`;
}

function knownEvent(index: number): KnownEvent {
  return {
    event_id: eventId(index),
    stream_sequence: String(index + 1),
  };
}

function representativeBatch(knownCount: number, incomingCount: number): {
  cursor: bigint;
  incoming: KnownEvent[];
  known: KnownEvent[];
} {
  return {
    cursor: BigInt(knownCount),
    incoming: Array.from(
      { length: incomingCount },
      (_, index) => knownEvent(knownCount + index),
    ),
    known: Array.from({ length: knownCount }, (_, index) => knownEvent(index)),
  };
}

function predictedLinearVisits(knownCount: number, incomingCount: number): number {
  return 2 * knownCount * incomingCount + incomingCount * (incomingCount - 1);
}

function predictedMapOperations(knownCount: number, incomingCount: number): number {
  return 2 * knownCount + 4 * incomingCount;
}

function applyLinearLookups(
  known: readonly KnownEvent[],
  incoming: readonly KnownEvent[],
  cursor: bigint,
): KnownEvent[] {
  const working = [...known];
  const accepted: KnownEvent[] = [];
  for (const event of incoming) {
    if (working.find((candidate) => candidate.event_id === event.event_id)) {
      continue;
    }
    if (working.find((candidate) => candidate.stream_sequence === event.stream_sequence)) {
      continue;
    }
    if (BigInt(event.stream_sequence) <= cursor) {
      continue;
    }
    working.push(event);
    accepted.push(event);
  }
  return accepted;
}

function applyIndexedLookups(
  known: readonly KnownEvent[],
  incoming: readonly KnownEvent[],
  cursor: bigint,
): KnownEvent[] {
  const working = [...known];
  const knownByEventId = new Map<string, KnownEvent>();
  const knownByStreamSequence = new Map<string, KnownEvent>();
  for (const event of working) {
    if (!knownByEventId.has(event.event_id)) {
      knownByEventId.set(event.event_id, event);
    }
    if (!knownByStreamSequence.has(event.stream_sequence)) {
      knownByStreamSequence.set(event.stream_sequence, event);
    }
  }
  const accepted: KnownEvent[] = [];
  for (const event of incoming) {
    if (knownByEventId.get(event.event_id)) {
      continue;
    }
    if (knownByStreamSequence.get(event.stream_sequence)) {
      continue;
    }
    if (BigInt(event.stream_sequence) <= cursor) {
      continue;
    }
    working.push(event);
    knownByEventId.set(event.event_id, event);
    knownByStreamSequence.set(event.stream_sequence, event);
    accepted.push(event);
  }
  return accepted;
}

function measure(rounds: number, apply: () => KnownEvent[]): [number, KnownEvent[]] {
  let result: KnownEvent[] = [];
  let best = Number.POSITIVE_INFINITY;
  for (let sample = 0; sample < 7; sample += 1) {
    const start = performance.now();
    for (let round = 0; round < rounds; round += 1) {
      result = apply();
    }
    best = Math.min(best, (performance.now() - start) / rounds);
  }
  return [best, result];
}

function runCase(label: string, knownCount: number, incomingCount: number, rounds: number): void {
  const batch = representativeBatch(knownCount, incomingCount);
  const linear = applyLinearLookups(batch.known, batch.incoming, batch.cursor);
  const indexed = applyIndexedLookups(batch.known, batch.incoming, batch.cursor);
  if (JSON.stringify(linear) !== JSON.stringify(indexed)) {
    throw new Error(`${label} linear and indexed accepted Events must be equal`);
  }
  const [linearMs, linearTimed] = measure(
    rounds,
    () => applyLinearLookups(batch.known, batch.incoming, batch.cursor),
  );
  const [indexedMs, indexedTimed] = measure(
    rounds,
    () => applyIndexedLookups(batch.known, batch.incoming, batch.cursor),
  );
  if (JSON.stringify(linearTimed) !== JSON.stringify(indexedTimed)) {
    throw new Error(`${label} timed linear and indexed accepted Events must be equal`);
  }
  console.log(
    `${label} K=${knownCount} F=${incomingCount}`
      + ` linear_visits=${predictedLinearVisits(knownCount, incomingCount)}`
      + ` map_ops=${predictedMapOperations(knownCount, incomingCount)}`
      + ` linear=${linearMs.toFixed(3)}ms indexed=${indexedMs.toFixed(3)}ms`,
  );
}

runCase("admitted-replay", 0, ADMITTED_REPLAY, 3);
runCase("seeded-known", SEEDED_KNOWN, SEEDED_INCOMING, 3);
