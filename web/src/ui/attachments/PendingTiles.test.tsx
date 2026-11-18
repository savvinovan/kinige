import { describe, expect, it, vi } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { fireEvent, render, screen } from "../../test/render";
import { demoFile, demoPhoto } from "./demo";
import { MediaSourceContext } from "./mediaUrl";
import { PendingTiles } from "./PendingTiles";

const source = (variant: string, id: string) => `test://${variant}/${id}`;

const ITEMS = [
  demoPhoto("p1", "IMG_0001.jpg", 1280, 960),
  demoPhoto("p2", "IMG_0002.jpg", 960, 1280),
  demoFile("pdf", "menu.pdf", "file", 2.4 * 1024 * 1024),
];

describe("PendingTiles", () => {
  it("ничего для пустого списка", () => {
    const { container } = render(<PendingTiles items={[]} onRemove={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("миниатюры фото и карточка файла, у каждой плитки крестик", () => {
    const { container } = render(
      <MediaSourceContext.Provider value={source}>
        <PendingTiles items={ITEMS} onRemove={vi.fn()} />
      </MediaSourceContext.Provider>,
      { language: "en" },
    );
    expect(screen.getByRole("list", { name: "Attachments to send" })).toBeInTheDocument();
    expect([...container.querySelectorAll("img")].map((image) => image.getAttribute("src"))).toEqual(["test://thumb/p1", "test://thumb/p2"]);
    expect(screen.getByText("PDF · 2.4 MB")).toBeInTheDocument();
    expect(screen.getAllByRole("button").map((button) => button.getAttribute("aria-label"))).toEqual([
      "Remove IMG_0001.jpg",
      "Remove IMG_0002.jpg",
      "Remove menu.pdf",
    ]);
  });

  it("крестик отдаёт id вложения; подпись по-русски", () => {
    const onRemove = vi.fn();
    render(<PendingTiles items={ITEMS} onRemove={onRemove} />);
    fireEvent.click(screen.getByRole("button", { name: "Убрать menu.pdf" }));
    expect(onRemove).toHaveBeenCalledWith("pdf");
  });

  it("с onOpen фото-плитка кнопка: клик и Enter отдают id, файл и пропавшее фото не открываются; × лежит рядом, не внутри", () => {
    const onOpen = vi.fn();
    const items = [...ITEMS, demoPhoto("gone", "gone.jpg", 100, 100, { missing: true })];
    const { container } = render(<PendingTiles items={items} onRemove={vi.fn()} onOpen={onOpen} />, { language: "en" });
    const open = screen.getAllByRole("button", { name: /^Photo / });
    expect(open.map((button) => button.getAttribute("aria-label"))).toEqual(["Photo IMG_0001.jpg, 1280 by 960", "Photo IMG_0002.jpg, 960 by 1280"]);
    fireEvent.click(open[1] as HTMLElement);
    expect(onOpen).toHaveBeenCalledWith("p2");
    expect(onOpen).toHaveBeenCalledTimes(1);
    // The missing photo is announced as one picture with its state; the x is never nested in the open button.
    expect(screen.getByRole("img", { name: "Photo gone.jpg, 100 by 100. File is missing" })).toBeInTheDocument();
    expect(container.querySelectorAll("button button")).toHaveLength(0);
    expect(screen.getAllByRole("button", { name: /^Remove / })).toHaveLength(4);
  });

  it("форма плитки фото задана до загрузки миниатюры: --pending-aspect из размера, для фото без размера её нет", () => {
    const unknown = { ...demoPhoto("u", "u.jpg", 100, 100), width: null, height: null };
    const { container } = render(<PendingTiles items={[ITEMS[0] as AttachmentView, unknown]} onRemove={vi.fn()} />);
    const [sized, unsized] = [...container.querySelectorAll<HTMLElement>(".pending-tile--image")];
    expect(sized?.style.getPropertyValue("--pending-aspect")).toBe(String(1280 / 960));
    expect(unsized?.style.getPropertyValue("--pending-aspect")).toBe("");
  });

  it("десять плиток все с кнопкой; битое и пропавшее фото держат плитку", () => {
    const many = Array.from({ length: 8 }, (_, i) => demoPhoto(`m${i}`, `IMG_${i}.jpg`, 100, 100));
    const items = [...many, demoPhoto("bad", "bad.jpg", 100, 100, { broken: true }), demoPhoto("gone", "gone.jpg", 100, 100, { missing: true })];
    const { container } = render(<PendingTiles items={items} onRemove={vi.fn()} />, { language: "en" });
    expect(screen.getAllByRole("button")).toHaveLength(10);
    const broken = container.querySelectorAll("img")[8];
    if (broken) fireEvent.error(broken);
    expect(screen.getByText("No preview")).toBeInTheDocument();
    expect(screen.getByText("File is missing")).toBeInTheDocument();
    expect(container.querySelectorAll(".pending-tile")).toHaveLength(10);
  });
});
