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
  Plus,
  Sparkle,
  SidebarSimple,
  UserCircle,
  X,
} from "@phosphor-icons/react";
import {
  chapterDocuments,
  chapterTitles,
  chapterTwelveAuthorityDocument,
  chapterTwelveDocument,
  initialMessages,
  initialProposal,
  proposalParagraphs,
  volumes,
} from "./data.js";
import {
  classifyTransaction,
  findProposalEnvelope,
  PROPOSAL_META,
  ProposalProjection,
  proposalPluginKey,
} from "./proposal-extension.js";
import {
  captureRejectedProposalRevision,
  createReopenProposalTransaction,
} from "./proposal-revision.js";
import {
  clearPrototypeState,
  loadPrototypeState,
  savePrototypeState,
} from "./prototype-storage.js";
import { ContractProbe } from "./ContractProbe.jsx";
import {
  countCharacters,
  getUxScenario,
  preservedTextToParagraphs,
  resolveNarrowedRetry,
  resolveUxAction,
  settleWithoutProposalCandidate,
  UX_ACTIONS,
  UX_SCENARIO_IDS,
} from "./recovery-ux.js";

const STREAM_CHUNKS = [
  "可那行字像一根细针，",
  "从这三日的每一个念头里反复穿过，",
  "扎得他夜不能寐。",
  "他想起那人最后一次回头时的眼神。",
];

function timestamp() {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(new Date());
}

function readRequestedUxScenario() {
  const requested = new URLSearchParams(window.location.search).get("scenario");
  return UX_SCENARIO_IDS.includes(requested) ? requested : "proposal";
}

function writeUxScenarioToUrl(scenarioId) {
  const url = new URL(window.location.href);
  if (scenarioId === "proposal") url.searchParams.delete("scenario");
  else url.searchParams.set("scenario", scenarioId);
  window.history.replaceState(null, "", `${url.pathname}${url.search}${url.hash}`);
}

function chapterDocumentForScenario(scenarioId) {
  return ["proposal", "conflicted", "proposal-recovery-conflict"].includes(
    scenarioId,
  )
    ? chapterTwelveDocument
    : chapterTwelveAuthorityDocument;
}

function proposalForScenario(scenarioId) {
  if (
    ["authoritative", "refused", "no-effect", "recovery-draft"].includes(
      scenarioId,
    )
  ) {
    return settleWithoutProposalCandidate(
      initialProposal,
      `ux_${scenarioId.replaceAll("-", "_")}`,
    );
  }
  if (["conflicted", "proposal-recovery-conflict"].includes(scenarioId)) {
    return {
      ...initialProposal,
      validation: "conflicted",
      conflictReason:
        scenarioId === "conflicted"
          ? "target_revision_changed"
          : "pause_fence_ambiguous",
    };
  }
  return initialProposal;
}

