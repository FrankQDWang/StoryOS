// ADR 0107, section 2, item 5: one Editor Session is open in one tab.

/**
 * The name of the Web Lock of an Editor Session. The tab that has the Session open holds it, so that no other open tab
 * opens that Session.
 */
export function editorSessionLock(editorSessionId: string): string {
  return `storyos-editor-session:${editorSessionId}`;
}

// The Editor Sessions whose lock this page holds, and the end of each lock.
const heldSessions = new Map<string, () => void>();

/**
 * Takes the lock of an Editor Session for this page. It is false when another page holds the lock: the browser command
 * "Duplicate tab" copies `sessionStorage`, and two pages of one Editor Session would freeze and settle the Journal
 * records of each other.
 */
export async function lockEditorSession(editorSessionId: string): Promise<boolean> {
  const locks = globalThis.navigator?.locks;
  if (heldSessions.has(editorSessionId) || locks === undefined) return true;
  return new Promise<boolean>((resolve, reject) => {
    locks.request(editorSessionLock(editorSessionId), { ifAvailable: true }, (lock) => {
      if (lock === null) {
        resolve(false);
        return undefined;
      }
      return new Promise<void>((release) => {
        heldSessions.set(editorSessionId, release);
        resolve(true);
      });
    }).catch(reject);
  });
}

/** Ends the lock of each Editor Session of this page other than `editorSessionId`, after `sessionStorage` names it. */
export function releaseEarlierEditorSessions(editorSessionId: string): void {
  for (const [held, release] of heldSessions) {
    if (held === editorSessionId) continue;
    heldSessions.delete(held);
    release();
  }
}
