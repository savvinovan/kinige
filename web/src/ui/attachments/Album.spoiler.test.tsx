import { useState, type ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen } from "../../test/render";
import { Album } from "./Album";
import { demoPhoto } from "./demo";
import { MediaSourceContext } from "./mediaUrl";
import { RevealProvider } from "./reveal";

// KINIG-136: a hidden photo in an album is a cover until its click, and it loads nothing before that. Labels are Russian (the default test language).

const asked: string[] = [];
const source = (variant: string, id: string) => {
  asked.push(`${variant}/${id}`);
  return `test://${variant}/${id}`;
};

function Stage({ children, viewKey = "chat" }: { children: ReactElement; viewKey?: string }) {
  return (
    <MediaSourceContext.Provider value={source}>
      <RevealProvider viewKey={viewKey}>{children}</RevealProvider>
    </MediaSourceContext.Provider>
  );
}

const hidden = (id: string, extra = {}) => demoPhoto(id, `IMG_000${id}.jpg`, 1280, 960, { spoiler: true, ...extra });
const plain = (id: string) => demoPhoto(id, `IMG_000${id}.jpg`, 1280, 960);

describe("Album with hidden photos", () => {
  it("draws a cover and asks for no picture, then reveals only the photo that was pressed", () => {
    asked.length = 0;
    const onOpen = vi.fn();
    const images = [hidden("1"), hidden("2"), plain("3")];
    const { container } = render(
      <Stage>
        <Album images={images} onOpen={onOpen} />
      </Stage>,
    );
    expect(container.querySelectorAll("[data-spoiler-face]")).toHaveLength(2);
    // The plain neighbour has its picture, the hidden ones have no img and were never asked for.
    expect(container.querySelectorAll("img")).toHaveLength(1);
    expect([...new Set(asked)]).toEqual(["thumb/3"]);
    const covers = screen.getAllByRole("button", { name: "Скрытое фото, нажмите, чтобы показать" });
    expect(covers).toHaveLength(2);

    fireEvent.click(covers[0] as HTMLElement);
    expect(onOpen).not.toHaveBeenCalled();
    expect(container.querySelectorAll("[data-spoiler-face]")).toHaveLength(1);
    expect([...new Set(asked)].sort()).toEqual(["thumb/1", "thumb/3"]);
    // The revealed photo is an ordinary photo now: a click opens it, and it has a "Hide" button.
    fireEvent.click(screen.getByRole("button", { name: "Фото IMG_0001.jpg, 1280 на 960" }));
    expect(onOpen).toHaveBeenCalledWith(0);
    expect(screen.getByRole("button", { name: "Снова скрыть фото" })).toBeInTheDocument();
  });

  it("the focus stays on the photo's button through a reveal and through Hide", () => {
    const { container } = render(
      <Stage>
        <Album images={[hidden("1")]} onOpen={vi.fn()} />
      </Stage>,
    );
    const open = container.querySelector<HTMLButtonElement>(".album__open");
    open?.focus();
    expect(open).toHaveFocus();
    fireEvent.click(open as HTMLElement);
    expect(container.querySelector(".album__open")).toBe(open);
    expect(open).toHaveFocus();
    // Focus is on the Hide button, which disappears with the cover: it goes back to the picture's button, not to the page.
    const hide = screen.getByRole("button", { name: "Снова скрыть фото" });
    hide.focus();
    fireEvent.click(hide);
    expect(container.querySelector("[data-spoiler-face]")).not.toBeNull();
    expect(container.querySelector(".album__open")).toBe(open);
    expect(document.activeElement).toBe(open);
  });

  it("a hidden photo fails closed without a provider: the click reveals nothing", () => {
    const { container } = render(
      <MediaSourceContext.Provider value={source}>
        <Album images={[hidden("1")]} onOpen={vi.fn()} />
      </MediaSourceContext.Provider>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Скрытое фото, нажмите, чтобы показать" }));
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("[data-spoiler-face]")).not.toBeNull();
  });

  it("a hidden photo whose file is missing shows the cover, never the file's state, and only then the missing tile", () => {
    const { container } = render(
      <Stage>
        <Album images={[hidden("1", { missing: true })]} onOpen={vi.fn()} />
      </Stage>,
    );
    expect(screen.queryByText("Файла нет")).toBeNull();
    expect(screen.queryByRole("button", { name: /IMG_0001/ })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Скрытое фото, нажмите, чтобы показать" }));
    expect(screen.getByText("Файла нет")).toBeInTheDocument();
    expect(container.querySelector("img")).toBeNull();
  });

  it("the cover shows the eye only in a row of three, the word elsewhere", () => {
    const { container } = render(
      <Stage>
        <Album images={[hidden("1"), hidden("2"), hidden("3"), hidden("4"), hidden("5"), hidden("6")]} onOpen={vi.fn()} />
      </Stage>,
    );
    expect(container.querySelectorAll(".spoiler-face--compact")).toHaveLength(6);
    expect(screen.queryByText("Скрыто")).toBeNull();
    const { container: single } = render(
      <Stage>
        <Album images={[hidden("4")]} onOpen={vi.fn()} />
      </Stage>,
    );
    expect(single.querySelector(".spoiler-face--compact")).toBeNull();
    expect(single).toHaveTextContent("Скрыто");
  });

  it("a revealed photo with a description is announced by it; a covered one never says it", () => {
    const image = hidden("1", { description: "Пляж, июль", name: "пляж.jpg" });
    render(
      <Stage>
        <Album images={[image]} onOpen={vi.fn()} />
      </Stage>,
    );
    expect(screen.queryByText("Пляж, июль")).toBeNull();
    expect(screen.queryByLabelText("Пляж, июль")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Скрытое фото, нажмите, чтобы показать" }));
    expect(screen.getByRole("button", { name: "Пляж, июль" })).toBeInTheDocument();
  });

  it("covers everything again when the view changes and when the window is hidden", () => {
    function Switch() {
      const [view, setView] = useState("a");
      return (
        <Stage viewKey={view}>
          <>
            <button type="button" onClick={() => setView("b")}>
              next
            </button>
            <Album images={[hidden("1")]} onOpen={vi.fn()} />
          </>
        </Stage>
      );
    }
    const { container } = render(<Switch />);
    fireEvent.click(screen.getByRole("button", { name: "Скрытое фото, нажмите, чтобы показать" }));
    expect(container.querySelector("img")).not.toBeNull();
    // The window goes to the menu bar.
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "hidden" });
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    Object.defineProperty(document, "visibilityState", { configurable: true, get: () => "visible" });
    expect(container.querySelector("img")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Скрытое фото, нажмите, чтобы показать" }));
    expect(container.querySelector("img")).not.toBeNull();
    // Another chat.
    fireEvent.click(screen.getByRole("button", { name: "next" }));
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("[data-spoiler-face]")).not.toBeNull();
  });
});
