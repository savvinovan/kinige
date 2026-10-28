import { act, fireEvent, render, screen, within } from "../../test/render";
import { describe, expect, it, vi } from "vitest";
import type { TrashItem } from "../../api/generated/TrashItem";
import { TrashView } from "./TrashView";

const TODAY = "2026-10-27";

const item = (id: string, patch: Partial<TrashItem> = {}, body = "Купить хлеб", journalDate = "2026-10-20"): TrashItem => ({
  message: {
    id,
    chatId: "c1",
    bodyMd: body,
    journalDate,
    createdAt: Date.UTC(2026, 9, 20, 9, 12),
    createdTz: "UTC",
    isTask: false,
    taskDoneAt: null,
    favoriteAt: null,
    tags: [],
    readOnly: false,
    origin: "user",
  },
  chatTitle: "Книги",
  chatIcon: "📚",
  chatIsInbox: false,
  deletedAt: Date.UTC(2026, 9, 25),
  daysLeft: 28,
  ...patch,
});

type Props = Partial<React.ComponentProps<typeof TrashView>>;
const show = (props: Props = {}, language: "ru" | "en" = "ru") => {
  const onRestore = vi.fn();
  const onPurge = vi.fn();
  const view = render(
    <TrashView items={[item("m1")]} language={language} today={TODAY} onRestore={onRestore} onPurge={onPurge} {...props} />,
    { language },
  );
  return { onRestore, onPurge, ...view };
};
const flush = () => act(async () => void (await Promise.resolve()));

