import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, within } from "../../test/render";
import { demoPhoto } from "../attachments/demo";
import type { FeedMessage } from "./buildFeed";
import { type FeedActions, Feed } from "./Feed";
import { DemoMedia } from "../attachments/demo";

// Slice 5c: an entry with no text and one uncovered sticker is drawn as a sticker (no bubble). Anything else with a sticker in it is a picture.

const NOW = new Date(2026, 10, 20, 17, 0);
const CAT = "\u{1F63A}";
const sticker = (id = "s1", part: { spoiler?: boolean } = {}) => demoPhoto(id, "sticker.webp", 512, 512, { sticker: { emoji: CAT }, ...part });

const entry = (id: string, attachments: FeedMessage["attachments"], bodyMd = ""): FeedMessage => ({
  id,
  createdAt: NOW.getTime() - 60_000,
  createdTz: "UTC",
  journalDate: "2026-11-20",
  bodyMd,
  attachments,
});

const ACTIONS = (): FeedActions => ({
  onEdit: vi.fn(),
  onSaveEdit: vi.fn(),
  onCancelEdit: vi.fn(),
  onToggleTask: vi.fn(),
  onToggleDone: vi.fn(),
  onToggleFavorite: vi.fn(),
  onChangeDate: vi.fn(),
  onDelete: vi.fn(),
  onRestore: vi.fn(),
  onToggleTaskItem: vi.fn(),
  onTagClick: vi.fn(),
  onMoveTo: vi.fn(),
});

function show(message: FeedMessage, extra: { onOpenImage?: () => void; onAttachmentDetails?: () => void; onAttachmentSpoiler?: () => void } = {}) {
  const view = render(
    <DemoMedia>
      <Feed messages={[message]} today="2026-11-20" actions={ACTIONS()} {...extra} />
    </DemoMedia>,
    { language: "en" },
  );
  return { ...view, article: view.container.querySelector("article") as HTMLElement };
}

