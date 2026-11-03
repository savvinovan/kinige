import { useLayoutEffect, useRef } from "react";
import type { AgentAccess } from "../../api/generated/AgentAccess";
import type { AgentLogEntry } from "../../api/generated/AgentLogEntry";
import type { AgentMode } from "../../api/generated/AgentMode";
import type { Language } from "../../api/generated/Language";
import type { Settings } from "../../api/generated/Settings";
import type { SettingsFileState } from "../../api/generated/SettingsFileState";
import type { ShortcutKind } from "../../api/generated/ShortcutKind";
import type { TextSize } from "../../api/generated/TextSize";
import type { AgentLogView } from "../agents/AgentLog";
import { EnableWarning } from "../agents/EnableWarning";
import { SettingsTree } from "./SettingsTree";
import { AccessChatsPage } from "./pages/AccessChatsPage";
import { AccessLogPage } from "./pages/AccessLogPage";
import { AccessPage } from "./pages/AccessPage";
import { AppearancePage } from "./pages/AppearancePage";
import { FilePage } from "./pages/FilePage";
import { MirrorPage } from "./pages/MirrorPage";
import { ShortcutsPage } from "./pages/ShortcutsPage";
import { type Route, parentOf } from "./routes";
import "./prefs.css";

export interface SettingsWindowViewProps {
  route: Route;
  onNavigate: (route: Route) => void;
  settings: Settings;
  access: AgentAccess;
  file: SettingsFileState;
  log: AgentLogView;
  language: Language;
  /** "Now" for time marks; by default the window clock. Fixed in tests and stories. */
  now?: Date;
  /** Tree groups expanded on open (Storybook stories); without it the saved window state is used. */
  treeExpanded?: readonly Route[];
  /** The "the text will go to the model provider" warning is shown: a `Modal` over the window. */
  warning: boolean;
  choosing?: boolean;
  /** The already translated error texts. */
  pickError?: string;
  shortcutErrors?: Partial<Record<ShortcutKind, string>>;
  accessError?: string;
  onTextSizeChange: (size: TextSize) => void;
  onChooseFolder: () => void;
  onShortcutChange: (kind: ShortcutKind, accelerator: string) => void;
  onShortcutReset: (kind: ShortcutKind) => void;
  onShortcutRecording: (active: boolean) => void;
  onEnabledChange: (enabled: boolean) => void;
  onConfirmEnable: () => void;
  onCancelEnable: () => void;
  onModeChange: (mode: AgentMode) => void;
  onChatToggle: (id: string, on: boolean) => void;
  onToggleRow: (id: number) => void;
  onOpenEntry: (logId: number, entry: AgentLogEntry) => void;
  onMoreLog: () => void;
}

/**
 * The whole "Settings" window (slice 4b): the section tree on the left, a page with a single scroll on the right. View and behavior
 * only: data and actions come from outside, there are no backend calls here. The warning when enabling access is a single `Modal`
 * over the window. The subscreens ("Видимые чаты", "Журнал обращений") open from "Доступ ИИ-агентов" and are highlighted in the tree.
 */
export function SettingsWindowView(props: SettingsWindowViewProps) {
  const { onNavigate, file, access, now } = props;
  // Windows has no AI-agent access yet: the access page stands alone, without "Visible chats" and "Access log".
  const accessUnavailable = access.socket.error === "unsupported";
  const route: Route = accessUnavailable && parentOf(props.route) === "access" ? "access" : props.route;
  const without: Route[] = accessUnavailable ? ["access/chats", "access/log"] : [];
  const changed = file.changed;
  const problems: Route[] = file.problem || file.unsafeReason || file.issues.length > 0 ? ["file"] : [];
  const back = () => onNavigate(parentOf(route));
  // One scrollable block for all pages: on every navigation (the tree, "Настроить ›", "back") the page opens
  // at the top, not where the previous one stopped. (Recreating the block by the route `key` was checked with screenshots on the runner:
  // it does not remove the leftover WebKit scrollbar "tab", so the scroll is reset explicitly.)
  const page = useRef<HTMLElement>(null);
  useLayoutEffect(() => {
    if (page.current) page.current.scrollTop = 0;
  }, [route]);
  return (
    <div className="prefs">
      <SettingsTree route={route} onNavigate={onNavigate} withProblems={problems} defaultExpanded={props.treeExpanded} without={without} />
      <main className="prefs__page" ref={page}>
        <div className="prefs__column">
          {route === "appearance" ? (
            <AppearancePage textSize={props.settings.textSize} onTextSizeChange={props.onTextSizeChange} changedAt={changed.appearance} now={now} />
          ) : null}
          {route === "shortcuts" ? (
            <ShortcutsPage
              shortcuts={props.settings.shortcuts}
              errors={props.shortcutErrors}
              onChange={props.onShortcutChange}
              onReset={props.onShortcutReset}
              onRecording={props.onShortcutRecording}
              changedAt={changed.shortcuts}
              now={now}
            />
          ) : null}
          {route === "mirror" ? (
            <MirrorPage
              settings={props.settings}
              choosing={props.choosing}
              actionError={props.pickError}
              onChooseFolder={props.onChooseFolder}
              changedAt={changed.mirror}
              now={now}
            />
          ) : null}
          {route === "access" ? (
            <AccessPage
              access={access}
              error={props.accessError}
              onEnabledChange={props.onEnabledChange}
              onModeChange={props.onModeChange}
              onOpen={onNavigate}
              changedAt={changed.access}
              now={now}
            />
          ) : null}
          {route === "access/chats" ? (
            <AccessChatsPage chats={access.chats} disabled={!access.enabled} onToggle={props.onChatToggle} onBack={back} changedAt={changed.access} now={now} />
          ) : null}
          {route === "access/log" ? (
            <AccessLogPage
              log={props.log}
              language={props.language}
              now={now}
              onToggleRow={props.onToggleRow}
              onOpenEntry={props.onOpenEntry}
              onMore={props.onMoreLog}
              onBack={back}
            />
          ) : null}
          {route === "file" ? <FilePage file={file} /> : null}
        </div>
      </main>
      {props.warning ? <EnableWarning onConfirm={props.onConfirmEnable} onCancel={props.onCancelEnable} /> : null}
    </div>
  );
}
