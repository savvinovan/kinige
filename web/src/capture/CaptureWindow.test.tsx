import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { MockApi, type MockApiOptions, mockMessage } from "../api/MockApi";
import type { MessageView } from "../api/generated/MessageView";
import { act, fireEvent, render, screen, waitFor } from "../test/render";
import { CaptureWindow, HIDE_REQUEST_EVENT, SHOWN_EVENT } from "./CaptureWindow";

// 2026-10-21 06:12 UTC; граница дня 04:00 — запись в 06:09 «сегодня».
const NOW = new Date(Date.UTC(2026, 9, 21, 6, 12));
const clock = { now: NOW };

function entry(id: string, minutesAgo: number, bodyMd: string): MessageView {
  const createdAt = NOW.getTime() - minutesAgo * 60_000;
  return mockMessage({ id, bodyMd, createdAt });
}

async function setup(options: MockApiOptions = {}, language: "ru" | "en" = "ru") {
  const api = new MockApi({ now: () => clock.now, language, ...options });
  const initial = await api.captureState();
  render(<CaptureWindow api={api} initial={initial} />, { language });
  return api;
}

const field = () => screen.getByRole("textbox") as HTMLTextAreaElement;

function type(text: string) {
  fireEvent.change(field(), { target: { value: text } });
}

beforeEach(() => {
  clock.now = NOW;
});
afterEach(() => {
  vi.useRealTimers();
});

