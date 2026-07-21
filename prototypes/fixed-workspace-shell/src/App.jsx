import { useEffect, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowUp,
  BookOpen,
  CaretDown,
  CaretRight,
  Check,
  CircleNotch,
  DotsThree,
  GearSix,
  List,
  Plus,
  SidebarSimple,
  Sparkle,
  Square,
  UserCircle,
  X,
} from "@phosphor-icons/react";

const volumes = [
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

const chapterTitles = Object.fromEntries(volumes.flatMap((volume) => volume.chapters));

const chapterCopy = {
  "chapter-12": [
    "雨下得很大，像天上有人把盆打翻在城外。",
    "青石巷被冲刷得发亮，檐角的水一滴一滴落在油纸伞上，发出细碎的响声。苏砚站在廊下，望着巷口渐深的夜色，迟迟没有迈步。",
    "他手里攥着一封信，纸边被雨气沾得微湿，字迹却依旧清晰。信是三日前送来的，只有一行字：子时，旧仓。等你。",
    "旧仓在城西，早年是官府的粮仓，如今半废，杂草丛生，少有人去。",
  ],
};

const proposalCopy = [
  "他本不该去。可那行字像一根细针，从这三日的每一个念头里反复穿过，扎得他夜不能寐。他想起那人最后一次回头时的眼神，像是把什么托付出去，又像是诀别。",
  "更鼓将尽，苏砚收起信，撑伞往西。",
  "风从城墙的裂缝里钻出来，带着湿冷的土腥味。远处的钟楼在雨里模糊不清，偶尔传来一声闷响，像是夜的呼吸。",
  "旧仓的门半掩着，门轴锈得厉害，推开时发出长长的吱呀声。仓内一片漆黑，只有梁上的雨水从破洞滴下，滴在地上，汇成浅浅的水洼。",
  "苏砚踏进去，伞尖在地面点了点，水珠四溅。",
  "黑暗里有脚步声，缓慢而沉稳。",
  "“你来了。”一个低沉的声音在前方响起。",
  "苏砚没有立刻回答，只是闭了闭眼，再睁开时，目光已沉静如水。",
];

const initialMessages = [
  {
    id: "author-1",
    role: "author",
    time: "10:18",
    text: "请在“雨夜”章节后半段，补写苏砚前往旧仓的过程与氛围，保持克制、含蓄的叙述风格。",
  },
  {
    id: "assistant-1",
    role: "assistant",
    time: "10:19",
    text: "已为你补写这段过程与氛围。提案已经放在正文对应位置，你可以直接修改，再决定接受或拒绝。",
  },
];

function now() {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date());
}

function ProjectMenu({ onClose, onOpenSettings, onOpenEval }) {
  return (
    <div className="project-menu" role="menu">
      <button type="button" role="menuitem" onClick={onClose}>写作工作区</button>
      <button type="button" role="menuitem" onClick={onOpenEval}>Eval Studio</button>
      <div className="menu-divider" />
      <button type="button" role="menuitem" onClick={onOpenSettings}>项目设置</button>
    </div>
  );
}

function TreePanel({ activeChapter, expanded, onToggle, onSelect, onOpenSettings, onOpenEval }) {
  const [menuOpen, setMenuOpen] = useState(false);
  return (
    <aside className="tree-panel" aria-label="章节目录">
      <header className="project-header">
        <button
          type="button"
          className="project-identity"
          onClick={() => setMenuOpen((value) => !value)}
          aria-expanded={menuOpen}
        >
          <BookOpen size={20} />
          <span>雾尽时与月同归</span>
          <CaretDown size={13} />
        </button>
        <button type="button" className="icon-button" aria-label="项目菜单">
          <List size={21} />
        </button>
        {menuOpen && (
          <ProjectMenu
            onClose={() => setMenuOpen(false)}
            onOpenSettings={() => {
              setMenuOpen(false);
              onOpenSettings();
            }}
            onOpenEval={() => {
              setMenuOpen(false);
              onOpenEval();
            }}
          />
        )}
      </header>

      <div className="tree-heading">
        <span>目录</span>
        <button type="button" aria-label="添加章节"><Plus size={18} /></button>
      </div>

      <nav className="manuscript-tree">
        {volumes.map((volume) => {
          const isExpanded = expanded.has(volume.id);
          return (
            <section key={volume.id} className="volume-group">
              <button
                type="button"
                className="volume-row"
                onClick={() => onToggle(volume.id)}
                aria-expanded={isExpanded}
              >
                {isExpanded ? <CaretDown size={14} /> : <CaretRight size={14} />}
                {volume.title}
              </button>
              {isExpanded && (
                <div className="chapter-list">
                  {volume.chapters.map(([id, title]) => (
                    <button
                      key={id}
                      type="button"
                      className={`chapter-row ${activeChapter === id ? "active" : ""}`}
                      onClick={() => onSelect(id)}
                    >
                      {title}
                    </button>
                  ))}
                </div>
              )}
            </section>
          );
        })}
      </nav>

      <footer className="tree-footer">
        <div className="account-placeholder" aria-label="当前用户：作者">
          <UserCircle size={21} weight="fill" />
          <span>作者</span>
        </div>
        <button type="button" aria-label="项目设置" onClick={onOpenSettings}>
          <GearSix size={18} />
        </button>
      </footer>
    </aside>
  );
}

