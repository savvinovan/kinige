import { useRef } from "react";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";
import { installEmojiLayoutStub, StubEmojiData } from "../../test/emoji";
import { fireEvent, render, screen, within } from "../../test/render";
import { EmojiPopover } from "./EmojiPopover";

let restoreLayout: () => void;
beforeAll(() => {
  restoreLayout = installEmojiLayoutStub();
});
afterAll(() => restoreLayout());

function Host({ onClose, onPick }: { onClose: (returnFocus: boolean) => void; onPick: (emoji: string) => void }) {
  const anchor = useRef<HTMLButtonElement>(null);
  return (
    <StubEmojiData>
      <button ref={anchor}>кнопка</button>
      <button>другое место</button>
      <EmojiPopover recent={["\u{1F4DA}"]} language="ru" anchor={anchor} onClose={onClose} onPick={onPick} />
    </StubEmojiData>
  );
}

const setup = () => {
  const onClose = vi.fn();
  const onPick = vi.fn();
  render(<Host onClose={onClose} onPick={onPick} />);
  return { onClose, onPick, panel: screen.getByRole("dialog", { name: "Выбор эмодзи" }) };
};

describe("EmojiPopover", () => {
  it("панель-диалог лежит в body (поверх окна, ничего не сдвигает) и ставит фокус в поиск", () => {
    const { panel } = setup();
    expect(panel.parentElement).toBe(document.body);
    expect(within(panel).getByRole("searchbox", { name: "Найти эмодзи" })).toHaveFocus();
  });

  it("Esc закрывает с возвратом фокуса и не уходит дальше (окно под ней не закрывается)", () => {
    const { onClose, panel } = setup();
    const outer = vi.fn();
    document.addEventListener("keydown", outer);
    fireEvent.keyDown(within(panel).getByRole("searchbox"), { key: "Escape" });
    document.removeEventListener("keydown", outer);
    expect(onClose).toHaveBeenCalledExactlyOnceWith(true);
    expect(outer).not.toHaveBeenCalled();
  });

  it("Tab закрывает с возвратом фокуса: панель в конце документа, обхода по ней нет", () => {
    const { onClose, panel } = setup();
    fireEvent.keyDown(within(panel).getByRole("searchbox"), { key: "Tab" });
    expect(onClose).toHaveBeenCalledExactlyOnceWith(true);
  });

  it("щелчок мимо закрывает без возврата фокуса; по панели и по кнопке-якорю — не закрывает", () => {
    const { onClose, panel } = setup();
    fireEvent.pointerDown(within(panel).getByRole("searchbox"));
    fireEvent.pointerDown(screen.getByRole("button", { name: "кнопка" }));
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.pointerDown(screen.getByRole("button", { name: "другое место" }));
    expect(onClose).toHaveBeenCalledExactlyOnceWith(false);
  });

  it("смена размера и уход из окна закрывают без возврата фокуса", () => {
    const { onClose } = setup();
    fireEvent(window, new Event("resize"));
    fireEvent(window, new Event("blur"));
    expect(onClose).toHaveBeenCalledTimes(2);
    expect(onClose).toHaveBeenNthCalledWith(1, false);
    expect(onClose).toHaveBeenNthCalledWith(2, false);
  });

  it("выбор из «Недавних» вызывает onPick", () => {
    const { onPick, panel } = setup();
    fireEvent.click(within(panel).getByRole("button", { name: "\u{1F4DA}" }));
    expect(onPick).toHaveBeenCalledWith("\u{1F4DA}");
  });

  it("слушатели снимаются при закрытии", () => {
    const onClose = vi.fn();
    const { unmount } = render(<Host onClose={onClose} onPick={vi.fn()} />);
    unmount();
    fireEvent.pointerDown(document.body);
    fireEvent(window, new Event("resize"));
    expect(onClose).not.toHaveBeenCalled();
  });
});
