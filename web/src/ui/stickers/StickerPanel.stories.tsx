import type { Meta, StoryObj } from "@storybook/react-vite";
import { useEffect, useRef, useState, type ReactNode } from "react";
import type { JournalGif } from "../../api/generated/JournalGif";
import type { StickerLibrary } from "../../api/generated/StickerLibrary";
import { MockApi } from "../../api/MockApi";
import { DemoMedia, demoPhoto } from "../attachments/demo";
import { Tabs } from "../controls/Tabs";
import { Frame } from "../stories/Frame";
import { demoGifs, demoLibrary } from "./demoLibrary";
import { GifPanel } from "./GifPanel";
import { makeStoryHost } from "./storyHost";
import { PickerTabs } from "./PickerTabs";
import { StickerPanel } from "./StickerPanel";
import "./stickers-stories.css";

// The Stickers and GIFs tabs of the emoji window (slice 5c). Every picture is drawn by code; there is no real sticker or GIF anywhere in the stories.
const noop = () => undefined;
const LONG_TITLE = "A very long pack title that goes on and on, much longer than a single heading line could ever hold, cut by an ellipsis at the end of it ok";

const LIBRARY = demoLibrary(
  [
    { title: "From Telegram", count: 14, source: "telegram_export" },
    { title: "Evening cats", count: 9 },
    { title: "Pocket owls", count: 6 },
  ],
  { recent: 4 },
);

function Plate({ tab, children, focus = false }: { tab: "stickers" | "gifs"; children: ReactNode; focus?: boolean }) {
  const host = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (focus) host.current?.querySelector<HTMLElement>(".sticker-panel__cell, .gif-panel__cell")?.focus();
  }, [focus]);
  return (
    <Frame surface="paper">
      <DemoMedia>
        <div className="story-picker-plate" ref={host}>
          <Tabs
            tabs={[
              { value: "emoji", label: "Emoji" },
              { value: "stickers", label: "Stickers" },
              { value: "gifs", label: "GIFs" },
            ]}
            value={tab}
            onChange={noop}
            label="Emoji, stickers and GIFs"
            idPrefix="story"
          />
          <div className="picker-tabs__panel">{children}</div>
        </div>
      </DemoMedia>
    </Frame>
  );
}

interface DemoProps {
  library?: StickerLibrary | "failed" | null;
  focus?: boolean;
}

function StickersDemo({ library = LIBRARY, focus = false }: DemoProps) {
  return (
    <Plate tab="stickers" focus={focus}>
      <StickerPanel library={library} onSend={noop} onAddPack={noop} onRemovePack={noop} />
    </Plate>
  );
}

const meta = {
  title: "Компоненты/Стикеры: панель",
  component: StickersDemo,
} satisfies Meta<typeof StickersDemo>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Normal: the strip (Recent, From Telegram, two packs, +), Recent first with the newest sticker first, then the packs under their headings, five columns. */
export const Обычная: Story = {};

/** No recent: the strip and the grid start with the pack from the export. */
export const БезНедавних: Story = { args: { library: demoLibrary([{ title: "From Telegram", count: 14, source: "telegram_export" }]) } };

/** Nothing yet: one line says how to get stickers, the strip keeps its +. */
export const Пусто: Story = { args: { library: { recent: [], packs: [], hasToken: false } } };

/** The library is being read. */
export const Загрузка: Story = { args: { library: null } };

/** The library could not be read: a quiet line, the + still works. */
export const ОшибкаЗагрузки: Story = { args: { library: "failed" } };

/** A pack at the cap of 200 stickers: the grid scrolls inside the panel, the panel does not grow. */
export const ПакНаДвестиСтикеров: Story = { args: { library: demoLibrary([{ title: "Two hundred", count: 200 }]) } };

/** A 128-character pack title is one line cut with an ellipsis, in the strip's tooltip and in the heading. */
export const ДлинныйЗаголовок: Story = { args: { library: demoLibrary([{ title: LONG_TITLE, count: 7 }, { title: "Short", count: 3 }]) } };

/** Keyboard focus: the first sticker has the ring (arrows move it by cell and by row, Enter sends). */
export const ФокусСКлавиатуры: Story = { args: { focus: true } };

/** English interface: "Sticker 😺" labels, "Recent", "From Telegram". */
export const Английский: Story = { globals: { language: "en" } };

/** Large text: headings and the strip grow with the text, the pictures do not. */
export const КрупныйТекст: Story = { globals: { textScale: "large" } };

export const КрупныйТекстАнглийский: Story = { globals: { textScale: "large", language: "en" } };

// ---- the GIFs tab ----

const GIFS: JournalGif[] = demoGifs(14);
for (const gif of GIFS) demoPhoto(gif.attachmentId, "cat.gif", gif.width, gif.height, { mime: "image/gif", byteSize: gif.byteSize });

function GifsDemo({ gifs = GIFS, focus = false }: { gifs?: readonly JournalGif[] | "failed" | null; focus?: boolean }) {
  return (
    <Plate tab="gifs" focus={focus}>
      <GifPanel gifs={gifs} onSend={noop} onChoose={noop} />
    </Plate>
  );
}

/** The journal's own GIFs as still frames in three columns, each with the GIF badge, and "Choose a GIF file…" on top. */
export const GifСписок: StoryObj<typeof GifsDemo> = { render: (args) => <GifsDemo {...args} /> };

/** No GIFs yet: one line, the button stays. */
export const GifПусто: StoryObj<typeof GifsDemo> = { render: () => <GifsDemo gifs={[]} /> };

export const GifЗагрузка: StoryObj<typeof GifsDemo> = { render: () => <GifsDemo gifs={null} /> };

export const GifОшибка: StoryObj<typeof GifsDemo> = { render: () => <GifsDemo gifs="failed" /> };

export const GifФокус: StoryObj<typeof GifsDemo> = { render: () => <GifsDemo focus /> };

export const GifАнглийскийКрупный: StoryObj<typeof GifsDemo> = {
  render: () => <GifsDemo />,
  globals: { language: "en", textScale: "large" },
};

// ---- the real tabs on a MockApi: click and arrow through them ----

function allTabs() {
  const api = new MockApi({ language: "ru" });
  api.stickerLibrary.seedPack("From Telegram", 14, "telegram_export");
  api.stickerLibrary.seedPack("Evening cats", 9);
  for (const gif of GIFS) {
    api.addMessageWithAttachments({ id: `gm-${gif.attachmentId}`, bodyMd: "" }, [
      demoPhoto(gif.attachmentId, "cat.gif", gif.width, gif.height, { mime: "image/gif", byteSize: gif.byteSize }),
    ]);
  }
  return makeStoryHost(api);
}

function WholePanel({ language = "ru" }: { language?: "ru" | "en" }) {
  const [host] = useState(allTabs);
  return (
    <Frame surface="paper">
      <DemoMedia>
        <div className="story-picker-plate">
          <PickerTabs initialTab="stickers" recent={[]} language={language} onPick={noop} onClose={noop} host={host} />
        </div>
      </DemoMedia>
    </Frame>
  );
}

/** The whole panel on a MockApi, opened on Stickers: the tabs work, a click on a sticker sends it into the mock journal. */
export const ВсеВкладки: StoryObj<typeof WholePanel> = { render: () => <WholePanel /> };

export const ВсеВкладкиАнглийский: StoryObj<typeof WholePanel> = { render: () => <WholePanel language="en" />, globals: { language: "en" } };
