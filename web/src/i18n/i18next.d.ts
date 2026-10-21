import "i18next";
import type en from "../locales/en.json";

// Ключи типизированы по английскому файлу: нет ключа — ошибка `tsc` (ADR-0005).
declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: { translation: typeof en };
  }
}
