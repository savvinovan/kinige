import { afterEach, describe, expect, it, vi } from "vitest";
import { INBOX_ID } from "../../api/KinigeApi";
import { DEFAULT_SETTINGS, MockApi, type MockApiOptions, mockMessage } from "../../api/MockApi";
import type { MessageView } from "../../api/generated/MessageView";
import type { Settings } from "../../api/generated/Settings";
import { act, fireEvent, render, screen, waitFor } from "../../test/render";
import { DEFAULT_SHORTCUTS } from "../settings/demoShortcuts";
import { MainWindow } from "./MainWindow";

const SETTINGS: Settings = {
  ...DEFAULT_SETTINGS,
  mirror: { folder: "/Users/me/Documents/Kinige", isDefault: true, error: null, pendingDays: 0 },
  shortcuts: DEFAULT_SHORTCUTS,
};
const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));

function setup(options: MockApiOptions = {}) {
  const api = new MockApi({ now: NOW, ...options });
  render(<MainWindow api={api} initialSettings={SETTINGS} />);
  return api;
}

const field = () => screen.getByRole("textbox", { name: "Новая заметка" }) as HTMLTextAreaElement;
const type = (text: string) => fireEvent.change(field(), { target: { value: text } });

afterEach(() => {
  vi.useRealTimers();
  document.documentElement.removeAttribute("data-text-scale");
});

describe("MainWindow: черновик поля ввода (§5)", () => {
  it("сохранённый черновик подставляется в поле при запуске", async () => {
    const api = new MockApi({ now: NOW });
    await api.saveDraft(INBOX_ID, "недописанное с прошлого раза");
    render(<MainWindow api={api} initialSettings={SETTINGS} />);
    await waitFor(() => expect(field().value).toBe("недописанное с прошлого раза"));
  });

  it("то, что человек уже набрал, ответ с черновиком не затирает", async () => {
    const api = new MockApi({ now: NOW });
    await api.saveDraft(INBOX_ID, "старый");
    let release: (value: string) => void = () => undefined;
    api.getDraft = () => new Promise<string>((resolve) => (release = resolve));
    render(<MainWindow api={api} initialSettings={SETTINGS} />);
    await screen.findByRole("textbox", { name: "Новая заметка" });
    type("уже печатаю");
    release("старый");
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(field().value).toBe("уже печатаю");
  });

  it("черновик уходит в Rust через ~300 мс после последнего нажатия", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const api = setup();
    const save = vi.spyOn(api, "saveDraft");
    await screen.findByRole("textbox", { name: "Новая заметка" });
    type("а");
    type("аб");
    await vi.advanceTimersByTimeAsync(250);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(100);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith(INBOX_ID, "аб");
    expect(await api.getDraft(INBOX_ID)).toBe("аб");
  });

  it("потеря фокуса окном сохраняет черновик сразу", async () => {
    const api = setup();
    await screen.findByRole("textbox", { name: "Новая заметка" });
    type("не успел дописать");
    expect(await api.getDraft(INBOX_ID)).toBe("");
    fireEvent.blur(window);
    await waitFor(async () => expect(await api.getDraft(INBOX_ID)).toBe("не успел дописать"));
  });

  it("отправка очищает черновик, и запоздавшее сохранение его не возвращает", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const api = setup();
    await screen.findByRole("textbox", { name: "Новая заметка" });
    type("отправляю");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByRole("article");
    await vi.advanceTimersByTimeAsync(1000);
    expect(await api.getDraft(INBOX_ID)).toBe("");
    expect(field().value).toBe("");
  });

  it("при ошибке отправки текст остаётся в поле, а черновик сохраняется", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const api = setup();
    await screen.findByRole("textbox", { name: "Новая заметка" });
    api.failNextSend = "store_write_failed";
    type("не отправится");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByRole("alert");
    await vi.advanceTimersByTimeAsync(500);
    expect(field().value).toBe("не отправится");
    expect(await api.getDraft(INBOX_ID)).toBe("не отправится");
  });
});

