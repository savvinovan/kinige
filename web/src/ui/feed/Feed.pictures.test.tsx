import { describe, expect, it } from "vitest";
import { render } from "../../test/render";
import { demoFile, demoPhoto } from "../attachments/demo";
import type { FeedMessage } from "./buildFeed";
import { Feed } from "./Feed";

// KINIG-132: the feed tells the bubble when every attachment of an entry is a picture, so the picture fills the bubble.

const NOW = new Date(2026, 9, 20, 17, 0);
const entry = (id: string, attachments: FeedMessage["attachments"], bodyMd = ""): FeedMessage => ({
  id,
  createdAt: NOW.getTime() - 60_000,
  createdTz: "UTC",
  journalDate: "2026-10-20",
  bodyMd,
  attachments,
});

const render1 = (message: FeedMessage) => {
  const { container } = render(<Feed messages={[message]} today="2026-10-20" />);
  return container.querySelector("article")?.className ?? "";
};

describe("Feed entries with pictures", () => {
  it("an entry of pictures only is a bleed bubble with the time pill", () => {
    const classes = render1(entry("a", [demoPhoto("fa", "IMG_0001.jpg", 800, 600), demoPhoto("fb", "IMG_0002.jpg", 800, 600)]));
    expect(classes).toContain("message--bleed-pill");
  });

  it("an entry of pictures with text is a bleed bubble without the pill", () => {
    const classes = render1(entry("b", [demoPhoto("fc", "IMG_0003.jpg", 800, 600)], "Подпись"));
    expect(classes).toContain("message--bleed");
    expect(classes).not.toContain("message--bleed-pill");
  });

  it("a picture with a file stays in the plain bubble, as before", () => {
    const classes = render1(entry("c", [demoPhoto("fd", "IMG_0004.jpg", 800, 600), demoFile("fe", "Меню.pdf", "file", 1024)]));
    expect(classes).not.toContain("message--bleed");
  });

  it("an entry without attachments is not a bleed bubble", () => {
    expect(render1(entry("d", undefined, "Просто текст"))).not.toContain("message--bleed");
    expect(render1(entry("e", [], "Просто текст"))).not.toContain("message--bleed");
  });
});
