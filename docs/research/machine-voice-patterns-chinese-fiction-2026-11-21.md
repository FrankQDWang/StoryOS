# Machine-voice patterns in Chinese fiction prose, and how to word guidance against them (2026-11-21)

This report is an evidence record for the kickoff of Stage 10
([ADR 0100](../adr/0100-deliver-stage-10-with-writing-rules-distribution-skillpackages-and-marks-on-the-new-words-of-a-candidate.md)).
A research agent of the coordinator session made it on 2026-11-21. It read about 45
Chinese-language sources and the prompt guides of three model vendors through a fetch tool that
summarises each page. The text below is the result as the agent gave it. Section F lists what the
agent could not verify. The counts of sources are approximate, and the lists of the tools copy one
another, so the sources are not independent.

Section D is the draft that the first writing rules of the Host instructions started from. The
ticket of the writing rules owns the text that the product sends.

**Scope.** Counts are approximate tallies over about 45 Chinese-language sources that I opened; tool lists copy one another, so independence is weak. No search tool failed; one archive fetch was blocked and one vendor URL returned 404.

## A. Pattern classes, ranked by sources

| Class | Chinese examples | Sources | Strongest source |
|---|---|---|---|
| Negative-flip frames | 不是……而是……；这不仅是……更是……；没有X，没有Y，只有Z | ~20 | zh and en Wikipedia sign pages (not fiction) |
| Dash and colon framing | 每段一个破折号；A——不是B | ~17 | en Wikipedia, now under historical indicators; RUC counted fewer dashes in AI essays (0.58% vs 0.85%) |
| Elevating endings, narrator foreshadowing | 这只是开始；命运的齿轮开始转动；这一夜，注定无人入眠；他不知道的是 | ~15 | StoryScope: narrator explains the theme, 77% vs 52% human |
| Stock similes, adjective stacking | 如同石子投入湖面，泛起层层涟漪；眼神像淬了冰的刀；一句三个以上形容词 | ~15 | LAMP: cliché is 17% of 8,035 expert edits; disputed for essays by the sspai test |
| Rule of three, symmetry | 三连排比；对仗收尾；三段式 | ~14 | en Wikipedia; RUC (对偶句: AI 4, human 0.67) |
| Explained emotion and meaning | 心中涌起一股……；心中五味杂陈；他终于明白 | ~11 | LAMP: redundant exposition is 18% of edits |
| Uniform rhythm, connective glue | 然而；与此同时；值得一提的是 | ~11 | Zhou & Liu 2024 (abstract only); disputed by the sspai test |
| Stock body reactions | 眼中闪过一丝……；嘴角勾起一抹弧度；深吸一口气；瞳孔微缩；指节泛白 | ~9 | StoryScope: bodily emotion 81% vs 38% |
| Weak adverbs, hedges | 微微/缓缓/轻轻/淡淡；一丝/一抹；仿佛/似乎；不禁 | ~9 | oh-story list (8,800 stars) |
| Same-voice, complete dialogue | 说明书式对话；全员标准书面语 | ~8 | StoryScope: dialogue as debate, 59% vs 34% |
| Over-detailing | 开篇大段环境描写；设定报菜名；无意义的详细数字 | ~8 | Three web-novel editors (澎湃号) |
| Fast, tidy resolution | 冲突刚起来就有人递台阶；同一冲突同段解决 | ~7 | StoryScope: acceptance endings 47% vs 27%; no subplots 79% vs 57% |

Also named: 翻译腔 (~7; RUC reader analysis).

Conflict: readers name told emotion, but StoryScope measured explicit emotion labels at 8% (AI) vs 29% (human). The measured excess is bodily emotion and explained meaning.

## B. Fifteen most-named items (approximate source counts)

不是……而是…… 20 · 破折号 17 · 章尾/段尾升华句 15 · 三连排比 14 · 他感到……/她意识到…… 9 · 眼中闪过一丝…… 8 · 嘴角勾起一抹弧度 8 · 仿佛/似乎/宛如 6 · 段首"然而/与此同时" 6 · 微微/缓缓/轻轻/淡淡 5 · 一丝/一抹 5 · 心中涌起一股…… 5 · 不禁/不由得 5 · 深吸一口气 4 · 他不知道的是/殊不知 4.

