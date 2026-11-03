import i18next, { type i18n } from "i18next";
import en from "../locales/en.json";
import ru from "../locales/ru.json";

/** Interface languages (PRD §2.1): English first, Russian second. Rust chooses by the system languages (`get_language`). */
export type Language = "en" | "ru";

export const LANGUAGES: readonly Language[] = ["en", "ru"];

/**
 * Its own i18next instance per language: no global state, so tests and Storybook stories take the language independently.
 * The translations are in the bundle and initialization is synchronous (`initAsync: false`): the window is not shown with empty strings.
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
