import type { LocalUnsentText } from "./local-unsent-text.ts";

export const LOCAL_UNSENT_TEXT_FILE_NAME = "storyos-local-unsent-text.txt";

/** The file text. Each Chapter body, then each unsent Proposal candidate text, under its label. */
export function localUnsentTextFile(text: LocalUnsentText, chapterTitles: ReadonlyMap<string, string>): string {
  return ["StoryOS 本机未发送的文字（未保存到服务器）",
    ...text.chapters.map((chapter) => `\n章节：${chapterTitles.get(chapter.chapter_id) ?? chapter.chapter_id}\n${chapter.body}`),
    ...text.proposals.map((proposal) => `\n提案：${proposal.proposal_id}\n${proposal.text}`)].join("\n");
}

/** Lets the author download the unsent local text of a Journal that cannot upgrade (contract 3.2). */
export function LocalUnsentTextPanel({ text, chapterTitles }: {
  text: LocalUnsentText;
  chapterTitles: ReadonlyMap<string, string>;
}) {
  const empty = text.chapters.length === 0 && text.proposals.length === 0;
  if (text.readable && empty) return null;
  const download = () => {
    const url = URL.createObjectURL(new Blob([localUnsentTextFile(text, chapterTitles)],
      { type: "text/plain;charset=utf-8" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = LOCAL_UNSENT_TEXT_FILE_NAME;
    anchor.rel = "noopener";
    document.body.append(anchor);
    anchor.click();
    anchor.remove();
    URL.revokeObjectURL(url);
  };
  return (
    <section data-local-unsent-text="">
      {text.readable ? null : <p>本机有尚未发送的文字，但无法读取。请不要清除浏览器数据。</p>}
      {empty ? null : (
        <>
          <p>本机有尚未发送到服务器的文字。服务器导出不包含这些文字。</p>
          <button type="button" onClick={download}>下载本机未发送的文字</button>
        </>
      )}
    </section>
  );
}