describe("TrashView: список", () => {
  it("плашка: эмодзи и имя чата, день, «осталось N дн.», текст записи, две кнопки; заголовка и «Очистить корзину» нет", () => {
    show();
    const region = screen.getByRole("region", { name: "Корзина" });
    expect(within(region).getByText("📚")).toBeInTheDocument();
    expect(within(region).getByText("Книги")).toBeInTheDocument();
    expect(region).toHaveTextContent("20 октября · осталось 28 дней");
    expect(within(region).getByText("Купить хлеб")).toBeInTheDocument();
    expect(within(region).getByRole("button", { name: /^Восстановить/ })).toBeInTheDocument();
    expect(within(region).getByRole("button", { name: /^Удалить навсегда/ })).toBeInTheDocument();
    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.queryByRole("button", { name: /Очистить/ })).toBeNull();
  });

  it("пустая корзина — тихая строка", () => {
    show({ items: [] });
    expect(screen.getByText("Корзина пуста")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("метки дня как в ленте: Сегодня, Вчера, дата другого года", () => {
    show({ items: [item("a", {}, "раз", TODAY), item("b", {}, "два", "2026-10-26"), item("c", {}, "три", "2025-03-14")] });
    const rows = screen.getAllByRole("article").map((a) => a.querySelector(".trash-entry__when")?.textContent);
    expect(rows).toEqual(["Сегодня · осталось 28 дней", "Вчера · осталось 28 дней", "14 марта 2025 · осталось 28 дней"]);
  });

  it("склонение «осталось»: 1 день, 2 дня, 5 дней, 0 — «удалится сегодня»", () => {
    show({ items: [1, 2, 5, 0].map((daysLeft, i) => item(`m${i}`, { daysLeft })) });
    const when = screen.getAllByRole("article").map((a) => a.querySelector(".trash-entry__when")?.textContent);
    expect(when).toEqual([
      "20 октября · остался 1 день",
      "20 октября · осталось 2 дня",
      "20 октября · осталось 5 дней",
      "20 октября · удалится сегодня",
    ]);
  });

  it("английский: «Today · 1 day left», «deleted today»", () => {
    show({ items: [item("a", { daysLeft: 1 }, "x", TODAY), item("b", { daysLeft: 0 }, "y", TODAY)] }, "en");
    const when = screen.getAllByRole("article").map((a) => a.querySelector(".trash-entry__when")?.textContent);
    expect(when).toEqual(["Today · 1 day left", "Today · deleted today"]);
  });

  it("у Inbox имя — перевод («Входящие» / «Inbox»), у чата без эмодзи — 💬", () => {
    const inbox = item("a", { chatIsInbox: true, chatTitle: "", chatIcon: null });
    show({ items: [inbox] });
    expect(screen.getByText("Входящие")).toBeInTheDocument();
    expect(screen.getByText("💬")).toBeInTheDocument();
  });

  it("Inbox на английском", () => {
    show({ items: [item("a", { chatIsInbox: true, chatTitle: "", chatIcon: "📥" })] }, "en");
    expect(screen.getByText("Inbox")).toBeInTheDocument();
  });

  it("длинное имя чата целиком в title (обрезано многоточием в CSS)", () => {
    const long = "Очень длинное имя чата про всё на свете and English words";
    show({ items: [item("a", { chatTitle: long })] });
    expect(screen.getByTitle(long)).toBeInTheDocument();
  });

  it("текст записи отрисован как Markdown без остановок Tab и ссылок", () => {
    show({ items: [item("a", {}, "**жирный** [сайт](https://example.com) #идея\n\n- [ ] пункт")] });
    const text = document.querySelector(".trash-entry__text") as HTMLElement;
    expect(text.querySelector("strong")).toHaveTextContent("жирный");
    expect(text.querySelector("a")).toBeNull();
    expect(text.querySelector("[tabindex]")).toBeNull();
  });

  it("ошибка — role=alert над списком", () => {
    show({ error: "Запись не найдена." });
    expect(screen.getByRole("alert")).toHaveTextContent("Запись не найдена.");
  });
});

describe("TrashView: действия", () => {
  it("«Восстановить» вызывает onRestore(id) сразу", () => {
    const { onRestore } = show({ items: [item("m1"), item("m2")] });
    fireEvent.click(screen.getAllByRole("button", { name: /^Восстановить/ })[1] as HTMLElement);
    expect(onRestore).toHaveBeenCalledExactlyOnceWith("m2");
  });

  it("имена кнопок содержат видимый текст и называют чат", () => {
    show();
    expect(screen.getByRole("button", { name: "Восстановить запись из чата «Книги»" })).toHaveTextContent("Восстановить");
    expect(screen.getByRole("button", { name: "Удалить навсегда запись из чата «Книги»" })).toHaveTextContent("Удалить навсегда");
  });

  it("«Удалить навсегда» открывает подтверждение; onPurge ещё не вызван; фокус на «Отмена»", async () => {
    const { onPurge } = show();
    fireEvent.click(screen.getByRole("button", { name: /^Удалить навсегда/ }));
    const dialog = screen.getByRole("dialog", { name: "Удалить навсегда?" });
    expect(within(dialog).getByText("Запись нельзя будет вернуть.")).toBeInTheDocument();
    await flush();
    expect(within(dialog).getByRole("button", { name: "Отмена" })).toHaveFocus();
    expect(onPurge).not.toHaveBeenCalled();
  });

  it("«Отмена», Esc и кнопка закрытия не удаляют", async () => {
    const { onPurge } = show();
    const open = () => fireEvent.click(screen.getByRole("button", { name: /^Удалить навсегда запись/ }));
    open();
    fireEvent.click(screen.getByRole("button", { name: "Отмена" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    open();
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    open();
    fireEvent.click(screen.getByRole("button", { name: "Закрыть" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onPurge).not.toHaveBeenCalled();
  });

  it("подтверждение вызывает onPurge(id) и закрывает окно; для нужной записи", () => {
    const { onPurge } = show({ items: [item("m1"), item("m2")] });
    fireEvent.click(screen.getAllByRole("button", { name: /^Удалить навсегда запись/ })[1] as HTMLElement);
    const dialog = screen.getByRole("dialog");
    fireEvent.click(within(dialog).getByRole("button", { name: "Удалить навсегда" }));
    expect(onPurge).toHaveBeenCalledExactlyOnceWith("m2");
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("запись в работе (busyId): её кнопки недоступны и ничего не вызывают; соседняя работает", () => {
    const { onRestore, onPurge } = show({ items: [item("m1"), item("m2")], busyId: "m1" });
    const [restore1, restore2] = screen.getAllByRole("button", { name: /^Восстановить/ }) as [HTMLElement, HTMLElement];
    expect(restore1).toHaveAttribute("aria-disabled", "true");
    expect(restore2).not.toHaveAttribute("aria-disabled");
    fireEvent.click(restore1);
    fireEvent.click(screen.getAllByRole("button", { name: /^Удалить навсегда запись/ })[0] as HTMLElement);
    expect(onRestore).not.toHaveBeenCalled();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(onPurge).not.toHaveBeenCalled();
    fireEvent.click(restore2);
    expect(onRestore).toHaveBeenCalledWith("m2");
  });

  it("подтверждение на английском", () => {
    show({}, "en");
    fireEvent.click(screen.getByRole("button", { name: /^Delete forever/ }));
    const dialog = screen.getByRole("dialog", { name: "Delete forever?" });
    expect(within(dialog).getByText("The entry cannot be brought back.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeInTheDocument();
  });
});
