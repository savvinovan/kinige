import i18next, { type i18n } from "i18next";
import en from "../locales/en.json";
import ru from "../locales/ru.json";

/** Языки интерфейса (ТЗ §2.1): английский первым, русский вторым. Выбирает Rust по языкам системы (`get_language`). */
export type Language = "en" | "ru";

export const LANGUAGES: readonly Language[] = ["en", "ru"];

/**
 * Свой экземпляр i18next на язык: без глобального состояния, поэтому тесты и истории Storybook берут язык независимо.
 * Переводы лежат в бандле и инициализация синхронная (`initAsync: false`): окно не показывается с пустыми строками.
 */
export function createI18n(language: Language): i18n {
  const instance = i18next.createInstance();
  void instance.init({
    lng: language,
    fallbackLng: "en",
    resources: { en: { translation: en }, ru: { translation: ru } },
    interpolation: { escapeValue: false },
    initAsync: false,
  });
  return instance;
}
