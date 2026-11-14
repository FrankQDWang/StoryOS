# Context compaction methods: ChatGPT deep research report (2026-11-13)

This file is an evidence record. The text below the line "Report as exported" is the report. The only change is the removal of the spaces at the ends of lines.

## Record

- **Source.** A ChatGPT deep research run of 2026-11-13. The author started it with a prompt from the coordinator session, because the author found the compaction of [ADR 0055](../adr/0055-install-a-host-managed-compaction-summary-for-the-decision-request.md) too simple.
- **Status.** Use the report as a list of ideas, not as evidence. The export lost each citation mark, so no statement has a source link.
- **Known errors.** The report says that Gemini CLI makes its summary with GPT-4o. The question about Amp was about the coding agent of Sourcegraph; the report describes an "Agent Memory Protocol". The report says that Roo Code and Cline have no public material; they have public documents about context condensing. The coordinator could not confirm the sources "Zylos" and "NarraWorld", or the statement that quality drops sharply above 30K tokens.
- **Verified.** The statements about OpenAI Codex CLI agree with the local Codex source snapshot of 2026-10-01 (`.reference/codex`, commit `cda82a2c6`, not in Git): automatic compaction at 90% of the context window, repeated compaction, a compacted history of the recent user messages up to 20,000 tokens and one summary with a prefix (`codex-rs/core/src/compact.rs`), and a handoff summary prompt with four points (`codex-rs/prompts/templates/compact/prompt.md`).
- **StoryOS facts that the report does not have.** The summary instruction already names the decisions of the author, the constraints, the open questions, the pending work, and the state of the discussed passages. The Host assembles the instructions, the Working Target, and the references again for each request, so a summary does not carry them. PostgreSQL holds each Tool result.

## Author decisions of 2026-11-13