function Proposal({ status, onAccept, onReject }) {
  if (status === "rejected") return null;
  return (
    <>
      <div className={status === "accepted" ? "accepted-prose" : "proposal-surface"}>
        {proposalCopy.map((paragraph) => (
          <p key={paragraph} contentEditable suppressContentEditableWarning>{paragraph}</p>
        ))}
      </div>
      {status === "pending" && (
        <div className="proposal-actions">
          <button type="button" onClick={onAccept}><Check size={16} />接受</button>
          <button type="button" onClick={onReject}><X size={16} />拒绝</button>
        </div>
      )}
    </>
  );
}

function EditorPanel({ activeChapter, proposalStatus, onAccept, onReject }) {
  const title = chapterTitles[activeChapter] ?? "未命名章节";
  const paragraphs = chapterCopy[activeChapter] ?? [
    `${title.replace(/　/g, " ")}。`,
    "这一章仍在整理中。你可以在此直接写作，也可以在右侧告诉写作助手想继续推进的方向。",
  ];
  const showProposal = activeChapter === "chapter-12";
  return (
    <main className="editor-panel">
      <div className="editor-scroll">
        <article className="manuscript">
          <header className="chapter-heading"><h1>{title}</h1></header>
          <div className="manuscript-body" aria-label="小说正文编辑器">
            {paragraphs.map((paragraph) => (
              <p key={paragraph} contentEditable suppressContentEditableWarning>{paragraph}</p>
            ))}
            {showProposal && (
              <Proposal status={proposalStatus} onAccept={onAccept} onReject={onReject} />
            )}
          </div>
        </article>
      </div>
    </main>
  );
}

function RunSummary({ runState, expanded, onToggle }) {
  const runLabel = runState === "running"
    ? "正在读取当前章节"
    : runState === "paused"
      ? "已暂停 · 未写入正文"
      : "已完成 · r3";
  return (
    <article className={`run-summary ${runState === "running" ? "is-running" : ""}`}>
      {runState === "running" ? <CircleNotch size={17} className="spin" /> : <Sparkle size={17} weight="fill" />}
      <button type="button" className="run-copy" onClick={onToggle} aria-expanded={expanded}>
        <strong>写作建议 · 生成片段</strong>
        <span>{runLabel}</span>
      </button>
      <CaretDown size={15} />
      {expanded && (
        <div className="run-details">
          <span>读取了当前章节</span>
          <span>生成内容以 Proposal 放入编辑器</span>
          <span>没有直接改写权威正文</span>
        </div>
      )}
    </article>
  );
}