Lower: 心中五味杂陈 4; 命运的齿轮 3; 声音不大，却…… 3; 指节泛白 2; 瞳孔微缩 2; 没有A，没有B，只有C 2; 空气仿佛凝固 1.

## C. How to word the guidance

1. State the wanted behaviour and its reason. Anthropic: "Tell Claude what to do instead of what not to do"; OpenAI's help centre gives the same rule.
2. A named ban has limited effect and can prime the phrase. LAMP's generation prompt already told models to avoid clichés and ornament; writers still made about 8 edits per paragraph. arXiv 2601.08070 attributes 87.5% of violations to priming (single-author preprint).
3. A ban moves the model to the next default. Anthropic's design guidance bans named fonts, then notes convergence on another (analogy). One blogger saw 深吸一口气 return as 呼吸停了半拍 (ivisi.cc, community).
4. Define the mechanism and say when a device is appropriate. Anthropic's current model guide recommends a short definition of the anti-pattern, and when-appropriate rules in place of blanket bans.
5. Lists go stale and over-correct. en Wikipedia now lists em-dash overuse as historical and says that avoidance created a new tell. A corpus test reported on sspai kept 11 of 26 circulating Chinese criteria. oh-story warns of 电报体 after over-deletion.
6. No source gives a length limit for a style list. Indirect: at 500 simultaneous instructions the best models reach 68% and favour early ones (arXiv 2507.11538, abstract); OpenAI's GPT-5 guide says contradictory or emphatic wording does harm.
7. Examples steer most, and they are copied. Anthropic advises 3-5 diverse examples; Google warns of overfitting to many; OpenAI's GPT-4.1 guide notes sample phrases reused verbatim. So author samples must outrank the rules, and avoid-examples stay few.
8. Bound the body-language advice. Blanket show-through-the-body guidance reproduces the measured AI habit (StoryScope).

## D. Draft guidance

These are defaults for Chinese fiction prose. The author's instructions and sample passages take precedence; when a sample uses a device named here, follow the sample. The Chinese examples show a habit, not banned words: one use that does work in the scene is correct, and repetition is the fault.

1. State the fact directly. Use a contrast only when the scene corrects a belief that a character held. Avoid: 这不是结束，而是开始.
2. End a scene on its last event or line, inside what the viewpoint character knows. Leave the meaning and the future unstated. Avoid: 命运的齿轮开始转动；他不知道的是……
3. Show a feeling once, through a choice, a spoken line, or a cost. A small feeling can be named plainly. Avoid: 心中涌起一股暖流；他终于明白……
4. Give a physical reaction only when it belongs to this character at this moment. Avoid: 眼中闪过一丝……；嘴角勾起一抹弧度.
5. Use a degree or doubt word only when speed, force, or uncertainty changes what the reader understands. Avoid: 微微/缓缓 on consecutive actions；仿佛 where the character is certain.
6. Use a figure of speech only when it comes from this world and this character's experience. If not, give the literal detail. Avoid: 如同石子投入湖面，泛起层层涟漪.
7. Let the content set the count and the length: one exact item, not three parallel ones; sentences and paragraphs that change with the pace; a cut between paragraphs, not a connective. Avoid: 三连排比；段首"然而/与此同时".
8. Give each speaker their own words and aim. Lines can be incomplete, evasive, or off the point. Avoid: 说明书式对话.
9. Let a conflict cost something and stay open across scenes. Describe only what moves the scene. Avoid: 同一冲突同段解决.
10. Keep the dash and the one-sentence paragraph for a real interruption. When a habit repeats, rewrite the event; do not swap in a synonym.

Findings C2-C3 suggest testing a variant without the Chinese examples; the mechanism sentences carry each rule.

## E. Sources

(Stars: GitHub public API, 2026-11-21.)

