import type { ReactElement } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { demoFile, demoPhoto } from "./demo";
import { Lightbox } from "./Lightbox";
import { MediaSourceContext } from "./mediaUrl";
import { PendingTiles } from "./PendingTiles";
import { RevealProvider } from "./reveal";
import { FileCard } from "./FileCard";

// KINIG-136: the other surfaces that draw a picture: the viewer, the pending tile; and the file card with its new name and description.
// Labels are English here (`language: "en"`).

const asked: string[] = [];
const source = (variant: string, id: string) => {
  asked.push(`${variant}/${id}`);
  return `test://${variant}/${id}`;
};
const stage = (ui: ReactElement) => (
  <MediaSourceContext.Provider value={source}>
    <RevealProvider viewKey="chat">{ui}</RevealProvider>
  </MediaSourceContext.Provider>
);

const hidden = demoPhoto("h1", "IMG_0001.jpg", 1280, 960, { spoiler: true, name: "beach.jpg", description: "Beach, July" });
const plain = demoPhoto("p2", "IMG_0002.jpg", 960, 1280);

describe("the viewer with a hidden photo", () => {
  it("shows the cover and a Show button, asks for no picture and no caption, then the photo and its caption", () => {
    asked.length = 0;
    const { container } = render(stage(<Lightbox images={[hidden, plain]} index={0} onIndexChange={vi.fn()} onClose={vi.fn()} />), { language: "en" });
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("[data-spoiler-face]")).not.toBeNull();
    expect(asked).toEqual([]);
    expect(screen.queryByText("Beach, July")).toBeNull();
    // No zoom tools over a cover.
    expect(screen.queryByRole("button", { name: "Zoom in" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Show the hidden photo" }));
    expect(container.querySelector("img")).toHaveAttribute("src", "test://full/h1");
    expect(screen.getByText("Beach, July")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Show the hidden photo" })).toBeNull();
    // The focus is in the dialog, not on the page, after the button went with the cover.
    expect(document.activeElement).not.toBe(document.body);
    expect(screen.getByRole("dialog").contains(document.activeElement)).toBe(true);
  });

  it("a plain photo of the same viewer has its picture and no caption line", () => {
    const { container } = render(stage(<Lightbox images={[hidden, plain]} index={1} onIndexChange={vi.fn()} onClose={vi.fn()} />), { language: "en" });
    expect(container.querySelector("img")).toHaveAttribute("src", "test://full/p2");
    expect(container.querySelector(".lightbox__caption")).toBeNull();
  });
});

describe("the pending tile with a hidden photo", () => {
  const items = [hidden, plain, demoFile("pdf", "menu.pdf", "file", 2048, "local", { name: "lunch.pdf", description: "Lunch menu" })];

  it("is a cover with the Details and the x on top, and reveals in place on the first click", () => {
    asked.length = 0;
    const onOpen = vi.fn();
    const onDetails = vi.fn();
    const { container } = render(stage(<PendingTiles items={items} onRemove={vi.fn()} onOpen={onOpen} onDetails={onDetails} />), { language: "en" });
    expect(container.querySelectorAll("img")).toHaveLength(1);
    expect(asked).toEqual(["thumb/p2"]);
    expect(container.querySelector("[data-spoiler-face]")).not.toBeNull();
    // A hidden photo's name is not read out: its Details button says so, and the x names the photo (a name is not a picture).
    expect(screen.getByRole("button", { name: "Details of a hidden photo" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Details of IMG_0002.jpg" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Details of lunch.pdf" })).toBeInTheDocument();

    const open = screen.getByRole("button", { name: "Hidden photo, press to show" });
    open.focus();
    fireEvent.click(open);
    expect(onOpen).not.toHaveBeenCalled();
    expect(asked).toContain("thumb/h1");
    expect(container.querySelector("[data-spoiler-face]")).toBeNull();
    expect(open).toHaveFocus();
    // Revealed, a click opens the viewer as it always did.
    fireEvent.click(screen.getByRole("button", { name: "Beach, July" }));
    expect(onOpen).toHaveBeenCalledWith("h1");

    fireEvent.click(screen.getByRole("button", { name: "Details of lunch.pdf" }));
    expect(onDetails).toHaveBeenCalledWith("pdf");
  });

  it("Details is the first of the two round buttons and the file tile shows the display name", () => {
    const { container } = render(stage(<PendingTiles items={items} onRemove={vi.fn()} onDetails={vi.fn()} />), { language: "en" });
    const tile = container.querySelectorAll(".pending-tile")[2] as HTMLElement;
    const buttons = [...tile.querySelectorAll("button")].map((button) => button.getAttribute("aria-label"));
    expect(buttons).toEqual(["Details of lunch.pdf", "Remove lunch.pdf"]);
    expect(tile).toHaveTextContent("lunch.pdf");
    expect(tile).not.toHaveTextContent("menu.pdf");
  });

  it("without a Details handler there is no such button", () => {
    render(stage(<PendingTiles items={items} onRemove={vi.fn()} />), { language: "en" });
    expect(screen.queryByRole("button", { name: /Details/ })).toBeNull();
  });
});

describe("the file card", () => {
  it("shows the display name and the description in a second line", () => {
    const card = demoFile("f1", "menu.pdf", "file", 2048, "local", { name: "lunch.pdf", description: "Lunch menu for the week" });
    render(<FileCard attachment={card} onOpen={vi.fn()} />, { language: "en" });
    const button = screen.getByRole("button", { name: /lunch\.pdf\. Lunch menu for the week\./ });
    expect(button).toHaveTextContent("lunch.pdf");
    expect(button).toHaveTextContent("Lunch menu for the week");
    expect(button).not.toHaveTextContent("menu.pdf");
  });
});
