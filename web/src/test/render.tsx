import { render as baseRender, type RenderOptions } from "@testing-library/react";
import type { ReactElement } from "react";
import { I18n } from "../i18n/I18n";
import type { Language } from "../i18n";

/**
 * `render` с переводами. Язык по умолчанию — русский: тесты компонентов D1 проверяют русские подписи;
 * английский — `render(ui, { language: "en" })`.
 */
export function render(ui: ReactElement, { language = "ru", ...options }: RenderOptions & { language?: Language } = {}) {
  return baseRender(ui, { wrapper: ({ children }) => <I18n language={language}>{children}</I18n>, ...options });
}

export * from "@testing-library/react";
