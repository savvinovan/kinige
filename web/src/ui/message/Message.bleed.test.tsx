import { describe, expect, it } from "vitest";
import { render, screen } from "../../test/render";
import { demoFile, demoPhoto } from "../attachments/demo";
import { MessageAttachments } from "../attachments/MessageAttachments";
import { Message } from "./Message";

// KINIG-132: an entry of pictures only fills its bubble edge to edge and shows the time as a pill on the picture.

const sentAt = new Date(2026, 9, 20, 9, 12);
const noop = () => undefined;
const photos = [demoPhoto("b1", "IMG_0001.jpg", 1280, 960)];

const bubble = (props: Partial<Parameters<typeof Message>[0]> = {}, attachments = photos) => (
  <Message
    markdown=""
    sentAt={sentAt}
    imagesOnly={attachments.every(({ kind }) => kind === "image")}
    attachments={<MessageAttachments attachments={attachments} onOpenImage={noop} onOpenFile={noop} />}
    {...props}
  />
);

const classesOf = (container: HTMLElement) => container.querySelector("article")?.className.split(" ") ?? [];

describe("Message with pictures only", () => {
  it("fills the bubble and puts the time on the picture when there is no text and no quote", () => {
    const { container } = render(bubble());
    expect(classesOf(container)).toEqual(expect.arrayContaining(["message--bleed", "message--bleed-pill", "message--bleed-bare"]));
  });

  it("keeps the time, the star and the agent mark, all inside the pill element", () => {
    const { container } = render(bubble({ favorite: true, byAgent: true }));
    const pill = container.querySelector(".message__meta");
    const time = screen.getByText("09:12");
    expect(time.tagName).toBe("TIME");
    expect(time).toHaveAttribute("datetime", sentAt.toISOString());
    expect(pill).toContainElement(time);
    expect(pill).toContainElement(screen.getByRole("img", { name: "В избранном" }));
    expect(pill).toHaveTextContent("добавлено агентом");
  });

  it("with text: the picture fills the bubble, the time stays in the footer, the bubble keeps its fill", () => {
    const { container } = render(bubble({ markdown: "Скриншот" }));
    const classes = classesOf(container);
    expect(classes).toContain("message--bleed");
    expect(classes).not.toContain("message--bleed-pill");
    expect(classes).not.toContain("message--bleed-bare");
    expect(screen.getByText("Скриншот")).toBeInTheDocument();
  });

  it("with a reply quote and no text: the picture fills the bubble and carries the pill, but the bubble keeps its fill under the quote", () => {
    const { container } = render(bubble({ quote: <div className="reply-quote">Цитата</div> }));
    const classes = classesOf(container);
    expect(classes).toContain("message--bleed-pill");
    expect(classes).not.toContain("message--bleed-bare");
  });

  it("a picture with a file is not only pictures: the plain bubble", () => {
    const mixed = [photos[0]!, demoFile("b2", "Меню.pdf", "file", 1024)];
    const { container } = render(bubble({}, mixed));
    expect(classesOf(container).some((name) => name.startsWith("message--bleed"))).toBe(false);
  });

  it("falls back to the plain bubble while editing, when not sent, for a task and for a deleted entry", () => {
    for (const props of [{ mode: "editing" as const }, { status: "failed" as const }, { isTask: true }, { mode: "deleted" as const }]) {
      const { container, unmount } = render(bubble({ markdown: "Текст", ...props }));
      expect(classesOf(container).some((name) => name.startsWith("message--bleed")), JSON.stringify(props)).toBe(false);
      unmount();
    }
  });

  it("an entry that is not marked as pictures only keeps the plain bubble", () => {
    const { container } = render(bubble({ imagesOnly: false }));
    expect(classesOf(container).some((name) => name.startsWith("message--bleed"))).toBe(false);
  });

  it("keeps the action buttons and the picture button as separate controls: the pill is not a control", () => {
    const { container } = render(bubble({ showActions: true }));
    expect(screen.getByRole("button", { name: "Фото IMG_0001.jpg, 1280 на 960" })).toBeInTheDocument();
    expect(container.querySelector(".message__meta button")).toBeNull();
    expect(container.querySelector(".message-actions")).not.toBeNull();
  });
});
