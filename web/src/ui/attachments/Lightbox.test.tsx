import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { demoPhoto } from "./demo";
import { Lightbox } from "./Lightbox";
import { FULL_MAX_BYTES, MediaSourceContext } from "./mediaUrl";

const source = (variant: string, id: string) => `test://${variant}/${id}`;
const FOUR = ["a", "b", "c", "d"].map((id) => demoPhoto(id, `IMG_${id}.jpg`, 1280, 960));

function Harness({ start = 1, images = FOUR, onClose = vi.fn(), onOpenExternal = vi.fn() }) {
  const [index, setIndex] = useState(start);
  return (
    <MediaSourceContext.Provider value={source}>
      <Lightbox images={images} index={index} onIndexChange={setIndex} onClose={onClose} onOpenExternal={onOpenExternal} />
    </MediaSourceContext.Provider>
  );
}

const key = (name: string, init: KeyboardEventInit = {}) => fireEvent.keyDown(document, { key: name, ...init });

describe("Lightbox", () => {
  it("диалог с подписью, счётчик «2 / 4» и подпись фото", () => {
    render(<Harness />, { language: "en" });
    const dialog = screen.getByRole("dialog", { name: "Photo viewer" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(screen.getByText("2 / 4")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Photo IMG_b.jpg, 1280 by 960" })).toHaveAttribute("src", "test://full/b");
  });

  it("стрелки листают, на краях стоят на месте", () => {
    render(<Harness start={2} />, { language: "en" });
    key("ArrowRight");
    expect(screen.getByText("4 / 4")).toBeInTheDocument();
    key("ArrowRight");
    expect(screen.getByText("4 / 4")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next photo" })).toHaveAttribute("aria-disabled", "true");
    key("ArrowLeft");
    key("ArrowLeft");
    key("ArrowLeft");
    key("ArrowLeft");
    expect(screen.getByText("1 / 4")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Previous photo" })).toHaveAttribute("aria-disabled", "true");
  });

  it("кнопки вперёд и назад", () => {
    render(<Harness start={0} />, { language: "en" });
    fireEvent.click(screen.getByRole("button", { name: "Next photo" }));
    expect(screen.getByText("2 / 4")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Previous photo" }));
    expect(screen.getByText("1 / 4")).toBeInTheDocument();
  });

  it("стрелка с модификатором не листает", () => {
    render(<Harness start={1} />, { language: "en" });
    key("ArrowRight", { shiftKey: true });
    expect(screen.getByText("2 / 4")).toBeInTheDocument();
  });

  it("Escape и крестик закрывают", () => {
    const onClose = vi.fn();
    render(<Harness onClose={onClose} />, { language: "en" });
    key("Escape");
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("«Открыть в программе по умолчанию» отдаёт id показанного фото", () => {
    const onOpenExternal = vi.fn();
    render(<Harness onOpenExternal={onOpenExternal} />, { language: "en" });
    key("ArrowRight");
    fireEvent.click(screen.getByRole("button", { name: "Open in default app" }));
    expect(onOpenExternal).toHaveBeenCalledWith("c");
  });

  it("одно фото: без счётчика и без стрелок", () => {
    render(<Harness images={[FOUR[0] as (typeof FOUR)[number]]} start={0} />, { language: "en" });
    expect(screen.queryByText(/\//)).toBeNull();
    expect(screen.queryByRole("button", { name: "Next photo" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Previous photo" })).toBeNull();
  });

  it("фокус: Tab ходит по кругу внутри окна в обе стороны", () => {
    render(<Harness start={1} />, { language: "en" });
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveFocus();
    // The toolbar reads left to right: open, rotate, zoom out, the percentage, zoom in, close; then the arrows (KINIG-87).
    const order = [
      "Open in default app",
      "Rotate left",
      "Zoom out",
      "Fit to window, zoom 100%",
      "Zoom in",
      "Close",
      "Previous photo",
      "Next photo",
    ].map((name) => screen.getByRole("button", { name }));
    key("Tab");
    expect(order[0]).toHaveFocus();
    for (const next of order.slice(1)) {
      key("Tab");
      expect(next).toHaveFocus();
    }
    key("Tab");
    expect(order[0]).toHaveFocus();
    key("Tab", { shiftKey: true });
    expect(order[order.length - 1]).toHaveFocus();
  });

  it("фокус не уходит за окно", () => {
    render(
      <>
        <button type="button">outside</button>
        <Harness />
      </>,
      { language: "en" },
    );
    act(() => screen.getByRole("button", { name: "outside" }).focus());
    expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
  });

  it("после закрытия фокус возвращается на то, что открыло окно", () => {
    function Opener() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            opener
          </button>
          {open ? <Lightbox images={FOUR} index={0} onIndexChange={vi.fn()} onClose={() => setOpen(false)} onOpenExternal={vi.fn()} /> : null}
        </>
      );
    }
    render(
      <MediaSourceContext.Provider value={source}>
        <Opener />
      </MediaSourceContext.Provider>,
      { language: "en" },
    );
    const opener = screen.getByRole("button", { name: "opener" });
    act(() => opener.focus());
    fireEvent.click(opener);
    expect(screen.getByRole("dialog")).toHaveFocus();
    key("Escape");
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(opener).toHaveFocus();
  });

  it("полный размер для подходящего файла, миниатюра для большого или другого формата", () => {
    const big = { ...FOUR[0], byteSize: FULL_MAX_BYTES + 1 } as (typeof FOUR)[number];
    const heic = { ...FOUR[1], mime: "image/heic" } as (typeof FOUR)[number];
    const { unmount } = render(<Harness images={[big]} start={0} />);
    expect(screen.getByRole("img")).toHaveAttribute("src", "test://thumb/a");
    unmount();
    const second = render(<Harness images={[heic]} start={0} />);
    expect(screen.getByRole("img")).toHaveAttribute("src", "test://thumb/b");
    second.unmount();
    render(<Harness images={[FOUR[2] as (typeof FOUR)[number]]} start={0} />);
    expect(screen.getByRole("img")).toHaveAttribute("src", "test://full/c");
  });

  it("сбой полного размера переходит на миниатюру, сбой миниатюры даёт «Нет превью»", () => {
    render(<Harness images={[FOUR[0] as (typeof FOUR)[number]]} start={0} />, { language: "en" });
    fireEvent.error(screen.getByRole("img"));
    expect(screen.getByRole("img")).toHaveAttribute("src", "test://thumb/a");
    fireEvent.error(screen.getByRole("img"));
    expect(screen.queryByRole("img")).toBeNull();
    expect(screen.getByText("No preview")).toBeInTheDocument();
    // «Открыть в программе по умолчанию» остаётся.
    expect(screen.getByRole("button", { name: "Open in default app" })).toBeInTheDocument();
  });

  it("пропавший файл: «Файла нет» вместо картинки", () => {
    const gone = demoPhoto("gone", "gone.jpg", 100, 100, { missing: true });
    render(<Harness images={[gone]} start={0} />, { language: "en" });
    expect(screen.getByText("File is missing")).toBeInTheDocument();
    expect(screen.queryByRole("img")).toBeNull();
  });

  it("индекс за границами не ломает окно", () => {
    render(<Harness start={99} />, { language: "en" });
    expect(screen.getByText("4 / 4")).toBeInTheDocument();
  });
});