function proposalStatusLabel(proposal) {
  if (proposal.resolution === "applied") return "已接受";
  if (proposal.resolution === "none") return "没有待处理提案";
  if (proposal.validation === "conflicted") return "需要重新规划";
  if (proposal.resolution === "rejected") return "已拒绝";
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
        <div className="account-placeholder" aria-label="当前用户：作者">
          <UserCircle size={21} weight="fill" />
          <span>作者</span>
        </div>
        <button
          type="button"
          aria-label="设置"
          data-testid="open-lab"
          onClick={onOpenLab}
        >
          <GearSix size={18} />
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

function RecoverySurface({
  scenario,
  phase,
  attemptedText,
  narrowedText,
  outcomeDetail,
  onNarrowedTextChange,
  onAction,
}) {
  if (scenario.id === "proposal") return null;

  const hasPreservedText = Boolean(attemptedText);
  const isNarrowing = phase === "narrowing";
  const canRetryNarrowed =
    isNarrowing &&
    narrowedText.trim().length > 0 &&
    countCharacters(narrowedText.trim()) <
      countCharacters(attemptedText.trim());

  return (
    <section
      className={`recovery-surface ${scenario.tone}`}
      aria-labelledby="recovery-surface-title"
      data-result-kind={scenario.result}
      data-draft-artifact={scenario.draftArtifact ?? undefined}
      data-condition={scenario.condition ?? undefined}
      data-preserved-surface={scenario.preservedSurface ?? undefined}
      data-testid="recovery-surface"
    >
      <div className="recovery-heading">
        <span>{scenario.eyebrow}</span>
        <h2 id="recovery-surface-title">{scenario.title}</h2>
        <p>{outcomeDetail ?? scenario.description}</p>
      </div>

      {hasPreservedText && (
        <div className="preserved-copy">
          <div className="preserved-copy-heading">
            <strong>{scenario.textLabel}</strong>
            <span>全文 · {countCharacters(attemptedText)} 字</span>
          </div>
          <pre
            aria-label={scenario.textLabel}
            data-testid="preserved-text"
          >
            {attemptedText}
          </pre>
        </div>
      )}

      {isNarrowing && (
        <div className="narrowing-editor">
          <div
            className="narrowing-target"
            data-testid="narrowed-retry-target"
          >
            <span>重试位置</span>
            <strong>{scenario.narrowedRetry.authorLabel}</strong>
            <small>
              StoryOS 会重新检查当前位置；确认后只提交下面这段。
            </small>
          </div>
          <label htmlFor="narrowed-retry-text">
            在这个单一正文目标中重试的较小范围
          </label>
          <textarea
            id="narrowed-retry-text"
            value={narrowedText}
            onChange={(event) => onNarrowedTextChange(event.target.value)}
            aria-label="要重试的较小范围"
          />
        </div>
      )}

      {scenario.actions.length > 0 && (
        <div className="recovery-actions" aria-label="可用操作">
          {scenario.actions
            .filter((actionId) => !(isNarrowing && actionId === "narrow"))
            .map((actionId) => (
              <button
                key={actionId}
                type="button"
                data-action={actionId}
                onClick={() => onAction(actionId)}
              >
                {UX_ACTIONS[actionId].label}
              </button>
            ))}
          {canRetryNarrowed && (
            <button
              type="button"
              className="primary"
              data-action="retry-narrowed"
              onClick={() => onAction("retry-narrowed")}
            >
              重试这段
            </button>
          )}
        </div>
      )}
    </section>
  );
}

function AgentPanel({
  collapsed,
  messages,
  proposal,
  draft,
  onDraftChange,
  onSend,
  onUndoAcceptance,
  onReopenRejected,
}) {
  return (
    <aside
      id="writing-assistant-panel"
      className="agent-panel"
      aria-label={collapsed ? "写作助手已收起" : "写作助手对话"}
    >
      {!collapsed && (
        <>
          <header className="agent-header">
            <strong>写作助手</strong>
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
            {proposal.resolution === "rejected" && proposal.rejectedRevision && (
              <button
                className="transcript-action"
                type="button"
                onClick={onReopenRejected}
                disabled={proposal.validation === "conflicted"}
              >
                {proposal.validation === "conflicted"
                  ? "提案位置已变化，需要重新规划"
                  : "重新打开刚才拒绝的提案"}
              </button>
            )}
          </div>

          <div className="composer-dock">
            <form className="composer" onSubmit={onSend}>
              <textarea
                value={draft}
                onChange={(event) => onDraftChange(event.target.value)}
                placeholder="告诉写作助手你想做什么…"
                aria-label="给写作助手的消息"
              />
              <div className="composer-controls">
                <button type="button" aria-label="添加上下文">
                  <Plus size={16} />
                </button>
                <button type="submit" className="send-button" aria-label="发送" disabled={!draft.trim()}>
                  <ArrowUp size={15} weight="bold" />
                </button>
              </div>
            </form>
          </div>
        </>
      )}
    </aside>
  );
}

function PrototypeLab({
  proposal,
  refusedDraft,
  uxScenarioId,
  onClose,
  onStream,
  onConflict,
  onReload,
  onReset,
  onSelectUxScenario,
}) {
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
          <div><dt>receipt</dt><dd data-testid="acceptance-receipt">{proposal.acceptance?.receiptId ?? "—"}</dd></div>
          <div><dt>accept source</dt><dd>{proposal.acceptance?.source ?? "—"}</dd></div>
          <div><dt>editor undo depth</dt><dd>{proposal.editorHistoryDepth ?? 0}</dd></div>
        </dl>

        <div className="lab-actions">
          <button type="button" onClick={onStream}>模拟 Agent 流式续写</button>
          <button type="button" onClick={onConflict}>模拟目标冲突</button>
          <button type="button" onClick={onReload}>重载并恢复</button>
          <button type="button" className="quiet" onClick={onReset}>重置原型</button>
        </div>

        <section className="ux-matrix-controls" aria-label="Issue 45 UX 场景">
          <div>
            <span>Issue 45</span>
            <strong>production UX matrix</strong>
          </div>
          <div className="ux-scenario-buttons">
            {UX_SCENARIO_IDS.map((scenarioId) => (
              <button
                key={scenarioId}
                type="button"
                className={scenarioId === uxScenarioId ? "active" : ""}
                data-testid={`ux-scenario-${scenarioId}`}
                onClick={() => onSelectUxScenario(scenarioId)}
              >
                {scenarioId}
              </button>
            ))}
          </div>
        </section>

        <p>
          流式续写期间在提案内输入，RunWriteGate 会先同步关闭，晚到的 Agent
          片段不会进入编辑器。跨越正文与提案的替换会整体拒绝。
        </p>

        {refusedDraft && (
          <article className="main-refused-draft">
            <strong>最近一次主编辑器 Refused Edit Draft</strong>
            <p>{refusedDraft.attemptedText}</p>
          </article>
        )}

        <ContractProbe />
      </section>
    </div>
  );
}