describe("CaptureWindow на MockApi: сценарий слайса 2", () => {
  it("показывает контекст за сегодня и подсказку «⌘Enter — дописать» у последней записи", async () => {
    await setup({ messages: [entry("a", 8, "первая"), entry("b", 3, "вторая")] });
    expect(screen.getByText("первая")).toBeInTheDocument();
    expect(screen.getByText("вторая")).toBeInTheDocument();
    expect(screen.getAllByText("⌘Enter — дописать")).toHaveLength(1);
    expect(field()).toHaveFocus();
  });

  it("Enter: новая запись, окно прячется, поле пусто, никаких тостов и подтверждений", async () => {
    const api = await setup();
    type("Идея: попробовать");
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(api.hideCount).toBe(1));
    expect(field().value).toBe("");
    const state = await api.captureState();
    expect(state.context.map((m) => m.bodyMd)).toEqual(["Идея: попробовать"]);
    expect(document.querySelector(".toast, [role=status]")).toBeNull();
    expect(screen.queryByRole("alert", { name: /./ })).toBeNull();
  });

  it("⌘Enter в течение десяти минут: текст дописан новым абзацем в ту же запись, новой записи нет", async () => {
    const api = await setup({ messages: [entry("a", 3, "Идея: попробовать")] });
    type("и ещё");
    fireEvent.keyDown(field(), { key: "Enter", metaKey: true });
    await waitFor(() => expect(api.hideCount).toBe(1));
    const { context } = await api.captureState();
    expect(context).toHaveLength(1);
    expect(context[0]?.id).toBe("a");
    expect(context[0]?.bodyMd).toBe("Идея: попробовать\n\nи ещё");
  });

  it("через пятнадцать минут подсказки нет, а ⌘Enter даёт новую запись", async () => {
    const api = await setup({ messages: [entry("a", 15, "давняя")] });
    expect(screen.queryByText("⌘Enter — дописать")).toBeNull();
    type("новая");
    fireEvent.keyDown(field(), { key: "Enter", ctrlKey: true });
    await waitFor(() => expect(api.hideCount).toBe(1));
    const { context } = await api.captureState();
    expect(context.map((m) => m.bodyMd)).toEqual(["давняя", "новая"]);
  });

  it("при пустом Inbox за сегодня подсказки нет, ⌘Enter создаёт запись", async () => {
    const api = await setup({ messages: [{ ...entry("old", 5, "вчера"), journalDate: "2026-10-20" }] });
    expect(screen.queryByText("⌘Enter — дописать")).toBeNull();
    type("сегодня");
    fireEvent.keyDown(field(), { key: "Enter", metaKey: true });
    await waitFor(() => expect(api.hideCount).toBe(1));
    expect((await api.captureState()).context.map((m) => m.bodyMd)).toEqual(["сегодня"]);
  });

  it("пустой или пробельный текст не отправляется", async () => {
    const api = await setup();
    type("   ");
    fireEvent.keyDown(field(), { key: "Enter" });
    fireEvent.keyDown(field(), { key: "Enter", metaKey: true });
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(api.hideCount).toBe(0);
  });

  it("Esc: черновик сохранён, окно спрятано; при следующем показе текст на месте", async () => {
    const api = await setup();
    type("недописанная мысль");
    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(api.hideCount).toBe(1));
    expect((await api.captureState()).draft).toBe("недописанная мысль");
    expect(field().value).toBe("недописанная мысль");

    // Закрыли, открыли снова: страница перечитывает состояние.
    act(() => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
    });
    await waitFor(() => expect(field().value).toBe("недописанная мысль"));
  });

  it("просьба Rust спрятаться (хоткей при открытом окне, потеря фокуса) тоже сохраняет черновик", async () => {
    const api = await setup();
    type("писал и ушёл");
    act(() => {
      window.dispatchEvent(new Event(HIDE_REQUEST_EVENT));
    });
    await waitFor(() => expect(api.hideCount).toBe(1));
    expect((await api.captureState()).draft).toBe("писал и ушёл");
  });

  it("черновик уходит в Rust через ~300 мс после последнего нажатия, а не на каждый символ", async () => {
    vi.useFakeTimers();
    const api = await setup();
    const save = vi.spyOn(api, "captureSaveDraft");
    type("а");
    type("аб");
    type("абв");
    await vi.advanceTimersByTimeAsync(299);
    expect(save).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(2);
    expect(save).toHaveBeenCalledTimes(1);
    expect(save).toHaveBeenCalledWith("абв");
  });

  it("отправка отменяет отложенный черновик: поздняя запись не вернёт отправленный текст", async () => {
    vi.useFakeTimers();
    const api = await setup();
    const save = vi.spyOn(api, "captureSaveDraft");
    type("отправлю");
    fireEvent.keyDown(field(), { key: "Enter" });
    await vi.advanceTimersByTimeAsync(1000);
    expect(save).not.toHaveBeenCalled();
    expect((await api.captureState()).draft).toBe("");
  });

  it("ошибка записи: текст остаётся в поле, над ним ошибка, окно не прячется; повтор проходит", async () => {
    const api = await setup();
    api.failNextCapture = "store_write_failed";
    type("важное");
    fireEvent.keyDown(field(), { key: "Enter" });
    // Область alert есть в дереве всегда (пустая), текст приходит в неё позже.
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Не удалось сохранить запись. Текст остался в поле."));
    expect(field().value).toBe("важное");
    expect(api.hideCount).toBe(0);

    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(api.hideCount).toBe(1));
    expect((await api.captureState()).context.map((m) => m.bodyMd)).toEqual(["важное"]);
  });

  it("слишком длинный результат дописывания — ошибка по коду, текст в поле", async () => {
    const api = await setup({ messages: [entry("a", 1, "основа")] });
    api.failNextCapture = "message_too_large";
    type("ещё");
    fireEvent.keyDown(field(), { key: "Enter", metaKey: true });
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Запись слишком длинная."));
    expect(field().value).toBe("ещё");
  });

  it("ошибка исчезает, как только человек правит текст", async () => {
    const api = await setup();
    api.failNextCapture = "store_write_failed";
    type("текст");
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText(/Не удалось сохранить запись/);
    type("текст!");
    expect(screen.queryByText(/Не удалось сохранить запись/)).toBeNull();
  });

  it("при показе контекст и подсказка обновляются: запись появилась из главного окна или буфера", async () => {
    const api = await setup();
    expect(screen.queryByText("из буфера")).toBeNull();
    api.addExternalMessage(entry("clip", 1, "из буфера"));
    act(() => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
    });
    expect(await screen.findByText("из буфера")).toBeInTheDocument();
    expect(screen.getByText("⌘Enter — дописать")).toBeInTheDocument();
  });

  it("окно, открытое из-за сбоя буфера, показывает текст буфера и ошибку", async () => {
    const api = new MockApi({ now: () => clock.now });
    const initial = { ...(await api.captureState()), draft: "текст из буфера", error: "store_write_failed" as const };
    render(<CaptureWindow api={api} initial={initial} />);
    expect(field().value).toBe("текст из буфера");
    expect(screen.getByRole("alert")).toHaveTextContent("Не удалось сохранить запись");
  });

  it("по-английски: подсказка и ошибка", async () => {
    const api = await setup({ messages: [entry("a", 2, "first")] }, "en");
    expect(screen.getByText("⌘Enter — append")).toBeInTheDocument();
    api.failNextCapture = "store_write_failed";
    type("hello");
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not save the entry. The text is still in the field.");
  });

  it("размер текста применяется при показе: изменили в главном окне — окно захвата подхватило", async () => {
    const api = await setup();
    await api.setTextSize("large");
    act(() => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
    });
    await waitFor(() => expect(document.documentElement.getAttribute("data-text-scale")).toBe("large"));
    document.documentElement.removeAttribute("data-text-scale");
  });

  it("подсказка показала цель, к ⌘Enter прошло больше десяти минут: получается новая запись", async () => {
    const api = await setup({ messages: [entry("a", 3, "начало")] });
    expect(screen.getByText("⌘Enter — дописать")).toBeInTheDocument();
    clock.now = new Date(NOW.getTime() + 11 * 60_000);
    type("поздно");
    fireEvent.keyDown(field(), { key: "Enter", metaKey: true });
    await waitFor(() => expect(api.hideCount).toBe(1));
    const { context } = await api.captureState();
    expect(context.map((m) => m.bodyMd)).toEqual(["начало", "поздно"]);
  });

  it("восстановленный черновик: курсор в конце текста, при первом показе и при каждом следующем", async () => {
    const api = new MockApi({ now: () => clock.now });
    await api.captureSaveDraft("недописанная мысль");
    const initial = await api.captureState();
    render(<CaptureWindow api={api} initial={initial} />);
    expect(field().selectionStart).toBe("недописанная мысль".length);
    expect(field().selectionEnd).toBe("недописанная мысль".length);

    await api.captureSaveDraft("другой черновик");
    act(() => {
      window.dispatchEvent(new Event(SHOWN_EVENT));
    });
    await waitFor(() => expect(field().value).toBe("другой черновик"));
    await waitFor(() => expect(field().selectionStart).toBe("другой черновик".length));
  });

  describe("нажатие во время отправки и порядок сохранения черновика", () => {
    /** Отправка, которая завершается, когда тест её отпустит. */
    function holdSend(api: MockApi) {
      let release: () => void = () => undefined;
      const gate = new Promise<void>((resolve) => (release = resolve));
      const real = api.captureSend.bind(api);
      api.captureSend = (body) => gate.then(() => real(body));
      return release;
    }

    it("символ, набранный пока отправка в полёте, не возвращает отправленный текст черновиком", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const api = await setup();
      const save = vi.spyOn(api, "captureSaveDraft");
      const release = holdSend(api);
      type("отправляю");
      fireEvent.keyDown(field(), { key: "Enter" });
      // Отправка в полёте: нажатие не принимается и ничего не планирует.
      type("отправляю!");
      expect(field().value).toBe("отправляю");
      release();
      await waitFor(() => expect(api.hideCount).toBe(1));
      await vi.advanceTimersByTimeAsync(2000);
      expect(save).not.toHaveBeenCalled();
      expect((await api.captureState()).draft).toBe("");
      expect(field().value).toBe("");
    });

    it("уже отправленное сохранение черновика дожидается до отправки: порядок «сохранить, потом отправить»", async () => {
      vi.useFakeTimers({ shouldAdvanceTime: true });
      const api = await setup();
      const order: string[] = [];
      let finishSave: () => void = () => undefined;
      const realSave = api.captureSaveDraft.bind(api);
      api.captureSaveDraft = (body) =>
        new Promise<void>((resolve) => {
          order.push("save:start");
          finishSave = () => {
            void realSave(body).then(() => {
              order.push("save:done");
              resolve();
            });
          };
        });
      const realSend = api.captureSend.bind(api);
      api.captureSend = (body) => {
        order.push("send");
        return realSend(body);
      };
      type("текст");
      await vi.advanceTimersByTimeAsync(350); // таймер сработал: сохранение ушло и висит
      expect(order).toEqual(["save:start"]);
      fireEvent.keyDown(field(), { key: "Enter" });
      await vi.advanceTimersByTimeAsync(50);
      expect(order, "отправка ждёт сохранение").toEqual(["save:start"]);
      finishSave();
      await waitFor(() => expect(api.hideCount).toBe(1));
      expect(order).toEqual(["save:start", "save:done", "send"]);
      expect((await api.captureState()).draft, "отправленный текст не вернулся черновиком").toBe("");
    });
  });
});

describe("CaptureWindow: высота окна", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("высота содержимого уходит Rust командой capture_resize при показе страницы", async () => {
    vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
    const content = 143.2;
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({ height: content }) as DOMRect);
    const api = await setup();
    expect(api.resizeLog).toEqual([144]);
  });

  it("сбой изменения размера не ломает окно: писать можно", async () => {
    vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
    const content = 150;
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({ height: content }) as DOMRect);
    const api = new MockApi({ now: () => clock.now });
    api.captureResize = () => Promise.reject(new Error("нет окна"));
    const initial = await api.captureState();
    render(<CaptureWindow api={api} initial={initial} />);
    type("всё работает");
    expect(field().value).toBe("всё работает");
  });
});
