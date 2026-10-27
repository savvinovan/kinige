import type { FeedMessage } from "../ui/feed/buildFeed";
import type { ChatSummary } from "./generated/ChatSummary";
import { INBOX_ID } from "./KinigeApi";

// Записи и чаты из брифа концепта (design/concepts/brief-v2.md, раздел «Записи»): один набор для Storybook и тестов.
// Дни — относительно `now`, чтобы метки «Сегодня» и «Вчера» были верны. Время записей — в поясе UTC: снимки и тесты не
// зависят от часового пояса машины.

const two = (value: number): string => String(value).padStart(2, "0");

/** `YYYY-MM-DD` по местным компонентам даты. */
export function journalDateOf(date: Date): string {
  return `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
}

/** Запись демо-ленты: день записи и время `HH:MM` (UTC). */
export function demoEntry(
  id: string,
  journalDate: string,
  time: string,
  bodyMd: string,
  extra: { status?: FeedMessage["status"]; highlighted?: boolean } = {},
): FeedMessage {
  const [year, month, day] = journalDate.split("-").map(Number);
  const [hour, minute] = time.split(":").map(Number);
  return {
    id,
    journalDate,
    createdAt: Date.UTC(year ?? 1970, (month ?? 1) - 1, day ?? 1, hour ?? 0, minute ?? 0),
    createdTz: "UTC",
    bodyMd,
    ...extra,
  };
}

const CODE = ["```", "fn main() {", "    let today = Day::new(2026, 10, 20);", '    println!("Записей: {}", today.count());', "}", "```"].join("\n");

/** Записи чата «Входящие»: прошлый год, вчера, сегодня (серия из двух сообщений, задачи, код, не отправилось). */
export function demoMessages(now: Date): FeedMessage[] {
  const day = (offset: number) => journalDateOf(new Date(now.getFullYear(), now.getMonth(), now.getDate() - offset));
  const lastYear = `${now.getFullYear() - 1}-03-14`;
  return [
    demoEntry("m1", lastYear, "09:12", "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям."),
    demoEntry("m2", day(1), "08:40", "### Планы на неделю\nЗакончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда."),
    demoEntry("m3", day(1), "21:05", "> Лучше сделано, чем идеально.\n\nЗаписал, чтобы перечитать, когда начну полировать пятый день подряд."),
    demoEntry(
      "m4",
      day(0),
      "09:12",
      "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать.",
    ),
    demoEntry("m5", day(0), "09:14", "Ещё: позвонить маме и купить хлеб."),
    demoEntry("m6", day(0), "11:30", "### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [x] Почитать книгу"),
    demoEntry("m7", day(0), "13:45", `Починил историю командой \`git rebase --onto main\`, вот что получилось:\n\n${CODE}`),
    demoEntry("m9", day(0), "15:20", "Прочитал [The Psychology of Habits — fs.blog](https://fs.blog/habits) и записал вывод: **привычка — это петля**."),
    demoEntry("m8", day(0), "16:10", "Купить билеты на субботу", { status: "failed" }),
  ];
}

// Числа записей — демонстрационные, чтобы увидеть бейдж в разных видах (42, «1 234», «12 тыс.»); у «Входящих» — настоящее
// число демо-записей из ленты, чтобы счётчик совпадал с тем, что на экране.
export const DEMO_INBOX: ChatSummary = {
  id: INBOX_ID,
  title: "",
  icon: "📥",
  isInbox: true,
  count: demoMessages(new Date(0)).length,
  pinnedAt: null,
  archivedAt: null,
};

export const DEMO_CHATS: ChatSummary[] = [
  DEMO_INBOX,
  { id: "ideas", title: "Идеи", icon: "💡", isInbox: false, count: 42, pinnedAt: null, archivedAt: null },
  { id: "books", title: "Книги", icon: "📚", isInbox: false, count: 1234, pinnedAt: null, archivedAt: null },
  { id: "work", title: "Работа", icon: "💼", isInbox: false, count: 987, pinnedAt: null, archivedAt: null },
  { id: "recipes", title: "Рецепты", icon: "🍳", isInbox: false, count: 12345, pinnedAt: null, archivedAt: null },
];
