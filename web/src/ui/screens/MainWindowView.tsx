import { useState, type PointerEvent, type ReactNode } from "react";
import type { Language } from "../../api/generated/Language";
import type { TrashItem } from "../../api/generated/TrashItem";
import { useLanguage } from "../../i18n/I18n";
import { Composer, type ComposerButtons } from "../composer/Composer";
import { Feed, type FeedActions, type FeedUiState } from "../feed/Feed";
import type { FeedMessage } from "../feed/buildFeed";
import type { MoveTarget } from "../message/moveTargets";
import { type ChatAction, Sidebar, type SidebarChat } from "../sidebar/Sidebar";
import { TagFilterBar } from "../tag/TagFilterBar";
import { TrashView } from "../trash/TrashView";
import "./main-window.css";

/** Свёртка сайдбара, создание чата, меню чата и корзина. */
export interface SidebarControls {
  collapsed: boolean;
  onToggleCollapsed: () => void;
  onCreateChat: () => void;
  onChatAction: (chatId: string, action: ChatAction) => void;
  trashCount: number;
  onOpenTrash: () => void;
}

/** Корзина на месте ленты. */
export interface TrashControls {
  items: readonly TrashItem[];
  today: string;
  busyId?: string | null;
  error?: string;
  onRestore: (id: string) => void;
  onPurge: (id: string) => void;
}

interface MainWindowViewProps {
  chats: readonly SidebarChat[];
  /** Выбранный чат; `null` — открыта корзина. */
  selectedChatId: string | null;
  messages: readonly FeedMessage[];
  /** Сегодняшний `journal_date` для меток «Сегодня» и «Вчера»: в приложении его присылает Rust, в историях он фиксирован. */
  today: string;
  onSelectChat?: (chatId: string) => void;
  onRetry?: (messageId: string) => void;
  onOpenLink?: (url: string) => void;
  /** Какие кнопки показывать в поле ввода. По умолчанию все три нарисованы и объявлены недоступными («Появится позже»), как в D1; приложение показывает только рабочие. */
  composerButtons?: ComposerButtons;
  /** Поле ввода работает (приложение): без него оно только нарисовано. */
  interactive?: boolean;
  /** Текст в поле ввода. Без него вид хранит черновик сам (истории). */
  draft?: string;
  onDraftChange?: (value: string) => void;
  onSubmit?: () => void;
  /** Ошибка над полем ввода (уже переведённый текст). */
  composerError?: string;
  /** Эмодзи-пикер в поле ввода: недавние, язык и «выбрано» (запомнить в недавних). */
  emoji?: { recent: readonly string[]; language: Language; onPicked?: (emoji: string) => void };
  onOpenSettings?: () => void;
  /** Поле «Поиск» в сайдбаре: в приложении его нет до слайса 4. */
  showSearch?: boolean;
  sidebar?: SidebarControls;
  feedUi?: FeedUiState;
  feedActions?: FeedActions;
  /** Фильтр ленты по тегу: строка «#тег ×» над лентой. */
  tagFilter?: { tag: string; onClear: () => void };
  trash?: TrashControls;
  /** Чаты для «Переместить в чат» (текущий помечен). */
  moveTargets?: readonly MoveTarget[];
  /** Перетаскивание записи на чат (`useRecordDrag`): обработчик нажатия плашки, подсвеченный чат и «призрак». */
  drag?: {
    onDragStart: (messageId: string) => (event: PointerEvent<HTMLElement>, plate: HTMLElement) => void;
    dropTargetId: string | null;
    ghost: ReactNode;
  };
  /** Поверх окна: модальные окна (настройки, чат, календарь). */
  overlay?: ReactNode;
}

/**
 * Главное окно: сайдбар + лента (или корзина) + поле ввода. Строки с названием чата над лентой нет: выбранный чат выделен в
 * сайдбаре (§6). Только вид: данные приходят снаружи (в Storybook — сразу, в приложении — через `MainWindow`).
 */
export function MainWindowView({
  chats,
  selectedChatId,
  messages,
  today,
  onSelectChat,
  onRetry,
  onOpenLink,
  composerButtons,
  interactive = false,
  draft,
  onDraftChange,
  onSubmit,
  composerError,
  emoji,
  onOpenSettings,
  showSearch = true,
  sidebar,
  feedUi,
  feedActions,
  tagFilter,
  trash,
  moveTargets,
  drag,
  overlay,
}: MainWindowViewProps) {
  const [ownDraft, setOwnDraft] = useState("");
  const language = useLanguage();
  return (
    <div className="main-window">
      <Sidebar
        chats={chats}
        selectedId={selectedChatId}
        onSelect={onSelectChat}
        onOpenSettings={onOpenSettings}
        showSearch={showSearch}
        collapsed={sidebar?.collapsed}
        onToggleCollapsed={sidebar?.onToggleCollapsed}
        onCreateChat={sidebar?.onCreateChat}
        onChatAction={sidebar?.onChatAction}
        trashCount={sidebar?.trashCount}
        onOpenTrash={sidebar?.onOpenTrash}
        dropTargetId={drag?.dropTargetId}
      />
      <main className="main-window__chat">
        {selectedChatId === null && trash ? (
          <TrashView
            items={trash.items}
            language={language}
            today={trash.today}
            busyId={trash.busyId}
            error={trash.error}
            onRestore={trash.onRestore}
            onPurge={trash.onPurge}
          />
        ) : (
          <>
            {tagFilter ? <TagFilterBar tag={tagFilter.tag} onClear={tagFilter.onClear} /> : null}
            <Feed
              messages={messages}
              today={today}
              onRetry={onRetry}
              onOpenLink={onOpenLink}
              ui={feedUi}
              actions={feedActions}
              moveTargets={moveTargets}
              onDragStart={drag?.onDragStart}
            />
            <Composer
              key={selectedChatId ?? "trash"}
              value={draft ?? ownDraft}
              onChange={onDraftChange ?? setOwnDraft}
              onSubmit={onSubmit}
              buttons={composerButtons}
              viewOnly={!interactive}
              error={composerError}
              autoFocus={interactive}
              emoji={emoji}
            />
          </>
        )}
      </main>
      {drag?.ghost}
      {overlay}
    </div>
  );
}