describe("MainWindow: нажатие во время отправки и порядок сохранения черновика", () => {
  /** Отправка, которая завершается, когда тест её отпустит. */
  function holdSend(api: MockApi) {
    let release: () => void = () => undefined;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const real = api.sendMessage.bind(api);
    api.sendMessage = (chatId, body) => gate.then(() => real(chatId, body));
    return release;
  }

  it("символ, набранный пока отправка в полёте, не возвращает отправленный текст черновиком", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const api = setup();
    const save = vi.spyOn(api, "saveDraft");
    await screen.findByRole("textbox", { name: "Новая заметка" });
    const release = holdSend(api);
    type("отправляю");
    fireEvent.keyDown(field(), { key: "Enter" });
    type("отправляю!");
    expect(field().value, "ввод в полёте не принимается").toBe("отправляю");
    release();
    await screen.findByRole("article");
    await vi.advanceTimersByTimeAsync(2000);
    expect(save).not.toHaveBeenCalled();
    expect(await api.getDraft(INBOX_ID)).toBe("");
    expect(field().value).toBe("");
  });

  it("уже отправленное сохранение дожидается до отправки: порядок «сохранить, потом отправить»", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const api = setup();
    await screen.findByRole("textbox", { name: "Новая заметка" });
    const order: string[] = [];
    let finishSave: () => void = () => undefined;
    const realSave = api.saveDraft.bind(api);
    api.saveDraft = (chatId, body) =>
      new Promise<void>((resolve) => {
        order.push("save:start");
        finishSave = () => {
          void realSave(chatId, body).then(() => {
            order.push("save:done");
            resolve();
          });
        };
      });
    const realSend = api.sendMessage.bind(api);
    api.sendMessage = (chatId, body) => {
      order.push("send");
      return realSend(chatId, body);
    };
    type("текст");
    await vi.advanceTimersByTimeAsync(350);
    expect(order).toEqual(["save:start"]);
    fireEvent.keyDown(field(), { key: "Enter" });
    await vi.advanceTimersByTimeAsync(50);
    expect(order, "отправка ждёт сохранение").toEqual(["save:start"]);
    finishSave();
    await screen.findByRole("article");
    expect(order).toEqual(["save:start", "save:done", "send"]);
    expect(await api.getDraft(INBOX_ID), "отправленный текст не вернулся черновиком").toBe("");
  });
});

describe("MainWindow: событие «записи изменились»", () => {
  const external: MessageView = mockMessage({
    id: "from-capture",
    bodyMd: "записано из окна захвата",
    createdAt: Date.UTC(2026, 9, 21, 6, 10),
  });

  it("запись из окна захвата появляется в ленте сразу, счётчик растёт", async () => {
    const api = setup();
    await screen.findByText(/Пока пусто/);
    api.addExternalMessage(external);
    act(() => api.emitEntriesChanged());
    expect(await screen.findByText("записано из окна захвата")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Входящие/ })).toHaveTextContent("1");
  });

  it("событие не сбрасывает уже загруженную историю: свежая страница сливается с ней", async () => {
    const mk = (id: string, minute: number): MessageView => ({
      ...external,
      id,
      bodyMd: `запись ${id}`,
      createdAt: Date.UTC(2026, 9, 21, 5, minute),
    });
    const old = [mk("a", 1), mk("b", 2)];
    const api = new MockApi({ now: NOW });
    // Первая загрузка: три записи и «есть более старые»; после события — страница короче (только свежие), тоже с «есть старые».
    const pages = [
      { messages: [...old, mk("c", 3)], older: { journalDate: "2026-10-21", createdAt: 1, id: "z" }, newer: null, today: "2026-10-21" },
      { messages: [mk("c", 3), mk("d", 4)], older: { journalDate: "2026-10-21", createdAt: 1, id: "z" }, newer: null, today: "2026-10-21" },
    ];
    api.listMessages = () => Promise.resolve(pages.shift() ?? pages[0]!);
    render(<MainWindow api={api} initialSettings={SETTINGS} />);
    await screen.findByText("запись a");
    act(() => api.emitEntriesChanged());
    expect(await screen.findByText("запись d")).toBeInTheDocument();
    for (const id of ["a", "b", "c"]) expect(screen.getByText(`запись ${id}`)).toBeInTheDocument();
  });

  it("дописанный текст виден в той же плашке, а не отдельной записью", async () => {
    const api = setup({ messages: [{ ...external, bodyMd: "Идея" }] });
    await screen.findByText("Идея");
    await api.captureAppend("и ещё");
    expect(await screen.findByText("и ещё")).toBeInTheDocument();
    expect(screen.getAllByRole("article")).toHaveLength(1);
  });

  it("после размонтирования подписка снята", async () => {
    const api = new MockApi({ now: NOW });
    const off = vi.fn();
    vi.spyOn(api, "onEntriesChanged").mockResolvedValue(off);
    const { unmount } = render(<MainWindow api={api} initialSettings={SETTINGS} />);
    await screen.findByRole("textbox", { name: "Новая заметка" });
    await waitFor(() => expect(api.onEntriesChanged).toHaveBeenCalled());
    unmount();
    expect(off).toHaveBeenCalled();
  });
});
