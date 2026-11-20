import { describe, expect, it } from "vitest";
import type { TrashItem } from "../../api/generated/TrashItem";
import { mockMessage } from "../../api/MockApi";
import { render, screen, within } from "../../test/render";
import { DemoMedia, demoPhoto } from "../attachments/demo";
import { TrashView } from "./TrashView";

// Slice 5c: a sticker in the trash is drawn as a sticker (no viewer behind it), and one with text or a cover is an ordinary picture.
const CAT = "\u{1F63A}";
const item = (id: string, bodyMd: string, spoiler = false): TrashItem => ({
  message: mockMessage({ id, bodyMd, attachments: [demoPhoto(`a-${id}`, "sticker.webp", 512, 512, { sticker: { emoji: CAT }, spoiler })] }),
  chatTitle: "Books",
  chatIcon: "📚",
  chatIsInbox: false,
  chatColor: "violet",
  deletedAt: Date.UTC(2026, 10, 19),
  daysLeft: 28,
});

const view = (items: TrashItem[]) =>
  render(
    <DemoMedia>
      <TrashView items={items} language="en" today="2026-11-20" onRestore={() => undefined} onPurge={() => undefined} />
    </DemoMedia>,
    { language: "en" },
  );

describe("TrashView: stickers", () => {
  it("an entry that is only a sticker shows the sticker, not a photo tile", () => {
    view([item("s1", "")]);
    const entry = screen.getByRole("article");
    expect(within(entry).getByRole("img", { name: `Sticker ${CAT}` })).toBeInTheDocument();
    expect(entry.querySelector(".sticker-image")).not.toBeNull();
    expect(entry.querySelector(".album")).toBeNull();
    // Restore and delete forever work as for any entry.
    expect(within(entry).getByRole("button", { name: /Restore/ })).toBeInTheDocument();
  });

  it("a GIF in the trash is a GIF cell too: the same playback rules apply there", () => {
    const gif = demoPhoto("tg1", "cat.gif", 480, 270, { mime: "image/gif", byteSize: 1000 });
    view([{ ...item("g1", ""), message: mockMessage({ id: "g1", bodyMd: "", attachments: [gif] }) }]);
    expect(screen.getByRole("article").querySelector("[data-gif]")).not.toBeNull();
  });

  it("with text or covered, it is an ordinary picture", () => {
    view([item("s2", "my note"), item("s3", "", true)]);
    for (const entry of screen.getAllByRole("article")) {
      expect(entry.querySelector(".sticker-image")).toBeNull();
      expect(entry.querySelector(".album")).not.toBeNull();
    }
  });
});