function AppView({ state, onRequest, onClose, onReopen }) {
  if (state === "fallback") {
    return (
      <article className="app-fallback">
        <Sparkle size={17} />
        <div>
          <strong>意象回声 · 已关闭</strong>
          <p>静态结果仍保留：雨、旧仓与信件在本章形成三次呼应。</p>
        </div>
        <button type="button" onClick={onReopen}>重新打开</button>
      </article>
    );
  }
  return (
    <article className="app-view">
      <header>
        <div>
          <span className="app-kicker">MCP APP</span>
          <strong>意象回声</strong>
        </div>
        <button type="button" aria-label="关闭 App View" onClick={onClose}><X size={16} /></button>
      </header>
      <p className="app-intro">本章中反复出现的意象，以及它们连接的正文位置。</p>
      <div className="echo-list">
        <button type="button"><span>雨</span><small>4 处</small></button>
        <button type="button"><span>信件</span><small>2 处</small></button>
        <button type="button"><span>旧仓</span><small>3 处</small></button>
      </div>
      <footer>
        <span>只读视图，不直接修改正文</span>
        <button type="button" onClick={onRequest} disabled={state === "requested"}>
          {state === "requested" ? "已交给写作助手" : "交给写作助手"}
        </button>
      </footer>
    </article>
  );
}

function AgentPanel({
  collapsed,
  onToggle,
  messages,
  runState,
  onPauseRun,
  onSend,
  onStartFromApp,
}) {
  const [draft, setDraft] = useState("");
  const [runExpanded, setRunExpanded] = useState(false);
  const [appState, setAppState] = useState("ready");
  const listRef = useRef(null);

  useEffect(() => {
    listRef.current?.scrollTo({ top: listRef.current.scrollHeight, behavior: "smooth" });
  }, [messages, runState, appState]);

  const submit = (event) => {
    event.preventDefault();
    if (!draft.trim()) return;
    onSend(draft.trim());
    setDraft("");
  };

  return (
    <>
      <aside className="agent-panel" aria-label={collapsed ? "写作助手已收起" : "写作助手对话"}>
        {!collapsed && (
          <>
            <header className="agent-header">
              <strong>写作助手</strong>
            </header>
            <div className="message-list" ref={listRef} aria-live="polite">
              {messages.map((message) => (
                <article key={message.id} className={`message ${message.role}`}>
                  <p>{message.text}</p>
                  <time>{message.time}</time>
                </article>
              ))}
              <RunSummary
                runState={runState}
                expanded={runExpanded}
                onToggle={() => setRunExpanded((value) => !value)}
              />
              <AppView
                state={appState}
                onClose={() => setAppState("fallback")}
                onReopen={() => setAppState("ready")}
                onRequest={() => {
                  setAppState("requested");
                  onStartFromApp();
                }}
              />
              {appState === "requested" && (
                <p className="mediated-note">App 请求已由 Host 转成新的写作助手任务。</p>
              )}
            </div>
            <div className="composer-dock">
              <form className="composer" onSubmit={submit}>
                <textarea
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  placeholder="告诉写作助手你想做什么…"
                  aria-label="给写作助手的消息"
                />
                <div className="composer-controls">
                  <button type="button" aria-label="添加上下文"><Plus size={16} /></button>
                  {runState === "running" && !draft.trim() ? (
                    <button type="button" className="send-button pause-button" aria-label="暂停当前运行" onClick={() => onPauseRun()}>
                      <Square size={11} weight="fill" />
                    </button>
                  ) : (
                    <button type="submit" className="send-button" aria-label="发送" disabled={!draft.trim()}>
                      <ArrowUp size={15} weight="bold" />
                    </button>
                  )}
                </div>
              </form>
            </div>
          </>
        )}
      </aside>
      <button
        type="button"
        className="assistant-toggle"
        onClick={onToggle}
        aria-controls="writing-assistant-panel"
        aria-expanded={!collapsed}
        aria-label={collapsed ? "展开写作助手" : "收起写作助手"}
      >
        <SidebarSimple size={20} weight="regular" />
      </button>
    </>
  );
}

function SettingsDialog({ value, onChange, onClose }) {
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section className="settings-dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title" onMouseDown={(event) => event.stopPropagation()}>
        <header>
          <div>
            <span>项目设置</span>
            <h2 id="settings-title">写作助手</h2>
          </div>
          <button type="button" aria-label="关闭设置" onClick={onClose}><X size={19} /></button>
        </header>
        <label htmlFor="project-instruction">项目说明 <small>可选</small></label>
        <textarea
          id="project-instruction"
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder="例如：语言克制，避免替作者预先规划后续情节。"
        />
        <p>留空时写作助手也完全可用。配置后，新开始的顶层任务会使用最新说明；当前正文不会因此改变。</p>
        <footer><button type="button" onClick={onClose}>完成</button></footer>
      </section>
    </div>
  );
}

