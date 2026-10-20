import { Mic, Paperclip, Search, Smile } from "lucide-react";
import "./icon.css";

// Линейные значки интерфейса из набора Lucide (npm lucide-react, лицензия ISC; происхождение и лицензии — icons/README.md).
// Эмодзи как декор интерфейса запрещены (§11.2), поэтому значок только отсюда. Размер и толщина линии — токены icon.*.
const GLYPHS = {
  attach: Paperclip,
  emoji: Smile,
  voice: Mic,
  search: Search,
} as const;

export type IconName = keyof typeof GLYPHS;

/** Декоративный значок: у него нет подписи для диктора, подпись всегда у кнопки или поля, где он стоит. */
export function Icon({ name }: { name: IconName }) {
  const Glyph = GLYPHS[name];
  return <Glyph className="icon" aria-hidden="true" focusable="false" />;
}
