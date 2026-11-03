import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { demoFile, demoPhoto } from "./demo";
import { FileCard } from "./FileCard";
import { MessageAttachments } from "./MessageAttachments";
import { fileExtension, splitFileName } from "./fileName";

const MEGABYTE = 1024 * 1024;
const LONG = "Квартальный отчёт по продажам и расходам, сводная таблица для бухгалтерии, финальная версия (2026).xlsx";

describe("FileCard", () => {
  it("значок, имя и «PDF · 2.4 MB»; вся карточка одна кнопка", () => {
    const onOpen = vi.fn();
    render(<FileCard attachment={demoFile("f", "menu.pdf", "file", 2.4 * MEGABYTE)} onOpen={onOpen} />, { language: "en" });
    const button = screen.getByRole("button", { name: "menu.pdf. PDF · 2.4 MB" });
    expect(button).toHaveAttribute("title", "menu.pdf");
    expect(button.querySelector(".file-name")).toHaveAttribute("title", "menu.pdf");
    fireEvent.click(button);
    expect(onOpen).toHaveBeenCalledWith("f");
  });

  it("по-русски: десятичная запятая", () => {
    render(<FileCard attachment={demoFile("f", "меню.pdf", "file", 2.4 * MEGABYTE)} onOpen={vi.fn()} />);
    expect(screen.getByRole("button", { name: /^меню\.pdf\. PDF · 2,4\s/ })).toBeInTheDocument();
  });

  it("длинное имя режется в середине: расширение отдельно и всегда видно", () => {
    const { container } = render(<FileCard attachment={demoFile("f", LONG, "file", 18 * MEGABYTE)} onOpen={vi.fn()} />);
    const stem = container.querySelector(".file-name__stem");
    const tail = container.querySelector(".file-name__tail");
    expect(tail?.textContent?.endsWith(".xlsx")).toBe(true);
    expect(`${stem?.textContent}${tail?.textContent}`).toBe(LONG);
    expect(container.querySelector(".file-name")).toHaveAttribute("title", LONG);
    expect(screen.getByRole("button")).toHaveAttribute("aria-label", expect.stringContaining(LONG));
  });

  it("видео и аудио: те же карточки, расширение в верхнем регистре", () => {
    render(
      <>
        <FileCard attachment={demoFile("v", "walk.mp4", "video", 148 * MEGABYTE)} onOpen={vi.fn()} />
        <FileCard attachment={demoFile("a", "note.mp3", "audio", 3.1 * MEGABYTE)} onOpen={vi.fn()} />
      </>,
      { language: "en" },
    );
    expect(screen.getByRole("button", { name: /^walk\.mp4\. MP4 · / })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^note\.mp3\. MP3 · / })).toBeInTheDocument();
  });

  it("без расширения только размер", () => {
    render(<FileCard attachment={demoFile("f", "README", "file", 1200)} onOpen={vi.fn()} />, { language: "en" });
    expect(screen.getByRole("button", { name: /^README\. 1\.2 [kK]B$/ })).toBeInTheDocument();
  });

  it("пропавший файл: имя и «Файла нет», клик ничего не открывает", () => {
    const onOpen = vi.fn();
    render(<FileCard attachment={demoFile("f", "menu.pdf", "file", MEGABYTE, "missing")} onOpen={onOpen} />, { language: "en" });
    const button = screen.getByRole("button", { name: "menu.pdf. File is missing" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(button);
    expect(onOpen).not.toHaveBeenCalled();
  });
});

describe("fileName", () => {
  it("расширение: без точки; ведущая точка и слишком длинный хвост не расширение", () => {
    expect(fileExtension("a.tar.gz")).toBe("gz");
    expect(fileExtension(".gitignore")).toBe("");
    expect(fileExtension("README")).toBe("");
    expect(fileExtension("trailing.")).toBe("");
    expect(fileExtension("a.thisisnotanextension")).toBe("");
  });

  it("разбиение: начало и хвост вместе дают имя", () => {
    for (const name of ["menu.pdf", "a.b", "README", ".gitignore", "😀😀😀😀😀😀.png", LONG, ""]) {
      const { stem, tail } = splitFileName(name);
      expect(stem + tail).toBe(name);
    }
    expect(splitFileName("Quarterly-report-2026.pdf").tail).toBe("2026.pdf");
  });
});

describe("MessageAttachments", () => {
  it("пустой список: ничего", () => {
    const { container } = render(<MessageAttachments attachments={[]} onOpenImage={vi.fn()} onOpenFile={vi.fn()} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("фото в одном альбоме по порядку, потом файлы по порядку; открытие даёт id вложения", () => {
    const onOpenImage = vi.fn();
    const onOpenFile = vi.fn();
    const attachments = [
      demoFile("pdf", "a.pdf", "file", 10),
      demoPhoto("p1", "one.jpg", 100, 100),
      demoFile("mov", "b.mp4", "video", 10),
      demoPhoto("p2", "two.jpg", 100, 100),
    ];
    const { container } = render(<MessageAttachments attachments={attachments} onOpenImage={onOpenImage} onOpenFile={onOpenFile} />, { language: "en" });
    expect(container.querySelectorAll(".album")).toHaveLength(1);
    const names = screen.getAllByRole("button").map((button) => button.getAttribute("aria-label"));
    expect(names).toEqual(["Photo one.jpg, 100 by 100", "Photo two.jpg, 100 by 100", expect.stringMatching(/^a\.pdf/), expect.stringMatching(/^b\.mp4/)]);
    fireEvent.click(screen.getAllByRole("button")[1] as HTMLElement);
    expect(onOpenImage).toHaveBeenCalledWith("p2");
    fireEvent.click(screen.getAllByRole("button")[3] as HTMLElement);
    expect(onOpenFile).toHaveBeenCalledWith("mov");
  });

  it("корень отдаёт класс для пузыря", () => {
    const { container } = render(<MessageAttachments attachments={[demoFile("f", "a.pdf", "file", 1)]} onOpenImage={vi.fn()} onOpenFile={vi.fn()} className="x" />);
    expect(container.firstElementChild).toHaveClass("message-attachments", "x");
  });
});