function EvalPage({ onBack }) {
  return (
    <main className="eval-page">
      <header>
        <button type="button" onClick={onBack}><ArrowLeft size={17} />返回写作</button>
        <div><span>独立页面</span><h1>Eval Studio</h1></div>
        <button type="button" className="quiet-button"><DotsThree size={20} /></button>
      </header>
      <section className="eval-content">
        <div className="eval-intro">
          <h2>评估写作助手，不占用写作工作区</h2>
          <p>这里使用固定数据集和评分标准运行评估。它不会成为 Transcript App，也不会读取或改写当前编辑器状态。</p>
        </div>
        <div className="eval-table" role="table" aria-label="最近评估">
          <div className="eval-row eval-heading" role="row"><span>评估集</span><span>状态</span><span>结果</span></div>
          <div className="eval-row" role="row"><strong>克制叙事 · 12 例</strong><span>已完成</span><span>10 / 12</span></div>
          <div className="eval-row" role="row"><strong>Proposal 边界 · 8 例</strong><span>已完成</span><span>8 / 8</span></div>
          <div className="eval-row" role="row"><strong>长章节上下文 · 20 例</strong><span>草稿</span><span>—</span></div>
        </div>
      </section>
    </main>
  );
}

export function App() {
  const [view, setView] = useState("workspace");
  const [activeChapter, setActiveChapter] = useState("chapter-12");
  const [expanded, setExpanded] = useState(() => new Set(["volume-1", "volume-2"]));
  const [assistantCollapsed, setAssistantCollapsed] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [projectInstruction, setProjectInstruction] = useState("");
  const [proposalStatus, setProposalStatus] = useState("pending");
  const [runState, setRunState] = useState("complete");
  const [messages, setMessages] = useState(initialMessages);
  const runTimer = useRef(null);

  useEffect(() => () => window.clearTimeout(runTimer.current), []);

  const toggleVolume = (id) => {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const pauseRun = (message = "已暂停当前任务；尚未完成的内容没有进入正文。") => {
    window.clearTimeout(runTimer.current);
    setRunState("paused");
    setMessages((current) => [...current, { id: `assistant-${Date.now()}`, role: "assistant", time: now(), text: message }]);
  };

  const startRun = (authorText, source = "composer") => {
    window.clearTimeout(runTimer.current);
    setRunState("running");
    if (authorText) {
      setMessages((current) => [...current, { id: `author-${Date.now()}`, role: "author", time: now(), text: authorText }]);
    }
    runTimer.current = window.setTimeout(() => {
      setRunState("complete");
      setProposalStatus("pending");
      setMessages((current) => [
        ...current,
        {
          id: `assistant-${Date.now()}`,
          role: "assistant",
          time: now(),
          text: source === "app"
            ? "我已结合 App 的只读观察重新检查当前段落。新的修改仍作为正文 Proposal 出现在编辑器中。"
            : "我已处理你的请求。建议内容已经以 Proposal 放在正文对应位置，没有直接改写权威正文。",
        },
      ]);
    }, 6000);
  };

  if (view === "eval") return <EvalPage onBack={() => setView("workspace")} />;

  return (
    <div className={`workspace ${assistantCollapsed ? "assistant-is-collapsed" : ""}`}>
      <TreePanel
        activeChapter={activeChapter}
        expanded={expanded}
        onToggle={toggleVolume}
        onSelect={setActiveChapter}
        onOpenSettings={() => setSettingsOpen(true)}
        onOpenEval={() => setView("eval")}
      />
      <EditorPanel
        activeChapter={activeChapter}
        proposalStatus={proposalStatus}
        onAccept={() => setProposalStatus("accepted")}
        onReject={() => setProposalStatus("rejected")}
      />
      <AgentPanel
        collapsed={assistantCollapsed}
        onToggle={() => setAssistantCollapsed((value) => !value)}
        messages={messages}
        runState={runState}
        onPauseRun={pauseRun}
        onSend={(text) => startRun(text)}
        onStartFromApp={() => startRun("", "app")}
      />
      {settingsOpen && (
        <SettingsDialog
          value={projectInstruction}
          onChange={setProjectInstruction}
          onClose={() => setSettingsOpen(false)}
        />
      )}
    </div>
  );
}
