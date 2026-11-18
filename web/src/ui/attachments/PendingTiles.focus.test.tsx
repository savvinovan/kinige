import { useState } from "react";
import { describe, expect, it } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { fireEvent, render, screen } from "../../test/render";
import { Composer } from "../composer/Composer";
import { demoFile, demoPhoto } from "./demo";
import { PendingTiles } from "./PendingTiles";

// Keyboard focus after the x of a tile (KINIG-135, PRD §12.2): it must not drop to the page.

const ITEMS = [demoPhoto("a", "a.jpg", 100, 100), demoPhoto("b", "b.jpg", 100, 100), demoFile("c", "c.pdf", "file", 1024)];

/** The composer with its tiles; the x removes a tile at once (as the window does once Rust has removed it). */
function Window({ initial }: { initial: readonly AttachmentView[] }) {
  const [items, setItems] = useState(initial);
  const [value, setValue] = useState("");
  return (
    <Composer
      value={value}
      onChange={setValue}
      tiles={<PendingTiles items={items} onRemove={(id) => setItems((was) => was.filter((item) => item.id !== id))} />}
      hasTiles={items.length > 0}
    />
  );
}

const remove = (name: string) => screen.getByRole("button", { name: `Remove ${name}` });

/** The x gets the focus and is pressed (a key press on a focused button is a click). */
function pressRemove(name: string) {
  const button = remove(name);
  button.focus();
  expect(button).toHaveFocus();
  fireEvent.click(button);
}

describe("плитки: фокус после крестика с клавиатуры", () => {
  it("уходит на крестик следующей плитки", () => {
    render(<Window initial={ITEMS} />, { language: "en" });
    pressRemove("a.jpg");
    expect(remove("b.jpg")).toHaveFocus();
  });

  it("у последней плитки уходит на крестик предыдущей", () => {
    render(<Window initial={ITEMS} />, { language: "en" });
    pressRemove("c.pdf");
    expect(remove("b.jpg")).toHaveFocus();
  });

  it("убрана единственная плитка: фокус в поле ввода, а не на странице", () => {
    render(<Window initial={[ITEMS[0] as AttachmentView]} />, { language: "en" });
    pressRemove("a.jpg");
    expect(screen.queryByRole("list", { name: "Attachments to send" })).toBeNull();
    expect(screen.getByRole("textbox")).toHaveFocus();
  });

  it("после клика мышью (у крестика нет фокуса) фокус не уводят", () => {
    render(<Window initial={ITEMS} />, { language: "en" });
    const outside = document.createElement("button");
    document.body.append(outside);
    outside.focus();
    fireEvent.click(remove("a.jpg"));
    expect(outside).toHaveFocus();
    outside.remove();
  });

  it("пока плитка не убрана (Rust ещё не ответил), фокус остаётся на её крестике", () => {
    const { rerender } = render(<PendingTiles items={ITEMS} onRemove={() => undefined} />, { language: "en" });
    pressRemove("a.jpg");
    expect(remove("a.jpg")).toHaveFocus();
    rerender(<PendingTiles items={ITEMS} onRemove={() => undefined} />);
    expect(remove("a.jpg")).toHaveFocus();
  });
});
