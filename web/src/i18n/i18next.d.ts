import "i18next";
import type en from "../locales/en.json";

// Keys are typed from the English file: a missing key is a `tsc` error (ADR-0005).
declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: { translation: typeof en };
  }
}
