import { useEffect, useRef, useState } from "react";

import type { GetManuscriptTreeResponse } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { addressParameter, removeAddressParameter } from "./writing-workspace.tsx";

/** The sentence of an address Chapter that the workspace cannot open. */
export const ADDRESS_CHAPTER_NOT_FOUND_SENTENCE = "找不到这个章节。";

/**
 * The address `/projects/{project_id}?chapter={chapter_id}` (ADR 0094 section 9). When the manuscript tree is read, the
 * workspace opens the Chapter one time with `open`, the steps of a click in the tree. The Current Chapter sends no
 * command. A Chapter that the tree of this Project does not have (also a value that is not a Chapter identity or a
 * Chapter of another Project), or a workspace that cannot change the Current Chapter, opens nothing and calls
 * `onNotFound`. The result is true while the Conversation of the address must wait: the workspace opens the
 * Conversation after `open` ends, also when the workspace opened again at the Chapter.
 */
export function useAddressChapter(options: {
  tree: GetManuscriptTreeResponse | undefined;
  currentChapterId: string | undefined;
  canChangeCurrentChapter: boolean;
  open: (chapterId: string) => Promise<boolean>;
  onNotFound: () => void;
}): boolean {
  const requested = useRef(addressParameter("chapter"));
  const [hold, setHold] = useState(requested.current !== undefined);
  const { tree } = options;
  useEffect(() => {
    const chapterId = requested.current;
    if (chapterId === undefined || tree === undefined) return;
    requested.current = undefined;
    removeAddressParameter("chapter");
    const release = () => setHold(false);
    if (chapterId === options.currentChapterId) {
      release();
      return;
    }
    const known = tree.volumes.some((volume) => volume.chapters.some((chapter) => chapter.chapter_id === chapterId));
    if (!known || !options.canChangeCurrentChapter) {
      options.onNotFound();
      release();
      return;
    }
    void options.open(chapterId).then(release);
  }, [tree]);
  return hold;
}