- https://www.pnas.org/doi/10.1073/pnas.2422455122 — Reinhart et al. 2025. Peer-reviewed; English, not fiction. Found: instruction-tuned models overuse participial clauses (GPT-4o 5.3x human) and nominalizations, also when told to write informally.
- https://arxiv.org/abs/2409.14509 — LAMP, CHI 2025. Peer-reviewed; English literary fiction; 18 trained writers edited 1,057 paragraphs.
- https://arxiv.org/abs/2604.03136 — StoryScope; exists (v6). Preprint; 61,608 stories, 304 narrative features, five models.
- https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing — editor field guide; encyclopedic prose.
- https://zh.wikipedia.org/zh-cn/Wikipedia:AI生成文的特徵 — exists; information page, not policy.
- https://mp.weixin.qq.com/s/BUDe6HU_hIuCGKDD0uVp8w — RUC新闻坊; university data journalism; 142 小红书 notes; essay test.
- https://arxiv.org/abs/2507.11538; https://arxiv.org/abs/2601.08070; https://arxiv.org/abs/2510.15061 — preprints (instruction count; negative constraints; Antislop).
- https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices; https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/prompting-claude-fable-5-1 — Anthropic, official.
- https://help.openai.com/en/articles/6654000-best-practices-for-prompt-engineering-with-the-openai-api; https://developers.openai.com/cookbook/examples/gpt4-1_prompting_guide; https://developers.openai.com/cookbook/examples/gpt-5/gpt-5_prompting_guide — OpenAI, official.
- https://aistudio.google.com/docs/prompting-strategies — Google, official.
- https://github.com/zenstory-ai/oh-story-claudecode (8,800 stars) — tool; most detailed fiction list; self-reported calibration.
- https://github.com/lingfengQAQ/webnovel-writer (8,039) — tool; 200+ listed phrases.
- Smaller tools, github.com/: ExplosiveCoderflome/AI-Novel-Writing-Assistant (3,515); Raymondhou0917/speak-human-tw (1,193); modoojunko/awesome-novel-agent (885); wordflowlab/novel-writer-skills (273); akira399/dsh-novel-writer (120); tance-mang/chinese-webnovel-skills (110); hestudy/snowflake-fiction (37).
- https://m.thepaper.cn/newsDetail_forward_30189980 — trade media; three web-novel editors.
- https://www.chinawriter.com.cn/n1/2025/0905/c404027-40557712.html — scholar's journal article; reader-listed tells.
- https://www.huxiu.com/article/4887016.html — tech media; StoryScope summary.
- Community posts: https://ivisi.cc/post/2026-10-01-ai-novel-deslop (blog); https://pwa.sspai.com/post/115346 and https://sspai.com/post/109288 (少数派); https://zhuanlan.zhihu.com/p/2073463344644764879 (知乎); https://www.v2ex.com/t/1226775; https://www.douban.com/group/topic/321889503/; https://linux.do/t/topic/2199661; https://bbs.jjwxc.net/showmsg.php?board=4&id=324229.
- Lower-trust articles: https://www.woshipm.com/ai/6316640.html; https://m.thepaper.cn/newsDetail_forward_30934476; https://www.163.com/dy/article/L6ABBB3R0521DVI5.html; https://maliangwriter.com/blog/ai-web-novel-template-voice-fix/ (vendor).

## F. Could not verify

- Google's sentence that pattern examples work better than anti-pattern examples: absent from the live page; seen only in mirrors.
- A Gemini warning about broad negative constraints: unsourced.
- The em-dash study that en Wikipedia cites, the Pangram 3x figure, and StoryScope's venue: secondary only. StoryScope's English scope comes from the 虎嗅 summary.
- The sspai test numbers and the oh-story calibration: self-reported; the tested repository was not opened; expository text.
- Chinese academic items (Zhou & Liu 2024; Wu et al. 2025; Chang & Lv 2024; arXiv 2603.14430): abstracts or reposts only.
- LAMP and StoryScope were read to 100,000 characters. All pages came through a summarising fetch tool.
- Not opened: any 小红书, B站专栏, 龙的天空, or 起点 post; the contents of op7418/Humanizer-zh (23,450 stars), blader/humanizer (71,715), Narcooo/inkos (11,374), KKKKhazix/human-writing (4,684).
- No source read names 喉结滚动, 与其说……不如说……, or the exact 而这，仅仅是开始 (variants: 3).