describe("a sticker in the feed", () => {
  it("no text and one uncovered sticker: the frameless bubble with the picture, the time pill on it and the label 'Sticker 😺'", () => {
    const { article } = show(entry("a", [sticker()]));
    expect(article.className).toContain("message--sticker");
    expect(article.className).toContain("message--bleed-pill");
    expect(article.className).toContain("message--bleed-bare");
    const picture = within(article).getByRole("img", { name: `Sticker ${CAT}` });
    expect(picture.tagName).toBe("IMG");
    // The time is inside the sticker's own box, so the pill sits on the picture's corner.
    const frame = article.querySelector(".message__sticker") as HTMLElement;
    expect(frame.querySelector("img")).toBe(picture);
    expect(frame.querySelector(".message__meta time")).not.toBeNull();
    // The box is fitted from the picture's shape before it loads.
    expect((article.querySelector(".sticker-image") as HTMLElement).style.getPropertyValue("--sticker-aspect")).toBe("1");
  });

  it("the box follows the shape of a wide and a tall sticker (the long side is the token, in CSS)", () => {
    const wide = demoPhoto("w", "sticker.webp", 512, 256, { sticker: { emoji: CAT } });
    const { article, unmount } = show(entry("w", [wide]));
    expect((article.querySelector(".sticker-image") as HTMLElement).style.getPropertyValue("--sticker-aspect")).toBe("2");
    unmount();
    const tall = demoPhoto("t", "sticker.webp", 256, 512, { sticker: { emoji: CAT } });
    const second = show(entry("t", [tall]));
    expect((second.article.querySelector(".sticker-image") as HTMLElement).style.getPropertyValue("--sticker-aspect")).toBe("0.5");
  });

  it("a click does nothing: no viewer, no file opened", () => {
    const onOpenImage = vi.fn();
    const { article } = show(entry("a", [sticker()]), { onOpenImage });
    fireEvent.click(within(article).getByRole("img", { name: `Sticker ${CAT}` }));
    expect(onOpenImage).not.toHaveBeenCalled();
    expect(within(article).queryByRole("button", { name: /Sticker|Photo/ })).toBeNull();
  });

  it("the hover bar has no Edit; its other buttons and its menu keep working; the menu has no Edit and no attachment details", () => {
    const { article } = show(entry("a", [sticker()]));
    const wrap = article.parentElement as HTMLElement;
    const bar = within(wrap).getByRole("toolbar", { name: "Entry actions" });
    expect(within(bar).queryByRole("button", { name: "Edit" })).toBeNull();
    for (const name of ["Copy", "Make a task", "Add to favorites", "More"]) {
      expect(within(bar).getByRole("button", { name })).toBeInTheDocument();
    }
    // One Tab stop in the row, and the arrows still count the buttons that are there.
    expect(within(bar).getAllByRole("button").filter((b) => b.tabIndex === 0)).toHaveLength(1);
    fireEvent.contextMenu(article);
    const menu = screen.getByRole("menu");
    for (const name of ["Copy", "Make a task", "Add to favorites", "Change date…", "Move to chat", "Delete"]) {
      expect(within(menu).getByRole("menuitem", { name })).toBeInTheDocument();
    }
    expect(within(menu).queryByRole("menuitem", { name: "Edit" })).toBeNull();
    expect(within(menu).queryByRole("menuitem", { name: /details/i })).toBeNull();
    expect(within(menu).queryByRole("menuitem", { name: /spoiler/i })).toBeNull();
  });

  it("an entry whose body is not empty (an edited import) is an ordinary picture in the bubble, with its Edit", () => {
    const { article } = show(entry("a", [sticker()], "an old note"));
    expect(article.className).not.toContain("message--sticker");
    expect(article.querySelector(".sticker-image")).toBeNull();
    expect(article.querySelector(".album")).not.toBeNull();
    expect(within(article.parentElement as HTMLElement).getByRole("button", { name: "Edit" })).toBeInTheDocument();
    expect(within(article).getByText("an old note")).toBeInTheDocument();
  });

  it("a covered sticker is the 7b cover, not a sticker; uncovering it makes it a sticker again", () => {
    const { article } = show(entry("a", [sticker("c", { spoiler: true })]));
    expect(article.className).not.toContain("message--sticker");
    expect(article.querySelector(".sticker-image")).toBeNull();
    expect(within(article).getByRole("button", { name: "Hidden photo, press to show" })).toBeInTheDocument();
  });

  it("a sticker is not a sticker entry with a second attachment", () => {
    const { article } = show(entry("a", [sticker("s1"), sticker("s2")]));
    expect(article.className).not.toContain("message--sticker");
    expect(article.querySelectorAll(".album__cell")).toHaveLength(2);
  });

  it("chips go under the sticker and a reply quote above it, both outside the sticker's box", () => {
    const message: FeedMessage = {
      ...entry("a", [sticker()]),
      reactions: [{ emoji: "\u{2764}\u{FE0F}", putAt: NOW.getTime(), putTz: "UTC", yearsLater: 0 }],
      replyTo: { id: "q", chatId: "c", journalDate: "2026-11-19", excerpt: "the earlier entry", state: "live", yearsLater: 0, attachments: { photos: 0, files: 0, voices: 0, voiceMs: null, name: null, sticker: false, stickerEmoji: null } },
    };
    const { article } = show(message);
    const frame = article.querySelector(".message__sticker") as HTMLElement;
    const chips = article.querySelector(".reaction-row") as HTMLElement;
    const quote = article.querySelector(".reply-quote") as HTMLElement;
    expect(chips).not.toBeNull();
    expect(quote).not.toBeNull();
    expect(frame.contains(chips)).toBe(false);
    expect(frame.contains(quote)).toBe(false);
    // Order in the bubble: the quote, then the sticker, then the chips.
    expect(quote.compareDocumentPosition(frame) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(frame.compareDocumentPosition(chips) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("the sticker whose file is missing keeps its box and shows its emoji", () => {
    const missing = demoPhoto("m", "sticker.webp", 512, 512, { sticker: { emoji: CAT }, missing: true });
    const { article } = show(entry("a", [missing]));
    expect(article.querySelector("img")).toBeNull();
    expect(within(article).getByRole("img", { name: `Sticker ${CAT}` })).toHaveTextContent(CAT);
  });

  it("a sticker without an emoji is just 'Sticker'", () => {
    const { article } = show(entry("a", [demoPhoto("n", "sticker.webp", 512, 512, { sticker: { emoji: null } })]));
    expect(within(article).getByRole("img", { name: "Sticker" })).toBeInTheDocument();
  });
});
