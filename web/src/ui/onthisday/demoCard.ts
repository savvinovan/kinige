import type { OnThisDay } from "../../api/generated/OnThisDay";
import type { OnThisDayYear } from "../../api/generated/OnThisDayYear";
import { mockMessage } from "../../api/mockData";

/** "Today" in the card's stories and tests: Saturday, 31 October 2026. */
export const CARD_TODAY = "2026-10-31";

const at = (year: number, hour: number, minute: number) => Date.UTC(year, 9, 31, hour, minute);

/** A card year with detailed entries (synthetic texts). */
export function detailedYear(year: number, texts: readonly string[], total = texts.length): OnThisDayYear {
  return {
    journalDate: `${year}-10-31`,
    yearsAgo: 2026 - year,
    total,
    firstId: `m-${year}-0`,
    entries: texts.map((text, index) => ({
      message: mockMessage({
        id: `m-${year}-${index}`,
        bodyMd: text,
        journalDate: `${year}-10-31`,
        createdAt: at(year, 9 + index * 2, 12),
      }),
      text,
    })),
  };
}

/** An old year on a single line. */
export function compactYear(year: number, total: number): OnThisDayYear {
  return { journalDate: `${year}-10-31`, yearsAgo: 2026 - year, total, firstId: `m-${year}-0`, entries: [] };
}

export const ONE_YEAR: OnThisDay = {
  today: CARD_TODAY,
  years: [detailedYear(2025, ["Проснулся рано, до завтрака записал мысль про журнал."])],
};

export const MANY_YEARS: OnThisDay = {
  today: CARD_TODAY,
  years: [
    detailedYear(2025, ["Купил билеты на субботу.", "Позвонил маме, договорились на воскресенье."], 3),
    detailedYear(2023, ["Дочитал «Конец вечности». Сильная концовка."]),
    detailedYear(2022, ["Первый день на новой работе.", "Вечером разбирал фотографии."]),
    compactYear(2019, 5),
    compactYear(2016, 1),
  ],
};

export const LONG_TEXT: OnThisDay = {
  today: CARD_TODAY,
  years: [
    detailedYear(2024, [
      "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. Потом вернуться к этой записи через неделю и проверить, на сколько удалось ответить. Ещё раз пройтись по конспекту.",
    ]),
  ],
};