export function App() {
  const bootScenarioId = useMemo(readRequestedUxScenario, []);
  const saved = useMemo(
    () => (bootScenarioId === "proposal" ? loadPrototypeState() : null),
    [bootScenarioId],
  );
  const [activeChapter, setActiveChapter] = useState("chapter-12");
  const [expandedVolumes, setExpandedVolumes] = useState(
    () => new Set(["volume-1", "volume-2"]),
  );
  const [agentCollapsed, setAgentCollapsed] = useState(false);
  const [proposal, setProposal] = useState(
    saved?.proposal ?? proposalForScenario(bootScenarioId),
  );
  const [messages, setMessages] = useState(saved?.messages ?? initialMessages);
  const [draft, setDraft] = useState("");
  const [labOpen, setLabOpen] = useState(false);
  const [notice, setNotice] = useState("");
  const [refusedDraft, setRefusedDraft] = useState(null);
  const [documentEpoch, setDocumentEpoch] = useState(0);
  const [uxScenarioId, setUxScenarioId] = useState(bootScenarioId);
  const [uxPhase, setUxPhase] = useState("review");
  const [uxAttemptOverride, setUxAttemptOverride] = useState(null);
  const [uxOutcomeDetail, setUxOutcomeDetail] = useState(null);
  const [narrowedText, setNarrowedText] = useState("");

  const proposalRef = useRef(proposal);
  const activeChapterRef = useRef(activeChapter);
  const runGateRef = useRef(false);
  const streamTimerRef = useRef(null);
  const chapterDocumentsRef = useRef({
    ...chapterDocuments,
    "chapter-12":
      saved?.document ?? chapterDocumentForScenario(bootScenarioId),
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

  const applyAcceptance = useCallback((source = "direct") => {
    const current = proposalRef.current;
    if (
      current.generation !== "ready" ||
      current.validation !== "valid" ||
      current.resolution !== "pending"
    ) {
      setNotice("提案尚未满足接受条件");
      return false;
    }

    const suffix = `${Date.now()}-${current.revision + 1}`;
    setProposal((previous) => ({
      ...previous,
      resolution: "applied",
      acceptance: {
        commandId: `accept-command-${suffix}`,
        commitId: `accept-commit-${suffix}`,
        receiptId: `accept-receipt-${suffix}`,
        source,
      },
      acceptanceRedoAvailable: false,
      editorHistoryDepth: 0,
      authorAction: { kind: "acceptance", safe: true },
    }));
    addAgentMessage(
      source === "redo"
        ? "已作为一次新的接受尝试重新应用提案；新的 Command、Commit 与 Receipt 已生成。"
        : "提案已写入当前章节。接受记录已保留，你仍可以安全撤销这次接受。",
    );
    setUxScenarioId("authoritative");
    setUxPhase("review");
    setUxAttemptOverride(null);
    setUxOutcomeDetail("提案全文已作为一次原子接受写入正文，没有留下部分结果。");
    writeUxScenarioToUrl("authoritative");
    setNotice(source === "redo" ? "已重新接受提案（新 Receipt）" : "已接受提案");
    return true;
  }, [addAgentMessage]);

  const undoAcceptanceFromInput = useCallback(() => {
    const current = proposalRef.current;
    if (current.authorAction?.safe === false) {
      setNotice("最新动作无法安全撤销；没有跳过它去撤销更早内容");
      return true;
    }
    if (
      current.resolution !== "applied" ||
      current.authorAction?.kind !== "acceptance" ||
      (current.editorHistoryDepth ?? 0) > 0
    ) {
      return false;
    }

    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      resolution: "pending",
      validation: "valid",
      creator: "author",
      acceptanceRedoAvailable: true,
      authorAction: { kind: "undo_acceptance", safe: true },
    }));
    addAgentMessage("已补偿刚才的接受，并用一个新修订重新打开提案；原接受记录没有被改写。");
    setUxScenarioId("proposal");
    setUxPhase("review");
    setUxAttemptOverride(null);
    setUxOutcomeDetail(null);
    writeUxScenarioToUrl("proposal");
    setNotice("已撤销接受并重新打开提案");
    return true;
  }, [addAgentMessage]);

  const redoAcceptanceFromInput = useCallback(() => {
    const current = proposalRef.current;
    if (current.authorAction?.safe === false) {
      setNotice("最新动作无法安全重做；没有跳过它去恢复更早内容");
      return true;
    }
    if (!current.acceptanceRedoAvailable) return false;
    return applyAcceptance("redo");
  }, [applyAcceptance]);

  const onRefusedTransaction = useCallback((nextDraft) => {
    setRefusedDraft(nextDraft);
    setUxScenarioId("refused");
    setUxPhase("review");
    setUxAttemptOverride(nextDraft.attemptedText);
    setUxOutcomeDetail(null);
    setNarrowedText("");
    writeUxScenarioToUrl("refused");
    setNotice("这次编辑跨越正文与提案，已整笔拒绝并保存 Refused Edit Draft");
  }, []);

  const proposalExtension = useMemo(
    () =>
      ProposalProjection.configure({
        initialBlockIds: proposalRef.current.blockIds,
        onAuthorSignal: stopStream,
        onRefusedTransaction,
        onUndoRequest: undoAcceptanceFromInput,
        onRedoRequest: redoAcceptanceFromInput,
      }),
    [onRefusedTransaction, redoAcceptanceFromInput, stopStream, undoAcceptanceFromInput],
  );

  const editor = useEditor({
    content: saved?.document ?? chapterDocumentForScenario(bootScenarioId),
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

      setDocumentEpoch((value) => value + 1);

      const ownership = classifyTransaction(
        transaction,
        transaction.before,
        proposalRef.current.blockIds,
      );
      const history = transaction.getMeta("history$");
      setProposal((previous) => {
        const wasApplied = previous.resolution === "applied";
        const currentDepth = previous.editorHistoryDepth ?? 0;
        const editorHistoryDepth = wasApplied
          ? history
            ? Math.max(0, currentDepth + (history.redo ? 1 : -1))
            : currentDepth + 1
          : currentDepth;

        return {
          ...previous,
          ...(ownership === "proposal"
            ? {
                revision: previous.revision + 1,
                validation: "pending",
                creator: "author",
              }
            : {}),
          acceptanceRedoAvailable: false,
          editorHistoryDepth,
          authorAction:
            wasApplied && editorHistoryDepth === 0
              ? { kind: "acceptance", safe: true }
              : { kind: "editor", safe: true },
        };
      });
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
    if (
      !editor ||
      activeChapter !== "chapter-12" ||
      uxScenarioId !== "proposal"
    ) {
      return undefined;
    }
    const timer = window.setTimeout(() => {
      savePrototypeState({
        document: editor.getJSON(),
        proposal,
        messages,
      });
    }, 200);
    return () => window.clearTimeout(timer);
  }, [
    activeChapter,
    documentEpoch,
    editor,
    messages,
    proposal,
    uxScenarioId,
  ]);

  useEffect(
    () => () => {
      if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    },
    [],
  );

  const setUxResult = ({
    scenarioId,
    attemptedText = null,
    outcomeDetail = null,
  }) => {
    setUxScenarioId(scenarioId);
    setUxPhase("review");
    setUxAttemptOverride(attemptedText);
    setUxOutcomeDetail(outcomeDetail);
    setNarrowedText("");
    writeUxScenarioToUrl(scenarioId);
  };

  const selectUxScenario = (scenarioId) => {
    if (!editor) return;
    if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    streamTimerRef.current = null;
    runGateRef.current = false;

    const document = chapterDocumentForScenario(scenarioId);
    replaceEditorDocument(editor, document, `ux_scenario_${scenarioId}`);
    chapterDocumentsRef.current["chapter-12"] = document;
    setActiveChapter("chapter-12");
    setProposal(proposalForScenario(scenarioId));
    setRefusedDraft(null);
    setUxResult({ scenarioId });
    setLabOpen(false);
    setNotice(`已切换到 ${scenarioId} 场景`);
  };

  const materializeTextAsProposal = (text, source) => {
    if (!editor) return;
    const suffix = `${Date.now()}`;
    const paragraphs = preservedTextToParagraphs(
      text,
      `proposal-recovery-${suffix}`,
    );
    const blockIds = paragraphs.map((paragraph) => paragraph.attrs.blockId);
    const candidate = editor.schema.nodeFromJSON({
      type: "doc",
      content: paragraphs,
    });
    const envelope = findProposalEnvelope(
      editor.state.doc,
      proposalRef.current.blockIds,
    );
    const from = envelope?.from ?? editor.state.doc.content.size;
    const to = envelope?.to ?? from;
    const transaction = editor.state.tr
      .replaceWith(from, to, candidate.content)
      .setMeta(PROPOSAL_META, { allow: true, source })
      .setMeta("addToHistory", false);
    editor.view.dispatch(transaction);
    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      blockIds,
      resolution: "pending",
      generation: "ready",
      validation: "pending",
      conflictReason: null,
      creator: "author",
    }));
  };

  const appendTextToAuthority = (text, source) => {
    if (!editor) return;
    const suffix = `${Date.now()}`;
    const recovered = editor.schema.nodeFromJSON({
      type: "doc",
      content: preservedTextToParagraphs(
        text,
        `recovered-authority-${suffix}`,
      ),
    });
    const position = editor.state.doc.content.size;
    const transaction = editor.state.tr
      .replaceWith(position, position, recovered.content)
      .setMeta(PROPOSAL_META, { allow: true, source })
      .setMeta("addToHistory", false);
    editor.view.dispatch(transaction);
    setProposal((previous) =>
      settleWithoutProposalCandidate(previous, source),
    );
  };

  const removeCurrentProposal = (source) => {
    if (!editor) return;
    const envelope = findProposalEnvelope(
      editor.state.doc,
      proposalRef.current.blockIds,
    );
    if (!envelope) return;
    const transaction = editor.state.tr
      .delete(envelope.from, envelope.to)
      .setMeta(PROPOSAL_META, { allow: true, source })
      .setMeta("addToHistory", false);
    editor.view.dispatch(transaction);
    setProposal((previous) => ({
      ...previous,
      blockIds: [],
      resolution: "rejected",
      conflictReason: null,
    }));
  };

  const handleUxAction = async (actionId) => {
    const scenario = getUxScenario(uxScenarioId);
    const attemptedText =
      uxAttemptOverride ?? scenario.attemptedText ?? "";

    if (actionId === "retry-narrowed") {
      if (
        !narrowedText.trim() ||
        countCharacters(narrowedText.trim()) >=
          countCharacters(attemptedText.trim())
      ) {
        return;
      }
      const retry = resolveNarrowedRetry({
        scenarioId: uxScenarioId,
        attemptedText,
        retryText: narrowedText,
      });
      appendTextToAuthority(
        retry.retryText,
        "narrowed_refused_edit_retry_authoritative",
      );
      setUxResult({
        scenarioId: retry.nextScenarioId,
        outcomeDetail:
          `较小范围已写入“${retry.target.authorLabel}”。StoryOS 已重新检查当前位置，只提交了你确认的这段。` +
          "完整 Refused Edit Draft 在结算前始终保留。",
      });
      setNotice("较小范围已写入当前正文");
      return;
    }

    const transition = resolveUxAction({
      scenarioId: uxScenarioId,
      actionId,
      attemptedText,
    });

    if (actionId === "copy") {
      try {
        await navigator.clipboard.writeText(transition.preservedText);
        setUxOutcomeDetail(
          "完整编辑尝试已复制；原文仍完整保留在这里，没有被清空或截断。",
        );
        setNotice("完整文本已复制");
      } catch {
        setUxOutcomeDetail(
          "浏览器没有完成复制。完整编辑尝试仍保留在这里，你可以直接选择文本。",
        );
        setNotice("复制未完成，原文仍保留");
      }
      return;
    }

    if (actionId === "narrow") {
      setUxPhase("narrowing");
      setNarrowedText(scenario.narrowedRetry.initialText);
      setNotice("完整尝试仍保留；已选择单一正文目标");
      return;
    }

    if (transition.nextScenarioId === "authoritative") {
      appendTextToAuthority(
        transition.preservedText,
        "retry_recovery_draft",
      );
      setUxResult({
        scenarioId: "authoritative",
        outcomeDetail:
          "恢复草稿已作为一次新的显式尝试完整写入正文，没有拆分或重复。",
      });
      setNotice("恢复草稿已完整保存");
      return;
    }

    if (transition.nextScenarioId === "proposal") {
      materializeTextAsProposal(
        transition.preservedText,
        actionId === "expand" ? "expand_proposal_retry" : "replan_proposal",
      );
      setUxResult({
        scenarioId: "proposal",
        outcomeDetail: "完整文本已进入一份新的 Proposal，正在检查当前目标。",
      });
      setNotice("完整文本已转为新的 Proposal");
      return;
    }

    if (actionId === "reject" || actionId === "withdraw") {
      removeCurrentProposal(
        actionId === "reject" ? "reject_conflicted_proposal" : "withdraw_proposal",
      );
    }
    const detail =
      actionId === "discard"
        ? "草稿已由作者明确丢弃；正文和提案没有发生变化。"
        : actionId === "reject"
          ? "冲突提案已由作者拒绝；当前正文没有发生变化。"
          : "恢复冲突中的提案已由作者撤回；当前正文没有发生变化。";
    setUxResult({ scenarioId: "no-effect", outcomeDetail: detail });
    setNotice("没有内容写入正文");
  };

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

  const acceptProposal = () => applyAcceptance("direct");

  const undoAcceptance = () => undoAcceptanceFromInput();

  const rejectProposal = () => {
    if (!editor) return;
    const envelope = findProposalEnvelope(editor.state.doc, proposal.blockIds);
    const captured = captureRejectedProposalRevision({
      doc: editor.state.doc,
      proposalId: proposal.id,
      proposalRevision: proposal.revision,
      chapterId: activeChapter,
      blockIds: proposal.blockIds,
    });
    if (!envelope || !captured.ok) {
      setProposal((previous) => ({
        ...previous,
        validation: "conflicted",
        conflictReason: captured.reason ?? "proposal_envelope_missing",
      }));
      addAgentMessage("当前提案片段不完整，StoryOS 没有执行破坏性删除；需要先重新规划提案范围。");
      setUxResult({ scenarioId: "conflicted" });
      setNotice("提案范围已变化，未执行拒绝");
      return;
    }

    const transaction = editor.state.tr
      .delete(envelope.from, envelope.to)
      .setMeta(PROPOSAL_META, { allow: true, source: "reject" })
      .setMeta("addToHistory", false);
    editor.view.dispatch(transaction);
    setProposal((previous) => ({
      ...previous,
      resolution: "rejected",
      validation: "valid",
      conflictReason: null,
      rejectedRevision: captured.revision,
    }));
    addAgentMessage("已拒绝这次提案，章节正文没有被改写。你可以从这里重新打开它。");
    setUxResult({
      scenarioId: "no-effect",
      outcomeDetail: "提案已由作者拒绝；当前章节正文没有发生变化。",
    });
    setNotice("已拒绝提案");
  };

  const reopenRejected = () => {
    if (!editor) return;

    const rejectedRevision = proposalRef.current.rejectedRevision;
    if (!rejectedRevision) {
      addAgentMessage("被拒绝的修订记录不可用，因此没有改写当前章节。请重新生成一份提案。");
      setNotice("没有可重新打开的提案修订");
      return;
    }

    if (activeChapter !== rejectedRevision.chapterId) {
      selectChapter(rejectedRevision.chapterId);
    }

    const reopened = createReopenProposalTransaction(
      editor.state,
      rejectedRevision,
    );
    if (!reopened.ok) {
      setProposal((previous) => ({
        ...previous,
        validation: "conflicted",
        conflictReason: reopened.reason,
      }));
      addAgentMessage("当前章节的提案锚点已经变化，StoryOS 没有猜测插入位置；需要基于现有正文重新规划。");
      setUxResult({ scenarioId: "conflicted" });
      setNotice("提案位置已变化，需要重新规划");
      return;
    }

    editor.view.dispatch(reopened.transaction);
    setProposal((previous) => ({
      ...previous,
      revision: previous.revision + 1,
      blockIds: reopened.blockIds,
      resolution: "pending",
      generation: "ready",
      validation: "pending",
      conflictReason: null,
      derivedFromRevision: rejectedRevision.proposalRevision,
      creator: "author",
    }));
    addAgentMessage("已从被拒绝的历史内容创建新修订并重新打开，正在重新检查当前目标。");
    setUxResult({ scenarioId: "proposal" });
    setNotice("已重新打开提案");
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
    setUxResult({ scenarioId: "proposal" });

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
      authorAction: { kind: "conflict", safe: false },
    }));
    setUxResult({ scenarioId: "conflicted" });
    setLabOpen(false);
    setNotice("目标修订已变化，提案不能静默重基或接受");
  };

  const resetPrototype = () => {
    if (streamTimerRef.current) window.clearInterval(streamTimerRef.current);
    streamTimerRef.current = null;
    runGateRef.current = false;
    clearPrototypeState();
    if (editor) replaceEditorDocument(editor, chapterTwelveDocument, "prototype_reset");
    chapterDocumentsRef.current = { ...chapterDocuments };
    setActiveChapter("chapter-12");
    setProposal(initialProposal);
    setMessages(initialMessages);
    setDraft("");
    setRefusedDraft(null);
    setUxResult({ scenarioId: "proposal" });
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

  const uxScenario = getUxScenario(uxScenarioId);
  const attemptedText =
    uxAttemptOverride ?? uxScenario.attemptedText ?? null;

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
            {activeChapter === "chapter-12" &&
              uxScenarioId === "proposal" && (
              <ProposalActions
                proposal={proposal}
                onAccept={acceptProposal}
                onReject={rejectProposal}
                onCompletePartial={completePartial}
              />
            )}
            {activeChapter === "chapter-12" && (
              <RecoverySurface
                scenario={uxScenario}
                phase={uxPhase}
                attemptedText={attemptedText}
                narrowedText={narrowedText}
                outcomeDetail={uxOutcomeDetail}
                onNarrowedTextChange={setNarrowedText}
                onAction={handleUxAction}
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
        onUndoAcceptance={undoAcceptance}
        onReopenRejected={reopenRejected}
      />

      <button
        type="button"
        className="assistant-toggle"
        onClick={() => setAgentCollapsed((value) => !value)}
        aria-controls="writing-assistant-panel"
        aria-expanded={!agentCollapsed}
        aria-label={agentCollapsed ? "展开写作助手" : "收起写作助手"}
      >
        <SidebarSimple size={20} />
      </button>

      {notice && <div className="notice" role="status">{notice}</div>}

      {labOpen && (
        <PrototypeLab
          proposal={proposal}
          refusedDraft={refusedDraft}
          uxScenarioId={uxScenarioId}
          onClose={() => setLabOpen(false)}
          onStream={startStream}
          onConflict={simulateConflict}
          onReload={() => window.location.reload()}
          onReset={resetPrototype}
          onSelectUxScenario={selectUxScenario}
        />
      )}
    </main>
  );
}
