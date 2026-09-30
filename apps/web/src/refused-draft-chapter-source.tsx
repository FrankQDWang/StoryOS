import { useEffect, useState } from "react";
import type { ProjectScope }
  from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import { openSelectedChapter } from "./chapter-navigation.ts";

export function RefusedDraftChapterSource({ scope, sourceChapterId, currentChapterId, baseUrl, fetchImpl }: {
  scope: ProjectScope; sourceChapterId: string; currentChapterId: string | undefined;
  baseUrl: string; fetchImpl: typeof fetch;
}) {
  const identity = `${scope.owner_user_id}:${scope.project_id}:${sourceChapterId}`;
  const [read, setRead] = useState<{ identity: string; kind: "loading" | "unavailable" | "known"; title?: string }>();
  useEffect(() => {
    let active = true;
    setRead({ identity, kind: "loading" });
    void openSelectedChapter({ baseUrl, fetchImpl, projectId: scope.project_id,
      chapterId: sourceChapterId, expectedScope: scope }).then((result) => {
      if (!active) return;
      setRead(result.kind === "opened" && typeof result.chapter.chapter.title === "string"
        ? { identity, kind: "known", title: result.chapter.chapter.title }
        : { identity, kind: "unavailable" });
    }).catch(() => { if (active) setRead({ identity, kind: "unavailable" }); });
    return () => { active = false; };
  }, [identity, scope.owner_user_id, scope.project_id, sourceChapterId, baseUrl, fetchImpl]);
  const current = read?.identity === identity ? read : undefined;
  return <p data-draft-chapter-source>
    {current?.kind === "known" ? `Source chapter: ${current.title}. `
      : current?.kind === "unavailable" ? "Source chapter details are unavailable. "
        : "Reading source chapter details. "}
    {currentChapterId === undefined ? "The chapter shown here could not be confirmed."
      : currentChapterId === sourceChapterId ? "This edit belongs to the chapter shown here."
        : "This edit belongs to another chapter, not the chapter shown here."}
  </p>;
}
