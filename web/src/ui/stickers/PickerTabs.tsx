import { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import type { KinigeApi } from "../../api/KinigeApi";
import type { JournalGif } from "../../api/generated/JournalGif";
import type { StickerLibrary } from "../../api/generated/StickerLibrary";
import type { StickerPackView } from "../../api/generated/StickerPackView";
import type { Language } from "../../i18n";
import { Tabs, tabPanelProps } from "../controls/Tabs";
import { EmojiPicker } from "../emoji/EmojiPicker";
import { GifPanel } from "./GifPanel";
import { StickerPanel } from "./StickerPanel";

/** What the Stickers and GIFs tabs need from the window that owns the composer (the main window only; the capture window has no tabs). */
export interface StickersHost {
  api: KinigeApi;
  /** Sends the sticker as its own entry in the open chat, with the pending reply. Rejects after the window has shown the error. */
  sendSticker: (stickerId: string) => Promise<void>;
  /** Sends a GIF of the journal again as a new entry. Rejects after the window has shown the error. */
  sendGif: (attachmentId: string) => Promise<void>;
  /** "+" in the strip: the window opens the add-pack dialog (the panel closes first). */
  onAddPack: () => void;
  /** "Remove pack" in a pack icon's menu: the window asks, then removes (the panel closes first). */
  onRemovePack: (pack: StickerPackView) => void;
  /** "Choose a GIF file…": the paperclip flow (the panel closes first). */
  onChooseGif: () => void;
}

export type PickerTab = "emoji" | "stickers" | "gifs";

interface PickerTabsProps {
  /** The tab the panel opens on: Emoji (stories show the others from the start). */
  initialTab?: PickerTab;
  recent: readonly string[];
  language: Language;
  onPick: (emoji: string) => void;
  /** Closes the panel; `returnFocus` as for `EmojiPopover`. */
  onClose: (returnFocus: boolean) => void;
  host: StickersHost;
}

/**
 * The three tabs of the emoji window in the main window (slice 5c): Emoji, Stickers, GIFs, as in Telegram. The panel opens on Emoji with focus in its
 * search, as before; a tab's data is read only while that tab is shown (never at startup, never while only Emoji is open). Tab, with the arrows, walks
 * the tab list and the open tab's controls; Esc closes the whole panel (`EmojiPopover`).
 */
export function PickerTabs({ initialTab = "emoji", recent, language, onPick, onClose, host }: PickerTabsProps) {
  const { t } = useTranslation();
  const idPrefix = useId();
  const [tab, setTab] = useState<PickerTab>(initialTab);
  // Focus goes to the emoji search only when the panel opens: coming back to the Emoji tab with the arrows must leave focus on the tab list.
  const [switched, setSwitched] = useState(false);
  const change = (next: PickerTab) => {
    setSwitched(true);
    setTab(next);
  };

  return (
    <>
      <Tabs<PickerTab>
        tabs={[
          { value: "emoji", label: t("stickers.tabs.emoji") },
          { value: "stickers", label: t("stickers.tabs.stickers") },
          { value: "gifs", label: t("stickers.tabs.gifs") },
        ]}
        value={tab}
        onChange={change}
        label={t("stickers.tabs.label")}
        idPrefix={idPrefix}
      />
      <div className="picker-tabs__panel" {...tabPanelProps(idPrefix, tab)}>
        {tab === "emoji" ? <EmojiPicker recent={recent} language={language} autoFocus={!switched} onPick={onPick} /> : null}
        {tab === "stickers" ? <StickersTab host={host} onClose={onClose} /> : null}
        {tab === "gifs" ? <GifsTab host={host} onClose={onClose} /> : null}
      </div>
    </>
  );
}

interface TabProps {
  host: StickersHost;
  onClose: (returnFocus: boolean) => void;
}

/** Reads the library when the tab is shown. The panel stays open after a send (as in Telegram); an error closes it so the line above the field is seen. */
function StickersTab({ host, onClose }: TabProps) {
  const library = useLoaded<StickerLibrary>(() => host.api.getStickerLibrary(), [host.api]);
  return (
    <StickerPanel
      library={library}
      onSend={(sticker) => host.sendSticker(sticker.id).catch(() => onClose(true))}
      onAddPack={() => {
        onClose(true);
        host.onAddPack();
      }}
      onRemovePack={(pack) => {
        onClose(true);
        host.onRemovePack(pack);
      }}
    />
  );
}

function GifsTab({ host, onClose }: TabProps) {
  const gifs = useLoaded<JournalGif[]>(() => host.api.listJournalGifs(), [host.api]);
  return (
    <GifPanel
      gifs={gifs}
      onSend={(gif) => host.sendGif(gif.attachmentId).catch(() => onClose(true))}
      onChoose={() => {
        onClose(true);
        host.onChooseGif();
      }}
    />
  );
}

/** One read when the component mounts: `null` while it runs, `"failed"` if it was refused. */
function useLoaded<T>(read: () => Promise<T>, deps: readonly unknown[]): T | "failed" | null {
  const [value, setValue] = useState<T | "failed" | null>(null);
  useEffect(() => {
    let alive = true;
    read().then(
      (loaded) => {
        if (alive) setValue(loaded);
      },
      () => {
        if (alive) setValue("failed");
      },
    );
    return () => {
      alive = false;
    };
    // `read` is built from `deps` on every render; the data is read once per mount.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  return value;
}
