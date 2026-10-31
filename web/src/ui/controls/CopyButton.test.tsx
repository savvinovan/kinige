import { afterEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, waitFor } from "../../test/render";
import { COPIED_ICON_MS, CopyButton } from "./CopyButton";

afterEach(() => vi.useRealTimers());

const setup = () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", { value: { writeText }, configurable: true });
  render(<CopyButton text="/путь/к/файлу" label="Копировать путь" copiedLabel="Скопировано" failedLabel="Не удалось скопировать" />);
  return writeText;
};
const glyph = () => screen.getByRole("button").querySelector("svg")?.getAttribute("class") ?? "";

describe("Кнопка «Копировать» значком", () => {
  it("значок копирования с подписью и подсказкой; текста на кнопке нет", () => {
    setup();
    const button = screen.getByRole("button", { name: "Копировать путь" });
    expect(button).toHaveAttribute("title", "Копировать путь");
    expect(button.textContent).toBe("");
    expect(glyph()).toContain("lucide-copy");
  });

  it("нажатие кладёт текст в буфер, значок становится галочкой, подпись — «Скопировано»", async () => {
    const writeText = setup();
    fireEvent.click(screen.getByRole("button", { name: "Копировать путь" }));
    await waitFor(() => expect(writeText).toHaveBeenCalledWith("/путь/к/файлу"));
    const done = await screen.findByRole("button", { name: "Скопировано" });
    expect(done).toHaveAttribute("title", "Скопировано");
    expect(glyph()).toContain("lucide-check");
  });

  it("через полторы секунды возвращается значок копирования и прежняя подпись", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    expect(COPIED_ICON_MS).toBe(1500);
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Копировать путь" }));
    await screen.findByRole("button", { name: "Скопировано" });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1400);
    });
    expect(screen.getByRole("button", { name: "Скопировано" })).toBeInTheDocument();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(200);
    });
    expect(screen.getByRole("button", { name: "Копировать путь" })).toBeInTheDocument();
    expect(glyph()).toContain("lucide-copy");
  });

  it("буфер недоступен: значок ошибки и честная подпись «Не удалось скопировать»", async () => {
    Object.defineProperty(navigator, "clipboard", { value: { writeText: vi.fn().mockRejectedValue(new Error("нет")) }, configurable: true });
    render(<CopyButton text="x" label="Копировать" copiedLabel="Скопировано" failedLabel="Не удалось скопировать" />);
    fireEvent.click(screen.getByRole("button", { name: "Копировать" }));
    expect(await screen.findByRole("button", { name: "Не удалось скопировать" })).toBeInTheDocument();
    expect(glyph()).toContain("lucide-circle-alert");
  });

  it("ни тоста, ни статуса: подтверждение — только значок и подпись самой кнопки", async () => {
    setup();
    fireEvent.click(screen.getByRole("button", { name: "Копировать путь" }));
    await screen.findByRole("button", { name: "Скопировано" });
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
