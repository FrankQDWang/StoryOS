export const volumes = [
  {
    id: "volume-1",
    title: "卷一 · 雾起",
    chapters: [
      ["chapter-1", "第一章　潮声"],
      ["chapter-2", "第二章　断桥"],
      ["chapter-3", "第三章　灯火"],
      ["chapter-4", "第四章　漫天"],
      ["chapter-5", "第五章　客舟"],
      ["chapter-6", "第六章　归人"],
    ],
  },
  {
    id: "volume-2",
    title: "卷二 · 风起",
    chapters: [
      ["chapter-7", "第七章　古道"],
      ["chapter-8", "第八章　旧友"],
      ["chapter-9", "第九章　迷雾"],
      ["chapter-10", "第十章　亡局"],
      ["chapter-11", "第十一章　暗涌"],
      ["chapter-12", "第十二章　雨夜"],
      ["chapter-13", "第十三章　归期"],
      ["chapter-14", "第十四章　破晓"],
    ],
  },
  {
    id: "volume-3",
    title: "卷三 · 将明",
    chapters: [
      ["chapter-15", "第十五章　余烬"],
      ["chapter-16", "第十六章　长明"],
    ],
  },
];
export const chapterTitles = Object.fromEntries(
  volumes.flatMap((volume) => volume.chapters),
);

const paragraph = (blockId, text) => ({
  type: "paragraph",
  attrs: { blockId },
  content: text ? [{ type: "text", text }] : undefined,
});

export const proposalParagraphs = [
  paragraph(
    "proposal-12-1",
    "他本不该去。可那行字像一根细针，从这三日的每一个念头里反复穿过，扎得他夜不能寐。他想起那人最后一次回头时的眼神，像是把什么托付出去，又像是诀别。",
  ),
  paragraph("proposal-12-2", "更鼓将尽，苏砚收起信，撑伞往西。"),
  paragraph(
    "proposal-12-3",
    "风从城墙的裂缝里钻出来，带着湿冷的土腥味。远处的钟楼在雨里模糊不清，偶尔传来一声闷响，像是夜的呼吸。",
  ),
  paragraph(
    "proposal-12-4",
    "旧仓的门半掩着，门轴锈得厉害，推开时发出长长的吱呀声。仓内一片漆黑，只有梁上的雨水从破洞滴下，滴在地上，汇成浅浅的水洼。",
  ),
  paragraph("proposal-12-5", "苏砚踏进去，伞尖在地面点了点，水珠四溅。"),
  paragraph("proposal-12-6", "黑暗里有脚步声，缓慢而沉稳。"),
  paragraph("proposal-12-7", "“你来了。”一个低沉的声音在前方响起。"),
  paragraph(
    "proposal-12-8",
    "苏砚没有立刻回答，只是闭了闭眼，再睁开时，目光已沉静如水。",
  ),
];

export const proposalBlockIds = proposalParagraphs.map(
  (node) => node.attrs.blockId,
);

export const chapterTwelveDocument = {
  type: "doc",
  content: [
    paragraph("chapter-12-p1", "雨下得很大，像天上有人把盆打翻在城外。"),
    paragraph(
      "chapter-12-p2",
      "青石巷被冲刷得发亮，檐角的水一滴一滴落在油纸伞上，发出细碎的响声。苏砚站在廊下，望着巷口渐深的夜色，迟迟没有迈步。",
    ),
    paragraph(
      "chapter-12-p3",
      "他手里攥着一封信，纸边被雨气沾得微湿，字迹却依旧清晰。信是三日前送来的，只有一行字：子时，旧仓。等你。",
    ),
    paragraph(
      "chapter-12-p4",
      "旧仓在城西，早年是官府的粮仓，如今半废，杂草丛生，少有人去。",
    ),
    ...proposalParagraphs,
  ],
};

export const chapterDocuments = Object.fromEntries(
  Object.entries(chapterTitles).map(([id, title]) => [
    id,
    id === "chapter-12"
      ? chapterTwelveDocument
      : {
          type: "doc",
          content: [
            paragraph(`${id}-p1`, `${title.replace(/　/g, " ")}。`),
            paragraph(
              `${id}-p2`,
              "这一章仍在整理中。你可以在此直接写作，也可以在右侧告诉写作助手想继续推进的方向。",
            ),
          ],
        },
  ]),
);

export const initialProposal = {
  id: "proposal-rain-night-continuation",
  revision: 3,
  blockIds: proposalBlockIds,
  generation: "ready",
  validation: "valid",
  resolution: "pending",
  closure: "open",
  lastAppliedStreamSeq: 8,
  pauseFence: null,
  baseRevision: "chapter-12-r7",
  derivedFromRevision: null,
  rejectedRevision: null,
  conflictReason: null,
  creator: "agent",
};

export const initialMessages = [
  {
    id: "message-author-1",
    role: "author",
    time: "10:18",
    text: "请在“雨夜”章节后半段，补写苏砚前往旧仓的过程与氛围，保持克制、含蓄的叙述风格。",
  },
  {
    id: "message-agent-1",
    role: "agent",
    time: "10:19",
    text: "已为你补写苏砚前往旧仓的过程与氛围，延续克制、含蓄的叙述风格，突出雨夜的环境与内心的张力。你可以直接在正文中的提案里修改。",
  },
];
