import { useState, type ReactNode } from "react";
import { Composer, type ComposerButtons } from "../composer/Composer";
import { Feed } from "../feed/Feed";
import type { FeedMessage } from "../feed/buildFeed";
import { Sidebar, type SidebarChat } from "../sidebar/Sidebar";
import { UndoToast } from "../toast/UndoToast";
import "./main-window.css";

export interface UndoToastState {
  /** Меняется с каждым новым тостом: таймер в 5 секунд начинается заново. */
  key: string;
  message: string;
  onUndo: () => void;
  onExpire: () => void;
}

interface MainWindowViewProps {
  chats: readonly SidebarChat[];
  selectedChatId: string;
  messages: readonly FeedMessage[];
  /** Сегодняшний `journal_date` для меток «Сегодня» и «Вчера»: в приложении его присылает Rust, в историях он фиксирован. */
  today: string;
  onRetry?: (messageId: string) => void;
  onOpenLink?: (url: string) => void;
  /** Какие кнопки показывать в поле ввода. По умолчанию все три нарисованы и объявлены недоступными («Появится позже»), как в D1; приложение слайса 1 не показывает ни одной. */
  composerButtons?: ComposerButtons;
  /** Поле ввода работает (приложение): без него оно только нарисовано. */
  interactive?: boolean;
  /** Текст в поле ввода. Без него вид хранит черновик сам (истории). */
  draft?: string;
  onDraftChange?: (value: string) => void;
  onSubmit?: () => void;
  /** Ошибка отправки над полем ввода (уже переведённый текст). */
  composerError?: string;
  /** Тост «Отменить»; `undefined` — тоста нет. */
  toast?: UndoToastState;
  /** Растёт, когда фокус нужно вернуть в поле ввода. */
  composerFocusSignal?: number;
  onOpenSettings?: () => void;
  /** Поле «Поиск» в сайдбаре: в приложении его нет до слайса 4. */
  showSearch?: boolean;
  /** Поверх окна: модальное окно настроек. */
  overlay?: ReactNode;
}

/**
 * Главное окно: сайдбар + лента + поле ввода. Строки с названием чата над лентой нет: выбранный чат выделен в сайдбаре (§6).
 * Только вид: данные приходят снаружи (в Storybook — сразу, в приложении — через `MainWindow`).
 */
export function MainWindowView({
  chats,
  selectedChatId,
  messages,
  today,
  onRetry,
  onOpenLink,
  composerButtons,
  interactive = false,
  draft,
  onDraftChange,
  onSubmit,
  composerError,
  toast,
  composerFocusSignal,
  onOpenSettings,
  showSearch = true,
  overlay,
}: MainWindowViewProps) {
  const [ownDraft, setOwnDraft] = useState("");
  return (
    <div className="main-window">
      <Sidebar chats={chats} selectedId={selectedChatId} onOpenSettings={onOpenSettings} showSearch={showSearch} />
      <main className={`main-window__chat${toast ? " main-window__chat--toast" : ""}`}>
        <Feed messages={messages} today={today} onRetry={onRetry} onOpenLink={onOpenLink} />
        {toast ? <UndoToast key={toast.key} open message={toast.message} onUndo={toast.onUndo} onExpire={toast.onExpire} /> : null}
        <Composer
          value={draft ?? ownDraft}
          onChange={onDraftChange ?? setOwnDraft}
          onSubmit={onSubmit}
          buttons={composerButtons}
          viewOnly={!interactive}
          error={composerError}
          autoFocus={interactive}
          focusSignal={composerFocusSignal}
        />
      </main>
      {overlay}
    </div>
  );
}
