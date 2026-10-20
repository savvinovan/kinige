import { continuesSeries, formatDayLabel, isSameDay } from "./dates";

/** Сообщение для ленты (форма совпадает с тем, что отдаёт API; слайс 1 подставит настоящие типы). */
export interface FeedMessage {
  id: string;
  sentAt: Date;
  /** Текст в Markdown. */
  markdown: string;
  status?: "sent" | "failed";
  highlighted?: boolean;
}

export type FeedItem =
  | { kind: "day"; key: string; label: string }
  | { kind: "message"; key: string; message: FeedMessage; /** Продолжает серию: промежуток сверху меньше. */ inSeries: boolean };

/**
 * Лента: метка дня перед первым сообщением каждого дня, у каждого сообщения признак серии (пауза < 5 минут,
 * тот же день). Сообщения сортируются по времени; слияния в «страницы» нет, у каждого своё время (ТЗ §6).
 */
export function buildFeed(messages: readonly FeedMessage[], now: Date): FeedItem[] {
  const sorted = [...messages].sort((a, b) => a.sentAt.getTime() - b.sentAt.getTime());
  const items: FeedItem[] = [];
  let previous: FeedMessage | undefined;
  for (const message of sorted) {
    const newDay = !previous || !isSameDay(previous.sentAt, message.sentAt);
    if (newDay) items.push({ kind: "day", key: `day-${message.id}`, label: formatDayLabel(message.sentAt, now) });
    items.push({
      kind: "message",
      key: message.id,
      message,
      inSeries: previous !== undefined && continuesSeries(previous.sentAt, message.sentAt),
    });
    previous = message;
  }
  return items;
}
