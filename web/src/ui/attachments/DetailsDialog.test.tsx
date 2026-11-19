import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import type { AttachmentView } from "../../api/generated/AttachmentView";
import { fireEvent, render, screen, within } from "../../test/render";
import { DetailsDialog } from "./DetailsDialog";
import { demoPhoto } from "./demo";
import { RevealProvider } from "./reveal";

// KINIG-136, the details dialog's keyboard and focus (PRD §12.2) and its fields. English labels.

const THREE = [
  demoPhoto("a", "IMG_0001.jpg", 800, 600),
  demoPhoto("b", "IMG_0002.jpg", 800, 600),
  demoPhoto("c", "IMG_0003.jpg", 800, 600),
];
const HIDDEN = [demoPhoto("h", "IMG_0009.jpg", 800, 600, { spoiler: true }), demoPhoto("p", "IMG_0010.jpg", 800, 600)];

function Window({ list, start = 0 }: { list: readonly AttachmentView[]; start?: number }) {
  const [index, setIndex] = useState(start);
  return (
    <RevealProvider viewKey="chat">
      <DetailsDialog attachments={list} index={index} onIndexChange={setIndex} onSave={() => Promise.resolve()} onClose={vi.fn()} />
    </RevealProvider>
  );
}

const open = (list: readonly AttachmentView[]) => render(<Window list={list} />, { language: "en" });
const dialog = () => screen.getByRole("dialog", { name: "Attachment details" });
const stepBack = () => screen.getByRole("button", { name: "Previous attachment" });
const stepForward = () => screen.getByRole("button", { name: "Next attachment" });

describe("the arrows", () => {
  it("step between attachments right after the dialog opens, when the focus is on the close button", () => {
    open(THREE);
    // The window puts the focus on the header's close button, which is not inside the form.
    expect(screen.getByRole("button", { name: "Close the details" })).toHaveFocus();
    expect(within(dialog()).getByText("1 / 3")).toBeInTheDocument();
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: "ArrowRight" });
    expect(within(dialog()).getByText("2 / 3")).toBeInTheDocument();
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: "ArrowLeft" });
    expect(within(dialog()).getByText("1 / 3")).toBeInTheDocument();
    // Not past the ends.
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: "ArrowLeft" });
    expect(within(dialog()).getByText("1 / 3")).toBeInTheDocument();
  });

  it("move the caret, not the attachment, in a text field", () => {
    open(THREE);
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Name" }), { key: "ArrowRight" });
    fireEvent.keyDown(screen.getByRole("textbox", { name: "Description" }), { key: "ArrowRight" });
    expect(within(dialog()).getByText("1 / 3")).toBeInTheDocument();
  });

  it("are not heard outside the dialog", () => {
    open(THREE);
    const outside = document.createElement("button");
    document.body.append(outside);
    fireEvent.keyDown(outside, { key: "ArrowRight" });
    expect(within(dialog()).getByText("1 / 3")).toBeInTheDocument();
    outside.remove();
  });
});

describe("the focus", () => {
  it("goes into the dialog, not to the page, when Show uncovers the photo and the button goes with the cover", () => {
    render(<Window list={HIDDEN} />, { language: "en" });
    const show = screen.getByRole("button", { name: "Show the hidden photo" });
    show.focus();
    expect(show).toHaveFocus();
    fireEvent.click(show);
    expect(screen.queryByRole("button", { name: "Show the hidden photo" })).toBeNull();
    expect(document.activeElement).not.toBe(document.body);
    expect(dialog().contains(document.activeElement)).toBe(true);
    // And the arrows still work from there.
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: "ArrowRight" });
    expect(within(dialog()).getByText("2 / 2")).toBeInTheDocument();
  });

  it("stays on the arrow button that was pressed when the next attachment's form replaces the last one", () => {
    open(THREE);
    stepForward().focus();
    fireEvent.click(stepForward());
    expect(within(dialog()).getByText("2 / 3")).toBeInTheDocument();
    expect(stepForward()).toHaveFocus();
    fireEvent.click(stepForward());
    expect(within(dialog()).getByText("3 / 3")).toBeInTheDocument();
    expect(stepForward()).toHaveFocus();
    stepBack().focus();
    fireEvent.click(stepBack());
    expect(within(dialog()).getByText("2 / 3")).toBeInTheDocument();
    expect(stepBack()).toHaveFocus();
  });

  it("does not fall to the page when the arrows step with the focus on a field's neighbour", () => {
    open(THREE);
    screen.getByRole("checkbox", { name: /Hide as spoiler/ }).focus();
    fireEvent.keyDown(document.activeElement as HTMLElement, { key: "ArrowRight" });
    expect(within(dialog()).getByText("2 / 3")).toBeInTheDocument();
    expect(document.activeElement).not.toBe(document.body);
    expect(dialog().contains(document.activeElement)).toBe(true);
  });
});

describe("the fields", () => {
  it("have visible captions, so an empty description is not an unlabeled box", () => {
    open(THREE);
    // `getByText` finds the caption itself, not the aria-label: the field's accessible name is still read once.
    expect(within(dialog()).getByText("Name")).toBeVisible();
    expect(within(dialog()).getByText("Description")).toBeVisible();
    expect(within(dialog()).getAllByRole("textbox")).toHaveLength(2);
    expect(screen.getByRole("textbox", { name: "Name" })).toBeInTheDocument();
  });

  it("count the characters of the description as Unicode scalar values, as Rust does", () => {
    open(THREE);
    const field = screen.getByRole("textbox", { name: "Description" });
    // 450 astral characters are 900 UTF-16 units: by `.length` the counter would be on, by scalar values it is not.
    fireEvent.change(field, { target: { value: "\u{1F600}".repeat(450) } });
    expect(within(dialog()).queryByText(/characters/)).toBeNull();
    fireEvent.change(field, { target: { value: "\u{1F600}".repeat(900) } });
    expect(within(dialog()).getByText("900 of 1,024 characters")).toBeInTheDocument();
  });
});
