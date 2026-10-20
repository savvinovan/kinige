import { useState } from "react";
import { Composer, type ComposerButtons } from "../composer/Composer";
import { Feed } from "../feed/Feed";
import type { FeedMessage } from "../feed/buildFeed";
import { Sidebar, type SidebarChat } from "../sidebar/Sidebar";
import "./main-window.css";

interface MainWindowViewProps {
  chats: readonly SidebarChat[];
  selectedChatId: string;
  messages: readonly FeedMessage[];
  /** «Сейчас» для меток дней: в историях фиксируется. */
  now: Date;
  onRetry?: (messageId: string) => void;
  /** Какие кнопки показывать в поле ввода; в D1 все три нарисованы и объявлены недоступными («Появится позже»). */
  composerButtons?: ComposerButtons;
}

/**
 * Главное окно: сайдбар + лента + поле ввода. Строки с названием чата над лентой нет: выбранный чат выделен в сайдбаре (§6).
 * Только вид: данные приходят снаружи (в Storybook — сразу, в приложении — через `MainWindow`).
 */
export function MainWindowView({ chats, selectedChatId, messages, now, onRetry, composerButtons }: MainWindowViewProps) {
  const [draft, setDraft] = useState("");
  return (
    <div className="main-window">
      <Sidebar chats={chats} selectedId={selectedChatId} />
      <main className="main-window__chat">
        <Feed messages={messages} now={now} onRetry={onRetry} />
        <Composer value={draft} onChange={setDraft} buttons={composerButtons} viewOnly />
      </main>
    </div>
  );
}
