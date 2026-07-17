import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import UniqueID from "@tiptap/extension-unique-id";
import {
  decideProposalEditingAdmission,
  PROPOSAL_EDITING_PROFILE,
} from "./proposal-contract.js";
import {
  InlineProposalContract,
  INLINE_PROPOSAL_META,
  inlineProposalPluginKey,
  resetInlineProposal,
} from "./inline-proposal-extension.js";

const PREFIX = "权威开头｜";
const CANDIDATE = "提案片段";
const SUFFIX = "｜权威结尾";
const INLINE_DOCUMENT = {
  type: "doc",
  content: [
    {
      type: "paragraph",
      attrs: { blockId: "inline-contract-block" },
      content: [{ type: "text", text: `${PREFIX}${CANDIDATE}${SUFFIX}` }],
    },
  ],
};
const INLINE_RANGE = {
  from: 1 + PREFIX.length,
  to: 1 + PREFIX.length + CANDIDATE.length,
};

const IDENTITY_DOCUMENT = {
  type: "doc",
  content: [
    {
      type: "paragraph",
      attrs: { blockId: "block-a" },
      content: [{ type: "text", text: "甲段" }],
    },
    {
      type: "paragraph",
      attrs: { blockId: "block-b" },
      content: [{ type: "text", text: "乙段" }],
    },
  ],
};

function readIds(editor) {
  const blocks = [];
  editor.state.doc.forEach((node) => {
    blocks.push({ id: node.attrs.blockId, type: node.type.name });
  });
  return blocks;
}

function runtimeCapabilityChecks(editor) {
  if (!editor) return [];
  const probe = inlineProposalPluginKey.getState(editor.state);
  const dryRun = editor.state.tr.insertText("测", 2);
  return [
    { name: "beforeinput", pass: typeof InputEvent !== "undefined" },
    { name: "composition events", pass: typeof CompositionEvent !== "undefined" },
    { name: "transaction dry-run", pass: dryRun.docChanged },
    {
      name: "exclusive decoration state",
      pass: Boolean(probe?.range && probe.range.from < probe.range.to),
    },
  ];
}

function isSupportedChromeSession() {
  const brands = navigator.userAgentData?.brands ?? [];
  if (brands.length) {
    return brands.some(({ brand }) => brand === "Google Chrome");
  }
  return /Chrome\//.test(navigator.userAgent) && !/(Edg|OPR)\//.test(navigator.userAgent);
}

function eventLabel(event) {
  if (event.type === "composition_end") {
    return `composition_end · ${event.ownership}${event.refused ? " · refused" : ""}`;
  }
  if (event.ownership) return `${event.type} · ${event.ownership}`;
  return event.type;
}