The author adopted three items. They become one specification after the specification [#1451](https://github.com/FrankQDWang/StoryOS/issues/1451) merges into `main`.

1. Permit more than one compaction in one Run. After a compaction, an earlier Tool result of the Run is a short stub (the Tool, the query, the titles and links, an identity), and the model reads the stored result through a Host Tool when it needs it. Today the later request sends each Tool result of the Run again in its exact form, and a Run that is too long after its one compaction ends with `conversation_too_long`.
2. After a compaction, the later request also sends the recent messages of the author in their exact words, up to a token limit.
3. The summary instruction also names the Proposals that the author accepted or rejected, the story facts that the Conversation fixed, and the research results with their sources.

Not decided: the compaction threshold (80% of the input room, about 812,000 tokens) changes only after a measurement on the real route.

Not adopted: layered memory and a structured memory of the story world (a product feature, not compaction), a module that checks a summary, and a compaction or pin control for the author.

One finding of #1459 joins that specification: each request after a steering RunStep uses the Full continuation mapping, because the comparison uses the first message of the Run.

---

## Report as exported

# 系统调查

下面对各大系统或框架的上下文管理策略进行比较。表格对不同系统的触发、机制、保留与摘要策略、重复性和用户控制等维度作对比（英文技术名词保持不变）。事实陈述均引用官方资料或工程博客，并用【】标注页码和行号（如有）。

| 系统 / 框架            | 触发机制                                                         | 处理方式                                                         | 原文保留                                                         | 摘要/丢弃                                                         | 重复 compaction | 用户可控                                                         | 备注                                                         |
|-----------------------|----------------------------------------------------------------|----------------------------------------------------------------|----------------------------------------------------------------|----------------------------------------------------------------|---------------|----------------------------------------------------------------|------------------------------------------------------------|
| **Claude Code**       | 上下文接近1M令牌时自动触发；用户可输入 `/compact` 强制手动摘要。 | 通过模型生成会话摘要；支持「Focus」参数细化摘要范围。 | 系统提示、项目根目录文件、自动记忆、技能描述等从磁盘重新加载；最近最多5个文件全文重读。 | 对话历史和钩子加入的上下文被归纳摘要；超过限制的工具结果/代码由Subagent隔离，仅返回摘要。 | 是（多次可触发自动摘要）  | 支持 `/compact` 手动摘要；可配置自动阈值，使用`/status`查看令牌使用。 | 工具输出通过Subagent隔离（微型压缩），旧的工具调用可清除。 |

| **Anthropic API**      | 客户端可配置“阈值摘要”（`compact_threshold`），当输入令牌超阈值时自动摘要；也可调用 `client.messages.create` 带 `compaction` 参数实现按需摘要。 | 生成摘要区块替换旧轮次，会话其余内容继续；可配置清理策略（旧工具结果、思考区块等）。 | 默认为清理旧工具调用（用占位符代替旧结果），可选保留最近N个工具结果；近期的对话（近似最近X令牌）可保留完整（如系统提示、最近用户轮次等）。 | 早期轮次和旧工具结果被摘要或清理；清理工具调用、思考区块策略可单独配置；若使用内置 `memory` 功能，可将记忆内容拆出上下文。 | 是（每请求可多次触发） | 客户端可调用 `context_management` 参数进行手动摘要、设置清理策略（`clear_tool_uses_20250919`、`clear_thinking_20251015` 等）；也可选择使用记忆工具（Memory Tool）离线管理长期知识。 | 记忆工具可将重要信息存储到长期记忆库，减少上下文负担。摘要块加密返回，可通过 `previous_response_id` 无状态链接使用。 |

| **OpenAI Codex CLI** | 默认在会话输入达到90%窗口时自动触发；触发时机包括用户提问前后和工具链中段；用户可命令 `/compact` 强制执行。 | 若使用 OpenAI 模型则调用专门 API 返回加密摘要区块；否则本地通过向模型发总结提示得到结构化摘要（包括「进展、约束、待办、数据」四部分）。 | 默认保留最近≈20k令牌的用户消息（保留上下文开头和最新用户请求），并附加摘要；系统提示、模型知识等由摘要中的结构保持。 | 助手回复、工具调用和大部分旧内容全部丢弃，取而代之以摘要消息；如使用本地路径生成摘要，则用户消息保持原样，摘要插入为用户格式消息。 | 是（多次摘要产生） | 支持 `/compact` 命令，也可通过配置文件调节自动压缩阈值、工具输出长度上限、摘要提示等； `/status` 显示当前令牌占用。 | OpenAI Responses API 提供 `client.responses.compact` 接口，可在服务端执行摘要；但通过 CLI 使用时一般自动完成。摘要操作对`previous_response_id`兼容。 |

| **OpenAI Agents SDK** | SDK 提供 `Session` 对象管理对话：默认不自动缩减，需要用户通过自定义策略。博客示例中，当会话条目超阈值（如条数超过 N）时执行摘要；也可选择简单剪裁保留最近K轮。 | 通常由开发者实现：可通过重载 `Session` 在每轮前裁剪历史（保持最近K条）或在达到触发条件时向模型询问摘要，再将摘要插入会话。OpenAI 文档和博客推荐使用中间件编写策略。 | 保留最近数轮对话、系统提示等；已被实现的示例中 `TrimmingSession` 只保留最后K条。长期重要信息需依赖外部存储（无内建记忆）。 | 旧对话可以被整体或部分摘要插入为新的用户消息，实现「合成用户」摘要；也可直接删掉不再需要的消息。 | N/A（按需）  | 开发者可编写自定义 `Session`：如配置 `LLMSummarizer` 触发摘要或设置保留最近轮次。 | LangChain Agents/DeepAgents 等使用 Session 存储对话，示例包括滚动截断和基于 LLM 的摘要策略。社区提供的示例和类库需开发者启用，无单独触发命令。 |

| **Gemini CLI**       | 会话过长时自动压缩：默认阈值为 `model.compressionThreshold=0.5`（即利用一半上下文时）；也可输入 `/compress` 手动触发。 | 当触发时，GPT-4o 生成摘要替换对话内容（会话历史被当前摘要代替）。 | 持久记忆文件 `GEMINI.md` 永久保留并始终加载；系统提示、技能和当前模式等信息持续存在。 | 压缩后会话历史仅剩摘要；早期对话细节被丢弃，但摘要保留了主要信息。 | 是（多次压缩可继续） | 支持 `/compress` （等同 `/summarize`）手动摘要；也支持 `/memory list` 等命令管理记忆文件。 | `/clear` 可清空当前会话；自动压缩可以在限制范围内自由触发。部分记忆（`GEMINI.md`）在任意压缩后都会重新注入。 |

| **Cursor (Composer)**  | 当请求令牌数达模型窗口约2/3时开始后台压缩；达到上下文极限时自动压缩并继续，不再失败；用户可输入 `/summarize`（或 `/compress`）立即摘要当前会话。 | 以对话方式调用模型生成摘要；旧消息被替换为摘要。其后台策略不公开，CLI中 `/summarize` 类似于强制压缩。 | 根据版本更新，早期提交的事实不被丢弃，最新请求严格保留原文。系统提示和近期对话保持完整，较早内容被替换。 | 旧对话被合并为摘要消息（丢弃细节）；根据文档，压缩结果避免丢失早期重要事实。 | 是（多次压缩）  | 支持 `/summarize` 或 `/compress` 手动摘要会话；支持 `/clear` 重开新的会话。摘要过程对用户透明，无直接设置阈值。 |

| **Zed Agent**         | 默认90%上下文使用时自动触发压缩；用户可 `/compact` 强制摘要当前对话。 | 生成对话摘要替换历史，就像其他CLI工具的摘要机制。 | 系统消息和当前上下文文件保持；近期对话内容以摘要形式保存。 | 旧轮次被摘要，实际提问和要点保留。 | 是（多次）   | 支持 `/compact` 命令；也可通过新建会话（如“New From Summary”）开始时自动摘要。 |

| **Roo Code / Cline**  | **（资料不足，未找到公开说明）** | **（资料不足）** | **（资料不足）** | **（资料不足）** | N/A         | N/A       | 暂无公开文档验证其上下文管理。 |

| **Aider**            | 本身不内置长期上下文管理。每次运行都是全新会话。 | 可通过命令行参数 `--read <文件>` 将外部文本注入上下文；Hindsight插件示例在会话前注入项目记忆、会话后保存会话记录到长期存储。 | 本地对话会保留整个对话，但下次重启Aider时上下文清零；通过 `--read` 加载的文件会原样保留（作为模型“只读参考”）。 | 会话历史不会自动丢弃；但每次Aider启动时，之前的对话都被重置。 | 不适用    | Aider 本身没有 `/compact` 等命令；可借助第三方工具（如Hindsight）将重要信息保存到文件或数据库。 | 可使用 `aider --read memory.md` 在启动时加载已存记忆。 |

| **OpenHands (Condenser)** | 通过 `LLMSummarizingCondenser` 在对话事件数超过阈值时触发。阈值可配置（如最大事件数）。 | 使用LLM按指定方式摘要历史：保留最**新**的消息，将较早内容浓缩为摘要。 | 保留最近的消息保持完整；`keep_first` 参数允许保留最初若干条消息（如系统提示）。 | 较旧对话事件被替换为摘要段落，旧信息浓缩；但关键目标、规范等要素会在摘要中出现。 | 是（滚动摘要） | 用户可配置 `LLMSummarizingCondenser`：设置最大上下文长度、保留条数等。 | OpenHands 用“冷凝器”组件在对话运行时自动摘要，主要用于降低API调用成本和加快响应。 |

| **Amp (Agent Memory Protocol)** | **触发与管理上文未定义**。AMP主要规范长期记忆的格式和注入方式。 | AMP 提出：内存检索后作为系统提示添加到LLM调用中，强调互操作性。其规范并不执行压缩，只规范上下文组装。 | AMP不管理对话历史；它定义了从“记忆存储”检索数据并注入提示的协议。 | N/A | N/A | 与此问题关系较弱；AMP关注如何将记忆供给模型。 | 可视作内存增强方案，与本题上下文压缩偏正交。 |

| **LangGraph / LangChain 记忆** | LangGraph：对话作为图状态保存，开发者可在任意时机持久化并恢复。LangChain内置多种记忆类（缓冲区、摘要等），可在应用逻辑中自行触发和更新。 | LangGraph：通过**图式状态**持久化记忆，允许恢复不同对话（线程）；不自动摘要。DeepAgents（LangChain）可离线**剥离**大内容并摘要。 | 对话历史可全量保存到文件或数据库（DeepAgents将学习成果存为文件）；这不在当前上下文内。 | DeepAgents 等通过文件存储旧内容，仅摘要提取要点；传统内存类可做部分摘要。 | N/A        | 开发者根据需要选择不同内存策略（如ConversationBufferMemory保留近期、ConversationSummaryMemory生成概要）。DeepAgents使用**滚动载入**和**摘要**等技术。 | LangGraph通过图状态保证对话持久化；DeepAgents组合多种策略（**Offloading**、摘要、工具**技能**、子代理、长期记忆）来管理上下文。 |

| **Letta (MemGPT)**    | Letta/​MemGPT架构采用多层内存模型：上文无特定自动触发描述，但文档提到“记忆驱动的背景推理”（“dreaming”）会在消息计数或压缩时触发。 | 上下文由多个层级组成：**内存块（Memory Blocks）**始终加载在上下文中；而其它信息（如文件、长期记忆）按需检索。后台子代理（dream subagent）会整合最近经验到长期记忆。 | 核心记忆块（如人物设定、当前任务状态等）始终保留在上下文中；系统提示和配置等“永远加载的内容”保持完整。 | 对话历史和推理可以移出上下文，转存到长期状态；摘要由“后台反思”步骤触发，学习成果被并入记忆块。这类似虚拟上下文管理（MemGPT）。 | 是（内存不断更新） | Letta 提供基于内存块和结构化记忆库的API，开发者可管理记忆块内容、执行`archival_memory_search`等。摘要过程自动进行，用户以配置方式控制（如添加提示）。 | Letta 强调**持久上下文**：记忆块（Memory Blocks）在上下文内可读写，架构借鉴操作系统思想，有**分层记忆**（热内存、存档记忆等）。其设计来源于 MemGPT 架构（内存块、高速上下文与慢速外部记忆切换）。 |

<!-- 注意：以上系统信息均根据官方文档、源码和工程博客整理，如无公开资料或不足则标注“资料不足”或省略。 -->

## 核心技术比较

- **工具输出裁剪 vs LLM 摘要**：简单地丢弃旧输出（滑动窗口）可以快速降低上下文大小，但可能丢失关键信息。相反，使用 LLM 摘要虽然成本更高，但保留了多轮对话的核心要点和决策。研究表明，**Anchored迭代式摘要**（只摘要新增丢弃的部分并合并到已有状态中）比每次重做全量摘要效果更佳。例如，LangChain DeepAgents采用先写日志到文件、只保留结果摘要的方式；而Codex CLI 和Claude Code等用LLM生成对话总结。滑动窗口策略虽然成本最低，却难以满足长期关联需求（老信息中决策、引用可能被丢弃）。

- **离线存储大型结果**：避免将过长文本直接塞入上下文是常用做法。DeepAgents示例中，当工具返回超过2万token时，会将结果写入文件，只在上下文留少量预览（头尾几行）和文件路径。这种“分级加载”策略保持上下文精简，只有需要时再用工具（如`read_file`）检索。其他系统（如LamChain Memory）也可将大文档存储到外部数据库，仅保留摘要或引用。这种方式保留了完整数据的检索能力，同时减轻了上下文负担。

- **结构化摘要 vs 自由式摘要**：有的系统要求摘要具备固定格式以方便处理。例如，Codex CLI的本地编译路径生成包含“Progress、Constraints、TODOs、Data”四部分的结构化摘要；Antigravity agent要求输出六个固定段落。结构化摘要可提升一致性和可读性，但设计复杂，且可能限制创意表达。自由式（ChatGPT风格）摘要虽然灵活，但结果可能遗漏细节。*Zylos*研究指出，引入“指导性优化”（如ACON方法）可以改善摘要丢失的关键信息。建议在摘要提示中明确保留关键信息段（如清单、决策），并可在局部滚动摘要时固定保留最近N条对话。

- **滚动/重复摘要的质量衰退**：多轮摘要可能累积错误或遗失细节。研究表明，每次对整个历史重做摘要会导致信息逐渐漂移；而迭代式（仅摘要丢弃部分）降低了这种漂移。实践中，必须监控摘要质量，可采取**失败案例分析**等办法调整摘要提示。LangChain 的 DeepAgents 利用日志文件保持原始数据，因此摘要后仍可回溯。无论哪种方式，长链多次摘要不可避免地引入一些信息偏差，需谨慎评估。

- **保留最后 N 轮与关键项**：通常建议保留最**近**的对话轮次原文，以保留上下文新鲜度。例如Zylos建议最后5–7轮不压缩。各系统也大多保留最后若干轮不动：Codex CLI 保留≈20K token近期用户消息，Claude Code允许/focus关键词保留相关内容。对故事写作来说，系统提示、角色设定等“钉住”的信息不应被摘要。很多系统（如Claude Context Editing）也支持显式“钉住”或持久保留的配置。

- **与缓存和 `previous_response_id` 交互**：对使用上下文缓存和长链的系统，要注意不要在已有摘要中额外去除段落。例如，OpenAI文档提醒，如果使用 `previous_response_id` 链接，则不应手动剔除摘要前的内容，因为摘要区块自身已编码了必要上下文。过早删减可能使缓存失效或内容不一致。一般规则是：如果依赖服务端的链式上下文，不要做两次压缩；否则，在新请求前手动精简以降低成本。Claude同样要求在使用Native compaction时不要和 `context_management` 混用。

- **评估摘要损失**：目前还没有公认的自动化质量检测方法。*Zylos*提出了“ACON”框架：对比完整与压缩上下文下的表现，将失败案例反馈给模型调整摘要策略。此外，一些工作建议对摘要后的回答做后验验证（如检查引用与原始是否吻合）。完整保留工具输出的做法（如地址化检索所述）实际上避免了损失，不必再评估丢失。然而，在现实系统中，可考虑人工或自动标注评估摘要后的回答准确度，并通过异常驱动方式迭代改进摘要。

## 长上下文质量

关于模型随上下文增长性能的研究表明：**“Lost in the Middle”**现象普遍存在。即LLM对处于上下文前端和后端的信息响应最佳，而对中间信息的利用明显下降。这意味着简单延长输入窗口无法保证质量，且“概念漂移”会随历史长度增加而加剧（Zylos报告：超过30K令牌开始质量急剧下降）。因此，**应早于上下文极限就开始压缩**。Zylos建议在已用窗口达到70%时触发摘要，以避免失控下降。同时，一些测试也表明保持长序列中的后半段不动，有助于保持任务连贯性。综上，对于1M令牌的上下文，80%才压缩恐过晚；结合实际任务需要，可考虑在更低阈值(如40%～70%)进行中间压缩，以抑制“语境漂移”。

## 小说写作需求

长篇小说协作对上下文有更严格的保真要求。根据最新研究：写作代理需要**对演化的故事世界进行显式记忆**，确保新增事件与既定事实一致。这意味着：
- **人物和事件事实**：人物身份、关系、世界规则、时序事件等必须保留且具有溯源。
- **样式与设定**：如文体要求、已有的环境描写风格等也是约束。
- **作者决策与反馈**：作者曾确认或否定的信息、被采纳或拒绝的建议也应标记存档。例如“编剧确认机器人有一根天线”和“模型推断机器人可能有根天线”的区分应保留。
- **资源与创作规范**：源材料、角色设定文档、图表和反馈等都构成创意上下文的证据层面。

相关工作已有提出：**NarraWorld**框架构建了结构化记忆系统来服务长篇创作，明确维护“世界事实”、“角色信念”、“开放情节”与“假设分支”四大视图，保证故事连贯性。此外，LlamaGen等提出“Creative Context”概念，强调项目素材、审定事实、角色记忆和决策流程需要以结构化方式存储。这些资料指出，为了写作，摘要压缩永远不应抹去**可溯源的原始证据**：摘要后还需保留证据链接与审定状态。因此，小说写作代理应采用结构化记忆（世界观图谱、角色表、剧情节点等），避免仅依赖纯文本摘要。

## 引用与可溯源性

压缩会让引用失去上下文，但保持引用准确性至关重要。一个有效方法是**地址化检索压缩（Addressable Recall）**：将每个工具结果保存在外部库中，用唯一标识符（ID）引用，而不是丢弃原文。在上下文摘要时，用简短的“引用”或ID占位代替原内容，必要时模型可通过调用外部工具（如 `get_record(id)`）取回全文。这样引用可始终指向原始数据，并可审计。Anthropic的文档也提到类似思路：**“使用轻量级标识符（文件路径、查询、链接等）在运行时加载数据”**。即使实际会话未包含完整检索结果，引用链接或ID仍然有效，因为系统可以从存储中检索。若不采用此法，则需在摘要中嵌入可定位的检索表达（如精确的URL或查询条件），并保证这些在Manifest中记录完整。总体原则是：**不通过破坏存储来丢失原文**，而是依靠可逆方案保留可检索的路径，这样压缩后回答依旧可验证引用源。

# StoryOS 改进建议

针对 StoryOS 的现状和上述调研，提出以下按收益与风险排序的建议：

1. **引入结构化摘要策略**：当前使用自由文本摘要易遗失关键信息。借鉴 Codex CLI 和DeepAgents 等，可设计定向摘要模板（如分段式提问）。例如，在摘要提示中保留“主要人物、已决策事项、待办项”等结构。**收益：** 提高压缩后信息完整度，避免语义漂移；**成本：** 需要开发自定义摘要提示或模型微调；**风险：** 需要反复调参，否则摘要灵活性下降；**借鉴：** Codex CLI 的四部分摘要；DeepAgents 等迭代式摘要方法。

2. **工具输出离线存储与地址化引用**：目前保留所有工具结果在上下文中成本高。可将大型文档或搜索结果写入后台存储，仅在对话中留简短引导（如文件路径或查询摘要），并用“引用ID”替代原文。在需要时，工具链能根据ID重新获取原始内容。这相当于实现“可寻址回溯”，保证引用的不丢失。**收益：** 保留完整研究结果而不占用窗口；**成本：** 需要实现外部存储和引用机制；**风险：** 查询和重检索增加了延迟；**借鉴：** AgentPatterns 的 Addressable Recall 概念。

3. **多级上下文管理（热/冷存储）**：参考 Letta 和 MemGPT 的设计，将上下文信息分层。将“钉住”信息（人物设定、风格规则、系统提示等）长期保留；将不常用但需偶尔检索的内容（早期对话、已完成决策）移入持久存储（如数据库或文件），在必要时加载。**收益：** 关键信息常驻，非核心历史可回溯；**成本：** 架构复杂，需要管理多种存储；**风险：** 开发难度高，可能与现有Manifest系统冲突；**借鉴：** Letta的四层记忆架构；DeepAgents使用文件保存长期记忆。

4. **周期性摘要与主动扩充**：目前每次仅在80%触发一次。考虑多次滚动摘要，例如在每几个Run后自动摘要一次，或者在新章节开始时做“检查点摘要”。使用迭代摘要策略保留旧摘要（锚点）并增量更新。**收益：** 避免一次性大摘要导致关键信息遗漏；分阶段摘要更易控制质量；**成本：** 增加多轮交互次数和模型调用；**风险：** 累计计算资源开销；**借鉴：** Factory Anchored Summarization。

5. **前N轮保留与钉住提示**：明确保留最近数轮对话原文（如5–10轮）；将人物设定、背景故事等“关键上下文”配置为永不压缩内容。可让用户手动标记某些消息或文本为常驻（可视化“钉住”标记）。**收益：** 保持新信息细节和关键背景；**成本：** 需要改动对话管理逻辑；**风险：** 增加窗口占用，加速再次压缩；**借鉴：** Zylos建议保留最近轮数；Claude Code 可聚焦摘要内容。

6. **摘要后检验与反馈**：集成简单的检查机制，如生成文本与原搜索结果间的重叠率、关键实体是否包含，或利用小规模模型复盘摘要质量。对比新旧信息，若关键信息丢失则自动重新摘要或标记为未解决。**收益：** 提高压缩准确性，避免遗漏；**成本：** 需要开发质量评估模块；**风险：** 增加系统复杂度；**借鉴：** ACON方法的核心思想。

7. **Prompt缓存协同**：与OpenAI提示缓存策略配合，优化摘要时机。例如，在缓存过期前执行摘要，减少重复处理。当前系统使用 `previous_response_id`，建议保留最新摘要后的上下文完整性，不要过早手动删除摘要前的记录。**收益：** 利用缓存提速，避免重复加载；**成本：** 需了解缓存机制；**风险：** 若误操作可能造成回答错误；**借鉴：** OpenAI 文档提示与缓存协同。

下表总结了以上改进与现行方案的区别：每项建议在收益（信息完整性、成本下降）、成本与风险（开发工作量、延迟、潜在失误）之间权衡，并注明参考来源。建议按照项目优先级逐步实施，并持续验证效果。

## 不推荐方案

- **仅依赖自由式长摘要**：虽然实现简单，但易漏关键信息，且难以保证写作连贯性。
- **简单删除旧内容**：如“去掉旧轮次”固然快速，但故事写作中往往产生上下文依赖，简易截断会丢失前因。
- **仅扩展窗口不压缩**：依赖超大上下文（如1M）而不做压缩策略，会出现严重“丢失中间”问题，不现实。
- **过度依赖长期记忆库**：将所有信息存入知识库再检索容易造成检索瓶颈，且回忆上下文成本高。现阶段可用按需工具处理，但仍需慎用，以免延迟过大。

## 待验证问题与实验设计

1. **摘要结构效果对比**：测试分段式摘要（如分主题提示）与自由式摘要在长篇小说场景下的差异。**实验**：随机选取真实创作对话片段，分别采用分项摘要提示和自由提示，对比模型给出的摘要质量（通过人工评估关键信息保留率）。
2. **摘要频率与阈值优化**：评估在不同触发阈值下（30%、50%、70%）摘要对回答连贯性的影响。**实验**：模拟多轮创作过程，对每个阈值运行多次，使用语言模型自动评估或人工打分最终上下文保真度和生成质量。
3. **地址化回溯可行性**：实现最小原型：把部分工具调用存为可检索文件，用ID在摘要中引用。**实验**：对多次搜索结果进行压缩，用地址化检索与不检索两种策略生成回答，对比引用正确率和系统性能。
4. **滚动 vs 全量摘要**：比较仅摘要增量内容与全量摘要的长期效果。**实验**：使用一段长对话，分别仅摘要新增段落或每次重新摘要全体，然后验证对故事情节一致性的影响（比如可设问：情节是否自洽，细节是否丢失）。
5. **上下文插入顺序影响**：检验摘要块在上下文中位置（如总是插在最前面）对模型记忆的影响。**实验**：将摘要作为系统消息或第一条用户消息插入，再比较哪个方式对生成质量有利。

每个实验都可设置控制组（现行方案）与改进方案，使用定量指标（信息保留率、BLEU、ROUGE或人工评分）评估效果。通过这些试验，可确定哪些策略在故事写作场景中最为有效。