import { describe, expect, it } from "vitest";
import { render, screen } from "../../test/render";
import { ChatTopBar } from "./ChatTopBar";

const FOREST = { hidden: false, onToggle: () => undefined };

describe("ChatTopBar", () => {
  it("names the chat with a heading and keeps the switch", () => {
    render(<ChatTopBar title="Books" forestMenu={FOREST} />, { language: "en" });
    expect(screen.getByRole("heading", { level: 1, name: "Books" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Forest" })).toBeInTheDocument();
  });

  it("has no heading when the open chat is not in the list (an empty heading is announced as nothing), and still the switch and the same bar", () => {
    const { container } = render(<ChatTopBar title="" forestMenu={FOREST} />, { language: "en" });
    expect(screen.queryByRole("heading")).toBeNull();
    expect(screen.getByRole("button", { name: "Forest" })).toBeInTheDocument();
    expect(container.querySelector(".main-window__topbar")).not.toBeNull();
  });
});
