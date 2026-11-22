import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from "react";

import type { ProjectScope } from "../../../generated/typescript/storyos-public-release-1/client.mjs";
import type { ProjectSettingsReadyState } from "./editor-types.ts";
import { CharacterSection } from "./character-section.tsx";
import { useMemoryEntries, type MemoryEntries } from "./memory-entries.ts";
import { MemorySection } from "./memory-section.tsx";
import { ProjectInstructionSection } from "./project-instruction-section.tsx";

export type SettingsSectionProps = {
  scope: ProjectScope;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  /** The one read of the Memory Entries of the page. */
  memory: MemoryEntries;
  /** The section tells the page when its field has a text that is not saved. */
  onUnsavedChange: (unsaved: boolean) => void;
};

/**
 * The sections of the settings page in the order of the navigation (ADR 0094 section 9, ADR 0101 section 6). The author
 * approved the item words with the sketch of the page on 2026-11-22.
 */
const SECTIONS: ReadonlyArray<{ key: string; item: string; Section: (props: SettingsSectionProps) => ReactNode }> = [
  { key: "instruction", item: "项目说明", Section: ProjectInstructionSection },
  { key: "memory", item: "记忆", Section: MemorySection },
  { key: "characters", item: "人物", Section: CharacterSection },
];

const projectSettingsPath = (projectId: string) => `/projects/${projectId}/settings`;

/** The gear control of the writing workspace. It opens the settings page with an ordinary page load. */
export function ProjectSettingsLink({ projectId }: { projectId: string }) {
  return (
    <a className="project-settings-link" href={projectSettingsPath(projectId)} aria-label="项目设置"
      data-project-settings-link="">
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="2.6" fill="none" stroke="currentColor" strokeWidth="1.4" />
        <path d="M10 2.5v2.2M10 15.3v2.2M2.5 10h2.2M15.3 10h2.2M4.7 4.7l1.6 1.6M13.7 13.7l1.6 1.6M4.7 15.3l1.6-1.6M13.7 6.3l1.6-1.6"
          fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" />
        <circle cx="10" cy="10" r="5.4" fill="none" stroke="currentColor" strokeWidth="1.4" />
      </svg>
    </a>
  );
}

/**
 * The settings page of a Project. With a text that is not saved, each navigation of the page shows one confirm step,
 * and a reload or a close of the tab shows the leave prompt of the browser.
 */
export function ProjectSettingsView({
  state, baseUrl, fetchImpl, cryptoImpl, navigate = (href) => globalThis.location.assign(href),
}: {
  state: ProjectSettingsReadyState;
  baseUrl: string;
  fetchImpl: typeof fetch;
  cryptoImpl: Crypto;
  navigate?: (href: string) => void;
}) {
  const { project_scope: scope, project } = state.project;
  const [unsaved, setUnsaved] = useState<ReadonlySet<string>>(new Set());
  const [leaving, setLeaving] = useState<string>();
  const [shown, setShown] = useState(SECTIONS[0]!.key);
  const memory = useMemoryEntries({ scope, baseUrl, fetchImpl, cryptoImpl });
  const discarded = useRef(false);
  const changes = useMemo(() => new Map(SECTIONS.map(({ key }) => [key, (value: boolean) => {
    setUnsaved((current) => {
      if (current.has(key) === value) return current;
      const next = new Set(current);
      if (value) next.add(key); else next.delete(key);
      return next;
    });
  }])), []);
  const hasUnsaved = unsaved.size > 0;
  useEffect(() => {
    if (!hasUnsaved) return;
    const prompt = (event: BeforeUnloadEvent) => {
      if (discarded.current) return;
      event.preventDefault();
      event.returnValue = "";
    };
    globalThis.addEventListener("beforeunload", prompt);
    return () => globalThis.removeEventListener("beforeunload", prompt);
  }, [hasUnsaved]);
  // Each link of the page into the application goes through the confirm step: `返回写作` and each source link of a
  // Memory entry (#1692).
  const follow = (event: MouseEvent<HTMLElement>) => {
    const href = (event.target as Element).closest("a[href]")?.getAttribute("href");
    // A new tab or window does not leave this page.
    if (href?.startsWith("/") !== true || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey
      || event.altKey) return;
    event.preventDefault();
    if (hasUnsaved) setLeaving(href); else navigate(href);
  };
  const workspace = `/projects/${project.project_id}`;
  return (
    <section className="project-settings" data-project-settings="" onClick={follow}>
      <header>
        <a href={workspace} data-return-to-writing="">返回写作</a>
        <p>{project.title}</p>
      </header>
      <h1>项目设置</h1>
      <div className="settings-body">
        <nav className="settings-nav">
          <ul>
            {SECTIONS.map(({ key, item }) => (
              <li key={key}>
                <button type="button" aria-current={key === shown ? "true" : undefined} data-settings-item={key}
                  onClick={() => setShown(key)}>{item}</button>
              </li>
            ))}
          </ul>
        </nav>
        {/* Each section stays mounted, so that a text that is not saved stays when the author selects another item. */}
        <div className="settings-panels">
          {SECTIONS.map(({ key, Section }) => (
            <div key={key} hidden={key !== shown} data-settings-panel={key}>
              <Section scope={scope} baseUrl={baseUrl} fetchImpl={fetchImpl} cryptoImpl={cryptoImpl} memory={memory}
                onUnsavedChange={changes.get(key)!} />
            </div>
          ))}
        </div>
      </div>
      {leaving === undefined ? null : (
        <div className="settings-leave-confirm" role="alertdialog" aria-labelledby="settings-leave-question"
          data-settings-leave-confirm="">
          <p id="settings-leave-question">项目指令有未保存的修改。确定离开吗？</p>
          <button type="button" data-settings-leave="" onClick={() => {
            discarded.current = true;
            navigate(leaving);
          }}>离开</button>
          <button type="button" autoFocus data-settings-stay="" onClick={() => setLeaving(undefined)}>留下</button>
        </div>
      )}
    </section>
  );
}
