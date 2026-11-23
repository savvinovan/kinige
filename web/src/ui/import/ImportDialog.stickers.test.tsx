import { describe, expect, it, vi } from "vitest";
import type { ImportPreview } from "../../api/generated/ImportPreview";
import { NO_MEDIA, NO_STICKERS } from "../../api/mockData";
import { render, screen } from "../../test/render";
import { ImportDialog } from "./ImportDialog";

// Slice 5c: the preview line "Stickers: 230 sent, 41 different · animated or video, skipped: 12", only when the export has stickers.
const preview = (stickers: ImportPreview["stickers"]): ImportPreview => ({
  chats: [{ id: "1000001", name: null, kind: "saved", total: 1682, existing: 0, ...NO_MEDIA }],
  skippedChats: 0,
  stickers,
});

const show = (stickers: ImportPreview["stickers"], language: "en" | "ru" = "en") =>
  render(
    <ImportDialog
      step="choose"
      preview={preview(stickers)}
      selected={new Set(["1000001"])}
      onPick={vi.fn()}
      onToggle={vi.fn()}
      onRun={vi.fn()}
      onStop={vi.fn()}
      onClose={vi.fn()}
    />,
    { language },
  );

describe("the import preview: stickers", () => {
  it("shows the counts in English and in Russian", () => {
    const { unmount } = show({ sent: 230, different: 41, skipped: 12 });
    expect(screen.getByText("Stickers: 230 sent, 41 different · animated or video, skipped: 12")).toBeInTheDocument();
    unmount();
    show({ sent: 230, different: 41, skipped: 12 }, "ru");
    expect(screen.getByText("Стикеры: отправлено 230, разных 41 · анимированных и видео, пропущено: 12")).toBeInTheDocument();
  });

  it("is there when only animated ones were sent, and absent when the export has no stickers", () => {
    const { unmount } = show({ sent: 0, different: 0, skipped: 7 });
    expect(screen.getByText(/^Stickers: 0 sent, 0 different/)).toBeInTheDocument();
    unmount();
    show(NO_STICKERS);
    expect(screen.queryByText(/^Stickers:/)).toBeNull();
  });
});
