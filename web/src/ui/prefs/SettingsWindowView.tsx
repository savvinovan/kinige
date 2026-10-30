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
  /** «Сейчас» для меток времени; по умолчанию часы окна. В тестах и историях фиксируется. */
  now?: Date;
  /** Показано предупреждение «текст уйдёт провайдеру модели»: `Modal` поверх окна. */
  warning: boolean;
  choosing?: boolean;
  /** Уже переведённые тексты ошибок. */
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
 * Окно «Настройки» целиком (слайс 4b): слева дерево разделов, справа страница с одной прокруткой. Только вид и поведение:
 * данные и действия приходят снаружи, вызовов бэкенда здесь нет. Предупреждение при включении доступа — один `Modal` поверх
 * окна. Подэкраны («Видимые чаты», «Журнал обращений») открываются из «Доступа ИИ-агентов» и подсвечены в дереве.
 */
export function SettingsWindowView(props: SettingsWindowViewProps) {
  const { route, onNavigate, file, access, now } = props;
  const changed = file.changed;
  const problems: Route[] = file.problem || file.unsafeReason || file.issues.length > 0 ? ["file"] : [];
  const back = () => onNavigate(parentOf(route));
  return (
    <div className="prefs">
      <SettingsTree route={route} onNavigate={onNavigate} withProblems={problems} />
      <main className="prefs__page">
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
