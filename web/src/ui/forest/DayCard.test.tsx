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
  voices: 0,
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

  it("counts the day's voice notes after the photos (slice 7a)", () => {
    const en = render(<DayCard day={{ ...DAY, photos: 0, voices: 1 }} pinned />, { language: "en" });
    expect(en.container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · 1 voice note · a large tree");
    en.unmount();
    const many = render(<DayCard day={{ ...DAY, voices: 3 }} pinned />, { language: "en" });
    expect(many.container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · 3 photos · 3 voice notes · a large tree");
    many.unmount();
    const ru = render(<DayCard day={{ ...DAY, photos: 0, voices: 2 }} pinned />, { language: "ru" });
    expect(ru.container.querySelector(".day-card__summary")?.textContent?.replace(/\s/g, " ")).toBe("5 записей · 2 голосовых · большое дерево");
  });

  it("has no photos part for a day without photos, and describes a first entry that has no text", () => {
    const plain = render(<DayCard day={{ ...DAY, photos: 0 }} pinned />, { language: "en" });
    expect(plain.container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · a large tree");
    plain.unmount();
    const photoOnly = render(
      <DayCard day={{ ...DAY, excerpt: "", photos: 1, firstSummary: { photos: 1, files: 0, voices: 0, voiceMs: null, name: "IMG_0001.jpg" } }} pinned />,
      { language: "en" },
    );
    expect(photoOnly.container.querySelector(".day-card__summary")?.textContent).toBe("5 notes · 1 photo · a large tree");
    expect(photoOnly.container.querySelector(".day-card__excerpt")?.textContent).toBe("Photo");
  });
});
