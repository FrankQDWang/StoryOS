import { useCallback, useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import {
  getSubrun,
  type GetAgentRunResponse, type GetSubrunResponse, type SubrunStateInspect,
  type SubrunStepInspect,
} from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { AssistantContext } from "./writing-assistant-panel.tsx";

/** The limit of RunSteps of a Subrun: `SUBRUN_MAX_RUN_STEPS` of `storyos-core`. */
const SUBRUN_MAX_RUN_STEPS = 12;

/** Line 1 of the card of each state (ADR 0103 section 3A). */
const CARD_LINES: Record<SubrunStateInspect, string> = {
  working: "正在通读前文",
  completed: "已通读前文",
  no_result: "通读前文没有得出结论",
  stopped: "已停止",
};

/**
 * The state text of the drawer. A working Subrun names its RunStep in work: `step_count` has only the RunSteps with a
 * settled response, so the RunStep in work is the next one.
 */
function stateText({ state, step_count: count }: GetSubrunResponse): string {
  if (state === "working") return `进行中 · 第 ${Math.min(count + 1, SUBRUN_MAX_RUN_STEPS)} 步`;
  if (state === "completed") return `已完成 · ${count} 步`;
  return state === "no_result" ? "没有结论" : "已停止";
}

/** The lines of the steps: `chapter_read` entries that follow one another are one line. */
function stepLines(steps: readonly SubrunStepInspect[]): { text: string; paragraph: boolean }[] {
  const lines: { text: string; paragraph: boolean; chapters?: string[] }[] = [];
  for (const step of steps) {
    const last = lines.at(-1);
    if (step.kind === "chapter_read" && last?.chapters !== undefined) {
      last.chapters.push(step.title);
      last.text = `读取 ${last.chapters.join("、")}`;
    } else if (step.kind === "chapter_read") {
      lines.push({ text: `读取 ${step.title}`, paragraph: false, chapters: [step.title] });
    } else if (step.kind === "text") {
      lines.push({ text: step.text, paragraph: true });
    } else if (step.kind === "keyword_search") {
      lines.push({ text: `检索 “${step.query}” — ${step.matches} 处`, paragraph: false });
    } else if (step.kind === "hybrid_search") {
      lines.push({ text: `检索 “${step.question}” — ${step.passages} 处`, paragraph: false });
    } else if (step.kind === "memory_read") {
      lines.push({ text: `查看记忆 ${step.title}`, paragraph: false });
    } else {
      lines.push({ text: `检索记忆 “${step.query}” — ${step.entries} 条`, paragraph: false });
    }
  }
  return lines;
}

/**
 * The record of one Subrun in a drawer from the right side of the window. The drawer reads `getSubrun` when it opens,
 * and again at each refresh of the panel (`RUN_REFRESH_MS`) while the state is `working`. After a failed read, the
 * last record stays, and the drawer reads again at the next refresh only while the card says `working`. It closes with
 * its close button, with a click on the dim layer, and with the Escape key.
 */
function SubrunDrawer({ context, subrunId, cardWorking, scheduleRefresh, onClose }: {
  context: AssistantContext;
  subrunId: string;
  /** The card of the Subrun says `working`. */
  cardWorking: boolean;
  scheduleRefresh: (refresh: () => void) => () => void;
  onClose: () => void;
}) {
  const [record, setRecord] = useState<GetSubrunResponse>();
  const close = useRef<HTMLButtonElement>(null);
  // The newest state of the card. A change of the card does not start a new read.
  const working = useRef(cardWorking);
  working.current = cardWorking;
  useEffect(() => { close.current?.focus(); }, []);
  useEffect(() => {
    // The close button is the only control, so Tab and Shift+Tab keep the focus on it, also after a click on the text
    // of the drawer. The dim layer takes each pointer event outside the drawer.
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
      if (event.key !== "Tab") return;
      event.preventDefault();
      close.current?.focus();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);
  useEffect(() => {
    let active = true;
    let cancel = () => {};
    const read = () => {
      void getSubrun({ baseUrl: context.baseUrl, fetchImpl: context.fetchImpl,
        projectId: context.scope.project_id, subrunId }).then((response) => {
        if (!active) return;
        setRecord(response);
        if (response.state === "working") cancel = scheduleRefresh(read);
      }, () => {
        // The card can change before the refresh, so the refresh checks it again.
        if (active && working.current) cancel = scheduleRefresh(() => { if (working.current) read(); });
      });
    };
    read();
    return () => {
      active = false;
      cancel();
    };
  }, [context.baseUrl, context.fetchImpl, context.scope.project_id, subrunId, scheduleRefresh]);

  // The drawer is outside the live region of the conversation, so a new step is not announced as a message.
  return createPortal(<>
    <div className="subrun-drawer-layer" data-subrun-drawer-layer="" onClick={onClose} />
    {/* A click on the text of the drawer keeps the focus in the drawer. */}
    <div className="subrun-drawer" role="dialog" aria-modal="true" aria-label="子任务记录" data-subrun-drawer=""
      tabIndex={-1}>
      <header>
        <h2>子任务：通读前文</h2>
        {/* The place of the state text. A failed read with no record shows no text here. */}
        {record === undefined ? null : <p data-subrun-drawer-state="">{stateText(record)}</p>}
        <button type="button" ref={close} onClick={onClose}>关闭</button>
      </header>
      {record === undefined ? null : <>
        <section>
          <h3>指令</h3>
          <p className="subrun-drawer-text">{record.brief}</p>
        </section>
        <section>
          <h3>过程</h3>
          {stepLines(record.steps).map((line, index) => <p key={index}
            className={line.paragraph ? "subrun-drawer-text" : "subrun-drawer-line"}>{line.text}</p>)}
        </section>
        {record.state !== "completed" || record.answer === undefined ? null : <section>
          <h3>结论</h3>
          <p className="subrun-drawer-text">{record.answer}</p>
        </section>}
      </>}
    </div>
  </>, document.body);
}

/**
 * One card for each Subrun of the Run, in the order of the read (ADR 0103 section 3A). A click on a card opens the
 * drawer with its record. The focus goes back to the card when the drawer closes. A Run with no Subrun shows nothing.
 */
export function SubrunList({ run, context, scheduleRefresh }: {
  run: GetAgentRunResponse;
  context: AssistantContext | undefined;
  scheduleRefresh: (refresh: () => void) => () => void;
}) {
  const [opened, setOpened] = useState<{ subrunId: string; card: HTMLButtonElement }>();
  const subruns = run.subruns ?? [];
  const openedEntry = subruns.find((subrun) => subrun.subrun_id === opened?.subrunId);
  // A Run that no longer has the opened Subrun, for example after a change of the Conversation, closes the drawer.
  useEffect(() => {
    if (opened !== undefined && openedEntry === undefined) setOpened(undefined);
  }, [opened, openedEntry]);
  const closeDrawer = useCallback(() => {
    opened?.card.focus();
    setOpened(undefined);
  }, [opened]);
  if (subruns.length === 0) return null;
  return <div className="assistant-subruns">
    {subruns.map((subrun) => <button type="button" key={subrun.subrun_id} data-assistant-subrun=""
      data-subrun-state={subrun.state}
      onClick={(event) => setOpened({ subrunId: subrun.subrun_id, card: event.currentTarget })}>
      <span className="assistant-subrun-mark" aria-hidden="true" />
      <span className="assistant-subrun-state">{CARD_LINES[subrun.state]}</span>
      <span className="assistant-subrun-title">{subrun.title}</span>
    </button>)}
    {opened === undefined || openedEntry === undefined || context === undefined ? null
      : <SubrunDrawer context={context} subrunId={opened.subrunId} cardWorking={openedEntry.state === "working"}
        scheduleRefresh={scheduleRefresh}
        onClose={closeDrawer} />}
  </div>;
}
