import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { DEFAULT_SETTINGS, MockApi, mockMessage } from "../../api/MockApi";
import { act, fireEvent, render, screen, waitFor, within } from "../../test/render";
import { MainWindow } from "./MainWindow";

// Лента на 2 000 записях (слайс 4, задача 4.14): подгрузка вверх без скачка, переход к записи из середины, подгрузка вниз,
// возврат в обычный режим, удалённая плашка переживает подгрузку, прокрутка без плавности. Плашки в jsdom рисуются долго,
// поэтому до конца всей истории интерфейс идёт на 450 записях, а 2 000 проверяют переход и цепочку страниц.
// Сотни плашек в jsdom рисуются долго: на загруженном CI запас больше обычных пяти секунд.
vi.setConfig({ testTimeout: 30_000 });

const NOW = () => new Date(Date.UTC(2026, 9, 21, 6, 12));
const HOUR = 3_600_000;
const START = Date.UTC(2024, 0, 1, 12);
const journalDate = (at: number) => new Date(at - 4 * HOUR).toISOString().slice(0, 10);

const make = (count: number) =>
  Array.from({ length: count }, (_, i) => {
    const createdAt = START + i * HOUR;
    return mockMessage({ id: `m${i}`, bodyMd: `запись ${i} слово${i}`, createdAt, journalDate: journalDate(createdAt) });
  });
const MANY = make(2000);

/** Подмена IntersectionObserver: тест сам «показывает» край списка. */
class FakeObserver {
  static all: FakeObserver[] = [];
  private readonly targets = new Set<Element>();
  constructor(private readonly callback: IntersectionObserverCallback) {
    FakeObserver.all.push(this);
  }
  observe(target: Element) {
    this.targets.add(target);
  }
  unobserve(target: Element) {
    this.targets.delete(target);
  }
  disconnect() {
    this.targets.clear();
    FakeObserver.all = FakeObserver.all.filter((o) => o !== this);
  }
  takeRecords() {
    return [];
  }
  fire(target: Element) {
    this.callback([{ isIntersecting: true, target } as IntersectionObserverEntry], this as unknown as IntersectionObserver);
  }
  has(target: Element) {
    return this.targets.has(target);
  }
}

/** В `column-reverse` первый в разметке край стоит внизу (новое), последний — вверху (старое). */
const edges = (container: HTMLElement) => [...container.querySelectorAll<HTMLElement>(".feed__edge")];

/** Край списка показался в области прокрутки. */
function showEdge(container: HTMLElement, which: "older" | "newer") {
  const list = edges(container);
  const edge = which === "older" ? list.at(-1) : list[0];
  const observer = FakeObserver.all.find((o) => edge && o.has(edge));
  if (!edge || !observer) throw new Error(`нет края «${which}»`);
  act(() => observer.fire(edge));
}

beforeEach(() => {
  FakeObserver.all = [];
  vi.stubGlobal("IntersectionObserver", FakeObserver);
});
afterEach(() => vi.unstubAllGlobals());

function setup(count = 2000) {
  const api = new MockApi({ now: NOW, messages: count === 2000 ? MANY : make(count) });
  const view = render(<MainWindow api={api} initialSettings={DEFAULT_SETTINGS} />);
  return { api, ...view };
}

const plateOf = (container: HTMLElement, id: string) =>
  container.querySelector<HTMLElement>(`[data-message-id="${id}"] article`);

async function openFromSearch(word: string) {
  fireEvent.change(screen.getByRole("combobox", { name: "Поиск" }), { target: { value: word } });
  fireEvent.click(await screen.findByRole("option", { name: new RegExp(word) }));
  await waitFor(() => expect(screen.queryByRole("region", { name: "Результаты поиска" })).toBeNull());
}

describe("Лента: старые страницы", () => {
  it("первая страница — 200 последних; край сверху подгружает следующие, нижняя запись остаётся, в конце истории края нет", async () => {
    const { container } = setup(450);
    await screen.findByText("запись 449 слово449");
    expect(screen.queryByText("запись 249 слово249")).toBeNull();
    expect(screen.getByText("запись 250 слово250")).toBeInTheDocument();
    expect(edges(container)).toHaveLength(1);
    showEdge(container, "older");
    await screen.findByText("запись 50 слово50");
    // Нижняя запись на месте: лента добавляет записи СВЕРХУ, а область `column-reverse` держит нижний край.
    expect(screen.getByText("запись 449 слово449")).toBeInTheDocument();
    expect(edges(container)).toHaveLength(1);
    showEdge(container, "older");
    await screen.findByText("запись 0 слово0");
    await waitFor(() => expect(edges(container)).toHaveLength(0));
    expect(container.querySelectorAll("article")).toHaveLength(450);
  });

  it("удалённая плашка «Удалено · Вернуть» переживает подгрузку старой страницы", async () => {
    const { container, api } = setup(450);
    await screen.findByText("запись 449 слово449");
    fireEvent.contextMenu(plateOf(container, "m440") as HTMLElement);
    fireEvent.click(within(screen.getByRole("menu")).getByRole("menuitem", { name: "Удалить" }));
    await screen.findByRole("button", { name: "Вернуть удалённую запись" });
    showEdge(container, "older");
    await screen.findByText("запись 50 слово50");
    expect(screen.getAllByRole("button", { name: "Вернуть удалённую запись" })).toHaveLength(1);
    expect(api.messageById("m440")).toBeDefined();
  });
});

