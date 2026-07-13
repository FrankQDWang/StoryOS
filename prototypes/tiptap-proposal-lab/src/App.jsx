import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import UniqueID from "@tiptap/extension-unique-id";
import {
  ArrowUp,
  BookOpen,
  CaretDown,
  CaretRight,
  Check,
  GearSix,
  List,
  MagnifyingGlass,
  PaperPlaneRight,
  Plus,
  Sparkle,
  SidebarSimple,
  X,
} from "@phosphor-icons/react";
import {
  chapterDocuments,
  chapterTitles,
  chapterTwelveDocument,
  initialMessages,
  initialProposal,
  proposalParagraphs,
  STORAGE_KEY,
  volumes,
} from "./data.js";
import {
  classifyTransaction,
  findProposalEnvelope,
  PROPOSAL_META,
  ProposalProjection,
  proposalPluginKey,
} from "./proposal-extension.js";

const STREAM_CHUNKS = [
  "可那行字像一根细针，",
  "从这三日的每一个念头里反复穿过，",
  "扎得他夜不能寐。",
  "他想起那人最后一次回头时的眼神。",
];

function loadPrototypeState() {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    return saved ? JSON.parse(saved) : null;
  } catch {
    return null;
  }
}

function timestamp() {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date());
}

function proposalStatusLabel(proposal) {
  if (proposal.resolution === "applied") return "已接受";
  if (proposal.resolution === "rejected") return "已拒绝";
  if (proposal.validation === "conflicted") return "需要重新规划";
  if (proposal.generation === "generating") return "正在生成";
  if (proposal.generation === "ready_partial") return "生成已暂停";
  if (proposal.validation === "pending") return "正在检查";
  return "已完成";
}

function replaceEditorDocument(editor, document, source) {
  const nextDocument = editor.schema.nodeFromJSON(document);
  const transaction = editor.state.tr
    .replaceWith(0, editor.state.doc.content.size, nextDocument.content)
    .setMeta(PROPOSAL_META, { allow: true, source })
    .setMeta("addToHistory", false);
  editor.view.dispatch(transaction);
}

