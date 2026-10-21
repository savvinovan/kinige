import {
  ArrowUp,
  Check,
  ChevronDown,
  CircleAlert,
  Copy,
  Ellipsis,
  Info,
  Mic,
  Paperclip,
  Pencil,
  Search,
  Smile,
  SquareCheck,
  Star,
  X,
} from "lucide-react";
import "./icon.css";

// Линейные значки интерфейса из набора Lucide (npm lucide-react, лицензия ISC; происхождение и лицензии — icons/README.md).
// Эмодзи как декор интерфейса запрещены (§11.2), поэтому значок только отсюда. Размер и толщина линии — токены icon.*.
const GLYPHS = {
  attach: Paperclip,
  emoji: Smile,
  voice: Mic,
  search: Search,
  chevron: ChevronDown,
  send: ArrowUp,
  copy: Copy,
  edit: Pencil,
  task: SquareCheck,
  favorite: Star,
  more: Ellipsis,
  done: Check,
  close: X,
  alert: CircleAlert,
  info: Info,
} as const;

export type IconName = keyof typeof GLYPHS;

/** Декоративный значок: у него нет подписи для диктора, подпись всегда у кнопки или поля, где он стоит. */
export function Icon({ name }: { name: IconName }) {
  const Glyph = GLYPHS[name];
  return <Glyph className="icon" aria-hidden="true" focusable="false" />;
}
