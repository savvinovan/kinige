import { describe, expect, it } from "vitest";
import type { ForestDay } from "../../api/generated/ForestDay";
import { render } from "../../test/render";
import { DayCard } from "./DayCard";

const DAY: ForestDay = {
  date: "2026-10-16",
  row: 4,
  col: 40,
  season: "autumn",
  notes: 5,
  stage: 6,
  excerpt: "Прогулка вдоль реки",
  photos: 3,
  firstSummary: null,
};

describe("the card of a day with photos (slice 7)", () => {
  it("says \"5 notes · 3 photos · a large tree\" in English", () => {
    const { container } = render(<DayCard day={DAY} pinned />, { language: "en" });
    expect(container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · 3 photos · a large tree");
  });

  it("says it in Russian", () => {
    const { container } = render(<DayCard day={DAY} pinned />, { language: "ru" });
    expect(container.querySelector(".day-card__summary")?.textContent?.replace(/\s/g, " ")).toBe("5 записей · 3 фото · большое дерево");
  });

  it("has no photos part for a day without photos, and describes a first entry that has no text", () => {
    const plain = render(<DayCard day={{ ...DAY, photos: 0 }} pinned />, { language: "en" });
    expect(plain.container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · a large tree");
    plain.unmount();
    const photoOnly = render(
      <DayCard day={{ ...DAY, excerpt: "", photos: 1, firstSummary: { photos: 1, files: 0, name: "IMG_0001.jpg" } }} pinned />,
      { language: "en" },
    );
    expect(photoOnly.container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · 1 photo · a large tree");
    expect(photoOnly.container.querySelector(".day-card__excerpt")?.textContent).toBe("Photo");
  });
});
