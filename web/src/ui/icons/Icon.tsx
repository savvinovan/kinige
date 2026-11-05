import {
  Archive,
  ArchiveRestore,
  ArrowRight,
  ArrowUp,
  AudioLines,
  Calendar,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  CircleAlert,
  Copy,
  Database,
  Download,
  Ellipsis,
  EyeOff,
  File,
  Info,
  Maximize2,
  Mic,
  Minimize2,
  PanelLeftClose,
  PanelLeftOpen,
  Palette,
  Paperclip,
  Pause,
  Pencil,
  Pin,
  PinOff,
  Play,
  Plus,
  Reply,
  RotateCcw,
  Search,
  Settings,
  Smile,
  SquareCheck,
  Star,
  Trash2,
  Trees,
  Undo2,
  Upload,
  Video,
  X,
  ZoomIn,
  ZoomOut,
} from "lucide-react";
import "./icon.css";

// Line icons of the interface from the Lucide set (npm lucide-react, ISC license; origin and licenses are in icons/README.md).
// Emoji as interface decoration are forbidden (§11.2), so icons come only from here. The size and line thickness are the icon.* tokens.
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
  // Slice 3: the chat and entry menu, the sidebar, the trash, the database backup.
  trash: Trash2,
  calendar: Calendar,
  pin: Pin,
  unpin: PinOff,
  archive: Archive,
  unarchive: ArchiveRestore,
  plus: Plus,
  collapse: PanelLeftClose,
  expand: PanelLeftOpen,
  undo: Undo2,
  chevronRight: ChevronRight,
  database: Database,
  // Acceptance decision: "Move to chat" (the entry menu) and the return from the submenu.
  moveTo: ArrowRight,
  chevronLeft: ChevronLeft,
  // Slice 5: "Reply" in the "On this day" card and the reply row.
  reply: Reply,
  // D2: icons for the profile menu items.
  settings: Settings,
  download: Download,
  upload: Upload,
  // Slice 6: the forest above the feed (expand and collapse the pane, hide it, its profile menu item).
  expandPane: Maximize2,
  collapsePane: Minimize2,
  hidePane: EyeOff,
  forest: Trees,
  // Slice 7: the file card (a plain file, audio and video are shown as files).
  file: File,
  audio: AudioLines,
  video: Video,
  // Slice 7a: the voice note player (the microphone and the trash button of the recording bar are `voice` and `trash` above).
  play: Play,
  pause: Pause,
  // KINIG-59: "Colour" in the chat menu.
  palette: Palette,
  // KINIG-87: the photo viewer toolbar (turn left, zoom out, zoom in).
  rotate: RotateCcw,
  zoomOut: ZoomOut,
  zoomIn: ZoomIn,
} as const;

export type IconName = keyof typeof GLYPHS;

/** A decorative icon: it has no label for the screen reader, the label always belongs to the button or field where it stands. */
export function Icon({ name }: { name: IconName }) {
  const Glyph = GLYPHS[name];
  return <Glyph className="icon" aria-hidden="true" focusable="false" />;
}
