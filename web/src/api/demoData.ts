import type { DemoChat, DemoMessage } from "./demo";

// Записи и чаты из брифа концепта (design/concepts/brief-v2.md, раздел «Записи»): один набор для Storybook,
// временного экрана приложения и тестов. Дни — относительно `now`, чтобы метки «Сегодня» и «Вчера» были верны.

const CODE = ["```", "fn main() {", "    let today = Day::new(2026, 10, 20);", '    println!("Записей: {}", today.count());', "}", "```"].join("\n");

/** Записи чата «Входящие»: прошлый год, вчера, сегодня (серия из двух сообщений, задачи, код, не отправилось). */
export function demoMessages(now: Date): DemoMessage[] {
  const day = (offset: number, hour: number, minute: number) =>
    new Date(now.getFullYear(), now.getMonth(), now.getDate() - offset, hour, minute);
  return [
    {
      id: "m1",
      sentAt: new Date(now.getFullYear() - 1, 2, 14, 9, 12),
      markdown: "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям.",
    },
    {
      id: "m2",
      sentAt: day(1, 8, 40),
      markdown: "### Планы на неделю\nЗакончить основу дизайн-системы, выбрать шрифты, не открывать почту до обеда.",
    },
    {
      id: "m3",
      sentAt: day(1, 21, 5),
      markdown: "> Лучше сделано, чем идеально.\n\nЗаписал, чтобы перечитать, когда начну полировать пятый день подряд.",
    },
    {
      id: "m4",
      sentAt: day(0, 9, 12),
      markdown:
        "Проснулся рано и до завтрака записал мысль про журнал: _пишешь как в мессенджере, перечитываешь как книгу_. Главное — **не выбирать**, куда писать.",
    },
    { id: "m5", sentAt: day(0, 9, 14), markdown: "Ещё: позвонить маме и купить хлеб." },
    {
      id: "m6",
      sentAt: day(0, 11, 30),
      markdown: "### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [x] Почитать книгу",
    },
    {
      id: "m7",
      sentAt: day(0, 13, 45),
      markdown: `Починил историю командой \`git rebase --onto main\`, вот что получилось:\n\n${CODE}`,
    },
    {
      id: "m9",
      sentAt: day(0, 15, 20),
      markdown: "Прочитал [The Psychology of Habits — fs.blog](https://fs.blog/habits) и записал вывод: **привычка — это петля**.",
    },
    { id: "m8", sentAt: day(0, 16, 10), markdown: "Купить билеты на субботу", status: "failed" },
  ];
}

// Числа записей — демонстрационные, чтобы увидеть бейдж в разных видах (42, «1 234», «12 тыс.»); у «Входящих» — настоящее
// число демо-записей из ленты, чтобы счётчик совпадал с тем, что на экране.
export const DEMO_CHATS: DemoChat[] = [
  { id: "inbox", name: "Входящие", emoji: "📥", count: demoMessages(new Date(0)).length },
  { id: "ideas", name: "Идеи", emoji: "💡", count: 42 },
  { id: "books", name: "Книги", emoji: "📚", count: 1234 },
  { id: "work", name: "Работа", emoji: "💼", count: 987 },
  { id: "recipes", name: "Рецепты", emoji: "🍳", count: 12345 },
];