describe("Лента: переход к записи из поиска", () => {
  it("страница вокруг записи; запись подсвечена, найденное слово выделено, прокрутка к центру без плавности; вверх и вниз подгружается", async () => {
    const scrolled: (ScrollIntoViewOptions | boolean | undefined)[] = [];
    const original = Element.prototype.scrollIntoView;
    Element.prototype.scrollIntoView = function (options?: boolean | ScrollIntoViewOptions) {
      if (this instanceof HTMLElement && this.dataset.messageId === "m500") scrolled.push(options);
    };
    try {
      const { container } = setup();
      await screen.findByText("запись 1999 слово1999");
      await openFromSearch("слово500");
      await waitFor(() => expect(plateOf(container, "m500")).not.toBeNull());
      const plate = plateOf(container, "m500") as HTMLElement;
      expect(plate).toHaveClass("message--highlighted");
      expect(plate.querySelector("mark")?.textContent).toBe("слово500");
      // Около ста записей до и ста после; конца истории на экране нет.
      expect(plateOf(container, "m450")).not.toBeNull();
      expect(plateOf(container, "m1999")).toBeNull();
      await waitFor(() => expect(scrolled.length).toBeGreaterThan(0));
      expect(scrolled.at(-1)).toEqual({ block: "center" });
      expect(edges(container)).toHaveLength(2);

      showEdge(container, "older");
      await waitFor(() => expect(plateOf(container, "m399")).not.toBeNull());
      showEdge(container, "newer");
      await waitFor(() => expect(plateOf(container, "m600")).not.toBeNull());
      expect(plateOf(container, "m500")).toHaveClass("message--highlighted");
    } finally {
      Element.prototype.scrollIntoView = original;
    }
  });

  it("дойдя до конца истории, лента снова обычная: нижнего края нет, новая запись снаружи появляется в конце", async () => {
    const { container, api } = setup(450);
    await screen.findByText("запись 449 слово449");
    await openFromSearch("слово100");
    await waitFor(() => expect(plateOf(container, "m100")).not.toBeNull());
    // Пока лента в середине истории, событие «записи изменились» её не трогает.
    api.addExternalMessage(
      mockMessage({ id: "ext1", bodyMd: "пришло снаружи", createdAt: Date.UTC(2026, 9, 21, 5, 55), journalDate: "2026-10-21" }),
    );
    act(() => api.emitEntriesChanged());
    await act(async () => void (await Promise.resolve()));
    expect(screen.queryByText("пришло снаружи")).toBeNull();
    // Вниз до конца истории: две страницы. Нижний край — первый в разметке области (`column-reverse`), до списка.
    const hasNewerEdge = () => container.querySelector(".feed-scroll")?.firstElementChild?.classList.contains("feed__edge") === true;
    expect(hasNewerEdge()).toBe(true);
    showEdge(container, "newer");
    await waitFor(() => expect(plateOf(container, "m300")).not.toBeNull());
    expect(hasNewerEdge()).toBe(true);
    showEdge(container, "newer");
    await waitFor(() => expect(plateOf(container, "m449")).not.toBeNull());
    await waitFor(() => expect(hasNewerEdge()).toBe(false));
    act(() => api.emitEntriesChanged());
    expect(await screen.findByText("пришло снаружи")).toBeInTheDocument();
  });

  it("подгрузка снизу не двигает читаемое: прокрутка смещается на добавленную высоту", async () => {
    const height = { value: 1000 };
    const heightDescriptor = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollHeight");
    Object.defineProperty(HTMLElement.prototype, "scrollHeight", { configurable: true, get: () => height.value });
    try {
      const { container } = setup();
      await screen.findByText("запись 1999 слово1999");
      await openFromSearch("слово500");
      await waitFor(() => expect(plateOf(container, "m500")).not.toBeNull());
      const scroller = container.querySelector(".feed-scroll") as HTMLElement;
      scroller.scrollTop = -300;
      showEdge(container, "newer");
      // К моменту отрисовки новых записей область стала выше на 700.
      height.value = 1700;
      await waitFor(() => expect(plateOf(container, "m600")).not.toBeNull());
      await waitFor(() => expect(scroller.scrollTop).toBe(-1000));
    } finally {
      if (heightDescriptor) Object.defineProperty(HTMLElement.prototype, "scrollHeight", heightDescriptor);
    }
  });
});
