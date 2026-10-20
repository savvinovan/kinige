/**
 * Данные временного экрана D1 («Главное окно» на MockApi). Слайс 1 заменяет их настоящими типами из Rust
 * (ts-rs, `src/api/generated`) и настоящими командами; до тех пор эти типы живут только в `MockApi`.
 */
export interface DemoChat {
  id: string;
  name: string;
  /** Эмодзи чата — данные пользователя (`chats.icon`), системный эмодзи-шрифт. */
  emoji: string;
}

export interface DemoMessage {
  id: string;
  sentAt: Date;
  /** Текст в Markdown. */
  markdown: string;
  status?: "sent" | "failed";
  highlighted?: boolean;
}

/** Только для временного экрана D1: настоящий `KinigeApi` слайс 1 расширяет командами Rust. */
export interface DemoApi {
  listChats(): Promise<DemoChat[]>;
  listMessages(chatId: string): Promise<DemoMessage[]>;
}