export function ContractProbe() {
  const [supportProfileMatches] = useState(isSupportedChromeSession);
  const [compatibilityMatches, setCompatibilityMatches] = useState(true);
  const [forceRuntimeMismatch, setForceRuntimeMismatch] = useState(false);
  const [invariantViolation, setInvariantViolation] = useState(false);
  const [runtimeChecks, setRuntimeChecks] = useState([]);
  const [probeState, setProbeState] = useState({
    range: INLINE_RANGE,
    lastOwnership: "none",
  });
  const [refusedDraft, setRefusedDraft] = useState(null);
  const [events, setEvents] = useState([]);
  const [compositionActive, setCompositionActive] = useState(false);
  const [identityResults, setIdentityResults] = useState([]);
  const admissionRef = useRef({ mode: "full" });

  const recordEvent = useCallback((event) => {
    setEvents((current) => [
      { ...event, id: `${Date.now()}-${current.length}` },
      ...current.slice(0, 5),
    ]);
  }, []);

  const inlineExtension = useMemo(
    () =>
      InlineProposalContract.configure({
        initialRange: INLINE_RANGE,
        onEvent: recordEvent,
        onRefusedDraft: (draft) => {
          setRefusedDraft(draft);
          recordEvent({ type: "refused_edit_draft" });
        },
        onCompositionStart: () => setCompositionActive(true),
        onCompositionEnd: () => setCompositionActive(false),
      }),
    [recordEvent],
  );

  const inlineEditor = useEditor({
    content: INLINE_DOCUMENT,
    extensions: [
      StarterKit.configure({ heading: false }),
      UniqueID.configure({
        types: ["paragraph"],
        attributeName: "blockId",
      }),
      inlineExtension,
    ],
    editorProps: {
      attributes: {
        class: "contract-prosemirror",
        spellcheck: "false",
        "aria-label": "同块内 Proposal 契约验证编辑器",
      },
    },
    onTransaction: ({ editor }) => {
      const state = inlineProposalPluginKey.getState(editor.state);
      if (state) setProbeState(state);
    },
  });

  const identityEditor = useEditor({
    content: IDENTITY_DOCUMENT,
    extensions: [
      StarterKit,
      UniqueID.configure({
        types: ["paragraph", "heading"],
        attributeName: "blockId",
      }),
    ],
    editorProps: {
      attributes: {
        class: "identity-prosemirror",
        spellcheck: "false",
        "aria-label": "Block ID 结构验证编辑器",
      },
    },
  });

  const runtimePass =
    runtimeChecks.length > 0 &&
    runtimeChecks.every((check) => check.pass) &&
    !forceRuntimeMismatch;
  const admission = decideProposalEditingAdmission({
    supportProfileMatches,
    compatibilityEvidenceMatches: compatibilityMatches,
    runtimeCapabilitiesPass: runtimePass,
    invariantViolation,
  });
  admissionRef.current = admission;

  useEffect(() => {
    if (!inlineEditor) return;
    setRuntimeChecks(runtimeCapabilityChecks(inlineEditor));
  }, [inlineEditor]);

  useEffect(() => {
    if (!inlineEditor) return;
    const transaction = inlineEditor.state.tr
      .setMeta(inlineProposalPluginKey, {
        type: "setMode",
        mode: admission.mode,
      })
      .setMeta(INLINE_PROPOSAL_META, {
        allow: true,
        intent: "admission_change",
      })
      .setMeta("addToHistory", false);
    inlineEditor.view.dispatch(transaction);
    recordEvent({ type: `admission_${admission.mode}`, reason: admission.reason });
  }, [admission.mode, admission.reason, inlineEditor, recordEvent]);

  const positionCaret = (kind) => {
    if (!inlineEditor) return;
    const { range } = inlineProposalPluginKey.getState(inlineEditor.state);
    const position =
      kind === "start"
        ? range.from
        : kind === "end"
          ? range.to
          : range.from + Math.max(1, Math.floor((range.to - range.from) / 2));
    inlineEditor.chain().focus().setTextSelection(position).run();
    recordEvent({ type: `caret_${kind}`, position });
  };

  const selectMixedBoundary = () => {
    if (!inlineEditor) return;
    const { range } = inlineProposalPluginKey.getState(inlineEditor.state);
    inlineEditor
      .chain()
      .focus()
      .setTextSelection({ from: range.from - 1, to: range.from + 1 })
      .run();
    recordEvent({ type: "selection_mixed", from: range.from - 1, to: range.from + 1 });
  };

  const selectProposal = () => {
    if (!inlineEditor) return;
    const { range } = inlineProposalPluginKey.getState(inlineEditor.state);
    inlineEditor
      .chain()
      .focus()
      .setTextSelection({ from: range.from, to: range.to })
      .run();
    recordEvent({ type: "selection_proposal", from: range.from, to: range.to });
  };

  const resetInline = () => {
    if (!inlineEditor) return;
    resetInlineProposal(inlineEditor, {
      document: INLINE_DOCUMENT,
      range: INLINE_RANGE,
      mode: admissionRef.current.mode,
    });
    setProbeState({
      range: INLINE_RANGE,
      mode: admissionRef.current.mode,
      lastOwnership: "none",
    });
    setRefusedDraft(null);
    setEvents([]);
    setCompositionActive(false);
  };

  const resetIdentity = useCallback(() => {
    if (!identityEditor) return;
    identityEditor.commands.setContent(IDENTITY_DOCUMENT, {
      emitUpdate: true,
      errorOnInvalidContent: true,
    });
  }, [identityEditor]);

  const runIdentityMatrix = () => {
    if (!identityEditor) return;
    const results = [];

    resetIdentity();
    identityEditor.chain().setTextSelection(3).splitBlock().run();
    const split = readIds(identityEditor);
    results.push({
      case: "split",
      pass:
        split.length === 3 &&
        split[0].id === "block-a" &&
        Boolean(split[1].id) &&
        split[1].id !== "block-a",
      actual: split.map((block) => block.id).join(" → "),
    });
    const splitIds = split.map((block) => block.id);
    identityEditor.commands.undo();
    identityEditor.commands.redo();
    const redoIds = readIds(identityEditor).map((block) => block.id);
    results.push({
      case: "exact undo/redo",
      pass: JSON.stringify(splitIds) === JSON.stringify(redoIds),
      actual: redoIds.join(" → "),
    });

    resetIdentity();
    const firstSize = identityEditor.state.doc.child(0).nodeSize;
    identityEditor.chain().setTextSelection(firstSize + 1).joinBackward().run();
    const joined = readIds(identityEditor);
    results.push({
      case: "join",
      pass: joined.length === 1 && joined[0].id === "block-a",
      actual: joined.map((block) => block.id).join(" → "),
    });

    resetIdentity();
    const secondFrom = identityEditor.state.doc.child(0).nodeSize;
    const second = identityEditor.state.doc.child(1);
    identityEditor.view.dispatch(
      identityEditor.state.tr
        .delete(secondFrom, secondFrom + second.nodeSize)
        .insert(0, second),
    );
    const moved = readIds(identityEditor);
    results.push({
      case: "atomic move",
      pass: moved[0]?.id === "block-b" && moved[1]?.id === "block-a",
      actual: moved.map((block) => block.id).join(" → "),
    });

    resetIdentity();
    const copied = identityEditor.state.doc.child(0);
    const copiedWithoutIdentity = copied.type.create(
      { ...copied.attrs, blockId: null },
      copied.content,
      copied.marks,
    );
    identityEditor.view.dispatch(
      identityEditor.state.tr.insert(
        identityEditor.state.doc.content.size,
        copiedWithoutIdentity,
      ),
    );
    const copy = readIds(identityEditor);
    results.push({
      case: "StoryOS copy command",
      pass:
        copy.length === 3 &&
        Boolean(copy[2].id) &&
        copy[2].id !== copy[0].id,
      actual: copy.map((block) => block.id).join(" → "),
    });

    resetIdentity();
    const first = identityEditor.state.doc.child(0);
    identityEditor.view.dispatch(
      identityEditor.state.tr.setNodeMarkup(
        0,
        identityEditor.schema.nodes.heading,
        { ...first.attrs, level: 2 },
      ),
    );
    const retyped = readIds(identityEditor);
    results.push({
      case: "one-to-one retype",
      pass: retyped[0]?.type === "heading" && retyped[0]?.id === "block-a",
      actual: `${retyped[0]?.type}:${retyped[0]?.id}`,
    });

    setIdentityResults(results);
    recordEvent({ type: "block_identity_matrix_complete" });
  };

  return (
    <div className="contract-probe" data-testid="contract-probe">
      <section className="contract-section">
        <div className="contract-section-heading">
          <div>
            <span>同块内边界</span>
            <strong>Inline Proposal 所有权</strong>
          </div>
          <output
            className={`admission-badge ${admission.mode}`}
            data-testid="admission-mode"
          >
            {admission.mode === "full" ? "FULL" : `SAFE · ${admission.reason}`}
          </output>
        </div>

        <EditorContent editor={inlineEditor} />

        <div className="contract-controls">
          <button type="button" onClick={() => positionCaret("start")}>
            光标：提案起点
          </button>
          <button type="button" onClick={() => positionCaret("inside")}>
            光标：提案内部
          </button>
          <button type="button" onClick={() => positionCaret("end")}>
            光标：提案终点
          </button>
          <button type="button" onClick={selectMixedBoundary}>
            选择：跨起点
          </button>
          <button type="button" onClick={selectProposal}>
            选择：完整 Proposal
          </button>
          <button type="button" className="quiet" onClick={resetInline}>
            重置 Inline
          </button>
        </div>

        <dl className="contract-readout">
          <div><dt>range</dt><dd>{probeState.range.from}…{probeState.range.to}</dd></div>
          <div><dt>last owner</dt><dd data-testid="last-ownership">{probeState.lastOwnership}</dd></div>
          <div><dt>composition</dt><dd>{compositionActive ? "fenced" : "idle"}</dd></div>
          <div><dt>draft</dt><dd data-testid="draft-status">{refusedDraft ? "preserved" : "—"}</dd></div>
        </dl>

        {refusedDraft && (
          <article className="refused-draft" data-testid="refused-draft">
            <strong>Refused Edit Draft</strong>
            <p>{refusedDraft.attemptedText}</p>
            <small>{refusedDraft.intent} · {refusedDraft.recoveryChoices.join(" / ")}</small>
          </article>
        )}

        <p className="contract-help">
          点击定位后直接键入；起点与终点属于相邻权威正文，内部属于 Proposal。
          选择“跨起点”再键入或粘贴，应整笔拒绝并生成 Draft。IME 在 compositionend
          后一次分类。
        </p>
      </section>

      <section className="contract-section">
        <div className="contract-section-heading">
          <div>
            <span>准入门</span>
            <strong>版本证据与运行时能力</strong>
          </div>
        </div>
        <div className="profile-line">
          支持范围：{PROPOSAL_EDITING_PROFILE.browser} · {PROPOSAL_EDITING_PROFILE.authorInputLanguages.join(" / ")}
          {" · "}Tiptap {PROPOSAL_EDITING_PROFILE.tiptap} · model {PROPOSAL_EDITING_PROFILE.prosemirrorModel}
          {" · "}state {PROPOSAL_EDITING_PROFILE.prosemirrorState} · view {PROPOSAL_EDITING_PROFILE.prosemirrorView}
        </div>
        <div className="gate-grid">
          <span className={supportProfileMatches ? "pass" : "fail"}>
            {supportProfileMatches ? "通过" : "不支持"} · Chrome 产品支持范围
          </span>
          <label>
            <input
              type="checkbox"
              checked={compatibilityMatches}
              onChange={(event) => setCompatibilityMatches(event.target.checked)}
            />
            版本证据匹配
          </label>
          <label>
            <input
              type="checkbox"
              checked={forceRuntimeMismatch}
              onChange={(event) => setForceRuntimeMismatch(event.target.checked)}
            />
            模拟能力不匹配
          </label>
          <label>
            <input
              type="checkbox"
              checked={invariantViolation}
              onChange={(event) => setInvariantViolation(event.target.checked)}
            />
            模拟不变量破坏
          </label>
        </div>
        <ul className="capability-list">
          {runtimeChecks.map((check) => (
            <li key={check.name} className={check.pass ? "pass" : "fail"}>
              {check.pass ? "通过" : "失败"} · {check.name}
            </li>
          ))}
        </ul>
      </section>

      <section className="contract-section">
        <div className="contract-section-heading">
          <div>
            <span>结构操作</span>
            <strong>Block ID 不变量</strong>
          </div>
          <button type="button" onClick={runIdentityMatrix}>运行矩阵</button>
        </div>
        <EditorContent editor={identityEditor} />
        <ul className="identity-results" data-testid="identity-results">
          {identityResults.map((result) => (
            <li key={result.case} className={result.pass ? "pass" : "fail"}>
              <strong>{result.pass ? "PASS" : "FAIL"} · {result.case}</strong>
              <span>{result.actual}</span>
            </li>
          ))}
        </ul>
      </section>

      <section className="contract-section event-log">
        <div className="contract-section-heading">
          <div>
            <span>浏览器证据</span>
            <strong>最近事件</strong>
          </div>
        </div>
        <ol>
          {events.map((event) => <li key={event.id}>{eventLabel(event)}</li>)}
        </ol>
      </section>
    </div>
  );
}
