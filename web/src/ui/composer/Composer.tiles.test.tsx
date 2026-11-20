import { describe, expect, it, vi } from "vitest";
import { render, screen } from "../../test/render";
import { staticVoice } from "../../voice/staticVoice";
import type { VoiceController } from "../../voice/useVoiceRecorder";
import { Composer } from "./Composer";

// The pending attachments inside the composer's box (KINIG-135).
const TILES = (
  <ul aria-label="tiles">
    <li>one</li>
  </ul>
);

describe("Composer: плитки вложений внутри поля", () => {
  it("плитки стоят в той же коробке, что и текст, на строке выше него", () => {
    const { container } = render(<Composer value="" onChange={vi.fn()} tiles={TILES} hasTiles />, { language: "en" });
    const box = container.querySelector(".composer__bar") as HTMLElement;
    const tiles = container.querySelector(".composer__tiles") as HTMLElement;
    expect(box).toContainElement(tiles);
    expect(box.firstElementChild).toBe(tiles);
    expect(tiles).toContainElement(screen.getByRole("list", { name: "tiles" }));
    // The text field is in the same box, after the tiles.
    expect(box).toContainElement(screen.getByRole("textbox"));
    expect(tiles.compareDocumentPosition(screen.getByRole("textbox")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("без плиток коробка прежняя: ни обёртки, ни лишних классов", () => {
    const { container } = render(<Composer value="" onChange={vi.fn()} tiles={TILES} />, { language: "en" });
    expect(container.querySelector(".composer__tiles")).toBeNull();
    expect(container.querySelector(".composer__bar")?.className).toBe("composer__bar");
  });

  it("запись голоса: поле скрыто, плитки остаются над полосой записи", () => {
    const voice: VoiceController = {
      ...staticVoice({ kind: "recording", elapsedMs: 3_000 }),
      start: vi.fn(),
      send: vi.fn(),
      requestCancel: vi.fn(),
      keepRecording: vi.fn(),
      discard: vi.fn(),
      retry: vi.fn(),
      openSettings: vi.fn(),
      dismissNotice: vi.fn(),
    };
    const { container } = render(<Composer value="" onChange={vi.fn()} voice={voice} tiles={TILES} hasTiles />, { language: "en" });
    const box = container.querySelector(".composer__bar") as HTMLElement;
    expect(box).toHaveClass("composer__bar--hidden", "composer__bar--tiles-only");
    const recording = screen.getByRole("group", { name: /Recording/ });
    expect(box.compareDocumentPosition(recording) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(box.querySelector(".composer__tiles")).not.toBeNull();
  });
});
