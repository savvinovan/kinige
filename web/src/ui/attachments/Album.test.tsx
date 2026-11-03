import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { Album } from "./Album";
import { demoPhoto } from "./demo";
import { MediaSourceContext } from "./mediaUrl";

const source = (variant: string, id: string) => `test://${variant}/${id}`;
const withSource = (ui: ReactElement) => <MediaSourceContext.Provider value={source}>{ui}</MediaSourceContext.Provider>;

const photo = (id: string, width = 1280, height = 960) => demoPhoto(id, `IMG_000${id}.jpg`, width, height);

describe("Album", () => {
  it("ничего для пустого списка", () => {
    const { container } = render(<Album images={[]} onOpen={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("одно фото: коробка берёт форму из размеров и не меняется, когда миниатюра пришла", () => {
    const { container } = render(withSource(<Album images={[photo("1", 600, 900)]} onOpen={vi.fn()} />));
    const box = container.querySelector<HTMLElement>(".album");
    const image = container.querySelector("img");
    expect(box).toHaveClass("album--single");
    const before = box?.getAttribute("style");
    expect(before).toContain("--album-aspect");
    expect(before).toContain("--album-natural");
    expect(image).toHaveAttribute("src", "test://thumb/1");

    if (image) fireEvent.load(image);

    expect(container.querySelector(".album")).toBe(box);
    expect(box?.getAttribute("style")).toBe(before);
    expect(box?.className).toBe("album album--single");
  });

  it("одно фото неизвестного размера получает форму по умолчанию, не ломаясь", () => {
    const unknown = { ...photo("1"), width: null, height: null };
    const { container } = render(<Album images={[unknown]} onOpen={vi.fn()} />);
    expect(container.querySelector(".album")?.getAttribute("style") ?? "").not.toContain("NaN");
    expect(screen.getByRole("button", { name: "Фото IMG_0001.jpg" })).toBeInTheDocument();
  });

  it("ошибка загрузки заменяет картинку плиткой «Нет превью» в той же коробке", () => {
    const { container } = render(withSource(<Album images={[photo("1")]} onOpen={vi.fn()} />));
    const box = container.querySelector(".album");
    const before = box?.getAttribute("style");
    const image = container.querySelector("img");
    if (image) fireEvent.error(image);

    expect(container.querySelector("img")).toBeNull();
    expect(screen.getByText("Нет превью")).toBeInTheDocument();
    expect(container.querySelector(".album")).toBe(box);
    expect(box?.getAttribute("style")).toBe(before);
    // Кнопка остаётся: файл есть, его можно открыть в программе по умолчанию.
    expect(screen.getByRole("button", { name: "Фото IMG_0001.jpg, 1280 на 960" })).toBeInTheDocument();
  });

  it("пропавший файл: плитка «Файла нет», не кнопка", () => {
    const missing = demoPhoto("9", "IMG_0009.jpg", 800, 600, { missing: true });
    const { container } = render(withSource(<Album images={[missing]} onOpen={vi.fn()} />));
    expect(container.querySelector("img")).toBeNull();
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByRole("img", { name: "Фото IMG_0009.jpg, 800 на 600. Файла нет" })).toBeInTheDocument();
    expect(screen.getByText("Файла нет")).toBeInTheDocument();
  });

  it("подписи: по-русски и по-английски", () => {
    const { unmount } = render(<Album images={[photo("1")]} onOpen={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Фото IMG_0001.jpg, 1280 на 960" })).toBeInTheDocument();
    unmount();
    render(<Album images={[photo("1")]} onOpen={vi.fn()} />, { language: "en" });
    expect(screen.getByRole("button", { name: "Photo IMG_0001.jpg, 1280 by 960" })).toBeInTheDocument();
  });

  it("сетка: ряды по таблице, все десять ячеек доступны, клик даёт индекс", () => {
    const onOpen = vi.fn();
    const images = Array.from({ length: 10 }, (_, i) => photo(String(i)));
    const { container } = render(<Album images={images} onOpen={onOpen} />);
    const rows = [...container.querySelectorAll(".album__row")].map((row) => row.children.length);
    expect(rows).toEqual([2, 2, 3, 3]);
    const buttons = screen.getAllByRole("button");
    expect(buttons).toHaveLength(10);
    fireEvent.click(buttons[7] as HTMLElement);
    expect(onOpen).toHaveBeenCalledWith(7);
  });

  it("в сетке пропавший и битый файл держат ячейку", () => {
    const images = [photo("1"), demoPhoto("2", "IMG_0002.jpg", 800, 600, { missing: true }), photo("3")];
    const { container } = render(<Album images={images} onOpen={vi.fn()} />);
    expect([...container.querySelectorAll(".album__row")].map((row) => row.children.length)).toEqual([1, 2]);
    expect(container.querySelectorAll(".album__cell")).toHaveLength(3);
  });
});
