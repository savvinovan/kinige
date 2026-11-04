import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "../../test/render";
import { Message } from "./Message";

// KINIG-47.1: `Message` is memoised. `MessageActions` is drawn once per render of `Message` (in view mode), so counting its renders
// counts the renders of the real `Message`; the counter itself is not memoised, so removing `memo` from `Message` fails this test.
const renders = vi.hoisted(() => ({ actions: vi.fn() }));

vi.mock("./MessageActions", async (importOriginal) => {
  const real = await importOriginal<typeof import("./MessageActions")>();
  const Counted = (props: React.ComponentProps<typeof real.MessageActions>) => {
    renders.actions();
    return <real.MessageActions {...props} />;
  };
  return { ...real, MessageActions: Counted };
});

const sentAt = new Date(2026, 9, 20, 9, 12);

/** A parent that renders again on a click and hands `Message` the same values every time. */
function Parent({ markdown }: { markdown: string }) {
  const [ticks, setTicks] = useState(0);
  return (
    <>
      <button type="button" onClick={() => setTicks(ticks + 1)}>
        tick {ticks}
      </button>
      <Message markdown={markdown} sentAt={sentAt} />
    </>
  );
}

describe("Message: memo", () => {
  it("перерисовка родителя с теми же значениями не перерисовывает сообщение, новый текст перерисовывает", () => {
    const { rerender } = render(<Parent markdown="Купить хлеб" />);
    const first = renders.actions.mock.calls.length;
    expect(first).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: /tick/ }));
    fireEvent.click(screen.getByRole("button", { name: /tick/ }));
    expect(screen.getByRole("button", { name: "tick 2" })).toBeInTheDocument();
    expect(renders.actions).toHaveBeenCalledTimes(first);

    rerender(<Parent markdown="Купить молоко" />);
    expect(screen.getByText("Купить молоко")).toBeInTheDocument();
    expect(renders.actions.mock.calls.length).toBeGreaterThan(first);
  });
});
