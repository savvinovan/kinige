import type { EmojiDataResolver } from "frimousse";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { installEmojiLayoutStub, StubEmojiData } from "../../test/emoji";
import { fireEvent, render, screen, waitFor, within } from "../../test/render";
import { EmojiPicker } from "./EmojiPicker";

// Раскладку jsdom не считает, поэтому размеры области подставляет заглушка (test/emoji.ts): здесь проверяется обвязка
// (поиск, «Недавние», выбор, клавиатура), а вид и раскладку показывают истории Storybook.
let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

const NEVER_LOADED: EmojiDataResolver = () => new Promise(() => {});

const setup = (props: Partial<React.ComponentProps<typeof EmojiPicker>> = {}, resolver?: EmojiDataResolver) => {
  const onPick = vi.fn();
  const language = props.language ?? "ru";
  const view = render(
    <StubEmojiData resolver={resolver}>
      <EmojiPicker recent={[]} language="ru" onPick={onPick} {...props} />
    </StubEmojiData>,
    { language },
  );
  return { onPick, ...view };
};

describe("EmojiPicker: обвязка", () => {
  it("группа с подписью и поле поиска с подписью на языке интерфейса", () => {
    setup();
    expect(screen.getByRole("group", { name: "Эмодзи" })).toBeInTheDocument();
    expect(screen.getByRole("searchbox", { name: "Найти эмодзи" })).toBeInTheDocument();
  });

  it("подпись поиска по-английски", () => {
    setup({ language: "en" });
    expect(screen.getByRole("searchbox", { name: "Search emoji" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Emoji" })).toBeInTheDocument();
  });

  it("пока данные грузятся, показано состояние загрузки", () => {
    setup({}, NEVER_LOADED);
    expect(screen.getByText("Загрузка эмодзи…")).toBeInTheDocument();
  });

  it("autoFocus ставит фокус в поиск", () => {
    setup({ autoFocus: true });
    expect(screen.getByRole("searchbox")).toHaveFocus();
  });

  it("сетка рисуется: категории и эмодзи из данных", async () => {
    setup();
    const grid = await screen.findByRole("grid");
    expect(await within(grid).findByRole("gridcell", { name: "Плачет" })).toBeInTheDocument();
    expect(screen.getByText("Смайлики")).toBeInTheDocument();
    expect(screen.getByText("Животные")).toBeInTheDocument();
  });

  it("щелчок по эмодзи вызывает onPick", async () => {
    const { onPick } = setup();
    fireEvent.click(await screen.findByRole("gridcell", { name: "Морда собаки" }));
    expect(onPick).toHaveBeenCalledWith("\u{1F436}");
  });

  it("поиск по русскому названию и по английскому тегу; пустой результат — тихая строка", async () => {
    setup();
    const search = screen.getByRole("searchbox");
    await screen.findByRole("gridcell", { name: "Плачет" });
    fireEvent.change(search, { target: { value: "собак" } });
    await waitFor(() => expect(screen.queryByRole("gridcell", { name: "Плачет" })).toBeNull());
    expect(screen.getByRole("gridcell", { name: "Морда собаки" })).toBeInTheDocument();
    fireEvent.change(search, { target: { value: "cat" } });
    await screen.findByRole("gridcell", { name: "Морда кошки" });
    expect(screen.queryByRole("gridcell", { name: "Морда собаки" })).toBeNull();
    fireEvent.change(search, { target: { value: "ъъъ" } });
    expect(await screen.findByText("Ничего не найдено")).toBeInTheDocument();
  });

  it("клавиатура: стрелка вниз выбирает первую ячейку, Enter отдаёт её", async () => {
    const { onPick } = setup({ autoFocus: true });
    await screen.findByRole("gridcell", { name: "Плачет" });
    const search = screen.getByRole("searchbox");
    fireEvent.keyDown(search, { key: "ArrowDown" });
    await waitFor(() => expect(screen.getByRole("gridcell", { name: "Широко улыбается" })).toHaveAttribute("data-active"));
    fireEvent.keyDown(search, { key: "Enter" });
    expect(onPick).toHaveBeenCalledWith("\u{1F600}");
  });

  it("пока в поиске есть текст, стрелки влево и вправо не доходят до сетки (каретка остаётся за полем)", async () => {
    setup({ autoFocus: true });
    await screen.findByRole("gridcell", { name: "Плачет" });
    const search = screen.getByRole("searchbox");
    fireEvent.change(search, { target: { value: "соб" } });
    const stopped = !fireEvent.keyDown(search, { key: "ArrowLeft", cancelable: true });
    // Событие не погашено (preventDefault не вызван): каретка поля двигается обычным образом.
    expect(stopped).toBe(false);
  });
});

describe("EmojiPicker: «Недавние»", () => {
  it("секция над сеткой, до 24 эмодзи без повторов", () => {
    const recent = Array.from({ length: 30 }, (_, i) => String.fromCodePoint(0x1f600 + i));
    setup({ recent: [...recent, recent[0] ?? ""] });
    const section = screen.getByRole("region", { name: "Недавние" });
    expect(within(section).getAllByRole("button")).toHaveLength(24);
  });

  it("щелчок по недавнему вызывает onPick", () => {
    const { onPick } = setup({ recent: ["\u{1F4DA}", "\u{1F4A1}"] });
    fireEvent.click(within(screen.getByRole("region", { name: "Недавние" })).getByRole("button", { name: "\u{1F4A1}" }));
    expect(onPick).toHaveBeenCalledWith("\u{1F4A1}");
  });

  it("пустой список: секции нет", () => {
    setup({ recent: [] });
    expect(screen.queryByRole("region", { name: "Недавние" })).toBeNull();
  });

  it("пока идёт поиск, «Недавние» скрыты и возвращаются, когда поиск пуст", () => {
    setup({ recent: ["\u{1F4DA}"] });
    const search = screen.getByRole("searchbox");
    fireEvent.change(search, { target: { value: "соб" } });
    expect(screen.queryByRole("region", { name: "Недавние" })).toBeNull();
    fireEvent.change(search, { target: { value: "" } });
    expect(screen.getByRole("region", { name: "Недавние" })).toBeInTheDocument();
  });

  it("одна остановка Tab на секцию; стрелки и Home/End двигают фокус внутри", () => {
    setup({ recent: ["\u{1F4DA}", "\u{1F4A1}", "\u{1F373}"] });
    const buttons = within(screen.getByRole("region", { name: "Недавние" })).getAllByRole("button");
    expect(buttons.map((button) => button.getAttribute("tabindex"))).toEqual(["0", "-1", "-1"]);
    buttons[0]?.focus();
    fireEvent.keyDown(buttons[0] as HTMLElement, { key: "ArrowRight" });
    expect(buttons[1]).toHaveFocus();
    expect(buttons.map((button) => button.getAttribute("tabindex"))).toEqual(["-1", "0", "-1"]);
    fireEvent.keyDown(buttons[1] as HTMLElement, { key: "End" });
    expect(buttons[2]).toHaveFocus();
    fireEvent.keyDown(buttons[2] as HTMLElement, { key: "ArrowRight" });
    expect(buttons[2]).toHaveFocus();
    fireEvent.keyDown(buttons[2] as HTMLElement, { key: "Home" });
    expect(buttons[0]).toHaveFocus();
    fireEvent.keyDown(buttons[0] as HTMLElement, { key: "ArrowLeft" });
    expect(buttons[0]).toHaveFocus();
  });

  it("стрелка вниз в «Недавних» переходит на строку ниже (столбцов 9)", () => {
    const recent = Array.from({ length: 12 }, (_, i) => String.fromCodePoint(0x1f600 + i));
    setup({ recent });
    const buttons = within(screen.getByRole("region", { name: "Недавние" })).getAllByRole("button");
    buttons[1]?.focus();
    fireEvent.keyDown(buttons[1] as HTMLElement, { key: "ArrowDown" });
    expect(buttons[10]).toHaveFocus();
    fireEvent.keyDown(buttons[10] as HTMLElement, { key: "ArrowUp" });
    expect(buttons[1]).toHaveFocus();
    fireEvent.keyDown(buttons[1] as HTMLElement, { key: "ArrowUp" });
    expect(buttons[1]).toHaveFocus();
  });
});
