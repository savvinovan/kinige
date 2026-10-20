import { useEffect, useState } from "react";
import type { DemoApi, DemoChat, DemoMessage } from "../../api/demo";
import { MainWindowView } from "./MainWindowView";

/**
 * ВРЕМЕННЫЙ экран слайса D1: главное окно на данных `MockApi` (чат «Входящие»). Слайс 1 заменяет его настоящим:
 * данные из Rust, выбор чата, отправка. Кнопка «Повторить» здесь только помечает сообщение отправленным.
 */
export function MainWindow({ api, now }: { api: DemoApi; now?: Date }) {
  const [chats, setChats] = useState<DemoChat[]>([]);
  const [messages, setMessages] = useState<DemoMessage[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [today] = useState(() => now ?? new Date());

  useEffect(() => {
    let current = true;
    Promise.all([api.listChats(), api.listMessages("inbox")])
      .then(([loadedChats, loadedMessages]) => {
        if (!current) return;
        setChats(loadedChats);
        setMessages(loadedMessages);
      })
      .catch((reason: unknown) => current && setError(String(reason)));
    return () => {
      current = false;
    };
  }, [api]);

  if (error) return <p role="alert">Не удалось загрузить: {error}</p>;

  return (
    <MainWindowView
      chats={chats}
      selectedChatId="inbox"
      messages={messages}
      now={today}
      onRetry={(id) => setMessages((all) => all.map((m) => (m.id === id ? { ...m, status: "sent" } : m)))}
    />
  );
}