function TreePanel({ activeChapter, expandedVolumes, onToggle, onSelect, onOpenLab }) {
  return (
    <aside className="tree-panel" aria-label="章节目录">
      <header className="project-header">
        <div className="project-identity">
          <BookOpen size={20} weight="regular" />
          <span>雾尽时与月同归</span>
          <CaretDown size={13} />
        </div>
        <List size={21} />
      </header>

      <div className="tree-heading">
        <span>目录</span>
        <button type="button" aria-label="添加章节">
          <Plus size={18} />
        </button>
      </div>

      <nav className="manuscript-tree">
        {volumes.map((volume) => {
          const expanded = expandedVolumes.has(volume.id);
          return (
            <section key={volume.id} className="volume-group">
              <button
                type="button"
                className="volume-row"
                onClick={() => onToggle(volume.id)}
                aria-expanded={expanded}
              >
                {expanded ? <CaretDown size={14} /> : <CaretRight size={14} />}
                <span>{volume.title}</span>
              </button>
              {expanded && (
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
        <button type="button" aria-label="搜索">
          <MagnifyingGlass size={20} />
        </button>
        <button
          type="button"
          aria-label="打开原型检查"
          data-testid="open-lab"
          onClick={onOpenLab}
        >
          <GearSix size={20} />
        </button>
      </footer>
    </aside>
  );
}

function ProposalActions({ proposal, onAccept, onReject, onCompletePartial }) {
  if (proposal.resolution !== "pending") return null;

  const canAccept =
    proposal.generation === "ready" && proposal.validation === "valid";

  return (
    <div className="proposal-actions" data-testid="proposal-actions">
      {proposal.generation === "ready_partial" && (
        <button type="button" onClick={onCompletePartial}>
          <Check size={16} />
          完成当前内容
        </button>
      )}
      <button
        type="button"
        onClick={onAccept}
        disabled={!canAccept}
        title={canAccept ? "接受这段提案" : proposalStatusLabel(proposal)}
      >
        <Check size={16} />
        接受
      </button>
      <button type="button" onClick={onReject}>
        <X size={16} />
        拒绝
      </button>
    </div>
  );
}

function AgentPanel({
  collapsed,
  messages,
  proposal,
  draft,
  onDraftChange,
  onSend,
  onCollapse,
  onUndoAcceptance,
  onReopenRejected,
}) {
  if (collapsed) {
    return (
      <aside className="agent-panel collapsed" aria-label="写作助手已收起">
        <button type="button" onClick={onCollapse} aria-label="展开写作助手">
          <SidebarSimple size={20} />
        </button>
      </aside>
    );
  }

  return (
    <aside className="agent-panel" aria-label="写作助手对话">
      <header className="agent-header">
        <strong>写作助手</strong>
        <button type="button" onClick={onCollapse} aria-label="收起写作助手">
          <SidebarSimple size={20} />
        </button>
      </header>

      <div className="message-list" aria-live="polite">
        {messages.map((message) => (
          <article key={message.id} className={`message ${message.role}`}>
            <p>{message.text}</p>
            <time>{message.time}</time>
          </article>
        ))}

        <article className="run-summary" data-testid="run-summary">
          <Sparkle size={17} weight="fill" />
          <div>
            <strong>写作建议 · 生成片段</strong>
            <span>{proposalStatusLabel(proposal)} · r{proposal.revision}</span>
          </div>
          <CaretDown size={15} />
        </article>

        {proposal.resolution === "applied" && (
          <button className="transcript-action" type="button" onClick={onUndoAcceptance}>
            撤销接受并重新打开提案
          </button>
        )}
        {proposal.resolution === "rejected" && (
          <button className="transcript-action" type="button" onClick={onReopenRejected}>
            重新打开刚才拒绝的提案
          </button>
        )}
      </div>

      <form className="composer" onSubmit={onSend}>
        <textarea
          value={draft}
          onChange={(event) => onDraftChange(event.target.value)}
          placeholder="告诉写作助手你想做什么…"
          aria-label="给写作助手的消息"
          rows={3}
        />
        <div className="composer-controls">
          <button type="button" aria-label="添加上下文">
            <Plus size={18} />
          </button>
          <button type="submit" className="send-button" aria-label="发送" disabled={!draft.trim()}>
            <ArrowUp size={17} weight="bold" />
          </button>
        </div>
      </form>
    </aside>
  );
}

function PrototypeLab({ proposal, onClose, onStream, onConflict, onReload, onReset }) {
  return (
    <div className="lab-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        className="prototype-lab"
        role="dialog"
        aria-modal="true"
        aria-label="原型检查"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header>
          <div>
            <span>一次性原型检查</span>
            <strong>Proposal 状态</strong>
          </div>
          <button type="button" onClick={onClose} aria-label="关闭原型检查">
            <X size={19} />
          </button>
        </header>

        <dl className="state-grid">
          <div><dt>revision</dt><dd>r{proposal.revision}</dd></div>
          <div><dt>generation</dt><dd>{proposal.generation}</dd></div>
          <div><dt>validation</dt><dd>{proposal.validation}</dd></div>
          <div><dt>resolution</dt><dd>{proposal.resolution}</dd></div>
          <div><dt>stream seq</dt><dd>{proposal.lastAppliedStreamSeq}</dd></div>
          <div><dt>pause fence</dt><dd>{proposal.pauseFence ?? "—"}</dd></div>
        </dl>

        <div className="lab-actions">
          <button type="button" onClick={onStream}>模拟 Agent 流式续写</button>
          <button type="button" onClick={onConflict}>模拟目标冲突</button>
          <button type="button" onClick={onReload}>重载并恢复</button>
          <button type="button" className="quiet" onClick={onReset}>重置原型</button>
        </div>

        <p>
          流式续写期间在提案内输入，RunWriteGate 会先同步关闭，晚到的 Agent
          片段不会进入编辑器。跨越正文与提案的替换会整体拒绝。
        </p>
      </section>
    </div>
  );
}

export function App() {
  const saved = useMemo(loadPrototypeState, []);
  const [activeChapter, setActiveChapter] = useState("chapter-12");
  const [expandedVolumes, setExpandedVolumes] = useState(
    () => new Set(["volume-1", "volume-2"]),
  );
  const [agentCollapsed, setAgentCollapsed] = useState(false);
  const [proposal, setProposal] = useState(saved?.proposal ?? initialProposal);
  const [messages, setMessages] = useState(saved?.messages ?? initialMessages);
  const [draft, setDraft] = useState("");
  const [labOpen, setLabOpen] = useState(false);
  const [notice, setNotice] = useState("");

  const proposalRef = useRef(proposal);
  const activeChapterRef = useRef(activeChapter);
  const runGateRef = useRef(false);
  const streamTimerRef = useRef(null);
  const rejectedDocumentRef = useRef(null);
  const chapterDocumentsRef = useRef({
    ...chapterDocuments,
    "chapter-12": saved?.document ?? chapterTwelveDocument,
  });

  proposalRef.current = proposal;
  activeChapterRef.current = activeChapter;

  const addAgentMessage = useCallback((text) => {
    setMessages((current) => [
      ...current,
      {
        id: `message-agent-${Date.now()}`,
        role: "agent",
        time: timestamp(),
        text,
      },
    ]);
  }, []);

  const stopStream = useCallback((signal) => {
    const current = proposalRef.current;
    if (current.generation !== "generating" || !runGateRef.current) return;

    runGateRef.current = false;
    if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    streamTimerRef.current = null;

    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      generation: "ready_partial",
      validation: "pending",
      pauseFence: previous.lastAppliedStreamSeq,
      creator: "author",
    }));
    setNotice("检测到作者输入，Agent 续写已暂停");
    addAgentMessage(
      `检测到你的输入，续写已在第 ${current.lastAppliedStreamSeq} 个片段后暂停；晚到内容不会写入正文。`,
    );
  }, [addAgentMessage]);

  const onRefusedTransaction = useCallback(() => {
    setNotice("这次编辑同时跨过正文与提案，已完整保留输入并拒绝应用");
  }, []);

  const proposalExtension = useMemo(
    () =>
      ProposalProjection.configure({
        initialBlockIds: proposalRef.current.blockIds,
        onAuthorSignal: stopStream,
        onRefusedTransaction,
      }),
    [onRefusedTransaction, stopStream],
  );

  const editor = useEditor({
    content: saved?.document ?? chapterTwelveDocument,
    extensions: [
      StarterKit.configure({ heading: false }),
      UniqueID.configure({
        types: ["paragraph"],
        attributeName: "blockId",
      }),
      proposalExtension,
    ],
    editorProps: {
      attributes: {
        class: "manuscript-prosemirror",
        spellcheck: "false",
        "aria-label": "小说正文编辑器",
      },
    },
    onTransaction: ({ transaction }) => {
      if (!transaction.docChanged) return;
      if (transaction.getMeta(PROPOSAL_META)?.allow) return;
      if (activeChapterRef.current !== "chapter-12") return;

      const ownership = classifyTransaction(
        transaction,
        transaction.before,
        proposalRef.current.blockIds,
      );
      if (ownership === "proposal") {
        setProposal((previous) => ({
          ...previous,
          revision: previous.revision + 1,
          validation: "pending",
          creator: "author",
        }));
      }
    },
  });

  useEffect(() => {
    if (!editor) return;
    const visibleIds =
      activeChapter === "chapter-12" && proposal.resolution === "pending"
        ? proposal.blockIds
        : [];
    const transaction = editor.state.tr
      .setMeta(proposalPluginKey, {
        type: "setProjection",
        blockIds: visibleIds,
      })
      .setMeta(PROPOSAL_META, { allow: true, source: "projection" })
      .setMeta("addToHistory", false);
    editor.view.dispatch(transaction);
  }, [activeChapter, editor, proposal.blockIds, proposal.resolution]);

  useEffect(() => {
    if (
      proposal.resolution !== "pending" ||
      proposal.generation !== "ready" ||
      proposal.validation !== "pending"
    ) {
      return undefined;
    }

    const timer = window.setTimeout(() => {
      setProposal((previous) =>
        previous.validation === "pending"
          ? { ...previous, validation: "valid" }
          : previous,
      );
    }, 500);
    return () => window.clearTimeout(timer);
  }, [proposal.generation, proposal.resolution, proposal.revision, proposal.validation]);

  useEffect(() => {
    if (!notice) return undefined;
    const timer = window.setTimeout(() => setNotice(""), 3600);
    return () => window.clearTimeout(timer);
  }, [notice]);

  useEffect(() => {
    if (!editor || activeChapter !== "chapter-12") return undefined;
    const timer = window.setTimeout(() => {
      const snapshot = {
        marker: "PROTOTYPE — safe to wipe",
        document: editor.getJSON(),
        proposal,
        messages,
      };
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(snapshot));
    }, 200);
    return () => window.clearTimeout(timer);
  }, [activeChapter, editor, messages, proposal]);

  useEffect(
    () => () => {
      if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    },
    [],
  );

  const toggleVolume = (volumeId) => {
    setExpandedVolumes((current) => {
      const next = new Set(current);
      if (next.has(volumeId)) next.delete(volumeId);
      else next.add(volumeId);
      return next;
    });
  };

  const selectChapter = (chapterId) => {
    if (!editor || chapterId === activeChapter) return;
    chapterDocumentsRef.current[activeChapter] = editor.getJSON();
    const nextDocument = chapterDocumentsRef.current[chapterId];
    replaceEditorDocument(editor, nextDocument, "chapter_switch");
    setActiveChapter(chapterId);
  };

  const acceptProposal = () => {
    if (
      proposal.generation !== "ready" ||
      proposal.validation !== "valid" ||
      proposal.resolution !== "pending"
    ) {
      setNotice("提案尚未满足接受条件");
      return;
    }
    setProposal((previous) => ({ ...previous, resolution: "applied" }));
    addAgentMessage("提案已写入当前章节。接受记录已保留，你仍可以安全撤销这次接受。");
    setNotice("已接受提案");
  };

  const undoAcceptance = () => {
    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      resolution: "pending",
      validation: "valid",
      creator: "author",
    }));
    addAgentMessage("已补偿刚才的接受，并用一个新修订重新打开提案；原接受记录没有被改写。");
    setNotice("已撤销接受并重新打开提案");
  };

  const rejectProposal = () => {
    if (!editor) return;
    const envelope = findProposalEnvelope(editor.state.doc, proposal.blockIds);
    if (!envelope) return;

    rejectedDocumentRef.current = editor.getJSON();
    const transaction = editor.state.tr
      .delete(envelope.from, envelope.to)
      .setMeta(PROPOSAL_META, { allow: true, source: "reject" });
    editor.view.dispatch(transaction);
    setProposal((previous) => ({ ...previous, resolution: "rejected" }));
    addAgentMessage("已拒绝这次提案，章节正文没有被改写。你可以从这里重新打开它。");
    setNotice("已拒绝提案");
  };

  const reopenRejected = () => {
    if (!editor || !rejectedDocumentRef.current) return;
    replaceEditorDocument(editor, rejectedDocumentRef.current, "reopen_rejected");
    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      resolution: "pending",
      generation: "ready",
      validation: "pending",
      creator: "author",
    }));
    addAgentMessage("已从被拒绝的历史内容创建新修订并重新打开，正在重新检查当前目标。");
  };

  const completePartial = () => {
    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      generation: "ready",
      validation: "pending",
    }));
    setNotice("当前部分已标记完成，正在检查");
  };

  const startStream = () => {
    if (!editor) return;
    if (activeChapter !== "chapter-12") selectChapter("chapter-12");
    if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);

    const envelope = findProposalEnvelope(editor.state.doc, proposal.blockIds);
    if (!envelope) {
      setNotice("请先重新打开提案，再测试流式续写");
      return;
    }

    const streamBlockId = "proposal-stream-1";
    const streamNode = editor.schema.nodes.paragraph.create(
      { blockId: streamBlockId },
      editor.schema.text("他本不该去。"),
    );
    const resetTransaction = editor.state.tr
      .replaceWith(envelope.from, envelope.to, streamNode)
      .setMeta(PROPOSAL_META, { allow: true, source: "agent_stream_reset" })
      .setMeta("addToHistory", false);
    editor.view.dispatch(resetTransaction);

    runGateRef.current = true;
    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      blockIds: [streamBlockId],
      generation: "generating",
      validation: "pending",
      resolution: "pending",
      lastAppliedStreamSeq: 0,
      pauseFence: null,
      creator: "agent",
    }));

    let index = 0;
    streamTimerRef.current = window.setInterval(() => {
      if (!runGateRef.current || !editor || index >= STREAM_CHUNKS.length) {
        window.clearInterval(streamTimerRef.current);
        streamTimerRef.current = null;
        if (runGateRef.current) {
          runGateRef.current = false;
          setProposal((previous) => ({
            ...previous,
            generation: "ready",
            validation: "valid",
          }));
        }
        return;
      }

      const currentEnvelope = findProposalEnvelope(editor.state.doc, [streamBlockId]);
      if (!currentEnvelope) return;
      const block = currentEnvelope.blocks[0];
      const insertAt = block.to - 1;
      const transaction = editor.state.tr
        .insertText(STREAM_CHUNKS[index], insertAt)
        .setMeta(PROPOSAL_META, { allow: true, source: "agent_stream" })
        .setMeta("addToHistory", false);
      editor.view.dispatch(transaction);
      index += 1;
      setProposal((previous) => ({
        ...previous,
        lastAppliedStreamSeq: index,
      }));
    }, 850);
  };

  const simulateConflict = () => {
    setProposal((previous) => ({
      ...previous,
      validation: "conflicted",
    }));
    setNotice("目标修订已变化，提案不能静默重基或接受");
  };

  const resetPrototype = () => {
    if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    streamTimerRef.current = null;
    runGateRef.current = false;
    window.localStorage.removeItem(STORAGE_KEY);
    if (editor) replaceEditorDocument(editor, chapterTwelveDocument, "prototype_reset");
    chapterDocumentsRef.current = { ...chapterDocuments };
    rejectedDocumentRef.current = null;
    setActiveChapter("chapter-12");
    setProposal(initialProposal);
    setMessages(initialMessages);
    setDraft("");
    setLabOpen(false);
    setNotice("原型已重置");
  };

  const sendMessage = (event) => {
    event.preventDefault();
    const text = draft.trim();
    if (!text) return;
    setMessages((current) => [
      ...current,
      {
        id: `message-author-${Date.now()}`,
        role: "author",
        time: timestamp(),
        text,
      },
    ]);
    setDraft("");

    window.setTimeout(() => {
      addAgentMessage(
        proposalRef.current.resolution === "pending"
          ? "我已经读到你的要求。当前提案仍在正文中，你可以直接继续修改，或先接受、拒绝后再让我生成新的版本。"
          : "我已经读到你的要求。你可以重新打开上一份提案，或在原型检查里开始一次新的流式续写。",
      );
    }, 550);
  };

  if (!editor) return null;

  return (
    <main className={`workspace ${agentCollapsed ? "agent-is-collapsed" : ""}`}>
      <TreePanel
        activeChapter={activeChapter}
        expandedVolumes={expandedVolumes}
        onToggle={toggleVolume}
        onSelect={selectChapter}
        onOpenLab={() => setLabOpen(true)}
      />

      <section className="editor-panel">
        <div className="editor-scroll">
          <article className="manuscript">
            <header className="chapter-heading">
              <h1>{chapterTitles[activeChapter]}</h1>
            </header>
            <EditorContent editor={editor} />
            {activeChapter === "chapter-12" && (
              <ProposalActions
                proposal={proposal}
                onAccept={acceptProposal}
                onReject={rejectProposal}
                onCompletePartial={completePartial}
              />
            )}
          </article>
        </div>
      </section>

      <AgentPanel
        collapsed={agentCollapsed}
        messages={messages}
        proposal={proposal}
        draft={draft}
        onDraftChange={setDraft}
        onSend={sendMessage}
        onCollapse={() => setAgentCollapsed((value) => !value)}
        onUndoAcceptance={undoAcceptance}
        onReopenRejected={reopenRejected}
      />

      {notice && <div className="notice" role="status">{notice}</div>}

      {labOpen && (
        <PrototypeLab
          proposal={proposal}
          onClose={() => setLabOpen(false)}
          onStream={startStream}
          onConflict={simulateConflict}
          onReload={() => window.location.reload()}
          onReset={resetPrototype}
        />
      )}
    </main>
  );
}
